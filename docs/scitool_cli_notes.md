# SCITool CLI Design

The majority of this file is generated through GitHub Copilot, used for
arranging ideas and context about the functionality of scitool.

## Core Principles

- **Unix Philosophy**: Each tool does one thing well
- **Round-trip Safety**: Extract → Modify → Import → Validate
- **Translation-Friendly**: Human-readable formats for text content
- **Pipeline-Compatible**: JSON/CSV output for scripting

## Global Options

```shell
--game-dir <PATH>     # SCI game directory (default: current dir)
--output <PATH>       # Output file/directory
--format <FORMAT>     # Output format
--verbose, -v         # Verbose output
--quiet, -q          # Suppress non-error output
--validate           # Validate after operations
```

## Resource Discovery & Inspection

```shell
scitool list [--type TYPE] [--format table|csv|json]
scitool info <type> <id> [--format json|yaml|text]
scitool validate <type> <id> [--repair] [--detailed]

# Batch operations
scitool list --type msg --format json | jq '.[].id' | xargs -I {} scitool validate msg {}
```

## Core Extract/Import System

### Universal Extract Command

```shell
scitool extract <type> <id> [OPTIONS]

Options:
  --format <FORMAT>     # Output format (see format matrix below)
  --output <PATH>       # Output file/directory
  --preserve-metadata   # Include SCI-specific metadata
  --with-dependencies   # Extract referenced resources too
  
Examples:
  scitool extract msg 42 --format csv --output msg042.csv
  scitool extract script 100 --format project --output script100/
  scitool extract view 200 --format png-sequence --output view200/
```

### Universal Import Command

```shell
scitool import <type> <id> <input> [OPTIONS]

Options:
  --format <FORMAT>     # Input format (auto-detected if not specified)
  --validate           # Validate before import
  --backup             # Create backup of original
  --dry-run           # Show what would be imported without doing it

Examples:
  scitool import msg 42 msg042.csv
  scitool import script 100 script100/ --validate
  scitool import view 200 view200/ --format png-sequence
```

## Format Matrix

### Message Resources (.msg)

| Format | Extract | Import | Description |
|--------|---------|---------|-------------|
| `csv` | ✓ | ✓ | Translation-friendly CSV with context |
| `json` | ✓ | ✓ | Full metadata preservation |
| `txt` | ✓ | ✓ | Plain text (loses formatting) |
| `po` | ✓ | ✓ | GNU gettext format |
| `xliff` | ✓ | ✓ | Translation industry standard |

### Script Resources (.scr)

| Format | Extract | Import | Description |
|--------|---------|---------|-------------|
| `json` | ✓ | ✓ | Metadata + bytecode as hex |
| `project` | ✓ | ✓ | Directory with decompiled source |
| `asm` | ✓ | ✓ | Assembly-like representation |
| `bytecode` | ✓ | ✓ | Raw bytecode dump |

### Graphics Resources (.v56, .p56)

| Format | Extract | Import | Description |
|--------|---------|---------|-------------|
| `png` | ✓ | ✓ | Single image (first cel/loop) |
| `png-sequence` | ✓ | ✓ | Directory of PNG files per cel |
| `gif` | ✓ | ✗ | Animated GIF (view resources) |
| `json` | ✓ | ✓ | Metadata + pixel data |
| `sprite-sheet` | ✓ | ✓ | Combined sprite sheet + JSON |

### Audio Resources (.aud)

| Format | Extract | Import | Description |
|--------|---------|---------|-------------|
| `wav` | ✓ | ✓ | Standard WAV format |
| `ogg` | ✓ | ✓ | Ogg Vorbis compression |
| `json` | ✓ | ✓ | Metadata + raw sample data |

## Resource-Specific Commands

### Message/Text Resources

```shell
scitool msg extract <id> [--format csv|json|txt|po|xliff] [--with-context]
scitool msg import <id> <input> [--encoding utf8|latin1]
scitool msg search <pattern> [--room ID] [--verb ID] [--format json]
scitool msg stats [--format table|json]  # Usage statistics
scitool msg validate <id> [--check-formatting] [--check-length]

# Translation workflow
scitool msg extract-all --format po --output messages.po
scitool msg import-all messages_translated.po --validate
```

### Script Resources

```shell
scitool script extract <id> [--format project|asm|json|bytecode]
scitool script import <id> <input> [--format project|asm|json|bytecode]
scitool script decompile <id> [--output DIR] [--include-headers]
scitool script compile <source-dir> [--output FILE] [--optimize]
scitool script analyze <id> [--format json]  # exports, classes, dependencies
scitool script patch <id> <patch.json> [--backup]
```

### Graphics Resources

```shell
scitool gfx extract <type> <id> [--format png|png-sequence|json|sprite-sheet]
scitool gfx import <type> <id> <input> [--format png-sequence|json|sprite-sheet]
scitool gfx info <type> <id> [--format json|yaml]
scitool gfx convert <input> <output> [--format png|gif]
scitool gfx optimize <type> <id> [--palette-reduction] [--lossless]

# Batch operations for graphics
scitool gfx extract-all view --format png-sequence --output views/
scitool gfx import-all view views/ --validate
```

