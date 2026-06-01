# MVP Definition

This document outlines the Minimum Viable Product (MVP) for `scidev` and `sciproj`.

## Goal

To create a cross-platform, command-line based toolset for creating and modifying Sierra Creative Interpreter (SCI) games, serving as a functional alternative to SCI Companion without the Windows/MFC dependencies.

## Scope

### scidev (Library)

*   **Resource Parsing**: Full read/write support for core SCI resource types:
    *   Views (`.v56`, etc.)
    *   Pics
    *   Scripts (bytecode manipulation)
    *   Text/Messages
    *   Vocab/Main
*   **Compression**: Support for relevant SCI compression algorithms (DCL, LZW).
*   **Game Versions**: Target SCI1.1 and SCI32 initially (Space Quest 5 era).

### sciproj (Tool)

*   **Project Management**:
    *   Initialize a new project from an existing game (decompilation/setup).
    *   Manifest-based project structure (`sciproj.toml`).
*   **Workflow Integrations**:
    *   **Views**: Convert SCI Views to/from Aseprite (`.ase`/`.aseprite`) format for editing.
    *   **Messages**: Export messages to human-readable text (YAML/CSV) and re-import.
    *   **Audio**: Basic extraction and insertion of audio resources.
*   **Build System**:
    *   Pack resources back into game files (`resource.map`, `resource.000`, etc.).
    *   Dependency tracking (rebuild only what changed).

## Out of Scope for MVP

*   Full Script Compiler (use existing compilers or raw bytecode editing initially, unless `scidev` compiler is ready).
*   GUI (Native or Web).
*   Real-time debugging.
*   Support for very old (SCI0) or very new (SCI3) variants not essential for the target use case.
