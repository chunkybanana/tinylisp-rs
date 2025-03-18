#![feature(let_chains)]
#![feature(macro_metavar_expr)]

use std::env::args;

use crate::env::Env;

#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

mod builtins;
mod env;
mod packed_value;
mod parse;

fn main() {
    let args = args();
}
