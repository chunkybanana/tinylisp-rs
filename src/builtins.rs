use std::{collections::HashMap, rc::Rc, sync::LazyLock};

use crate::{
    env::Env,
    list::LinkedList,
    value::{Builtin, Error, Value, ValueResult, lookup_str},
};

type BuiltinImpl = fn(&mut Env, &Rc<LinkedList>) -> ValueResult;

/* they call me the ginger */
macro_rules! arg_to_match {
    ($name:ident, any) => {
        $name
    };
    ($name:ident, int) => {
        Value::Int($name)
    };
    ($name:ident, str) => {
        Value::Name($name)
    };
    ($name:ident, list) => {
        Value::List($name)
    };
}

// tuple expressions are _really_ janky
macro_rules! get_last_arg {
    ($args:ident, 1) => {
        $args.0
    };
    ($args:ident, 2) => {
        $args.1
    };
    ($args:ident, 3) => {
        $args.2
    };
}

macro_rules! eval_builtin_arg {
    (macro, $env:ident, $expr:tt) => {
        $expr.unwrap()
    };
    (fn, $env:ident, $expr:tt) => {
        $env.eval($expr.unwrap())?
    };
}

macro_rules! types_to_str {
    ($name:ident) => {
        stringify!($name)
    };
    ($($name:ident),*) => {
        stringify!(($($name),*))
    };
}

// we get a tuple of &Values if a macro and a tuple of Values if a function - the pattern matching doesn't care but error cases do
macro_rules! clone_if_macro {
    ($expr:tt, macro) => {
        $expr.clone()
    };
    ($expr:tt, fn) => {
        $expr
    };
}

// Essentially, this is trying to avoid the performance overhead of allocating vecs whereever possible,
// instead using the defined iterators to create tuples and trusting rustc to unwrap them properly
// we love nightly macro features
macro_rules! builtin {
    ($f_type:ident $builtin:ident as $name:ident ($env:ident, $($arg:ident $colon:tt $type:tt),*) => $body:tt) => {
        (stringify!($name), Builtin::$builtin, |$env, raw_args| {
            let mut arg_iter = raw_args.iter();
            let args = ($(arg_iter.next() ${ignore($arg)},)*);

            if get_last_arg!(args, ${count($arg)}).is_none() || arg_iter.next().is_some() {
                return Err(Error::BuiltinArgumentCount {
                    name: stringify!($name).to_owned(),
                    argc: raw_args.to_vec().len(),
                    expected: ${count($arg)}
                });
            }

            // tuple access expressions are _incredibly_ janky
            let eval_args = ($(eval_builtin_arg!($f_type, $env, (args.${index()}) ${ignore($arg)}),)*);

            match eval_args {
                ($(arg_to_match!($arg, $type),)*) => $body,
                #[allow(unreachable_patterns)]
                _ => Err(Error::BuiltinArgumentType(format!(
                    "tried to call builtin {} with {}, expected {}",
                    stringify!($name),
                    get_types(&[$(clone_if_macro!((eval_args.${index()}), $f_type) ${ignore($arg)}),*]),
                    types_to_str!($($type),*)
                ))),
            }
        })
    };
}

