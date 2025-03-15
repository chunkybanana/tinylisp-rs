#![allow(dead_code)]

use refpool::Pool;
use thiserror::Error;

use crate::builtins::BUILTIN_LIST;
use crate::list::LinkedList;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::ops::Deref;
use std::rc::Rc;

#[derive(Debug, PartialEq)]
pub enum Value {
    Int(i64),
    List(Rc<LinkedList>),
    Name(usize), // interned
    Builtin(Builtin),
}

pub type ValueResult = Result<Value, Error>;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Builtin {
    Cons,
    Head,
    Tail,
    Sub,
    Less,
    Eq,
    Eval,
    Add,
    Quote,
    If,
    Def,
    Chars,
    String,
    Disp,
    Type,
}

impl Display for Builtin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "<built-in function {}>",
            BUILTIN_LIST
                .iter()
                .find(|(_, builtin, _)| builtin == self)
                .unwrap()
                .0
        )
    }
}
// DIY string interning
pub struct StringInterner {
    str_refs: Vec<&'static str>, // map from int to str
    string_to_int: HashMap<String, usize>,
}

impl StringInterner {
    fn new() -> StringInterner {
        StringInterner {
            str_refs: vec![],
            string_to_int: HashMap::new(),
        }
    }
    pub fn string_to_ref(&mut self, str: String) -> usize {
        if let Some(index) = self.string_to_int.get(&str) {
            return *index;
        }
        let str_ref = &*str.clone().leak();
        self.str_refs.push(str_ref);

        let index = self.str_refs.len() - 1;
        self.string_to_int.insert(str, index);
        index
    }
    pub fn ref_to_string(&self, index: usize) -> &'static str {
        self.str_refs[index]
    }
    pub fn add_string(&mut self, str: String) -> &'static str {
        let index = self.string_to_ref(str);
        self.str_refs[index]
    }
}

// I apologise to the Rust gods
thread_local! {
    pub static STRING_INTERNER: RefCell<StringInterner> = RefCell::new(StringInterner::new());
    pub static POOL: RefCell<Pool<Value>> = RefCell::new(Pool::new(1 << 21));
}

pub fn lookup_str(index: usize) -> &'static str {
    STRING_INTERNER.with_borrow(|interner| interner.ref_to_string(index))
}

impl Value {
    pub fn from_int(int: i64) -> Value {
        Value::Int(int)
    }

    pub fn from_str(str: String) -> Value {
        Value::Name(STRING_INTERNER.with_borrow_mut(|interner| interner.string_to_ref(str)))
    }

    pub fn from_builtin(builtin: Builtin) -> Value {
        Value::Builtin(builtin)
    }

    pub fn from_ll(list: Rc<LinkedList>) -> Value {
        Value::List(list)
    }

    pub fn from_vec(list: &[Value]) -> Value {
        Value::from_ll(LinkedList::from_vec(list))
    }

    pub fn nil() -> Value {
        Value::List(Rc::new(LinkedList::Nil))
    }

    pub fn tl_type(&self) -> &'static str {
        match self {
            Value::Builtin(_) => "builtin",
            Value::Int(_) => "int",
            Value::Name(_) => "name",
            Value::List(list) => match Rc::deref(list) {
                LinkedList::Nil => "nil",
                LinkedList::List { .. } => "list",
            },
        }
    }

    pub fn is_nil(&self) -> bool {
        if let Value::List(ls) = self {
            return matches!(Rc::deref(ls), LinkedList::Nil);
        }
        false
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::List(ls) => !matches!(Rc::deref(ls), LinkedList::Nil),
            Value::Int(int) => *int != 0,
            _ => true,
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(int) => write!(f, "{int}"),
            Value::Name(str) => write!(f, "{}", lookup_str(*str)),
            Value::Builtin(builtin) => write!(f, "{builtin}"),
            Value::List(list) => {
                write!(f, "(")?;
                let mut iter = list.iter();
                if let Some(val) = iter.next() {
                    write!(f, "{val}")?;
                }
                for val in iter {
                    write!(f, " {val}")?;
                }
                write!(f, ")")
            }
        }
    }
}

// copy the value, or clone the underlying Rc
impl Clone for Value {
    fn clone(&self) -> Self {
        match self {
            Value::Builtin(builtin) => Value::Builtin(*builtin),
            Value::Int(int) => Value::Int(*int),
            Value::Name(str) => Value::Name(*str),
            Value::List(list) => Value::List(Rc::clone(list)),
        }
    }
}

#[derive(Debug, PartialEq, Error)]
pub enum Error {
    #[error("could not find name {}", .0)]
    VariableNotFound(String),
    #[error("error while calling {name}: {error}")]
    FunctionCall { name: String, error: String },
    #[error("")] // converted to FunctionCalls
    _FunctionCall(String),
    #[error("Tried to call {name}, which is of type {type_}")]
    CalledNonFunction { name: String, type_: String },
    #[error("{}", .0)]
    BuiltinArgumentType(String),
    #[error("builtin {name} recieved {argc} arguments, expected {expected}")]
    BuiltinArgumentCount {
        name: String,
        argc: usize,
        expected: usize,
    },
    #[error("")] // internal
    _MalformedFunctionBody(String),
    #[error("function {name} is malformed: {error}")]
    MalformedFunctionBody { name: String, error: String },
}

#[macro_export]
macro_rules! val {
    ($num:literal) => {
        Value::from_int($num)
    };
    ($name:ident) => {
        Value::from_str(stringify!($name).to_owned())
    };
    (()) => {
        Value::nil()
    };
    (($($val:tt)*)) => {
        Value::from_vec(&[
            $(
                val!($val),
            )*
        ])
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_types() {
        assert_eq!(val!(5).tl_type(), "int");
        assert_eq!(val!(()).tl_type(), "nil");
        assert_eq!(val!(name).tl_type(), "name");
        assert_eq!(val!((5())).tl_type(), "list");
        assert_eq!(Value::Builtin(Builtin::Cons).tl_type(), "builtin");
    }

    #[test]
    fn test_eq() {
        assert_eq!(val!(5), val!(5));
        assert_eq!(val!(cutie), val!(cutie));
        assert_eq!(val!(()), val!(()));
        assert_eq!(val!((5 () cutie)), val!((5 () cutie)));

        assert_ne!(val!((5 () cutie)), val!(cutie));
    }
}
