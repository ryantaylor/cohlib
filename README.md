# cohlib

A Rust library for parsing Company of Heroes 3 replay files, extracting build orders, and accessing versioned game entity data.

cohlib centralises all CoH3 data parsing into a single dependency. It embeds 32 versions of historical game data directly into the compiled binary so consumers get a fully functional `VersionedStore` with no external setup required.

## Features

- **Replay parsing** — parse `.rec` files into structured Rust types
- **Build order extraction** — classify player commands into a chronological build order with suspect-building detection
- **Versioned game data** — entity, squad, upgrade, and ability lookups with automatic version fallback (exact → nearest older → nearest newer)
- **Bundled data** — 32 historical game versions compiled into the library binary; `VersionedStore::bundled()` works without any external files
- **SGA archive extraction** — read Relic's `.sga` archive format
- **Icon extraction** — convert RRTEX icon files to WebP (BC1/BC3)
- **Locale resolution** — English localization strings from `.ucs`, tab-separated `.txt`, or JSON sources
- **Ruby bindings** — see [cohlib-rb](../cohlib-rb/)

## Quick start

```toml
# Cargo.toml
[dependencies]
cohlib = { path = "path/to/cohlib" }
```

```rust
use cohlib::{parse_replay, extract_build_order, VersionedStore};

fn main() -> Result<(), cohlib::Error> {
    let bytes = std::fs::read("match.rec")?;
    let replay = parse_replay(&bytes)?;

    println!("version: {}", replay.version());
    println!("players: {}", replay.players().len());

    let store = VersionedStore::bundled();
    let build_order = extract_build_order(&replay, 0, &store)?;

    for action in &build_order.actions {
        println!("{:?} tick={} pbgid={}", action.kind, action.tick, action.pbgid);
    }

    Ok(())
}
```

## Modules

| Module | Description |
|---|---|
| `replay` | Parse `.rec` files into `Replay`, `Player`, `Message` types |
| `build_order` | Extract a player's build order from a parsed replay |
| `data` | `VersionedStore` with typed lookups for entities, squads, upgrades, abilities, and locale |
| `sga` | Open Relic SGA archives and enumerate their entries |
| `attrib` | Parse XML attribute files from ReferenceAttributes.sga into `GameData` |
| `locale` | Parse English localization from SGA archives, `.ucs` text, tab-separated `.txt`, or JSON |
| `json_import` | Import `GameData` from per-version JSON files (coh3-data format) |
| `image` | Convert RRTEX icon files to WebP |

## API reference

### `parse_replay`

```rust
pub fn parse_replay(bytes: &[u8]) -> Result<Replay, Error>
```

Parse the raw bytes of a `.rec` file. Returns an error if the bytes are not a valid CoH3 replay.

### `Replay`

```rust
impl Replay {
    pub fn from_bytes(input: &[u8]) -> Result<Replay, ParseError>
    pub fn version(&self) -> u16                        // build number, e.g. 10612
    pub fn timestamp(&self) -> &str                     // local recording time
    pub fn game_type(&self) -> GameType
    pub fn matchhistory_id(&self) -> Option<u64>        // None for skirmish
    pub fn mod_uuid(&self) -> Uuid                      // all-zeros = base game
    pub fn map(&self) -> Map
    pub fn map_filename(&self) -> &str
    pub fn map_localized_name_id(&self) -> &str
    pub fn map_localized_description_id(&self) -> &str
    pub fn players(&self) -> Vec<Player>
    pub fn length(&self) -> usize                       // total ticks; divide by 8 for seconds
}

pub enum GameType { Skirmish, Multiplayer, Automatch, Custom }
```

### `Player`

```rust
impl Player {
    pub fn name(&self) -> &str
    pub fn human(&self) -> bool
    pub fn faction(&self) -> Faction
    pub fn team(&self) -> Team
    pub fn battlegroup(&self) -> Option<u32>            // pbgid of selected battlegroup
    pub fn battlegroup_selected_at(&self) -> Option<u32>
    pub fn ai_takeover_at(&self) -> Option<u32>
    pub fn steam_id(&self) -> Option<u64>
    pub fn profile_id(&self) -> Option<u64>
    pub fn messages(&self) -> Vec<Message>
    pub fn commands(&self) -> Vec<Command>
    pub fn build_commands(&self) -> Vec<Command>
    pub fn battlegroup_commands(&self) -> Vec<Command>
}

pub enum Faction { Americans, British, Wehrmacht, AfrikaKorps }
pub enum Team { First, Second }    // Team::First.value() == 0
```

