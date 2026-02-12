# Tech Stack

## Core Technologies
- **Language:** Rust (Stable)
- **Build System & Package Manager:** Cargo
- **Architecture:** Cargo Workspace (multi-crate)

## Primary Libraries (Crates)
- **CLI Framework:** `clap` (v4+) for building the command-line interface.
- **Error Handling:**
    - `anyhow`: Used for high-level application error handling and context in the CLI crate.
    - `thiserror`: Used for defining structured, domain-specific error types in the library crates (`scidev`, `sciproj`).
    - *Note:* Future consideration for `exn` or similar crates to enhance human-readable error reporting as requirements evolve.
- **Data Serialization:** `serde` (implied for configuration and potential JSON output).
- **Testing:** `proptest` (detected in `scidev`) for property-based testing of resource parsing.

## Quality Assurance
- **Linting:** `clippy` (with workspace-level configurations and custom `clippy.toml`).
- **Formatting:** `rustfmt`.
- **Static Analysis:** Workspace-level lint rules enforced in `Cargo.toml`.
