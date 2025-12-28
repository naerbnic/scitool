# Gemini Code Assistant Context

## Project Overview

This project, `scitool`, is a command-line interface (CLI) tool developed in Rust for interacting with game files from Sierra On-Line's SCI (Sierra Creative Interpreter) engine. The primary goal of this tool is to support the "Space Quest 5 Fan Dub" project by providing utilities to extract, analyze, and manipulate the game's resources.

The project is structured as a Rust workspace containing three main crates:

*   **`scidev`**: A library crate that encapsulates the core logic for parsing and handling SCI game data. It provides the foundational functionalities used by the other CLI crates.
*   **`scitool-cli`**: The main CLI application that offers a range of commands for developers and researchers to work with SCI game resources. This includes functionalities for listing, extracting, and inspecting resources like scripts, messages, and other game assets.
*   **`scidub-cli`**: A specialized CLI tool tailored for the Space Quest 5 Fan Dub project. It likely contains commands and workflows specific to the needs of the fan dubbing process, such as audio and text management.

## Building and Running

To build and run this project, you need to have the Rust toolchain (including `cargo`) installed.

### Building the Project

To build the entire workspace, run the following command from the project's root directory:

```bash
cargo build
```

### Running the CLI

You can run the `scitool` CLI using `cargo run`. To see the available commands and options, use the `--help` flag:

```bash
cargo run -- --help
```

This will display the main help menu, which outlines the different categories of commands available, such as `resources`, `messages`, and `scripts`.

#### Example: Listing Game Resources

To list all the resources in a game directory, you can use the `resources list` command:

```bash
cargo run -- resources list /path/to/game
```

## Development Conventions

*   **Workspace Structure**: The project is organized as a Cargo workspace, with shared dependencies and settings defined in the root `Cargo.toml`.
*   **Crate Organization**: The code is modularized into a core library (`scidev`) and separate CLI applications (`scitool-cli`, `scidub-cli`), which is a common and recommended practice in Rust development.
*   **Command-Line Interface**: The CLIs are built using the `clap` crate, which is the standard for creating robust and user-friendly command-line applications in Rust.
*   **Error Handling**: The project uses the `anyhow` and `thiserror` crates for error handling, which is a common pattern for ergonomic and informative error management in Rust applications.
*   **Linting**: The project has `clippy.toml` and workspace lint configurations in `Cargo.toml` to enforce code quality and style.

## Rust Conventions

*   **Encapsulation**
    -   Structs should almost never have public fields, either global or otherwise. THIS APPLIES TO ALL TYPES, MODULE INTERNAL AND EXTERNAL.
    -   Factory functions and accessor methods should be used only when it would be appropriate for clients of the struct to have access to the values in the fields.
    -   Type definitions should be limited to only the scope needed for the module. Any variances from this must be documented.
