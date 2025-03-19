// _packed_ values are values that fit into a single 8-byte word, via liberal use of unsafe transmutations.
// As such, touching this code without sufficient care may cause undefined behaviour, which may result in forcefem. Sorry!

// Memory layout:
// - Lists are stored as Rc<LinkedList>s, with top two bits as 0 (since they're smart pointers),
//   so they can just be transmuted directly into Rcs and manipulated as such.
// - i64s are stored as bits 01 followed by the int plus 2^61 - allowing -2^61 .. 2^61-1
// - Names are stored as bits 10 followed by a 62-bit usize - not that it matters much
// - Builtins are stored as bits 11 followed by an enum member. this is overkill

use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::{self, Display},
    mem::transmute,
    ops::Deref,
    rc::Rc,
};

use thiserror::Error;

use crate::builtins::BUILTIN_LIST;

pub type ValueResult = Result<PackedValue, Error>;

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
    Load,
    Comment,
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
    string_to_int: HashMap<String, u32>,
}

impl StringInterner {
    fn new() -> StringInterner {
        StringInterner {
            str_refs: vec![],
            string_to_int: HashMap::new(),
        }
    }
    pub fn string_to_ref(&mut self, str: String) -> u32 {
        if let Some(index) = self.string_to_int.get(&str) {
            return *index;
        }
        let str_ref = &*str.clone().leak();
        self.str_refs.push(str_ref);

        let index = (self.str_refs.len() - 1) as u32;
        self.string_to_int.insert(str, index);
        index
    }
    pub fn ref_to_string(&self, index: u32) -> &'static str {
        self.str_refs[index as usize]
    }
}

// I apologise to the Rust gods
thread_local! {
    pub static STRING_INTERNER: RefCell<StringInterner> = RefCell::new(StringInterner::new());
    /*
    pub static POOL: RefCell<Pool<Value>> = RefCell::new(Pool::new(1 << 20));*/
}

pub fn lookup_str(index: u32) -> &'static str {
    STRING_INTERNER.with_borrow(|interner| interner.ref_to_string(index))
}

// todo: debug impl
#[derive(Debug)]
pub struct PackedValue(u64);

// I guess we doin C now

const TYPE_NUM: u64 = 0b01 << 62;
const TYPE_NAME: u64 = 0b10 << 62;
const TYPE_BUILTIN: u64 = 0b11 << 62;

// the exact ordering of this is very important, as we perform transmutations to it for typecheck purposes
#[repr(u8)]
#[derive(PartialEq, Debug)]
#[allow(dead_code)] // transmutation ≠ initialisation
pub enum Type {
    List,
    Num,
    Name,
    Builtin,
}

// unique representation of 0, for truthiness checking
const NUM_0: u64 = TYPE_NUM + (1 << 61);

const BITMASK_TYPE: u64 = 0b11 << 62;

// fun fact: since 00-tagged PackedValues are bitwise identical to Rc<LinkedList> pointers, they can never be null
// so it is _theoretically_ possible to implement null pointer optimisation for lists here
// there is no reason any sane individual would want to attempt this, but I think we're a bit past that point

const TWO_POW_61: i64 = 1 << 61;
const UNTAGGED_MASK: u64 = (1 << 62) - 1;

impl PackedValue {
    pub fn from_int(int: i64) -> Self {
        let represented_int = (int + TWO_POW_61) as u64;
        PackedValue(TYPE_NUM | represented_int)
    }

    pub fn from_name(ind: u32) -> Self {
        PackedValue(TYPE_NAME | (ind as u64))
    }

    pub fn from_str(str: String) -> Self {
        Self::from_name(STRING_INTERNER.with_borrow_mut(|interner| interner.string_to_ref(str)))
    }

    pub fn from_ll(ll: Rc<LinkedList>) -> Self {
        unsafe { transmute::<Rc<LinkedList>, Self>(ll) }
    }
    #[allow(dead_code)]
    pub fn from_builtin(builtin: Builtin) -> Self {
        let int = unsafe { transmute::<Builtin, u8>(builtin) };
        PackedValue(TYPE_BUILTIN | (int as u64))
    }

    pub fn nil() -> Self {
        Self::from_ll(LinkedList::nil())
    }

    pub fn is_list(&self) -> bool {
        self.0 & BITMASK_TYPE == 0
    }

    pub fn is_int(&self) -> bool {
        self.0 & BITMASK_TYPE == TYPE_NUM
    }

    pub fn is_str(&self) -> bool {
        self.0 & BITMASK_TYPE == TYPE_NAME
    }

    pub fn is_builtin(&self) -> bool {
        self.0 & BITMASK_TYPE == TYPE_BUILTIN
    }

    pub fn is_truthy(&self) -> bool {
        self.0 != NUM_0 && !self.is_nil()
    }

    pub fn is_nil(&self) -> bool {
        self.is_list() && matches!(Rc::deref(self.to_ll_ref()), LinkedList::Nil)
    }

    pub fn from_vec(vec: &[PackedValue]) -> Self {
        Self::from_ll(LinkedList::from_vec(vec))
    }

    // unchecked casts - use with caution, or when you feel like it
    pub fn to_int(&self) -> i64 {
        // remove the two leading tag bits
        let untagged = self.0 & UNTAGGED_MASK;
        (untagged as i64) - TWO_POW_61
    }

    pub fn to_name(&self) -> u32 {
        self.0 as u32 // truncated
    }

    pub fn _type(&self) -> Type {
        unsafe { std::mem::transmute::<u8, Type>(((self.0 & BITMASK_TYPE) >> 62) as u8) }
    }

