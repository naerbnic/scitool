# Copilot instructions for scitool

Purpose: Enable AI coding agents to be productive in this mono-repo by knowing the architecture, workflows, and house rules. Keep changes small, compile often, and cite real paths/commands.

## Big picture
- **Workspace members**:
  - `crates/scidev`: Core library (resource parsing, no side-effects).
  - `crates/scitool-cli`: Synchronous CLI tool (binary `scitool`) for resource inspection/extraction.
  - `crates/sciproj`: Project management library (dubbing, state, config).
  - `crates/scidev-cli`: Async CLI tool (binary `scidev`, package `sciproj-cli`) for project workflows.
  - `crates/crosslock` & `crates/atomic-dir`: Utilities for file system safety and locking.
- **Data flow (core)**:
  1. `ResourceSet::from_root_dir` (`scidev::resources::file`) loads `RESOURCE.MAP`/`000` and `MESSAGE.MAP`/`MSG`.
  2. It automatically scans for and overlays patch files (e.g., `.SCR`, `.HEP`) from the game directory.
  3. Resources are exposed via `ResourceSet` iterators.
  4. Parsers live in `scidev::resources::types` (e.g., `msg.rs`, `audio36.rs`).

## Key crates and entry points
- **`crates/scidev`**:
  - Resource I/O: `resources/file.rs` (entry: `ResourceSet::from_root_dir`).
  - Types: `resources/types/*`.
  - Utils: `utils/mem_reader.rs` (parsing), `utils/block.rs` (lazy data access).
  - **Rule**: No `anyhow` allowed here. Use `thiserror` and `crate::utils::errors`.
- **`crates/scitool-cli`** (Sync CLI):
  - Bin: `src/bin/scitool/main.rs`.
  - Commands: `resources` (list, dump, extract), `messages` (talkers), `script`.
  - Impls: `src/commands/*`.
- **`crates/scidev-cli`** (Async CLI, package `sciproj-cli`):
  - Bin: `src/main.rs` (binary name `scidev`).
  - Commands: `compile-audio`, `book`, `project`, `export-scannable`.
  - Uses `tokio` and `sciproj`.

## Build, run, test
- **Build workspace**: `cargo build`
- **Run `scitool` (Resource tool)**:
  - Help: `cargo run -p scitool-cli --bin scitool -- --help`
  - List resources: `cargo run -p scitool-cli --bin scitool -- resources list /path/to/game`
  - Dump resource: `cargo run -p scitool-cli --bin scitool -- resources dump /path/to/game --type script 100`
- **Run `scidev` (Project tool)**:
  - Help: `cargo run -p sciproj-cli --bin scidev -- --help`
  - Compile audio: `cargo run -p sciproj-cli --bin scidev -- compile-audio ...`
- **Test**: `cargo test` (Unit tests often use `datalit!` for binary blobs).

## House rules and patterns
- **Error Handling**:
  - `scidev`: Strict error types. Use `utils::errors::{ensure_other, bail_other}`.
  - CLIs/`sciproj`: `anyhow` is permitted.
- **Parsing**:
  - Use `utils::mem_reader::{MemReader, Parse}`.
  - Avoid reading whole files eagerly; use `utils::block::{Block, LazyBlock}`.
- **Resource Model**:
  - `ResourceType` enum maps extensions (e.g., `.scr` -> `Script`).
  - `ResourceId` combines type and number.
  - `ResourceSet` is the central access point.
- **Project Management (`sciproj`)**:
  - Manages state in `sciproj.state.json` and config in `sciproj.toml`.
  - Uses `crosslock` for safe concurrent access.

## Conventions and gotchas
- **Naming**: The async CLI package is `sciproj-cli` but the binary is `scidev`. The sync CLI is `scitool-cli` / `scitool`.
- **Paths**: Game root must contain `RESOURCE.MAP` and `RESOURCE.000`.
- **Map Layout**: SCI1.1 uses 5-byte location entries.
- **Async**: `scidev-cli` and `sciproj` are async (`tokio`). `scitool-cli` and `scidev` core are sync.

When in doubt, check `Cargo.toml` for package names and dependencies.
