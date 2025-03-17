#![allow(dead_code)]

use crate::value::Value;
use std::rc::Rc;

#[derive(Debug, PartialEq)]
#[allow(clippy::module_name_repetitions)]
pub enum LinkedList {
    Nil,
    List { head: Value, tail: Rc<LinkedList> },
}

impl LinkedList {
    pub fn to_vec(&self) -> Vec<&Value> {
        self.iter().collect::<Vec<_>>()
    }

    pub fn copy_to_vec(&self) -> Vec<Value> {
        self.iter().cloned().collect::<Vec<_>>()
    }

    pub fn from_vec(vec: &[Value]) -> Rc<LinkedList> {
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

    pub fn head(&self) -> Value {
        match self {
            LinkedList::Nil => Value::nil(),
            LinkedList::List { head, tail: _ } => head.clone(),
        }
    }

    pub fn tail(&self) -> Rc<LinkedList> {
        match self {
            LinkedList::Nil => LinkedList::nil(),
            LinkedList::List { head: _, tail } => Rc::clone(tail),
        }
    }

    pub fn cons(head: &Value, tail: &Rc<LinkedList>) -> Rc<LinkedList> {
        Rc::new(LinkedList::List {
            head: head.clone(),
            tail: Rc::clone(tail),
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &Value> {
        LinkedListIter { list: self }
    }
}

struct LinkedListIter<'a> {
    list: &'a LinkedList,
}

impl<'a> Iterator for LinkedListIter<'a> {
    type Item = &'a Value;
    fn next(&mut self) -> Option<&'a Value> {
        match self.list {
            LinkedList::Nil => None,
            LinkedList::List { head, tail } => {
                self.list = tail;
                Some(head)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::val;
    #[test]
    fn test_list() {
        let list = [val!(5), val!(3), val!(((3) cutie))];
        let ll = LinkedList::from_vec(&list);

        assert_eq!(Value::from_ll(Rc::clone(&ll)), val!((5 3 ((3) cutie))));
        assert_eq!(ll.copy_to_vec(), list);

        assert_eq!(ll.head(), val!(5));
        assert_eq!(ll.tail(), LinkedList::from_vec(&list[1..]));
    }
}
