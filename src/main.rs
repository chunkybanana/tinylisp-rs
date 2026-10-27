#![feature(let_chains)]
#![feature(macro_metavar_expr)]

use std::{fs::read_to_string, path::PathBuf};

use crate::env::Env;
use clap::{Args, Parser};

use env::{DefaultOutput, EnvSettings, WarningLevel};
#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

mod builtins;
mod env;
mod packed_value;
mod parse;

#[derive(Debug, Parser)]
#[command(name = "tinylisp-rs")]
struct Cli {
    #[command(flatten)]
    source: Option<Source>,

    /// Configure the warning level:
    ///  - strict: crash on error
    ///  - allow: print errors to console and treat them as nil
    ///  - suppress: don't print errors
    #[arg(short = 'E', long = "error", default_value_t, value_enum)]
    error: WarningLevel,

    /// Controls whether certain builtins - def, disp, comment, load - output at top level
    /// by default they do not, mimicking the reference implementation
    #[arg(short = 'T', long = "top-level-output")]
    show_top_level: bool,

    /// Controls whether the program can be parsed as multiple, autocompleted lines instead of one
    /// enabled by default, mostly for golf purposes
    #[arg(short = 'M', long = "no-multiline")]
    disable_multiline: bool,
}

#[derive(Debug, Args)]
#[group(multiple = false)]
struct Source {
    #[arg(short = 'c', long = "code")]
    /// Instead of using a file, execute code from the command line.
    code: Option<String>,
    /// The file to read the program from.
    file: Option<PathBuf>,
}

impl Source {
    fn read(self) -> Result<String, String> {
        self.code.map_or_else(
            || {
                self.file
                    .map(|path| {
                        read_to_string(path.clone())
                            .map_err(|err| format!("failed to open file {path:?}: {err}"))
                    })
                    .unwrap_or_else(|| Err("missing both code and path".to_string()))
            },
            Ok,
        )
    }
}

fn main() {
    let args = Cli::parse();
    if let Some(source) = args.source {
        match source.read() {
            Err(msg) => {
                eprintln!("error: {msg}")
            }
            Ok(code) => {
                let mut env = Env::from_settings(EnvSettings {
                    allow_multiline: !args.disable_multiline,
                    suppress_top_level: !args.show_top_level,
                    warning: args.error,
                    output: Box::new(DefaultOutput),
                });

                env.exec(&code);
            }
        }
    }
}
