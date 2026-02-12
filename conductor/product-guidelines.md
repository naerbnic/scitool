# Product Guidelines

## Tone and Style
- **Technical and Precise:** Communications and documentation should prioritize technical accuracy, using correct terminology for SCI engine concepts and data structures.
- **Context-Aware Output:** To manage complexity, the tool should provide distinct modes for output:
    - **Human-Readable (Default):** A technical yet accessible format optimized for developer reading and debugging.
    - **Machine-Readable:** A minimalist, stable format (e.g., JSON or delimited text) optimized for integration into shell scripts and CI/CD pipelines.

## User Experience (UX)
- **Informative and Actionable Errors:** Error messages must clearly state what went wrong and provide actionable guidance or suggestions on how the user can resolve the issue.
- **Built-in Discoverability:** The primary source of documentation for users should be the CLI's own `--help` system. Every command and flag must be thoroughly documented using `clap`'s descriptive capabilities.

## Technical Design
- **Separation of Concerns:** A strict boundary must be maintained between resource parsing (core logic), data conversion, and the CLI interface layer to ensure the codebase remains modular and testable.
- **Immutability:** Original game resources must be treated as read-only. Any operations involving modification or injection should involve creating new artifacts or working with temporary buffers to preserve the integrity of the source data.
- **Cross-Platform Consistency:**
    - **Platform-Agnostic Paths:** Use Rust's `std::path` and related abstractions exclusively to handle file paths, ensuring compatibility across Windows, macOS, and Linux.
    - **Uniform Interface:** The CLI behavior, including arguments, flags, and output formats, must be identical across all supported operating systems.
    - **CI/CD Validation:** Every change must be validated via automated tests running on Windows, macOS, and Linux environments to prevent platform-specific regressions.