fn get_types(args: &[Value]) -> String {
    if args.len() == 1 {
        return args[0].tl_type().to_string();
    }
    format!(
        "({})",
        args.iter()
            .map(Value::tl_type)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub const BUILTIN_LIST: &[(&str, Builtin, BuiltinImpl)] = &[
    builtin! {
        fn Cons as c (_env, head: any, tail: list) => {
            Ok(Value::List(LinkedList::cons(&head, &tail)))
        }
    },
    builtin! {
        fn Head as h (_env, list: list) => {
            Ok(list.head())
        }
    },
    builtin! {
        fn Tail as t (_env, list: list) => {
            Ok(Value::List(list.tail()))
        }
    },
    builtin! {
        fn Less as l (_env, int1: int, int2: int) => {
            Ok(Value::Int(i64::from(int1 < int2)))
        }
    },
    builtin! {
        fn Add as a (_env, int1: int, int2: int) => {
            Ok(Value::Int(int1 + int2))
        }
    },
    builtin! {
        fn Sub as s (_env, int1: int, int2: int) => {
            Ok(Value::Int(int1 - int2))
        }
    },
    builtin! {
        fn Eq as e (_env, val1: any, val2: any) => {
            Ok(Value::Int(i64::from(val1 == val2)))
        }
    },
    builtin! {
        fn Eval as v (env, val: any) => {
            env.eval(&val)
        }
    },
    builtin! {
        macro Quote as q (_env, val: any) => {
            Ok(val.clone())
        }
    },
    builtin! {
        macro If as i (env, cond: any, val1: any, val2: any) => {
            let cond = env.eval(cond)?;
            env.eval(if cond.is_truthy() { val1 } else { val2 })
        }
    },
    builtin! {
        macro Def as d (env, name: str, value: any) => {
            let val = env.eval(value)?;
            env.global_dict.insert(*name, val);
            Ok(Value::Name(*name))
        }
    },
    builtin! {
        fn Type as type (env, value: any) => {
            Ok(Value::from_str(match value {
                Value::Int(_) => "Int",
                Value::Name(_) => "Name",
                Value::List(_) => "List",
                Value::Builtin(_) => "Builtin"
            }.to_owned()))
        }
    },
    builtin! {
        fn Chars as chars (env, name: str) => {
            let string = lookup_str(name);
            Ok(Value::from_vec(&string.chars().map(|i| Value::from_int(i as i64)).collect::<Vec<_>>()))
        }
    },
    builtin! {
        fn String as string (env, value: any) => {
            Ok(match value {
                Value::Name(_) => value,
                Value::Int(int) => Value::from_str(int.to_string()),
                Value::Builtin(_) => Value::from_str(format!("{value}")),
                Value::List(list) =>
                    Value::from_str(list.iter().map(|val| match val {
                        Value::Int(i) => u32::try_from(*i).ok()
                            .and_then(char::from_u32)
                            .map_or_else(|| Err(Error::BuiltinArgumentType(format!("Cannot convert int {i} to char"))), Ok),
                        _ => Err(Error::BuiltinArgumentType(
                            format!("Cannot convert {val} to char, as it is of type {}", val.tl_type())
                        ))
                    }).collect::<Result<String,Error>>()?)
            })
        }
    },
    builtin! {
        fn Disp as disp (env, value: any) => {
            env.println(format!("{value}")); Ok(Value::nil())
        }
    },
];

pub static BUILTINS: LazyLock<HashMap<Builtin, BuiltinImpl>> = LazyLock::new(|| {
    let mut dict = HashMap::new();
    for (_name, builtin, func) in BUILTIN_LIST {
        dict.insert(*builtin, *func);
    }
    dict
});

#[macro_export]
macro_rules! assert_eval {
    ($env:ident, $expr:tt, $result:tt) => {
        assert_eq!($env.eval(&val!($expr)).unwrap(), val!($result))
    };
}

#[macro_export]
macro_rules! eval {
    ($env: ident, $expr: tt) => {
        $env.eval(&val!($expr)).unwrap()
    };
}

#[cfg(test)]
mod tests {
    use crate::{
        env::Env,
        val,
        value::{Error, Value},
    };

    #[test]
    fn cons() {
        let mut env = Env::new();
        assert_eval!(env, (c 5 ()), (5));
        assert_eval!(env, (c 1 (c (c 5 ()) (c 4 ()))), (1 (5) 4));

        // I'm not planning to test every type / arg count error case, since they're all basically the same
        // But I'm doing these ones
        assert_eq!(
            env.eval(&val!((c 5 4))).unwrap_err(),
            Error::BuiltinArgumentType(
                "tried to call builtin c with (int, int), expected (any, list)".to_owned()
            )
        );

        assert_eq!(
            env.eval(&val!((c 1 2 3))).unwrap_err(),
            Error::BuiltinArgumentCount {
                name: "c".to_owned(),
                argc: 3,
                expected: 2
            }
        );
    }

    #[test]
    fn head() {
        let mut env = Env::new();
        assert_eval!(env, (h()), ());
        assert_eval!(env, (h (c 5 ())), 5);
    }

    #[test]
    fn tail() {
        let mut env = Env::new();

        assert_eval!(env, (t()), ());
        assert_eval!(env, (t (c 5 ())), ());
        assert_eval!(env, (t (c 5 (c 2 ()))), (2));
    }

    #[test]
    fn less() {
        let mut env = Env::new();

        assert_eval!(env, (l 5 4), 0);
        assert_eval!(env, (l 5 5), 0);
        assert_eval!(env, (l 4 5), 1);
    }

    #[test]
    fn add() {
        let mut env = Env::new();

        assert_eval!(env, (a 4 2), 6);
        assert_eval!(env, (a 0 7), 7);
    }

    #[test]
    fn sub() {
        let mut env = Env::new();

        assert_eval!(env, (s 4 3), 1);
        assert_eval!(env, (s 12 2), 10);
        assert_eq!(
            env.eval(&val!((s 4 ()))).unwrap_err(),
            Error::BuiltinArgumentType(
                "tried to call builtin s with (int, nil), expected (int, int)".to_owned()
            )
        );
    }

    #[test]
    fn eq() {
        let mut env = Env::new();
        assert_eval!(env, (e 1 1), 1);
        assert_eval!(env, (e c c), 1);
        assert_eval!(env, (e 1 c), 0);
        assert_eval!(env, (e (q (1 2 3)) (q (1 2 3))), 1);
    }

    #[test]
    fn quote() {
        let mut env = Env::new();
        assert_eval!(env, (q 5), 5);
        assert_eval!(env, (q c), c);
        assert_eval!(env, (q()), ());
        assert_eval!(env, (q (1 2 (4))), (1 2 (4)));
    }

    #[test]
    fn test_if() {
        let mut env = Env::new();

        assert_eval!(env, (i 1 3 error), 3);
        assert_eval!(env, (i 0 error 3), 3);
    }

    #[test]
    fn eval() {
        let mut env = Env::new();

        assert_eval!(env, (v 4), 4);
        assert_eval!(env, (v (q (a 3 5))), 8);
    }

    #[test]
    fn def() {
        let mut env = Env::new();

        assert_eq!(
            env.eval(&val!(cutie)).unwrap_err(),
            Error::VariableNotFound("cutie".to_owned())
        );

        assert_eval!(env, (d cutie 5), cutie);
        assert_eval!(env, cutie, 5);
    }

    #[test]
    fn type_() {
        let mut env = Env::new();

        assert_eval!(env, (type 1), Int);
        assert_eval!(env, (type (q cutie)), Name);
        assert_eval!(env, (type ()), List);
        assert_eval!(env, (type q), Builtin);
    }

    #[test]
    fn chars() {
        let mut env = Env::new();
        assert_eval!(env, (chars (q abc)), (97 98 99));
    }

    #[test]
    fn string() {
        let mut env = Env::new();
        assert_eq!(format!("{}", eval!(env, q)), "<built-in function q>");
        assert_eq!(eval!(env, (string 423)), Value::from_str("423".to_owned()));

        assert_eval!(env, (string (q q)), q);
        assert_eval!(env, (string (q (97 98 99))), abc);
    }

    #[test]
    fn disp() {
        let mut env = Env::dummy();

        // since this prints to stdout we can't actually test the disp builtin (beyond that it doesn't error)
        // but we can also test value stringification here

        assert_eval!(env, (disp (q (1 2 (3) (4 q) hii))), ());
        assert_eq!(
            format!("{}", val!((q (1 2 (3) (4 q) hii)))),
            "(q (1 2 (3) (4 q) hii))"
        );
        println!("{:?}", env.settings.output.get_output());
    }
}