### Audio Resources

```shell
scitool audio extract <id> [--format wav|ogg|json]
scitool audio import <id> <input> [--format wav|ogg|json]
scitool audio info <id> [--format json]
scitool audio convert <input> <output> [--quality 192]
scitool audio normalize <id> [--peak -3db] [--backup]
```

## Project Management & Building

### Project Structure

```shell
scitool project init [--template basic|fan-translation] [--output DIR]
scitool project extract <game-dir> [--output project-dir] [--selective]
scitool project build <project-dir> [--output game-dir] [--validate]
scitool project validate <project-dir> [--detailed]
```

### Patch Management

```shell
scitool patch create <original> <modified> <patch-name> [--format scipatch|json]
scitool patch apply <patch-file> <target-dir> [--dry-run] [--backup]
scitool patch list [--applied] [--format table|json]
scitool patch info <patch-file> [--format json]
scitool patch revert <patch-name> [--confirm]
```

## Translation Workflow Examples

### Complete Fan Translation Pipeline

```shell
# 1. Initialize translation project
scitool project init --template fan-translation --output my-translation/

# 2. Extract game to project
scitool project extract /path/to/game --output my-translation/ --selective

# 3. Extract all translatable text
scitool msg extract-all --format po --output my-translation/messages.po

# 4. [Human translates messages.po file]

# 5. Import translated text
scitool msg import-all my-translation/messages_translated.po --validate

# 6. Build patched game
scitool project build my-translation/ --output patched-game/ --validate

# 7. Create distribution patch
scitool patch create /path/to/game patched-game/ translation-patch
```

### Incremental Modding Workflow

```shell
# Extract specific resources for modding
scitool extract script 42 --format project --output script42/
scitool extract view 100 --format png-sequence --output view100/

# [Modify files as needed]

# Import modified resources
scitool import script 42 script42/ --validate --backup
scitool import view 100 view100/ --validate --backup

# Test changes
scitool validate script 42 --detailed
scitool validate view 100 --detailed
```

## Data Formats & Round-trip Specifications

### Message CSV Format

```csv
id,noun,verb,condition,sequence,text,context,room,notes
42,1,2,0,1,"Hello world!","Greeting to player","Room 1","Keep friendly tone"
42,1,2,0,2,"Goodbye!","Farewell","Room 1","Short and sweet"
```

### Script Project Format

```plaintext
script042/
├── metadata.json      # SCI-specific metadata
├── exports.json       # Public exports/symbols  
├── classes/           # Class definitions
│   ├── MyClass.json
│   └── AnotherClass.json
├── methods/           # Method implementations
│   ├── method_001.asm
│   └── method_002.asm
└── strings.json       # String literals
```

### Graphics PNG Sequence Format

```plaintext
view100/
├── metadata.json      # View metadata (loops, cel count, etc.)
├── palette.json       # Color palette information
├── loop0/
│   ├── cel0.png
│   ├── cel1.png
│   └── cel2.png
├── loop1/
│   ├── cel0.png
│   └── cel1.png
└── preview.gif        # Optional: animated preview
```

## Error Handling & Validation

### Validation Levels

- `--validate basic`: Format compliance only
- `--validate full`: Format + SCI game logic validation  
- `--validate strict`: All checks + best practice warnings

### Common Error Types

- **Format errors**: Invalid file format or corruption
- **Reference errors**: Missing dependencies or broken links
- **Size errors**: Resource too large for SCI format limits
- **Encoding errors**: Character encoding issues in text

### Error Output Format

```json
{
  "status": "error",
  "resource": {"type": "msg", "id": 42},
  "errors": [
    {
      "level": "error",
      "code": "TEXT_TOO_LONG", 
      "message": "Text exceeds SCI string length limit",
      "location": {"line": 5, "column": 12},
      "suggestion": "Split into multiple messages"
    }
  ]
}
```

## Integration with Build Systems

### Makefile Integration

```makefile
# Extract all resources for translation
extract-all:
 scitool project extract game/ --output project/

# Build translated game
build-translation: project/
 scitool project build project/ --output translated-game/ --validate

# Create distribution package  
package: translated-game/
 scitool patch create game/ translated-game/ translation
 tar czf translation.tar.gz translation.scipatch
```

### Shell Script Examples

```bash
#!/bin/bash
# Batch validate all message resources
scitool list --type msg --format json | \
  jq -r '.[].id' | \
  while read id; do
    echo "Validating message $id..."
    scitool validate msg "$id" || echo "FAILED: msg $id"
  done
```

## Copyright-Safe Distribution & Version Control

### Problem Statement

SCI game resources are protected by copyright and cannot be redistributed. Fan translations and mods must distribute only:

- **Original content**: New assets created by modders
- **Modifications**: Patches/diffs of existing content  
- **Metadata**: Non-copyrightable structural information
- **Tools & Scripts**: Build scripts and tooling

