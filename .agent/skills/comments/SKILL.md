---
name: comments
description: Guidelines for writing developer and documentation comments. Use when modifying code, adding new features, or when explicitly asked to improve code documentation.
---

# Commenting Guidelines

Follow these rules when writing comments in this workspace. Differentiate clearly between **Developer Comments** (internal implementation details) and **Documentation Comments** (public API usage).

## General Rules

- **New Code:** MUST follow these rules.
- **Existing Code:** Do NOT add comments unless modifying the code or explicitly asked.
- **Context:** Keep comments local and pertinent to changed code.

## Developer Comments

**Purpose:** Clarify intent, implementation reasons, and complex logic for maintainers.

- **Content:** Supplement source code; do NOT restate it. Explain *why*, not just *what*.
- **When to use:**
    - **Non-obvious Logic:** Algorithms or optimizations that aren't self-evident (e.g., "Implements Dijkstra's algorithm").
    - **Workarounds/Debt:** Explanations for seemingly incorrect or inefficient code due to specific constraints.
    - **Invariants/State:** Document preconditions, postconditions, and mutable state invariants.

## Documentation Comments

**Purpose:** Instruct clients on how to use the library/module API.

- **Content:**
    - Purpose of the API.
    - Mental model for usage.
    - Concrete usage details.
- **Avoid:** Implementation details irrelevant to the consumer.
- **Scope:**
    - **Module-Level:** High-level purpose and mental model.
    - **Item-Level:** Specific usage for functions, types, etc.
- **Isolation:** Each comment should be understandable on its own.

## Rust-Specific Rules

### Developer Comments (`//`)

- Use `//` for single or multi-line comments.
- **Safety:** `unsafe` blocks MUST be prefaced with a `// SAFETY: ...` comment explaining why preconditions are satisfied.

### Documentation Comments (`///` or `//!`)

- Use `///` for the following item.
- Use `//!` for the containing item (e.g., module).
- Content is Markdown.
- **Safety:** Unsafe items MUST have a `# Safety` Markdown section describing caller obligations.