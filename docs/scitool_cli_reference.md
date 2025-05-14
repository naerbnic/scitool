# scitool CLI Reference

This document provides a reference for the `scitool` command-line interface.

## Top-Level Commands

The `scitool` utility is organized into several categories, each with its own set of subcommands.

```shell
scitool <CATEGORY> <SUBCOMMAND> [OPTIONS]
```

### Categories

* [`res`](#resource-commands-res): Commands for working with game resources.
* [`msg`](#message-commands-msg): Commands for working with game messages.
* [`gen`](#generation-commands-gen): Commands for generating various outputs from game data.
* [`script`](#script-commands-script): Commands for working with game scripts.

---

## Resource Commands (`res`)

Commands for listing, extracting, and dumping game resources.

### `scitool res list <root_dir> [--type <res_type>]`

Lists resources in the game.

* `<root_dir>`: Path to the game's root directory.
* `--type <res_type>` or `-t <res_type>` (Optional): Filter by resource type (e.g., `Script`, `Heap`, `View`, `Pic`, `Sound`, `Message`, `Font`, `Cursor`, `Patch`, `AudioPath`, `Vocab`, `Palette`, `Wave`, `Audio`, `Sync`).

### `scitool res extract-as-patch <root_dir> <resource_type> <resource_id> [--dry-run] [--output-dir <output_dir>]`

Extracts a resource and saves it as a patch file. Supported types are `Script` (SCR) and `Heap` (HEP).

* `<root_dir>`: Path to the game's root directory.
* `<resource_type>`: The type of the resource to extract (e.g., `Script`, `Heap`).
* `<resource_id>`: The ID of the resource to extract.
* `--dry-run` or `-n` (Optional): If set, prints what would be done without actually writing files. Defaults to `false`.
* `--output-dir <output_dir>` or `-o <output_dir>` (Optional): Directory to save the output file. Defaults to `<root_dir>`.

### `scitool res dump <root_dir> <resource_type> <resource_id>`

Dumps the hexadecimal content of a resource.

* `<root_dir>`: Path to the game's root directory.
* `<resource_type>`: The type of the resource to dump.
* `<resource_id>`: The ID of the resource to dump.

---

## Message Commands (`msg`)

Commands for exporting, printing, and checking game messages.

### `scitool msg export <root_dir> --output <output_path>`

Exports game messages to a JSON file.

* `<root_dir>`: Path to the game's root directory.
* `--output <output_path>` or `-o <output_path>`: Path to write the JSON output file.

### `scitool msg print <root_dir> [--config <config_path>] [--talker <talker_id>] [--room <room_id>] [--verb <verb_id>] [--noun <noun_id>] [--condition <condition_id>] [--sequence <sequence_id>]`

Prints messages from the game, with optional filters.

* `<root_dir>`: Path to the game's root directory.
* `--config <config_path>` (Optional): Path to a book configuration YAML file.
* `--talker <talker_id>` or `-t <talker_id>` (Optional): Filter by talker ID.
* `--room <room_id>` or `-r <room_id>` (Optional): Filter by room ID.
* `--verb <verb_id>` or `-v <verb_id>` (Optional): Filter by verb ID.
* `--noun <noun_id>` or `-n <noun_id>` (Optional): Filter by noun ID.
* `--condition <condition_id>` or `-c <condition_id>` (Optional): Filter by condition ID.
* `--sequence <sequence_id>` or `-s <sequence_id>` (Optional): Filter by sequence ID.

### `scitool msg check <root_dir> [--config <config_path>]`

Checks message data, building a "book" and printing statistics and validation errors.

* `<root_dir>`: Path to the game's root directory.
* `--config <config_path>` (Optional): Path to a book configuration YAML file.

### `scitool msg print-talkers <root_dir>`

Prints a list of all unique talker IDs found in the game messages.

* `<root_dir>`: Path to the game's root directory.

---

## Generation Commands (`gen`)

Commands for generating different file formats from game data.

### `scitool gen master <root_dir> <config_path> --output <output_path>`

Generates a master HTML script document from the game book.

* `<root_dir>`: Path to the game's root directory.
* `<config_path>`: Path to the book configuration YAML file.
* `--output <output_path>` or `-o <output_path>`: Path to write the HTML output file.

### `scitool gen json <root_dir> <config_path> --output <output_path>`

Generates a JSON representation of the game script.

* `<root_dir>`: Path to the game's root directory.
* `<config_path>`: Path to the book configuration YAML file.
* `--output <output_path>` or `-o <output_path>`: Path to write the JSON output file.

### `scitool gen json-schema`

Generates the JSON schema for the game script structure and prints it to stdout.

---

## Script Commands (`script`)

Commands for working with game scripts.

### `scitool script gen-headers --game-dir <game_dir> [--out-dir <out_dir>] [--selectors-path <selectors_path>] [--classdef-path <classdef_path>]`

Generates script header files (`selectors.sh` and `classdef.sh`) from game resources.

* `--game-dir <game_dir>` or `-d <game_dir>`: Path to the game's root directory.
* `--out-dir <out_dir>` or `-o <out_dir>` (Optional): Directory to write the header files. Defaults to the current directory (`.`).
* `--selectors-path <selectors_path>` or `-s <selectors_path>` (Optional): Filename for the selectors header. Defaults to `selectors.sh`.
* `--classdef-path <classdef_path>` or `-c <classdef_path>` (Optional): Filename for the class definition header. Defaults to `classdef.sh`.
