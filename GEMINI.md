# Gemini Code Assistant Context

## Project Overview

This project, `scitool`, is a command-line interface (CLI) tool developed in Rust for interacting with game files from Sierra On-Line's SCI (Sierra Creative Interpreter) engine. The primary goal of this tool is to support the "Space Quest 5 Fan Dub" project by providing utilities to extract, analyze, and manipulate the game's resources.

The project is structured as a Rust workspace containing three main crates:

* **`scidev`**: A library crate that encapsulates the core logic for parsing and handling SCI game data. It provides the foundational functionalities for reading and writing resources.
* **`sciproj`**: A library crate that manages project-level concerns, including configuration, file pattern matching, and project state management.
* **`scidev-cli`**: The unified CLI application (binary name: `scidev`) that offers a comprehensive set of commands. It includes functionalities for:
  * **Resources**: Listing, dumping, extracting, and packing game resources.
  * **Project**: initializing and managing projects.
  * **Fan Dub**: Specialized tools for audio compilation and script management (e.g., generating CSVs for recording).

## Building and Running

To build and run this project, you need to have the Rust toolchain (including `cargo`) installed.

### Building the Project

To build the entire workspace, run the following command from the project's root directory:

```bash
cargo build
```

### Running the CLI

The main binary is named `scidev`. You can run it using `cargo run --bin scidev` or by navigating to the cli crate.

```bash
cargo run --bin scidev -- --help
```

This will display the main help menu, which outlines the different categories of commands available, such as `resources` (alias `res`), `script`, `book`, etc.

#### Example: Listing Game Resources

To list all the resources in a game directory, you can use the `resources list` command:

```bash
cargo run --bin scidev -- resources list /path/to/game
```

## Development Conventions

* **Workspace Structure**: The project is organized as a Cargo workspace, with shared dependencies and settings defined in the root `Cargo.toml`.
* **Crate Organization**: The code is modularized into core libraries (`scidev`, `sciproj`) and a single command-line interface (`scidev-cli`). The CLI crate strictly separates interface definition (`src/cli/`) from command implementation (`src/cmds/`).
* **Command-Line Interface**: The CLIs are built using the `clap` crate, which is the standard for creating robust and user-friendly command-line applications in Rust.
* **Error Handling**: The project uses the `anyhow` and `thiserror` crates for error handling, which is a common pattern for ergonomic and informative error management in Rust applications.
* **Linting**: The project has `clippy.toml` and workspace lint configurations in `Cargo.toml` to enforce code quality and style.

## Rust Conventions

* **Encapsulation**
  * Structs should almost never have public fields, either global or otherwise. THIS APPLIES TO ALL TYPES, MODULE INTERNAL AND EXTERNAL.
  * Factory functions and accessor methods should be used only when it would be appropriate for clients of the struct to have access to the values in the fields.
  * Type definitions should be limited to only the scope needed for the module. Any variances from this must be documented.