### `Message`

```rust
impl Message {
    pub fn tick(&self) -> u32       // divide by 8 for seconds
    pub fn message(&self) -> &str
}
```

### `extract_build_order`

```rust
pub fn extract_build_order(
    replay: &Replay,
    player_index: usize,
    store: &VersionedStore,
) -> Result<BuildOrder, Error>
```

Classify a player's commands into a chronological build order. `player_index` is a zero-based index into `replay.players()`. Returns an error if the index is out of range.

### `BuildOrder` and `BuildAction`

```rust
pub struct BuildOrder {
    pub actions: Vec<BuildAction>,
}

pub struct BuildAction {
    pub tick: u32,              // divide by 8 for seconds
    pub index: u32,             // command index within tick (tie-breaking)
    pub kind: BuildActionKind,
    pub pbgid: u32,             // entity/ability/upgrade pbgid
    pub suspect: bool,          // building may have been cancelled before first use
    pub cancelled: bool,        // action was explicitly cancelled
}

pub enum BuildActionKind {
    ConstructBuilding,          // UseAbility (autobuild)
    TrainUnit,                  // BuildSquad or spawner ability
    ResearchUpgrade,            // BuildGlobalUpgrade
    SelectBattlegroup,
    SelectBattlegroupAbility,
    UseBattlegroupAbility,
    AITakeover,                 // player dropped; terminates the build order
}
```

**Suspect buildings**: when a building is cancelled, cohlib marks subsequent buildings of the same type as suspects until production resumes from one of them. Suspect actions should be validated against actual production before being displayed. Actions with `cancelled: true` are always excluded from the returned `actions` list.

### `VersionedStore`

```rust
impl VersionedStore {
    /// Load all compiled-in historical game data. No external files required.
    pub fn bundled() -> Self

    /// Start empty.
    pub fn new() -> Self

    /// Load from a directory of per-version game_data.json files.
    pub fn from_dir(dir: &Path) -> Result<Self, Error>

    /// Add or replace a version at runtime.
    pub fn add_version(&mut self, data: GameData)

    /// Number of loaded versions.
    pub fn version_count(&self) -> usize

    // Typed lookups — all use version fallback:
    // exact build → nearest older → nearest newer
    pub fn get_entity(&self, pbgid: u32, build: Version) -> Option<&Entity>
    pub fn get_squad(&self, pbgid: u32, build: Version) -> Option<&Squad>
    pub fn get_upgrade(&self, pbgid: u32, build: Version) -> Option<&Upgrade>
    pub fn get_ability(&self, pbgid: u32, build: Version) -> Option<&Ability>
    pub fn get_entity_by_path(&self, path: &str, build: Version) -> Option<&Entity>
    pub fn localize(&self, loc_id: u32, build: Version) -> Option<&str>
}
```

`Version` is `u32` — the numeric build number from the replay header (e.g. `10612`).

### Game data types

```rust
pub struct Entity {
    pub pbgid: u32,
    pub path: Vec<String>,      // hierarchical path, e.g. ["ebps", "races", "american", ...]
    pub loc_id: u32,
    pub icon_name: String,
    pub spawns: Vec<String>,    // squad paths this building can produce
    pub upgrades: Vec<String>,  // upgrade paths available from this building
}

pub struct Squad {
    pub pbgid: u32,
    pub path: Vec<String>,
    pub loc_id: u32,
    pub icon_name: String,
}

pub struct Upgrade {
    pub pbgid: u32,
    pub path: Vec<String>,
    pub loc_id: u32,
    pub icon_name: String,
}

pub struct Ability {
    pub pbgid: u32,
    pub path: Vec<String>,
    pub loc_id: u32,
    pub icon_name: String,
    pub autobuild: bool,        // true → UseAbility of this ability places a building
    pub builds: Option<String>, // entity path of the building it places
}

pub struct LocaleStore(pub HashMap<u32, String>);
impl LocaleStore {
    pub fn get(&self, id: u32) -> Option<&str>
}

pub struct GameData {
    pub version: Version,
    pub entities: HashMap<u32, Entity>,
    pub squads: HashMap<u32, Squad>,
    pub upgrades: HashMap<u32, Upgrade>,
    pub abilities: HashMap<u32, Ability>,
    pub locale: LocaleStore,
}
```

### SGA archive extraction

