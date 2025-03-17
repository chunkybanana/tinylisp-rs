use crate::packed_value::{LinkedList, PackedValue};
use std::rc::Rc;

pub fn parse(tokens: &mut impl Iterator<Item = String>) -> Rc<LinkedList> {
    let token = tokens.next();
    match token {
        None => LinkedList::nil(),
        Some(token) => {
            if token == ")" {
                return LinkedList::nil();
            }
            let element = if token == "(" {
                PackedValue::from_ll(parse(tokens))
            } else {
                match token.parse::<u64>() {
                    Ok(i) => PackedValue::from_int(i.try_into().unwrap()),
                    Err(_) => PackedValue::from_str(token.clone()),
                }
            };
            LinkedList::cons(&element, &parse(tokens))
        }
    }
}

pub fn tokenise(code: &str) -> impl Iterator<Item = String> {
    let mut tokens = Vec::<String>::new();
    let mut chars = code.chars().peekable();
    while chars.peek().is_some() {
        let char = chars.next().unwrap();
        if " \n".contains(char) {
            continue;
        }
        if "()".contains(char) {
            tokens.push(char.to_string());
        } else {
            let mut token = char.to_string();
            while chars.peek().is_some_and(|c| !"\n ()".contains(*c)) {
                token.push(chars.next().unwrap());
            }
            tokens.push(token);
        }
    }
    tokens.into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::val;
    #[test]
    fn test_tokenise() {
        let code = "(a bfjhs 90dfas() )\n(";
        let result = tokenise(code);
        assert_eq!(
            result.collect::<Vec<_>>(),
            vec!["(", "a", "bfjhs", "90dfas", "(", ")", ")", "("]
        );
    }

    #[test]
    fn test_parse() {
        let mut tokens = tokenise("(c (1 2 3) (q (4 5 6))");

        let ast = PackedValue::from_ll(parse(&mut tokens));

        assert_eq!(ast, val!(((c (1 2 3) (q (4 5 6))))));
    }
}
