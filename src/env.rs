use itertools::{EitherOrBoth, Itertools};
use nohash_hasher::IntMap;

use crate::{
    //builtins::{BUILTIN_LIST, BUILTINS},
    packed_value::{
        Builtin, Error, LinkedList, PackedValue, STRING_INTERNER, ValueResult, lookup_str,
    },
    parse::{parse, tokenise},
};
use std::{
    collections::{HashMap, HashSet},
    env,
    fs::read_to_string,
    ops::Deref,
    path::{Path, PathBuf},
    rc::Rc,
    sync::LazyLock,
};

#[derive(Debug, PartialEq)]
pub enum WarningLevel {
    Allow,
    Suppress,
    Strict,
}

pub struct EnvSettings {
    pub warning: WarningLevel,
    pub suppress_top_level: bool,
    pub output: Box<dyn Output>,
}

type Dict = IntMap<usize, PackedValue>;
pub struct Env {
    pub global_dict: Dict,
    local_scopes: Vec<Dict>, // literally a call stack
    loaded_modules: HashSet<PathBuf>,
    module_stack: Vec<PathBuf>,
    pub settings: EnvSettings,
}

pub trait Output {
    fn print(&mut self, str: String);
    fn get_output(&mut self) -> Vec<String> {
        vec![]
    }
}

struct DefaultOutput;

impl Output for DefaultOutput {
    fn print(&mut self, str: String) {
        println!("{str}");
    }
}

// collects output into a vec for testing purposes, mostly
pub struct DummyOutput {
    pub vec: Vec<String>,
}

// the JANK I have had to go through to get an object whose output can be extracted...
impl DummyOutput {
    pub fn new() -> DummyOutput {
        DummyOutput { vec: vec![] }
    }
}

impl Output for DummyOutput {
    fn print(&mut self, str: String) {
        self.vec.push(str);
    }
    fn get_output(&mut self) -> Vec<String> {
        self.vec.clone()
    }
}

macro_rules! load_lib {
    ($str:literal) => {
        (
            concat!("lib/", $str, ".tl"),
            include_str!(concat!("lib/", $str, ".tl")),
        )
    };
}

static STDLIB: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| {
    HashMap::from([
        ("library.tl", include_str!("lib/library.tl")),
        load_lib!("lists"),
        load_lib!("long-names"),
        load_lib!("math"),
        load_lib!("matrices"),
        load_lib!("metafunctions"),
        load_lib!("sorting"),
        load_lib!("string"),
        load_lib!("utilities"),
    ])
});

impl Env {
    pub fn build_global_dict() -> Dict {
        let mut dict: Dict = IntMap::default();

        /*for (str, builtin, _fn) in BUILTIN_LIST {
            let interned_ref =
                STRING_INTERNER.with_borrow_mut(|interner| interner.string_to_ref(str.to_string()));
            dict.insert(interned_ref, PackedValue::from_builtin(*builtin));
        }*/
        dict
    }

    pub fn default_settings() -> EnvSettings {
        EnvSettings {
            warning: WarningLevel::Strict,
            suppress_top_level: false,
            output: Box::new(DefaultOutput {}),
        }
    }

    pub fn new() -> Env {
        Self::from_settings(Self::default_settings())
    }

    pub fn from_settings(settings: EnvSettings) -> Env {
        Env {
            global_dict: Self::build_global_dict(),
            settings,
            local_scopes: vec![],
            loaded_modules: HashSet::<PathBuf>::new(),
            module_stack: vec![],
        }
    }

    pub fn dummy() -> Env {
        Env::from_settings(EnvSettings {
            suppress_top_level: false,
            warning: WarningLevel::Allow,
            output: Box::new(DummyOutput::new()),
        })
    }

    pub fn println(&mut self, str: String) {
        self.settings.output.print(str);
    }

