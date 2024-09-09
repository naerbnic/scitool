# `scitool`

`scitool` is a command line tool for working with Sierra adventure games written in the SCI engine.
Right now, this is mostly targetted for the Space Quest 5 Fan Dub project, so the tools that are
provided are mostly for extracting and analyzing the game's resources.

This is a work in progress, so full documentation is not available yet.

## Installation

You will need to install Rust's Cargo in order to build this project. You can install it by following
the instructions at [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).
Once installed, go to the root of the project and run:

```bash
$ cargo build
...
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.45s
```

This will download the dependencies and build the program. Once built, you can use the cargo
program to run it like so:

```bash
$ cargo run -- --help
Usage: scitool.exe <COMMAND>
...
```

I will try to add appropriate documentation for the CLI as I go along.