```rust
pub struct ArchiveEntry {
    pub path: String,       // normalized forward-slash path relative to archive root
    pub bytes: Vec<u8>,
}

impl ArchiveEntry {
    pub fn extension(&self) -> Option<&str>
}

/// Open an SGA archive and return all of its entries.
pub fn open_archive(path: &Path) -> Result<Vec<ArchiveEntry>, Error>

/// Read the NiceName string from an SGA archive header.
pub fn read_archive_name(path: &Path) -> Result<String, Error>
```

### Attribute extraction

```rust
/// Process XML attribute entries from ReferenceAttributes.sga into GameData.
pub fn extract_game_data(
    entries: &[ArchiveEntry],
    locale: LocaleStore,
    version: Version,
) -> Result<GameData, Error>
```

### Locale parsing

```rust
/// Decrypt and parse LocaleEnglish.sga (AES-128-CBC + zlib + UCS).
pub fn parse_locale_sga(path: &Path) -> Result<LocaleStore, Error>

/// Parse a decrypted .ucs text (UTF-16 LE or UTF-8).
pub fn parse_locale_ucs(text: &str) -> Result<LocaleStore, Error>

/// Parse tab-separated locale text (cohdata format).
pub fn parse_locale_txt(text: &str) -> Result<LocaleStore, Error>

/// Parse a JSON locale map.
pub fn parse_locale_json(json: &str) -> Result<LocaleStore, Error>
```

### JSON import

```rust
/// Import GameData from a directory containing abilities.json, ebps.json,
/// sbps.json, upgrade.json, and locale.json or locale.txt.
pub fn import_version(data_dir: &Path, version: Version) -> Result<GameData, Error>
```

### Icon extraction

```rust
/// Convert RRTEX bytes to WebP bytes. Supports BC1 (DXT1) and BC3 (DXT5).
pub fn extract_icon(rrtex_bytes: &[u8]) -> Result<Vec<u8>, Error>
```

### Error type

```rust
pub enum Error {
    Sga(String),
    Attrib(String),
    Locale(String),
    JsonImport(String),
    Replay(String),
    BuildOrder(String),
}
```

All public functions return `Result<T, cohlib::Error>` and never panic.

## Feature flags

| Flag | Description |
|---|---|
| `magnus` | Enables `#[magnus::wrap]` on public types for use in Ruby FFI bindings. Used by cohlib-rb. |
| `trace` | Enables nom-tracable tracing in the replay parser. For debugging only. |

## CLI

The `cohlib` binary provides maintainer tooling for managing the bundled game data. It is not required for normal library use.

### `cohlib populate`

Import historical game data from cohdata-format source directories.

```
cohlib populate <source_dir>... --output <data_dir>
```

Each `<source_dir>` is scanned for numeric subdirectories (e.g. `10612/`). Each subdirectory must contain `abilities.json`, `ebps.json`, `sbps.json`, `upgrade.json`, and optionally `locale.txt` or `locale.json`. The result is written to `<data_dir>/<version>/game_data.json`.

If multiple source directories contain the same version, the first one wins.

**Example:**

```sh
cohlib populate ~/cohdata/data ~/Code/reinforce/data --output data/
```

### `cohlib import`

Extract game data from a CoH3 SGA depot for a specific build.

```
cohlib import <depot_path> --version <build_number> --output <data_dir>
```

Reads `<depot_path>/anvil/archives/ReferenceAttributes.sga` for entity data and `LocaleEnglish.sga` for locale strings. Writes `<data_dir>/<version>/game_data.json`.

**Example:**

```sh
cohlib import ~/Steam/steamapps/common/coh3/depot_1677281 \
    --version 21283 \
    --output data/
```

After running `import`, commit the new `data/<version>/` directory. The next `cargo build` picks it up and incorporates it into the compiled-in bundle automatically.

## Bundled data workflow

When a new CoH3 patch ships:

1. Download the new depot via SteamCMD.
2. Run `cohlib import <depot_path> --version <build_number> --output data/`.
3. Commit `data/<build_number>/game_data.json`.
4. Run `cargo build` — the new version is incorporated into the bundle.
5. Publish a new crate release. All consumers of `VersionedStore::bundled()` get the new version automatically.

## Running tests

```sh
cargo test                  # all unit + integration tests
cargo test --test build_order   # end-to-end build order comparison vs reinforce
cargo test --test bundled       # bundled data version and lookup tests
cargo test --test replay        # replay parse tests
cargo clippy -- -D warnings
cargo fmt --check
```
