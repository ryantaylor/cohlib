# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
cargo build                         # build all crates
cargo test                          # all unit + integration tests
cargo test --test build_order       # end-to-end build order comparison vs reinforce
cargo test --test bundled           # bundled data version and lookup tests
cargo test --test replay            # replay parse tests
cargo test <name>                   # run a single test by name
cargo clippy -- -D warnings
cargo fmt --check
```

Feature flags (pass to `cohlib` or the workspace root):
```sh
cargo build -p cohlib --features magnus  # enable Ruby FFI bindings
cargo build -p cohlib --features trace   # enable nom-tracable replay parser tracing
```

## Architecture

cohlib is a Cargo workspace of narrow, self-contained crates for parsing CoH3 replay files, extracting build orders, and looking up versioned game entity data. All historical game versions are compiled into the binary via `crates/data/build.rs` so `VersionedStore::bundled()` requires no external files.

### Workspace layout

```
crates/
├── cohlib/        Public API facade — re-exports replay + data + build-order types
├── replay/        Parse .rec binary files into Replay, Player, Command, Message
├── data/          VersionedStore — multi-version entity/squad/upgrade/ability/locale lookups
├── build-order/   Classify player commands into a BuildOrder using a stateful Factory
├── sga/           Extract Relic .sga archives (wraps the external `sga` crate as `relic-sga`)
├── attrib/        Parse XML attribute files from ReferenceAttributes.sga into GameData
├── locale/        Parse locale from SGA (AES-128-CBC + zlib), .ucs, .txt, or JSON
├── json-import/   Import GameData from per-version JSON files (cohdata format)
├── image/         Convert RRTEX textures (BC1/BC3) to WebP
└── cli/           Maintainer CLI binary (`cohlib` + `discover` binaries)
```

Dependency graph:
```
data, replay          ← standalone
build-order           ← data + replay
sga                   ← standalone (wraps relic-sga)
locale                ← data + sga
attrib                ← data + sga
json-import           ← data
image                 ← standalone
cohlib                ← replay + data + build-order  (public facade)
cli                   ← cohlib + sga + attrib + locale + json-import + image
```

Each crate has its own `Error` type. `cohlib::Error` wraps `replay::Error`, `data::Error`, and `build_order::Error`.

Public API is re-exported from `crates/cohlib/src/lib.rs`: `parse_replay`, `extract_build_order`, `VersionedStore`, and all data types.

### Build data pipeline

`crates/data/build.rs` scans `data/<version>/game_data.json` files at the workspace root, concatenates them into a JSON array, gzip-compresses the result, and writes it to `OUT_DIR/game_data.bin`. `VersionedStore::bundled()` decompresses and deserializes this at runtime via `include_bytes!`.

Adding a new game version:
1. `cargo run --bin cohlib -- import <depot_path> --version <build_number> --output data/`
2. Commit `data/<build_number>/game_data.json`
3. `cargo build` picks it up automatically

### Replay parsing (`crates/replay/`)

Ported from the Ruby [vault](https://github.com/ryantaylor/vault) library. Uses `nom` for binary parsing with `nom_locate` for byte-position tracking.

Data flow: raw bytes → header → Chunky/Chunk/Tick parsing → command/message aggregation → `Replay`.

Key types: `Replay`, `Player`, `Command` (enum over 9 command types), `Message`, `Faction`, `Team`, `GameType`, `Map`.

**Tick timing**: tick values are raw engine ticks; divide by 8 to get seconds.

### Build order extraction (`crates/build-order/`)

`Factory` is a stateful classifier that walks a player's `commands()` and produces `BuildAction` items. It tracks pending buildings and production queues to detect cancellations and mark suspect buildings (buildings that may have been cancelled before first use — validate these against actual production before displaying).

`extract_build_order(replay, player_index, store)` is the top-level public function.

### Versioned data store (`crates/data/`)

`VersionedStore` resolves lookups with fallback: exact build → nearest older version → nearest newer version. This handles replays recorded on patch versions not in the bundle.

`GameData` holds `HashMap<u32, T>` for each entity type keyed by `pbgid` (Relic engine ID). `Entity` has `spawns` (squad paths) and `upgrades` (upgrade paths) lists. `Ability` has `autobuild: bool` and `builds: Option<String>` for building placement abilities.

### CLI (`crates/cli/`)

Maintainer tooling only — not needed for library use.

- `cohlib populate <source_dirs>... --output <data_dir>` — import from cohdata/reinforce JSON directories
- `cohlib import <depot_path> --version <build> --output <data_dir>` — extract from CoH3 SGA depot
- `discover` — developer binary for inspecting raw SGA archive contents

### Error handling

Each crate has its own `Error` enum. The `cohlib` facade re-exports a top-level `Error` that wraps `replay::Error`, `data::Error`, and `build_order::Error`. Pipeline crates (`sga`, `attrib`, `locale`, `json-import`, `image`) expose their own error types directly. All public functions return `Result<T, Error>` and never panic.
