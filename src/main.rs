#![feature(let_chains)]
#![feature(macro_metavar_expr)]
#![allow(dead_code)]

use std::env::args;

use crate::env::Env;

mod builtins;
mod env;
mod list;
mod parse;
mod value;

fn main() {
    let args = args();
}
