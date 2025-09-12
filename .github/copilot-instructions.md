# Copilot instructions for scitool

Purpose: Enable AI coding agents to be productive in this mono-repo by knowing the architecture, workflows, and house rules. Keep changes small, compile often, and cite real paths/commands.

## Big picture
- Workspace members: `crates/scidev` (core library), `crates/scitool-cli` (end-user CLI), `crates/scidub-cli` (VO/book/audio tooling), plus `fuzz/` targets. Cargo workspace is configured in `Cargo.toml` with shared lints.
- Games use Sierra SCI 1.1 resource files. We read `RESOURCE.MAP`/`RESOURCE.000` and `MESSAGE.MAP`/`RESOURCE.MSG`, optionally overlaying `.SCR/.HEP` patch files in the game dir.
- Data flow (core):
  1) Parse resource map → type-specific location tables → resource entries (`scidev::resources::file::{map, data}`)
  2) Read entry headers and lazy blocks → expose `ResourceSet` with `get_resource()`/iterators
  3) Type parsers live in `scidev::resources::types` (e.g., `msg.rs`, `audio36.rs`)
  4) CLIs consume `scidev` to list/extract/dump or to build artifacts (headers, audio)

## Key crates and entry points
- `crates/scidev` (no I/O side-effects other than resource reading):
  - Resource I/O: `resources/file.rs`, `resources/file/data/*`, `resources/file/map/*`
  - Resource identity: `resources::{ResourceId, ResourceType}`
  - Message parsing: `resources/types/msg.rs` (MessageId, RoomMessageSet)
  - Utilities: `utils/*` (buffer/mem_reader/compression/debug/validation)
- `crates/scitool-cli` (sync CLI):
  - Bin: `src/bin/scitool/main.rs` → `cli.rs`
  - Categories: `resources` (list, extract-as-patch, dump), `messages` (print-talkers), `script` (gen-headers)
  - Command impls: `src/commands/{resources.rs,messages.rs,scripts.rs}`
- `crates/scidub-cli` (async CLI for VO/book):
  - Bin: `src/bin/scidub/main.rs` → `cmds.rs`
  - Subcommands: `compile-audio` (requires ffmpeg/ffprobe), `export-scannable`, `try-scan`, `generate-csv`, `book` (see `src/book/*`)

## Build, run, test
- Build workspace: cargo build
- Run scitool help: cargo run -p scitool-cli --bin scitool -- --help
- Example: list Message resources for a game dir: cargo run -p scitool-cli --bin scitool -- resources list /path/to/game --type msg
- Dump a resource as hex: cargo run -p scitool-cli --bin scitool -- resources dump /path/to/game --type script 100
- Extract as patch: cargo run -p scitool-cli --bin scitool -- resources extract /path/to/game --type heap 100 -o /tmp
- Generate script headers: cargo run -p scitool-cli --bin scitool -- script gen-headers -d /path/to/game -o .
- scidub examples: cargo run -p scidub-cli --bin scidub -- compile-audio -s samples/ -o out/  (needs ffmpeg/ffprobe in PATH)
- Fuzz targets (optional): see `fuzz/Cargo.toml`; typical workflow uses cargo-fuzz, not configured here.

## House rules and patterns
- Lints: Unsafe code is denied; clippy pedantic is enabled with selected denies in root `Cargo.toml`. Keep new code clippy-clean.
- Error handling: Prefer `thiserror` and `crate::utils::errors::{ensure_other, bail_other, OtherError}` patterns. Wrap I/O/parse errors into crate-specific error types.
- Parsing: Use `utils::mem_reader::{MemReader, Parse}`; implement `Parse` when possible. For context-dependent parsing, add explicit read functions (see `ResourceTypeLocations::read_from`).
- Buffers/blocks: Work with `utils::block::{BlockSource, LazyBlock, MemBlock}` and avoid reading whole files eagerly unless needed; many APIs return lazy/open-on-demand blocks.
- Resource overlay semantics: `open_game_resources(root)` loads MAP/000 + MESSAGE.MAP/RESOURCE.MSG, then overlays any patch files in `root` (e.g., `100.SCR`, `100.HEP`). When writing patches, preserve type byte and zero header.
- Message model: SCI v4 messages parsed by `resources/types/msg.rs`. `MessageId` packs noun/verb/condition/sequence with defaults (verb=0, condition=0, sequence=1). Use `RoomMessageSet::messages()` to iterate.
- Script headers: `scitool script gen-headers` builds `selectors.sh` and `classdef.sh` by scanning resources; `--dry-run` prints to stdout.
- Audio (scidub): `resources::SampleDir` builds `VoiceSampleResources` via ffmpeg; uses `Ogg Vorbis` at 22.05kHz; concurrency via `buffer_unordered`. Outputs `resource.aud` and per-resource patch files named `<id>.<ext>`.

## Conventions and gotchas
- Paths: Game root must contain `RESOURCE.MAP` and `RESOURCE.000`; messages live in `MESSAGE.MAP` and `RESOURCE.MSG`. Patch overlay searches the same directory for files with SCI extensions.
- Map layout: SCI1.1 uses 5-byte location entries despite some docs; code enforces `(end-start) % 5 == 0` and computes offsets by `(u24 & 0x0FFF_FFFF) << 1`.
- Resource ID check: When reading data entries, header IDs must match the map-provided ID; mismatches are treated as errors.
- Platform tools: Some flows require `ffmpeg` and `ffprobe` on PATH. Use `LookupPath` helpers and surface clear errors if missing.
- Testing style: Unit tests use `datalit!` to synthesize binary blobs; see tests in `resources/file/*` and `resources/types/msg.rs` for examples.

## Good first places to extend
- Add new `ResourceType` parsers under `crates/scidev/src/resources/types/` with small unit tests.
- Add `scitool` subcommands that compose `open_game_resources()` and `ResourceSet` iterators for targeted tasks (follow the pattern in `src/commands`).
- Extend `scidub` CSV/Book tooling in `src/book/*`; keep IDs flowing through `common` types.

When in doubt, cite concrete files and run `cargo build` locally before sending large diffs.