### Version Control Strategy

#### What to Include in Git Repository

```plaintext
my-translation/
├── .gitignore                 # Exclude copyrighted content
├── build.sh                   # Build script
├── manifest.json              # Project metadata
├── patches/                   # Patch files (deltas only)
│   ├── msg_042.patch
│   └── script_100.patch
├── new-assets/                # Original assets (copyright-free)
│   ├── custom-icon.png
│   └── new-background.png
├── translations/              # Translation files (text only)
│   ├── messages.po
│   └── dialog.csv
└── tools/                     # Custom build tools
    └── validate-translation.sh
```

#### What to Exclude (.gitignore)

```gitignore
# Original game files (copyrighted)
original-game/
*.scr
*.v56
*.p56
*.aud
RESOURCE.*

# Extracted copyrighted content
extracted/
decompiled/
*.wav
*.png
*.gif

# Built game (contains copyrighted material)
build/
patched-game/

# Temporary files
*.tmp
*.bak
```

### Copyright-Safe Command Extensions

#### Patch-Only Operations

```shell
# Create minimal patches instead of full extracts
scitool diff create original-game/ modified-game/ --output patches/ --text-only
scitool diff apply patches/ target-game/ --validate

# Export only translation-relevant data
scitool msg extract-text-only --format po --output translations/
scitool script extract-strings --output script-strings.json

# Generate build instructions instead of built game
scitool project create-manifest --output manifest.json --include-patches
```

#### User-Provided Game Detection

```shell
# Verify user has legitimate copy before building
scitool verify-game <game-dir> --checksum-file supported-versions.json
scitool build-translation --require-original <original-game-dir>

# Generate installation instructions
scitool generate-install-guide --output INSTALL.md --platform windows|linux|macos
```

### Distribution Workflow

#### For Translation Teams

```shell
# 1. Team lead extracts text for translation (local only)
scitool msg extract-all --format po --output work/messages.po

# 2. Create patch from translated text
scitool msg create-translation-patch work/messages_translated.po --output patches/

# 3. Commit only patches and translations to git
git add patches/ translations/ manifest.json
git commit -m "Add Spanish translation"

# 4. Generate release package (no copyrighted content)
scitool package-translation . --output spanish-translation.zip --verify-clean
```

#### For End Users

```shell
# 1. User downloads translation package
# 2. User extracts to temporary directory
# 3. Build script applies patches to user's game copy

#!/bin/bash
# install-translation.sh
if [ ! -d "$GAME_DIR" ]; then
  echo "Please provide path to your SCI game installation"
  exit 1
fi

# Verify user has supported game version
scitool verify-game "$GAME_DIR" --checksum-file supported-versions.json

# Apply translation patches
scitool patch apply patches/ "$GAME_DIR" --backup --verify

echo "Translation installed successfully!"
```

### Advanced Copyright Protection

#### Content Fingerprinting

```shell
# Generate fingerprints of original content (for verification only)
scitool fingerprint create <game-dir> --output checksums.json --hash-only

# Verify patches apply to correct game version
scitool patch verify patches/ --against-fingerprint checksums.json
```

#### Minimal Delta Generation

```shell
# Create ultra-minimal patches (character-level diffs for text)
scitool diff create --mode minimal --text-only --output tiny-patches/

# Split patches by resource type for granular distribution
scitool patch split large-patch.json --by-type --output split-patches/
```

#### Legal Compliance Tools

```shell
# Scan project for potential copyright issues
scitool legal scan . --output compliance-report.json
scitool legal verify-clean . --strict

# Generate DMCA-safe distribution package
scitool package . --exclude-copyrighted --legal-review --output safe-package.zip
```

### Project Template Updates

#### Fan Translation Template

```shell
scitool project init --template fan-translation-safe --output my-project/
```

This creates:

```plaintext
my-project/
├── .gitignore                 # Pre-configured for copyright safety
├── README.md                  # Installation instructions for users
├── build.sh                   # Automated build script
├── verify-game.sh             # Game version verification
├── supported-versions.json    # Checksums of supported game versions
├── legal/
│   ├── LICENSE.md             # Project license
│   └── DISCLAIMER.md          # Legal disclaimer
└── tools/
    └── clean-check.sh         # Verify no copyrighted content in repo
```

### Integration with CI/CD

#### GitHub Actions Example

```yaml
name: Verify Translation
on: [push, pull_request]

jobs:
  legal-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Install scitool
        run: cargo install scitool
      - name: Verify no copyrighted content
        run: scitool legal verify-clean . --strict
      - name: Check patch integrity
        run: scitool patch validate patches/ --all
```

This approach ensures that:

1. **Only original work** is stored in version control
2. **Patches contain minimal deltas** rather than full copyrighted content  
3. **End users must provide** their own legitimate game copies
4. **Distribution packages** contain no copyrighted material
5. **Legal compliance** is automatically verified

The tools actively prevent accidental inclusion of copyrighted content while still enabling full modding workflows.
