# User Journeys

This document describes the high-level workflows for users interacting with `sciproj` and `scidev`. We categorize these into three main types of projects: **Modding**, **Full Import**, and **New Games**.



## Workflow: Installation and Setup

**Goal**: Prepare the developer environment for working with `sciproj`.

1.  **Install Toolchain**:
    *   **Cargo**: `cargo install scitool` (OR download prebuilt binaries for macOS/Windows/Linux).
    *   **External Dependencies**:
        *   `ffmpeg`: Required for audio processing/conversion.
        *   `scummvm` (optional): Recommended for running and testing games.
2.  **Environment Check**:
    *   User runs `sciproj doctor` to verify that all dependencies are in the `PATH` and compatible versions are installed.
3.  **Global Config**:
    *   User runs `sciproj config --global --set scummvm_path /path/to/scummvm`.

## Project Creation Modes

To support different copyright and workflow needs, `sciproj` offers three distinct project modes. These can be initialized in the current directory using `sciproj init <mode>` or in a new directory using `sciproj new <mode> <path>`.

**Modes:**

1.  **Modding (`mod`)**: Designed for modifying an existing game where the user *does not* own the original assets. The tool references the original game as a read-only source and only tracks the *changes* (patches) in version control.
2.  **Full Import (`import`)**: Designed for open-source projects or cases where the user *has rights* to the assets (or is fully decompiling for preservation/reverse-engineering). All resources are imported and committed to version control.
3.  **Create New Game (`create`)**: Designed for creating a game from scratch. All assets are created by the user and committed.

---

## Category 1: Modding an Existing Game (`sciproj init mod`)

These journeys involve taking an existing compiled SCI game (e.g., Space Quest 5, King's Quest 6) and modifying it. The modifications can range from simple text replacement (translations) to complex logic changes or asset replacement (fan dubs, remastering).

### Critical Constraint: Copyright & Version Control

**Principle**: Users must be able to host their mod projects publicly (GitHub, etc.) without distributing the original game's copyrighted assets.

*   **Repository Content**: The Git repository should contain *only* the new/modified assets, the project manifest (`sciproj.toml`), and patch logic.
*   **Excluded Content**: The original game resources (base views, scripts, audio) must never be committed.
*   **User Responsibility**: The end-user (player) or the developer must supply their own copy of the original game files to build or run the project.

### Workflow: Project Initialization (Decompilation)

**Goal**: Convert a retail game into a workable `sciproj` project, safe for version control.

1.  **Init**: User runs `sciproj init mod /path/to/original_game`.
2.  **Versioning Setup**:
    *   The tool creates a `.gitignore` file automatically.
    *   It adds the local copy of the original assets (e.g., `build/base`, `input/`, or similar) to `.gitignore`.
3.  **Analysis & Indexing**: The tool analyzes the source game but *does not copy* the full assets into the `src/` tree unless they are being modified. Instead, it builds an index or checksum manifest of the expected base game.
4.  **Extraction (On-Demand or Local-Only)**:
    *   The tool extracts resources to a local cache (ignored by git) for the tool's use.
    *   Only resources the user explicitly chooses to modify are "extracted" into the tracked `src/` directory.

## Category 2: Full Import / Open Source (`sciproj init import`)

**Goal**: Work on a game where all assets can be shared (e.g., specific open-source fan games, or full decompilation efforts).

### Workflow: Initialization

1.  **Init**: User runs `sciproj init import /path/to/project_or_game`.
2.  **Import**:
    *   The tool extracts *all* resources from the source.
    *   It converts them to editable source formats where possible (scripts -> .sc, views -> .aseprite).
    *   It places everything in `src/`.
3.  **Version Control**: The user commits *everything* in `src/` to Git. There is no dependency on an external "base game" directory for other contributors.

## Category 3: Creating a New Game (`sciproj new`)

These journeys involve creating a brand new SCI game from scratch, using the engine as a platform.

### Workflow: Fresh Start

**Goal**: Create a blank slate project.

1.  **Init**: User runs `sciproj new --template=sci1.1-standard`.
2.  **Scaffolding**: The tool creates a minimal directory structure:
    *   `src/scripts/`: Contains a generic `Main.sc` and necessary kernel headers.
    *   `src/resources/`: Standard system resources (fonts, cursors, system views) copied from the template.
    *   `sciproj.toml`: Configured for a specific interpreter version.
3.  **Version Control**:
    *   Since all content is original (or liberally licensed template code), everything in `src/` is committed.
    *   Build artifacts (`build/`, `dist/`) are ignored.

---

## Core Development Workflows

These workflows are shared across all project types (Modding, New Games, Imports).

### Resource Addressing

To consistently target specific resources or sub-resources (e.g., a specific loop in a view, or a line of text), `sciproj` uses a unified path-like addressing scheme.

**Syntax**: `type/id[/sub-type/sub-id...]`

*   **Resources**: `view/0`, `script/100`, `text/99`
*   **Sub-resources**:
    *   `view/0/loop/1` (Loop 1 of View 0)
    *   `text/20/noun/1/verb/2/cond/0/seq/1` (Specific message tuple)

This syntax is designed to be shell-friendly and map logically to a REST-like or directory structure.

### Workflow: Creating a new resource

**Goal**: Add a brand new resource (Script, View, Pic, etc.) to the project.

1.  **Create**:
    *   User runs `sciproj create view/100` (or `sciproj create view` to auto-assign the next free ID).
    *   The tool creates a new template file at the appropriate path (e.g., `src/views/100.aseprite`).
        *   *Note*: For views, this might be a blank 1x1 sprite or a default template.
        *   *Note*: For scripts, this would include the standard module headers.
2.  **Edit**:
    *   User opens the new file in an external editor and modifies it.
3.  **Build**:
    *   `sciproj build` compiles the new resource and includes it in the resource map.


### Workflow: Asset Modification

**Goal**: Modify an asset (Text, Audio, Graphics) that is already part of the project.

**Prerequisite**: The resource must be in `src/`. If it is a base game resource in a mod project, it must first be extracted (see "Extracting" above).

1.  **Locate**:
    *   User browses `src/` to find the target file (e.g., `src/views/0.aseprite`).
    *   *Alternative*: User runs `sciproj locate view/0` to get the absolute path.
2.  **Edit**:
    *   User opens the file in their preferred editor (Aseprite for views, VS Code for scripts/text, DAW for audio).
    *   User saves changes.
3.  **Verify**:
    *   User runs `sciproj build` (or `sciproj build view/0` for fast compilation).
    *   User runs `sciproj run` to test in-game.

### Workflow: Build & Run

**Goal**: Test changes in-game.

1.  **Build**: `sciproj build`
    *   Compiles scripts.
    *   Converts assets from source formats (Aseprite, WAV, YAML) to SCI formats.
    *   **Mod/Import**: Merges with base game / builds patches.
    *   **New**: Packs full resource volumes.
2.  **Run**: `sciproj run` launches the configured interpreter with the built resources.