    // the `load` builtin - eval the contents of a file
    // the standard library is just statically linked - we essentially
    pub fn load_file(&mut self, path: String) -> Result<(), Error> {
        //println!("loading {path}");

        if !path.ends_with(".tl") {
            return self.load_file(path + ".tl");
        }

        let default = &PathBuf::new();
        let current_path = self.module_stack.last().unwrap_or(default);

        let full_path = Path::new(&current_path).join(&path);

        if self.loaded_modules.contains(&full_path) {
            return Ok(());
        }
        self.loaded_modules.insert(full_path.clone());

        //println!("full path {full_path:?} {:?}", self.module_stack);

        if let Some(code) = STDLIB.get(full_path.to_str().unwrap()) {
            let parent = full_path.parent().unwrap(); // if this errors something has gone very wrong
            self.module_stack.push(parent.to_path_buf());
            self.exec(code);
            self.module_stack.pop();
            return Ok(());
        }

        // not in stdlib, load a file - relative to the current target directory

        let new_path = env::current_dir().unwrap().join(full_path.clone());

        match read_to_string(new_path) {
            Ok(code) => {
                self.exec(code.as_str());
                Ok(())
            }
            Err(err) => Err(Error::ModuleNotFound {
                path: full_path.to_string_lossy().to_string(),
                err: err.to_string(),
            }),
        }
    }

    fn exec(&mut self, code: &str) {
        let ast = parse(&mut tokenise(code));
        /*for expr in ast.iter() {
            match self.eval(expr) {
                Ok(val) => {
                    // and this is why I use nightly
                    if self.settings.suppress_top_level
                        && let PackedValue::List(ll) = expr
                        && let LinkedList::List { head, tail: _ } = Rc::deref(ll)
                        && let Ok(PackedValue::Builtin(builtin)) = self.eval(head)
                        && matches!(
                            builtin,
                            Builtin::Def | Builtin::Disp | Builtin::Load | Builtin::Comment
                        )
                    {
                    } else {
                        self.println(format!("{val}"));
                    }
                }
                Err(err) => self.println(format!("{err:?}")),
            }
        }*/
    }

    fn lookup(&self, key: usize) -> ValueResult {
        self.local_scopes
            .last()
            .and_then(|dict| dict.get(&key))
            .or_else(|| self.global_dict.get(&key))
            .map_or_else(
                || Err(Error::VariableNotFound(lookup_str(key).to_string())),
                |val| Ok(val.clone()),
            )
    }

    pub fn eval(&mut self, expr: &PackedValue) -> ValueResult {
        if expr.is_builtin() || expr.is_int() {
            return Ok(expr.clone());
        }

        if expr.is_str() {
            return self.lookup(expr.to_name());
        }

        let call = Rc::deref(expr.to_ll_ref());

        let (head, tail) = match call {
            LinkedList::Nil => return Ok(expr.clone()),
            LinkedList::List { head, tail } => (head, tail),
        };

        // we're in a function call of some sort
        let function = self.eval(head)?;
        let raw_args = tail;

        let result = if function.is_builtin() {
            self.call_builtin(function.to_builtin(), raw_args)
        } else if function.is_list() {
            self.call_function(function.to_ll_ref().clone(), Rc::clone(raw_args))
        } else {
            Err(Error::CalledNonFunction {
                name: format!("{}", head),
                type_: function.tl_type().to_string(),
            })
        };

        if self.settings.warning != WarningLevel::Strict
            && let Err(error) = result
        {
            if self.settings.warning == WarningLevel::Allow {
                self.println(format!("{error:?}"));
            }
            return Ok(PackedValue::nil());
        }
        match result {
            Ok(_) => result,
            Err(err) => match err {
                Error::_MalformedFunctionBody(message) => Err(Error::MalformedFunctionBody {
                    name: format!("{}", head),
                    error: message.clone(),
                }),
                Error::_FunctionCall(error) => Err(Error::FunctionCall {
                    name: format!("{}", head),
                    error,
                }),
                _ => Err(err),
            },
        }
    }