    pub fn tl_type(&self) -> &'static str {
        match self._type() {
            Type::Builtin => "builtin",
            Type::Num => "int",
            Type::Name => "name",
            Type::List => {
                let rc = self.to_ll_ref();
                let result = match Rc::deref(rc) {
                    LinkedList::Nil => "nil",
                    LinkedList::List { .. } => "list",
                };
                result
            }
        }
    }
    // consumes Self to yield a raw Rc
    // When an Rc is transmuted into a PackedValue, it is not dropped, and this returns the underlying Rc
    // As such, using these methods is safe, but performing a bitwise copy and then transmuting both back to Rcs
    // will result in the reference counter underflowing when dropped. this is probably a bad thing
    pub fn into_ll(self) -> Rc<LinkedList> {
        unsafe { transmute::<Self, Rc<LinkedList>>(self) }
    }

    // in _theory_ the result here should live as long as self
    pub fn to_ll_ref<'a>(&'a self) -> &'a Rc<LinkedList> {
        unsafe { transmute::<&'a Self, &'a Rc<LinkedList>>(self) }
    }

    pub fn to_builtin(&self) -> Builtin {
        // the truncating cast here is fine
        unsafe { transmute::<u8, Builtin>(self.0 as u8) }
    }
}

// We can't quite get away with directly bit-copying these - as that wouldn't update the Rcs - but we can get very close.
// To clone a PackedValue, all we need to do is check if the underlying representation is an Rc, and if so transmute that and clone it
// then copy the bits directly

impl Clone for PackedValue {
    fn clone(&self) -> Self {
        if self.is_list() {
            Self::from_ll(self.to_ll_ref().clone())
        } else {
            PackedValue(self.0) // copy
        }
    }
}

impl PartialEq for PackedValue {
    fn eq(&self, other: &PackedValue) -> bool {
        if self.is_list() && other.is_list() {
            return self.to_ll_ref() == other.to_ll_ref();
        }
        self.0 == other.0
    }
}

impl Display for PackedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self._type() {
            Type::Num => write!(f, "{}", self.to_int()),
            Type::Name => write!(f, "{}", lookup_str(self.to_name())),
            Type::Builtin => write!(f, "{}", self.to_builtin()),
            Type::List => {
                let mut iter = self.to_ll_ref().iter();
                write!(f, "(")?;
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

#[derive(Debug, PartialEq)]
#[allow(clippy::module_name_repetitions)]
pub enum LinkedList {
    Nil,
    List {
        head: PackedValue,
        tail: Rc<LinkedList>,
    },
}

impl LinkedList {
    pub fn to_vec(&self) -> Vec<&PackedValue> {
        self.iter().collect::<Vec<_>>()
    }

    pub fn copy_to_vec(&self) -> Vec<PackedValue> {
        self.iter().cloned().collect::<Vec<_>>()
    }

    pub fn from_vec(vec: &[PackedValue]) -> Rc<LinkedList> {
        let mut tail = LinkedList::nil();
        for val in vec.iter().rev() {
            tail = Rc::new(LinkedList::List {
                head: val.clone(),
                tail,
            });
        }
        tail
    }

    pub fn nil() -> Rc<LinkedList> {
        Rc::new(LinkedList::Nil)
    }

    pub fn head(&self) -> PackedValue {
        match self {
            LinkedList::Nil => PackedValue::nil(),
            LinkedList::List { head, tail: _ } => head.clone(),
        }
    }

    pub fn tail(&self) -> Rc<LinkedList> {
        match self {
            LinkedList::Nil => LinkedList::nil(),
            LinkedList::List { head: _, tail } => Rc::clone(tail),
        }
    }

    pub fn cons(head: &PackedValue, tail: &Rc<LinkedList>) -> Rc<LinkedList> {
        Rc::new(LinkedList::List {
            head: head.clone(),
            tail: Rc::clone(tail),
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &PackedValue> {
        LinkedListIter { list: self }
    }
}

struct LinkedListIter<'a> {
    list: &'a LinkedList,
}

impl<'a> Iterator for LinkedListIter<'a> {
    type Item = &'a PackedValue;
    fn next(&mut self) -> Option<&'a PackedValue> {
        match self.list {
            LinkedList::Nil => None,
            LinkedList::List { head, tail } => {
                self.list = tail;
                Some(head)
            }
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
    #[error("error loading module {path}: {err}")]
    ModuleNotFound { path: String, err: String },
}

#[macro_export]
macro_rules! val {
    ($num:literal) => {
        PackedValue::from_int($num)
    };
    ($name:ident) => {
        PackedValue::from_str(stringify!($name).to_owned())
    };
    (()) => {
        PackedValue::nil()
    };
    (($($val:tt)*)) => {
        PackedValue::from_vec(&[
            $(
                val!($val),
            )*
        ])
    };
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;

    #[test]
    fn test_values() {
        let int = PackedValue::from_int(21);
        assert_eq!(int.to_int(), 21);

        let int = PackedValue::from_int(-52);
        assert_eq!(int.to_int(), -52);

        let name = PackedValue::from_name(25);
        assert_eq!(name.to_name(), 25);

        let rc = Rc::new(LinkedList::Nil);
        let list = PackedValue::from_ll(rc.clone());
        assert_eq!(list.into_ll(), rc);

        let builtin = PackedValue::from_builtin(Builtin::Load);
        assert_eq!(builtin.to_builtin(), Builtin::Load);
    }

    #[test]
    fn test_list() {
        let list = [val!(5), val!(3), val!(((3) cutie))];
        let ll = LinkedList::from_vec(&list);

        assert_eq!(
            PackedValue::from_ll(Rc::clone(&ll)),
            val!((5 3 ((3) cutie)))
        );
        assert_eq!(ll.copy_to_vec(), list);

        assert_eq!(ll.head(), val!(5));
        assert_eq!(ll.tail(), LinkedList::from_vec(&list[1..]));
    }
}
