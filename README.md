# tinylisp-rs

This is a port of DLosc's [tinylisp](https://github.com/dloscutoff/Esolangs/tree/master/tinylisp) to Rust. As such, it attempts to maintain compatibility with the provided Python interpreter by default.


Usage: 
```sh
tinylisp-rs [filename] [options]
```

If no filename is provided, the interpreter will run a REPL similar to that of the reference implementation.


Command-line options:

- `-E`, `--error`: Takes an error level, one of `allow`, `suppress`, `strict`
  - `allow` (default behaviour) maintains the error behaviour of the reference implementation: almost any function call that would result in an error instead returns nil and prints an error to the console.
  - `suppress` behaves like `allow`, but suppresses console output of errors
  - `strict` halts the program if any errors occur
- `-c`, `--code`: Takes a literal string of code as input
- `-T`, `--top-level-output`: Makes certain builtins - load, disp, def, comment - output at the top level when they would otherwise be suppressed

Even with intmaps, hashmap lookups and creation is still taking up a huge amount of the runtime
and from the flamegraph it looks like 