    fn call_builtin(&mut self, builtin: Builtin, raw_args: &Rc<LinkedList>) -> ValueResult {
        todo!()
        //BUILTINS.get(&builtin).unwrap()(self, raw_args)
    }

    // Gets the relevant parameters to call a user-defined function
    // todo: maybe don't use vecs?
    fn call_info(
        &mut self,
        function: &Rc<LinkedList>,
        raw_args: &Rc<LinkedList>,
    ) -> Result<(PackedValue, PackedValue, Vec<PackedValue>), Error> {
        let func_list = function.to_vec();

        match func_list.len() {
            2 => Ok((
                func_list[0].clone(),
                func_list[1].clone(),
                raw_args
                    .iter()
                    .map(|val| self.eval(val))
                    .collect::<Result<Vec<PackedValue>, Error>>()?,
            )),
            3 => {
                if !func_list[0].is_nil() {
                    Err(Error::_MalformedFunctionBody(
                        "macro head is not nil".to_owned(),
                    ))?;
                }
                Ok((
                    func_list[1].clone(),
                    func_list[2].clone(),
                    raw_args.copy_to_vec(),
                ))
            }
            0 | 1 => Err(Error::_MalformedFunctionBody(
                "function body missing".to_owned(),
            ))?,
            len => Err(Error::_MalformedFunctionBody(format!(
                "length is {len} when it should be 2 or 3"
            )))?,
        }
    }

    // Parse parameters and init the local dict for a function call
    fn get_local_dict(params: &PackedValue, args: Vec<PackedValue>) -> Result<Dict, Error> {
        todo!();

        let mut local_dict: Dict = IntMap::default();
        /*
        match params {
            PackedValue::Name(name) => {
                local_dict.insert(*name, PackedValue::from_vec(&args));
            }
            PackedValue::List(list) => {
                let arg_count = args.len();
                for val in list.iter().zip_longest(args) {
                    match val {
                        EitherOrBoth::Both(name, param) => match name {
                            PackedValue::Name(str) => {
                                local_dict.insert(*str, param.clone());
                            }
                            _ => Err(Error::_FunctionCall(format!(
                                "malformed parameters: expected str, was {}",
                                name.tl_type()
                            )))?,
                        },
                        _ => {
                            Err(Error::_FunctionCall(format!(
                                "wrong number of args passed: expected {}, got {}",
                                list.to_vec().len(),
                                arg_count
                            )))?;
                        }
                    }
                }
            }
            _ => Err(Error::_FunctionCall(format!(
                "malformed parameters: was {}",
                params.tl_type()
            )))?,
        }

        Ok(local_dict)*/
    }

    fn call_function(
        &mut self,
        mut function: Rc<LinkedList>,
        mut raw_args: Rc<LinkedList>,
    ) -> ValueResult {
        let mut in_tail_call = false;
        todo!();
        /*
        // When we reach the point where we want to perform a tail call, we continue and return to this point
        'tco: loop {
            let (params, mut body, args) = self.call_info(&function, &raw_args)?;

            // Once we know we're doing a tail call (i.e. this loop has run more than once)
            // We want to pop the scope from the previous tail call after evaluating this one's args
            if in_tail_call {
                self.local_scopes.pop();
            }
            in_tail_call = true;

            self.local_scopes.push(Env::get_local_dict(&params, args)?);

            // Eliminate if / evals from the body
            'elim: loop {
                // if we hit a function call, eval the function and check
                if let PackedValue::List(f_call) = &body
                    && let LinkedList::List { head, tail } = Rc::deref(f_call)
                {
                    let func = self.eval(head)?;
                    let fn_args = tail;

                    if let PackedValue::List(func) = func {
                        // user-defined function - attempt to perform a tail call
                        function = func;
                        raw_args = Rc::clone(fn_args);
                        continue 'tco;
                    } else if let PackedValue::Builtin(builtin) = func {
                        if builtin == Builtin::If {
                            // due to various nonsense with the generated iterator somehow being owned by body - even though
                            // it really shouldn't, considering it's behind a Rc - body needs to be assigned after
                            // the iterator is dropped
                            body = {
                                let mut iter = fn_args.iter();
                                let args = (iter.next(), iter.next(), iter.next());
                                if args.2.is_none() || iter.next().is_some() {
                                    Err(Error::BuiltinArgumentCount {
                                        name: "if".to_owned(),
                                        argc: fn_args.to_vec().len(),
                                        expected: 3,
                                    })?;
                                }
                                let cond = self.eval(args.0.unwrap())?;
                                if cond.is_truthy() { args.1 } else { args.2 }
                                    .unwrap()
                                    .clone()
                            };
                            continue 'elim;
                        } else if builtin == Builtin::Eval {
                            body = {
                                let mut iter = fn_args.iter();
                                let arg = iter.next();
                                if arg.is_none() || iter.next().is_some() {
                                    Err(Error::BuiltinArgumentCount {
                                        name: "eval".to_owned(),
                                        argc: fn_args.to_vec().len(),
                                        expected: 1,
                                    })?;
                                }
                                self.eval(arg.unwrap())?
                            };
                            continue 'elim;
                        }
                    }
                }
                break;
            }
            let ret = self.eval(&body);
            self.local_scopes.pop();
            return ret;
        }
        */
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crate::{assert_eval, env::Env, eval, packed_value::PackedValue, val};

    use super::{DummyOutput, EnvSettings, WarningLevel};

    /*#[test]
    fn test_user_fns() {
        let mut env = Env::new();

        // functions
        eval!(env, (d plus3 (q ((v) (a v 3)))));
        assert_eval!(env, (plus3 5), 8);

        // functions calling other functions
        eval!(env, (d plus6 (q ((n) (plus3 (plus3 n))))));
        assert_eval!(env, (plus6 13), 19);

        // functions with multiple arguments
        eval!(env, (d add_triad (q ((x y z) (a (a x y) z)))));
        assert_eval!(env, (add_triad 4 7 12), 23);

        // macros
        eval!(env, (d eval_second (q (() (a1 a2 a3) (c a1 (c (v a2) (c a3 ())))))));
        assert_eval!(
            env,
            (eval_second q (a 13 11) hii),
            (q 24 hii)
        );

        // variadic functions
        eval!(env, (d enlist (q (args args))));
        assert_eval!(env, (enlist 5 () 2), (5 () 2));

        // and macros
        eval!(env, (d hargs (q (() x (h x)))));
        assert_eval!(env, (hargs q 2 3), q);

        // regular recursion
        eval!(env, (d length (q ((ls) (i ls (a 1 (length (t ls))) 0)))));
        assert_eval!(env, (length (q (8 4 2 q 91 3))), 6);

        // tail recursion - I don't have a test for this actually _being_ tail-recursive
        eval!(env, (d len_ (q ((ls acc) (i ls (len_ (t ls) (a acc 1)) acc)))));
        eval!(env, (d len (q ((ls) (len_ ls 0)))));
        assert_eval!(env, (len (q (8 4 2 q 91 3))), 6);
    }

    #[test]
    fn testcase_6() {
        let now = Instant::now();
        let mut env = Env::new();
        eval!(env, (d nil ()));
        eval!(env, (d list (q (args args))));
        eval!(env, (d lambda (q (() (params expr) (list params expr)))));
        eval!(env, (d quote_each (lambda (lyst)
          (i lyst
            (c (list (q q) (h lyst)) (quote_each (t lyst)))
            nil))));

        eval!(env, (d apply
          (lambda (func arglist)
            (v (c (q func) (quote_each arglist))))));
        eval!(env, (d map
          (lambda (func lyst)
            (i lyst
              (c (apply func (list (h lyst)))
                (map func (t lyst)))
              nil))));
        eval!(env, (d partial
          (lambda (func arg)
            (list (q arglist) (list (q apply) (list (q q) func) (list (q c) arg (q arglist)))))));

        assert_eval!(env,
           (map (partial s 10) (list 1 2 3 4)),
           (9 8 7 6)
        );

        eval!(env, (d foldlX
            (lambda (func lyst accum)
              (i lyst
                (foldlX func (t lyst) (apply func (list accum (h lyst))))
                accum))));
        eval!(env, (d M
            (lambda arglist
              (i arglist
                (i (t arglist)
                  (foldlX s (t arglist) (h arglist))
                  (s 0 (h arglist)))
                0))));
        eval!(env, (d P
            (lambda arglist
              (s 0 (apply M (c 0 arglist))))));

        assert_eval!(env, (M 10 4 3 2), 1);
        assert_eval!(env, (P 1 2 3 4 5), 15);

        eval!(env, (d dividesQ
          (lambda (factor int)
            (i int
              (i (l int factor)
                0
                (dividesQ factor (s int factor)))
              1))));
        eval!(env, (d primeQ_
          (lambda (int test_factor)
            (i (l 1 test_factor)
              (i (dividesQ test_factor int)
                0
                (primeQ_ int (s test_factor 1)))
              1))));
        eval!(env, (d primeQ
          (lambda (int)
            (i (e int 1)
              0
              (primeQ_ int (s int 1))))));

        assert_eval!(
            env,
            (map primeQ (list 1 2 3 4 5 23 10000)),
            (0 1 1 0 1 1 0)
        );

        eval!(env, (d evenQ
          (lambda (int)
            (i int
              (i (l int 0)
                (evenQ (s 0 int))
                (oddQ (s int 1)))
              1))));
        eval!(env, (d oddQ
          (lambda (int)
            (i int
              (i (l int 0)
                (oddQ (s 0 int))
                (evenQ (s int 1)))
              0))));

        assert_eval!(
            env,
            (map evenQ (list 0 1 2 3 14 159 2653 58979 323_846)),
            (1 0 1 0 1 0 0 0 1)
        );
        println!("elapsed: {:.4?}", now.elapsed())
    }

    #[test]
    fn test_large_list() {
        let mut env = Env::new();

        eval!(env, (d _zerolist (q ((num acc) (i num (_zerolist (s num 1) (c 0 acc)) acc)))));
        eval!(env, (d zerolist (q ((num) (_zerolist num ())))));
        eval!(env, (zerolist 2500));
    }

    #[test]
    fn test_exec() {
        let mut env = Env::dummy();
        env.exec("() a (a 2 3) (disp 2) (d cutie (q evie))");
        assert_eq!(
            vec!["()", "<built-in function a>", "5", "2", "()", "cutie"],
            env.settings.output.get_output()
        );
    }

    #[test]
    fn test_suppressed() {
        let mut env = Env::from_settings(EnvSettings {
            warning: WarningLevel::Strict,
            suppress_top_level: true,
            output: Box::new(DummyOutput::new()),
        });

        env.exec("() a (a 2 3) (disp 2) (d cutie (q evie))");
        assert_eq!(
            vec!["()", "<built-in function a>", "5", "2"],
            env.settings.output.get_output()
        );
    }

    #[test]
    fn test_warnings() {
        let mut env = Env::from_settings(EnvSettings {
            warning: WarningLevel::Allow,
            suppress_top_level: true,
            output: Box::new(DummyOutput::new()),
        });

        env.exec("(c 5 (c 0 4))");

        assert_eq!(
            env.settings.output.get_output(),
            vec![
                "BuiltinArgumentType(\"tried to call builtin c with (int, int), expected (any, list)\")",
                "(5)"
            ]
        );

        let mut env = Env::from_settings(EnvSettings {
            warning: WarningLevel::Suppress,
            suppress_top_level: true,
            output: Box::new(DummyOutput::new()),
        });

        env.exec("(c 5 (c 0 4))");

        assert_eq!(env.settings.output.get_output(), vec!["(5)"]);
    }*/
}
