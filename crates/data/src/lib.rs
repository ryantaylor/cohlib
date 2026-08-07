mod error;
pub use error::Error;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub type Version = u32;

/// Composed localized name built from a format template and positional arguments.
///
/// The template string contains `%1%`, `%2%`, … placeholders (1-indexed).
/// Each element of `arg_loc_ids` resolves to a localized string substituted into
/// the corresponding placeholder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenNameFormatter {
    /// Loc ID for the format template, e.g. 11261319 → "Unlock %1% Production"
    pub template_loc_id: u32,
    /// Loc IDs for positional arguments in order.
    pub arg_loc_ids: Vec<u32>,
}

trait Localizable {
    fn loc_id(&self) -> u32;
}

trait Iconable {
    fn icon_name(&self) -> &str;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub pbgid: u32,
    pub path: Vec<String>,
    pub loc_id: u32,
    pub icon_name: String,
    pub spawns: Vec<String>,
    pub upgrades: Vec<String>,
    /// Resource income, for territory point ebps (`resource_ext`). `None` for
    /// entities that don't produce resources, including victory points.
    #[serde(default)]
    pub resource: Option<ResourceIncome>,
    /// Capture/revert timing, for territory point ebps (`strategic_point_ext`).
    #[serde(default)]
    pub capture: Option<CaptureInfo>,
}

impl Localizable for &Entity {
    fn loc_id(&self) -> u32 {
        self.loc_id
    }
}

impl Iconable for &Entity {
    fn icon_name(&self) -> &str {
        &self.icon_name
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Squad {
    pub pbgid: u32,
    pub path: Vec<String>,
    pub loc_id: u32,
    pub icon_name: String,
}

impl Localizable for &Squad {
    fn loc_id(&self) -> u32 {
        self.loc_id
    }
}

impl Iconable for &Squad {
    fn icon_name(&self) -> &str {
        &self.icon_name
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upgrade {
    pub pbgid: u32,
    pub path: Vec<String>,
    pub loc_id: u32,
    pub icon_name: String,
    #[serde(default)]
    pub screen_name_formatter: Option<ScreenNameFormatter>,
}

impl Localizable for &Upgrade {
    fn loc_id(&self) -> u32 {
        self.loc_id
    }
}

impl Iconable for &Upgrade {
    fn icon_name(&self) -> &str {
        &self.icon_name
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ability {
    pub pbgid: u32,
    pub path: Vec<String>,
    pub loc_id: u32,
    pub icon_name: String,
    pub autobuild: bool,
    pub builds: Option<String>,
    #[serde(default)]
    pub spawns: Vec<String>,
    #[serde(default)]
    pub upgrades: Vec<String>,
    #[serde(default)]
    pub screen_name_formatter: Option<ScreenNameFormatter>,
}

impl Localizable for &Ability {
    fn loc_id(&self) -> u32 {
        self.loc_id
    }
}

impl Iconable for &Ability {
    fn icon_name(&self) -> &str {
        &self.icon_name
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleStore(pub BTreeMap<u32, String>);

impl LocaleStore {
    pub fn get(&self, id: u32) -> Option<&str> {
        self.0.get(&id).map(|s| s.as_str())
    }
}

/// Marketing semver triple `major.minor.patch` extracted from `RelicCoH3.exe`,
/// e.g. `2.4.0` for build 46121. The build number is tracked separately as
/// [`GameData::version`] (the PE ProductVersion build number).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Semver {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl std::fmt::Display for Semver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// World-space dimensions of a multiplayer scenario, as declared in its `.info`
/// file (`HeaderInfo.mapsize`). Not present in replay data — only available from
/// imported game data. Used to project in-replay world coordinates (starting
/// positions, territory/victory points) onto a minimap image: pixel fraction is
/// simply `coordinate / (size / 2) / 2 + 0.5` in each axis.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MapSize {
    pub width: f32,
    pub height: f32,
}

/// Resource type a territory point provides, from `resource_ext`'s
/// `default_provided_resource`. The XML uses the singular `"munition"`; this
/// crate always uses the plural `Munitions` to match `data:` command payloads
/// and existing `Value` resource naming elsewhere in cohlib.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceKind {
    Fuel,
    Munitions,
    Manpower,
}

/// Income an ebp provides while captured, read from `resource_ext`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ResourceIncome {
    pub kind: ResourceKind,
    /// Raw per-second rate as stored in the attribute file. Multiply by 60 for
    /// per-minute income — do not round the per-second rate first, or the
    /// result will drift from the game's displayed per-minute totals (e.g.
    /// `0.08335 * 60 = 5.001`, not `5`).
    pub per_second: f32,
}

/// Capture/revert timing an ebp provides while captured, read from
/// `strategic_point_ext`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CaptureInfo {
    pub capture_time: f32,
    pub revert_time: f32,
}

/// Axis-aligned rectangle in scenario world-space coordinates (same space as
/// [`ScenarioPoint::x`]/`y`, centered on the map origin).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl Rect {
    pub fn width(&self) -> f32 {
        self.max_x - self.min_x
    }

    pub fn height(&self) -> f32 {
        self.max_y - self.min_y
    }
}

/// What kind of point a [`ScenarioPoint`] is, derived from its ebp name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointKind {
    Fuel,
    Munitions,
    Manpower,
    Victory,
    Start,
    Other,
}

/// Income tier of a resource point, derived from its ebp name. `None` for
/// non-resource points (victory points, starting positions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointTier {
    ExtraLow,
    Low,
    Medium,
    ExtraMedium,
    High,
}

/// A single placed point on a scenario: a resource point, victory point, or
/// player starting position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioPoint {
    /// Ebp name as placed on the map, e.g. `territory_fuel_point_low_smaller`.
    pub ebp: String,
    pub x: f32,
    pub y: f32,
    pub kind: PointKind,
    #[serde(default)]
    pub tier: Option<PointTier>,
    /// Player index (0-based) for [`PointKind::Start`] points, `None` otherwise.
    #[serde(default)]
    pub owner: Option<u32>,
    /// `income_per_second * 60` resolved from [`Entity::resource`] via the ebp's
    /// pbgid; `0.0` for points with no resource (victory points).
    #[serde(default)]
    pub income_per_minute: f32,
    #[serde(default)]
    pub capture_time: Option<f32>,
    /// Id of the [`Sector`] this point falls within, if sector data is available.
    #[serde(default)]
    pub sector: Option<u32>,
}

/// A capturable territory sector traced from `<map>_territory.override`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sector {
    /// 1-based sector id, matching [`ScenarioPoint::sector`].
    pub id: u32,
    /// Whether this is a player base/HQ sector (uncapturable, starts owned).
    pub is_base: bool,
    pub neighbors: Vec<u32>,
    pub bounds: Rect,
    /// Indices into [`Scenario::points`] for points inside this sector.
    #[serde(default)]
    pub points: Vec<usize>,
    /// Outline(s) of the sector in world-space coordinates, one closed ring per
    /// polygon (normally one, but a sector split by terrain could have more).
    /// Traced directly from the sector-id grid — adjacent sectors' rings share
    /// exact boundary coordinates, so there are no rendering gaps between them.
    #[serde(default)]
    pub rings: Vec<Vec<[f32; 2]>>,
}

/// Full extracted metadata for a multiplayer scenario: dimensions, resource
/// points with tiers and income, sector boundaries, and playable area.
///
/// Constructed at import time by the `scenario` crate from `ScenariosMP.sga`;
/// looked up via [`VersionedStore::get_scenario`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub size: MapSize,
    /// World-space bounding box of the actually-playable region, as read from
    /// the map's authored soft-edge mask (`_softmapedge.override`) — distinct
    /// from [`Scenario::size`], which includes the unplayable border margin
    /// every scenario has around its declared world size. `None` if the mask
    /// could not be read.
    #[serde(default)]
    pub playable_area: Option<Rect>,
    #[serde(default)]
    pub max_players: u32,
    /// Player count per team, e.g. `[4, 4]` for a 4v4 map.
    #[serde(default)]
    pub teams: [u32; 2],
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub name_loc_id: u32,
    #[serde(default)]
    pub description_loc_id: u32,
    #[serde(default)]
    pub scenario_type: u32,
    /// `HeaderInfo.map_origin` — `2` indicates a community (Steam Workshop) map.
    #[serde(default)]
    pub map_origin: u32,
    #[serde(default)]
    pub visible_in_lobby: bool,
    /// Empty for scenarios where only dimensions could be recovered (e.g.
    /// versions migrated from the old size-only bundle format).
    #[serde(default)]
    pub points: Vec<ScenarioPoint>,
    #[serde(default)]
    pub sectors: Vec<Sector>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameData {
    pub version: Version,
    pub entities: BTreeMap<u32, Entity>,
    pub squads: BTreeMap<u32, Squad>,
    pub upgrades: BTreeMap<u32, Upgrade>,
    pub abilities: BTreeMap<u32, Ability>,
    pub locale: LocaleStore,
    #[serde(default)]
    pub data_checksum: Option<i32>,
    #[serde(default)]
    pub semver: Option<Semver>,
    /// Keyed by normalized scenario path (`scenarios/multiplayer/.../<name>`, no
    /// `data:` prefix, forward slashes) — see [`normalize_scenario`]. Values are
    /// content hashes into [`VersionedStore`]'s shared scenario table, not
    /// [`Scenario`] records themselves: scenarios rarely change between game
    /// versions, so records are deduplicated across the whole bundle rather than
    /// repeated per version.
    #[serde(default)]
    pub scenarios: BTreeMap<String, String>,
}

impl GameData {
    pub fn new(version: Version) -> Self {
        Self {
            version,
            entities: BTreeMap::new(),
            squads: BTreeMap::new(),
            upgrades: BTreeMap::new(),
            abilities: BTreeMap::new(),
            locale: LocaleStore(BTreeMap::new()),
            data_checksum: None,
            semver: None,
            scenarios: BTreeMap::new(),
        }
    }
}

/// Normalizes a scenario identifier for `GameData::scenarios` lookups: strips the
/// leading `data:` prefix used by replay files and converts backslashes to
/// forward slashes, matching the path form derived from scenario `.info` files
/// (e.g. `data:scenarios\multiplayer\wadi_darnah_4p\wadi_darnah_4p` →
/// `scenarios/multiplayer/wadi_darnah_4p/wadi_darnah_4p`).
fn normalize_scenario(scenario: &str) -> String {
    scenario
        .strip_prefix("data:")
        .unwrap_or(scenario)
        .replace('\\', "/")
}

/// Version-aware entity store that holds multiple game versions and resolves lookups
/// with fallback: exact match → nearest older version → nearest newer version.
///
/// Use [`VersionedStore::bundled()`] to get a store pre-loaded with all historical
/// game data compiled into the library, or [`VersionedStore::new()`] to start empty
/// and call [`VersionedStore::add_version()`] to populate it.
#[cfg_attr(feature = "magnus", magnus::wrap(class = "CohLib::VersionedStore"))]
pub struct VersionedStore {
    /// Sorted ascending by version number.
    versions: Vec<GameData>,
    /// Shared across all versions, keyed by content hash — see [`GameData::scenarios`].
    scenario_data: BTreeMap<String, Scenario>,
}

/// On-disk/bundled shape: versions plus the shared, deduplicated scenario table
/// they reference by hash. Used by both `build.rs` (embedding) and
/// [`VersionedStore::bundled`]/[`VersionedStore::from_dir`] (loading).
#[derive(Debug, Serialize, Deserialize)]
struct Bundle {
    versions: Vec<GameData>,
    #[serde(default)]
    scenarios: BTreeMap<String, Scenario>,
}

impl VersionedStore {
    /// Creates an empty store. Use [`add_version`] to populate.
    pub fn new() -> Self {
        Self {
            versions: Vec::new(),
            scenario_data: BTreeMap::new(),
        }
    }

    /// Loads all compiled-in historical game data. This is the primary constructor
    /// for library consumers — no file system access or setup required.
    pub fn bundled() -> Self {
        use std::io::Read;
        let compressed = include_bytes!(concat!(env!("OUT_DIR"), "/game_data.bin"));
        let mut decoder = flate2::read::GzDecoder::new(compressed.as_slice());
        let mut json = Vec::new();
        decoder
            .read_to_end(&mut json)
            .expect("bundled game data decompression failed");
        let bundle: Bundle = serde_json::from_slice(&json).expect("bundled game data is corrupt");
        let mut store = Self {
            versions: bundle.versions,
            scenario_data: bundle.scenarios,
        };
        store.versions.sort_by_key(|g| g.version);
        store
    }

    /// Load all `game_data.json` files from a directory tree organised as
    /// `{dir}/{version}/game_data.json`, plus the shared scenario table at
    /// `{dir}/scenarios/{hash}.json` (see [`GameData::scenarios`]), if present.
    pub fn from_dir(dir: &Path) -> Result<Self, Error> {
        let mut store = Self::new();
        let read =
            std::fs::read_dir(dir).map_err(|e| Error::Load(format!("cannot read dir: {e}")))?;
        for entry in read.flatten() {
            let path = entry.path().join("game_data.json");
            if path.exists() {
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| Error::Load(format!("cannot read {}: {e}", path.display())))?;
                let gd: GameData = serde_json::from_str(&text)
                    .map_err(|e| Error::Load(format!("cannot parse {}: {e}", path.display())))?;
                store.add_version(gd);
            }
        }

        let scenarios_dir = dir.join("scenarios");
        if scenarios_dir.is_dir() {
            let read = std::fs::read_dir(&scenarios_dir)
                .map_err(|e| Error::Load(format!("cannot read scenarios dir: {e}")))?;
            for entry in read.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Some(hash) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| Error::Load(format!("cannot read {}: {e}", path.display())))?;
                let scenario: Scenario = serde_json::from_str(&text)
                    .map_err(|e| Error::Load(format!("cannot parse {}: {e}", path.display())))?;
                store.scenario_data.insert(hash.to_string(), scenario);
            }
        }

        Ok(store)
    }

    /// Add a game version to the store. Replaces any existing entry for the same version number.
    pub fn add_version(&mut self, data: GameData) {
        if let Some(pos) = self.versions.iter().position(|g| g.version == data.version) {
            self.versions[pos] = data;
        } else {
            let idx = self.versions.partition_point(|g| g.version < data.version);
            self.versions.insert(idx, data);
        }
    }

    /// Adds a [`Scenario`] record to the shared table under `hash`, for a
    /// [`GameData::scenarios`] entry to reference. Replaces any existing record
    /// under the same hash.
    pub fn add_scenario(&mut self, hash: impl Into<String>, scenario: Scenario) {
        self.scenario_data.insert(hash.into(), scenario);
    }

    /// Returns the number of versions loaded.
    pub fn version_count(&self) -> usize {
        self.versions.len()
    }

    /// Returns entity for `pbgid` at `build`, with version fallback.
    pub fn get_entity(&self, pbgid: u32, build: Version) -> Option<&Entity> {
        self.resolve(build, |gd| gd.entities.get(&pbgid))
    }

    /// Returns entity with loc_id for `pbgid` at `build`, with version fallback.
    pub fn get_localizable_entity(&self, pbgid: u32, build: Version) -> Option<&Entity> {
        self.resolve_loc(build, |gd| gd.entities.get(&pbgid))
    }

    /// Returns squad for `pbgid` at `build`, with version fallback.
    pub fn get_squad(&self, pbgid: u32, build: Version) -> Option<&Squad> {
        self.resolve(build, |gd| gd.squads.get(&pbgid))
    }

    /// Returns squad with loc_id for `pbgid` at `build`, with version fallback.
    pub fn get_localizable_squad(&self, pbgid: u32, build: Version) -> Option<&Squad> {
        self.resolve_loc(build, |gd| gd.squads.get(&pbgid))
    }

    /// Returns upgrade for `pbgid` at `build`, with version fallback.
    pub fn get_upgrade(&self, pbgid: u32, build: Version) -> Option<&Upgrade> {
        self.resolve(build, |gd| gd.upgrades.get(&pbgid))
    }

    /// Returns upgrade with loc_id for `pbgid` at `build`, with version fallback.
    pub fn get_localizable_upgrade(&self, pbgid: u32, build: Version) -> Option<&Upgrade> {
        self.resolve_loc(build, |gd| gd.upgrades.get(&pbgid))
    }

    /// Returns ability for `pbgid` at `build`, with version fallback.
    pub fn get_ability(&self, pbgid: u32, build: Version) -> Option<&Ability> {
        self.resolve(build, |gd| gd.abilities.get(&pbgid))
    }

    /// Returns ability with loc_id for `pbgid` at `build`, with version fallback.
    pub fn get_localizable_ability(&self, pbgid: u32, build: Version) -> Option<&Ability> {
        self.resolve_loc(build, |gd| gd.abilities.get(&pbgid))
    }

    /// Returns entity with a non-empty icon_name for `pbgid` at `build`, with version fallback.
    pub fn get_iconable_entity(&self, pbgid: u32, build: Version) -> Option<&Entity> {
        self.resolve_icon(build, |gd| gd.entities.get(&pbgid))
    }

    /// Returns squad with a non-empty icon_name for `pbgid` at `build`, with version fallback.
    pub fn get_iconable_squad(&self, pbgid: u32, build: Version) -> Option<&Squad> {
        self.resolve_icon(build, |gd| gd.squads.get(&pbgid))
    }

    /// Returns upgrade with a non-empty icon_name for `pbgid` at `build`, with version fallback.
    pub fn get_iconable_upgrade(&self, pbgid: u32, build: Version) -> Option<&Upgrade> {
        self.resolve_icon(build, |gd| gd.upgrades.get(&pbgid))
    }

    /// Returns ability with a non-empty icon_name for `pbgid` at `build`, with version fallback.
    pub fn get_iconable_ability(&self, pbgid: u32, build: Version) -> Option<&Ability> {
        self.resolve_icon(build, |gd| gd.abilities.get(&pbgid))
    }

    /// Returns the localized string for `loc_id` at `build`, with version fallback.
    pub fn localize(&self, loc_id: u32, build: Version) -> Option<&str> {
        self.resolve(build, |gd| gd.locale.get(loc_id))
    }

    /// Returns the full [`Scenario`] record for `scenario` at `build`, with
    /// version fallback.
    ///
    /// `scenario` accepts either raw replay form (`data:scenarios\...`) or the
    /// normalized forward-slash form — see [`normalize_scenario`]. Only scenarios
    /// re-imported since this field was added carry a value, so builds before that
    /// fall through to the nearest version (older or newer) that does — maps are
    /// rarely resized or re-territoried between patches, so this is normally the
    /// correct answer. Scenario records are deduplicated by content hash across
    /// versions (see [`GameData::scenarios`]), so this resolves the hash for
    /// `build` first, then looks it up in the shared table.
    pub fn get_scenario(&self, scenario: &str, build: Version) -> Option<&Scenario> {
        let key = normalize_scenario(scenario);
        let hash = self.resolve(build, |gd| gd.scenarios.get(&key))?;
        self.scenario_data.get(hash)
    }

    /// Returns the [`MapSize`] for `scenario` at `build`, with version fallback.
    /// Shorthand for `get_scenario(...).map(|s| &s.size)` — see [`get_scenario`](Self::get_scenario).
    pub fn get_map_size(&self, scenario: &str, build: Version) -> Option<&MapSize> {
        self.get_scenario(scenario, build).map(|s| &s.size)
    }

    /// Returns the localized string name for `pbgid` at `build`, with version fallback,
    /// and skipping entries without valid localization.
    pub fn local_name_for(&self, pbgid: u32, build: Version) -> Option<&str> {
        let loc_id = self
            .get_localizable_entity(pbgid, build)
            .map(|e| e.loc_id)
            .or_else(|| self.get_localizable_squad(pbgid, build).map(|s| s.loc_id))
            .or_else(|| self.get_localizable_upgrade(pbgid, build).map(|u| u.loc_id))
            .or_else(|| self.get_localizable_ability(pbgid, build).map(|a| a.loc_id))?;
        self.localize(loc_id, build)
    }

    /// Returns the localized screen name for `pbgid` at `build`, supporting both
    /// plain `loc_id` lookup and formatter-based composition.
    ///
    /// Tries the direct `loc_id` path first. If that fails (e.g. `loc_id == 0`),
    /// falls back to a `screen_name_formatter` on the upgrade or ability. Returns
    /// `None` if neither resolves to a name.
    pub fn local_name_for_formatted(&self, pbgid: u32, build: Version) -> Option<String> {
        if let Some(s) = self.local_name_for(pbgid, build) {
            return Some(s.to_owned());
        }
        if let Some(fmt) = self.formatter_for_upgrade(pbgid, build) {
            return self.apply_formatter(&fmt, build);
        }
        if let Some(fmt) = self.formatter_for_ability(pbgid, build) {
            return self.apply_formatter(&fmt, build);
        }
        None
    }

    /// Returns `(data_checksum, app_binary_checksum)` for the exact `build` version.
    ///
    /// `app_binary_checksum` is identical to the build number (the PE build number
    /// is what the game sends as `appBinaryChecksum` in API requests).
    /// Returns `None` if the exact version is not present or has no `data_checksum`.
    /// No version fallback is applied — checksums are version-specific and cannot be
    /// substituted from a nearby patch.
    pub fn checksums_for(&self, build: Version) -> Option<(i32, u32)> {
        let idx = self.versions.partition_point(|g| g.version < build);
        let gd = self.versions.get(idx).filter(|g| g.version == build)?;
        gd.data_checksum.map(|dc| (dc, gd.version))
    }

    /// Returns the marketing [`Semver`] (e.g. `2.4.0`) for the exact `build` version,
    /// if extracted at import time. No version fallback — marketing versions are
    /// version-specific.
    pub fn semver_for(&self, build: Version) -> Option<Semver> {
        let idx = self.versions.partition_point(|g| g.version < build);
        let gd = self.versions.get(idx).filter(|g| g.version == build)?;
        gd.semver
    }

    /// Returns the marketing semver string (e.g. `"2.4.0"`) for the exact `build`
    /// version, if a semver was extracted. No version fallback.
    ///
    /// Minor hotfixes sometimes ship a new build without a marketing semver
    /// bump, so the same `Semver` triple can appear on more than one build in
    /// the store. Consumers (e.g. cohdb's `PatchVersion.version`, which is
    /// unique per build) need a distinct string per build, so any build that
    /// isn't the earliest to carry a given semver gets the build number
    /// appended as a fourth component, e.g. `2.5.0.48791`.
    pub fn semver_string_for(&self, build: Version) -> Option<String> {
        let semver = self.semver_for(build)?;
        let earliest = self
            .versions
            .iter()
            .find(|gd| gd.semver == Some(semver))
            .map(|gd| gd.version);
        if earliest == Some(build) {
            Some(semver.to_string())
        } else {
            Some(format!("{semver}.{build}"))
        }
    }

    /// Returns all build numbers currently loaded in the store, sorted ascending.
    pub fn builds(&self) -> Vec<Version> {
        self.versions.iter().map(|g| g.version).collect()
    }

    /// Returns the icon name for `pbgid` at `build`, skipping versions where the icon is empty,
    /// with version fallback across all entity types.
    pub fn icon_for(&self, pbgid: u32, build: Version) -> Option<&str> {
        self.get_iconable_entity(pbgid, build)
            .map(|e| e.icon_name.as_str())
            .or_else(|| {
                self.get_iconable_squad(pbgid, build)
                    .map(|s| s.icon_name.as_str())
            })
            .or_else(|| {
                self.get_iconable_upgrade(pbgid, build)
                    .map(|u| u.icon_name.as_str())
            })
            .or_else(|| {
                self.get_iconable_ability(pbgid, build)
                    .map(|a| a.icon_name.as_str())
            })
    }

    /// Returns the `screen_name_formatter` for an upgrade at `build`, cloned to
    /// avoid a borrow conflict with the subsequent `localize` calls.
    fn formatter_for_upgrade(&self, pbgid: u32, build: Version) -> Option<ScreenNameFormatter> {
        self.resolve(build, |gd| {
            gd.upgrades
                .get(&pbgid)
                .and_then(|u| u.screen_name_formatter.clone())
        })
    }

    /// Returns the `screen_name_formatter` for an ability at `build`, cloned.
    fn formatter_for_ability(&self, pbgid: u32, build: Version) -> Option<ScreenNameFormatter> {
        self.resolve(build, |gd| {
            gd.abilities
                .get(&pbgid)
                .and_then(|a| a.screen_name_formatter.clone())
        })
    }

    /// Resolves a formatter's template and arguments, then substitutes `%1%`, `%2%`, …
    /// placeholders with the localized argument strings.
    fn apply_formatter(&self, fmt: &ScreenNameFormatter, build: Version) -> Option<String> {
        let template = self.localize(fmt.template_loc_id, build)?;
        let mut result = template.to_owned();
        for (i, &arg_id) in fmt.arg_loc_ids.iter().enumerate() {
            let placeholder = format!("%{}%", i + 1);
            let arg = self.localize(arg_id, build).unwrap_or("");
            result = result.replace(&placeholder, arg);
        }
        Some(result)
    }

    /// Returns an entity whose joined path (e.g. `"ebps/races/american/buildings/production/barracks_us"`)
    /// matches `path`, at `build`, with version fallback.
    pub fn get_entity_by_path(&self, path: &str, build: Version) -> Option<&Entity> {
        self.resolve(build, |gd| {
            gd.entities.values().find(|e| e.path.join("/") == path)
        })
    }

    /// Fallback resolution: exact version → older versions descending → newer versions ascending.
    fn resolve<'a, T, F>(&'a self, build: Version, f: F) -> Option<T>
    where
        F: Fn(&'a GameData) -> Option<T>,
    {
        let idx = self.versions.partition_point(|g| g.version <= build);
        // idx is the first version strictly greater than build.
        // Versions at [0..idx] are <= build; [idx..] are > build.

        // Walk from idx-1 downward (exact match first, then older).
        for i in (0..idx).rev() {
            if let Some(v) = f(&self.versions[i]) {
                return Some(v);
            }
        }
        // Walk from idx upward (newer versions).
        for i in idx..self.versions.len() {
            if let Some(v) = f(&self.versions[i]) {
                return Some(v);
            }
        }
        None
    }

    /// Fallback resolution explicitly for localization: skips results that have loc_id == 0
    fn resolve_loc<'a, T, F>(&'a self, build: Version, f: F) -> Option<T>
    where
        F: Fn(&'a GameData) -> Option<T>,
        T: Localizable,
    {
        let idx = self.versions.partition_point(|g| g.version <= build);
        // idx is the first version strictly greater than build.
        // Versions at [0..idx] are <= build; [idx..] are > build.

        // Walk from idx-1 downward (exact match first, then older).
        for i in (0..idx).rev() {
            if let Some(v) = f(&self.versions[i]) {
                if v.loc_id() == 0 {
                    continue;
                }
                return Some(v);
            }
        }
        // Walk from idx upward (newer versions).
        for i in idx..self.versions.len() {
            if let Some(v) = f(&self.versions[i]) {
                if v.loc_id() == 0 {
                    continue;
                }
                return Some(v);
            }
        }
        None
    }

    /// Fallback resolution explicitly for icons: skips results that have an empty icon_name.
    fn resolve_icon<'a, T, F>(&'a self, build: Version, f: F) -> Option<T>
    where
        F: Fn(&'a GameData) -> Option<T>,
        T: Iconable,
    {
        let idx = self.versions.partition_point(|g| g.version <= build);

        for i in (0..idx).rev() {
            if let Some(v) = f(&self.versions[i]) {
                if v.icon_name().is_empty() {
                    continue;
                }
                return Some(v);
            }
        }
        for i in idx..self.versions.len() {
            if let Some(v) = f(&self.versions[i]) {
                if v.icon_name().is_empty() {
                    continue;
                }
                return Some(v);
            }
        }
        None
    }
}

impl Default for VersionedStore {
    fn default() -> Self {
        Self::new()
    }
}

/// A named game entity with its pbgid and the build-order action type used to produce it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BuildEntry {
    pub pbgid: u32,
    pub name: String,
    pub action_type: String,
    /// Slash-joined attribute path, e.g. `upgrade_tables/americans/squad_upgrades/grenade_package`.
    /// Useful for distinguishing same-named entities across factions.
    pub path: String,
}

impl VersionedStore {
    /// Returns all named build-order entities from the latest loaded version.
    ///
    /// Covers the three action types tracked by cohdb's build-order classifier:
    /// - `TrainUnit`          → squads (infantry, vehicles, etc.)
    /// - `ConstructBuilding`  → buildings/structures (entities)
    /// - `ResearchUpgrade`    → upgrades
    ///
    /// Names are resolved via the locale store bundled with the latest version.
    /// Entries whose name cannot be resolved are excluded.
    pub fn all_build_entries(&self) -> Vec<BuildEntry> {
        let Some(latest) = self.versions.last() else {
            return Vec::new();
        };
        let build = latest.version;
        let mut entries: Vec<BuildEntry> = Vec::new();

        for (&pbgid, squad) in &latest.squads {
            if let Some(name) = self.local_name_for(pbgid, build) {
                let path = squad.path.join("/");
                entries.push(BuildEntry {
                    pbgid,
                    name: name.to_owned(),
                    action_type: "TrainUnit".to_owned(),
                    path,
                });
            }
        }
        for (&pbgid, entity) in &latest.entities {
            if let Some(name) = self.local_name_for(pbgid, build) {
                let path = entity.path.join("/");
                entries.push(BuildEntry {
                    pbgid,
                    name: name.to_owned(),
                    action_type: "ConstructBuilding".to_owned(),
                    path,
                });
            }
        }
        for (&pbgid, upgrade) in &latest.upgrades {
            if let Some(name) = self
                .local_name_for_formatted(pbgid, build)
                .or_else(|| self.local_name_for(pbgid, build).map(str::to_owned))
            {
                let path = upgrade.path.join("/");
                entries.push(BuildEntry {
                    pbgid,
                    name,
                    action_type: "ResearchUpgrade".to_owned(),
                    path,
                });
            }
        }
        // Construction abilities (autobuild or explicit `builds` target) appear in replay
        // data as ConstructBuilding commands. They are not entities/squads/upgrades, so
        // they must be iterated separately here.
        //
        // Abilities may not be present in the latest version's data (they can be absent from
        // newer patches while still appearing in replays). Collect pbgids across ALL versions
        // and use version-fallback resolution so nothing gets missed.
        let ability_pbgids: std::collections::HashSet<u32> = self
            .versions
            .iter()
            .flat_map(|gd| gd.abilities.keys().copied())
            .collect();
        for pbgid in ability_pbgids {
            let Some(ability) = self.get_ability(pbgid, build) else {
                continue;
            };
            if ability.builds.is_none() {
                continue;
            }
            if let Some(name) = self
                .local_name_for_formatted(pbgid, build)
                .or_else(|| self.local_name_for(pbgid, build).map(str::to_owned))
            {
                let path = ability.path.join("/");
                entries.push(BuildEntry {
                    pbgid,
                    name,
                    action_type: "ConstructBuilding".to_owned(),
                    path,
                });
            }
        }

        entries.sort();
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_gd(version: Version, pbgid: u32, loc_id: u32, locale: LocaleStore) -> GameData {
        let mut gd = GameData::new(version);
        gd.locale = locale;
        gd.entities.insert(
            pbgid,
            Entity {
                pbgid,
                path: vec!["ebps".into(), "test".into()],
                loc_id,
                icon_name: String::new(),
                spawns: vec![],
                upgrades: vec![],
                resource: None,
                capture: None,
            },
        );
        gd
    }

    #[test]
    fn exact_version_match() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd(100, 1, 10, LocaleStore(BTreeMap::new())));
        store.add_version(make_gd(200, 1, 20, LocaleStore(BTreeMap::new())));
        store.add_version(make_gd(300, 1, 30, LocaleStore(BTreeMap::new())));
        assert_eq!(store.get_entity(1, 200).map(|e| e.loc_id), Some(20));
    }

    #[test]
    fn fallback_to_older_version() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd(100, 1, 10, LocaleStore(BTreeMap::new())));
        store.add_version(make_gd(300, 1, 30, LocaleStore(BTreeMap::new())));
        // Version 200 → falls back to 100 (nearest older)
        assert_eq!(store.get_entity(1, 200).map(|e| e.loc_id), Some(10));
    }

    #[test]
    fn fallback_to_newer_version() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd(200, 1, 20, LocaleStore(BTreeMap::new())));
        store.add_version(make_gd(300, 1, 30, LocaleStore(BTreeMap::new())));
        // Version 50 → no older, falls forward to 200
        assert_eq!(store.get_entity(1, 50).map(|e| e.loc_id), Some(20));
    }

    #[test]
    fn missing_pbgid_returns_none() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd(100, 1, 10, LocaleStore(BTreeMap::new())));
        assert_eq!(store.get_entity(999, 100), None);
    }

    #[test]
    fn add_version_replaces_existing() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd(100, 1, 10, LocaleStore(BTreeMap::new())));
        store.add_version(make_gd(100, 1, 99, LocaleStore(BTreeMap::new())));
        assert_eq!(store.version_count(), 1);
        assert_eq!(store.get_entity(1, 100).map(|e| e.loc_id), Some(99));
    }

    #[test]
    fn empty_store_returns_none() {
        let store = VersionedStore::new();
        assert_eq!(store.get_entity(1, 100), None);
    }

    #[test]
    fn from_dir_loads_versions() {
        let dir = tempfile::tempdir().unwrap();
        let v_dir = dir.path().join("10612");
        std::fs::create_dir_all(&v_dir).unwrap();
        let gd = make_gd(10612, 42, 7, LocaleStore(BTreeMap::new()));
        std::fs::write(
            v_dir.join("game_data.json"),
            serde_json::to_string(&gd).unwrap(),
        )
        .unwrap();
        let store = VersionedStore::from_dir(dir.path()).unwrap();
        assert_eq!(store.version_count(), 1);
        assert_eq!(store.get_entity(42, 10612).map(|e| e.loc_id), Some(7));
    }

    #[test]
    fn bundled_loads_all_versions() {
        let store = VersionedStore::bundled();
        // 32 versions were populated into data/ during the build
        assert!(store.version_count() > 0);
        // Version 10612 should have real game data (pathfinder squad entity)
        assert!(store.get_entity(203329, 10612).is_some());
    }

    #[test]
    fn local_name_for_version_match() {
        let mut store = VersionedStore::new();
        let mut locale: BTreeMap<u32, String> = BTreeMap::new();
        locale.insert(30, "test string".to_string());
        store.add_version(make_gd(200, 1, 0, LocaleStore(BTreeMap::new())));
        store.add_version(make_gd(300, 1, 30, LocaleStore(locale)));

        assert!(store
            .local_name_for(1, 300)
            .is_some_and(|s| s == "test string"));
    }

    #[test]
    fn local_name_for_version_mismatch() {
        let mut store = VersionedStore::new();
        let mut locale: BTreeMap<u32, String> = BTreeMap::new();
        locale.insert(30, "test string".to_string());
        store.add_version(make_gd(200, 1, 0, LocaleStore(BTreeMap::new())));
        store.add_version(make_gd(300, 1, 30, LocaleStore(locale)));

        assert!(store
            .local_name_for(1, 200)
            .is_some_and(|s| s == "test string"));
    }

    #[test]
    fn local_name_for_version_does_not_exist() {
        let mut store = VersionedStore::new();
        let mut locale: BTreeMap<u32, String> = BTreeMap::new();
        locale.insert(30, "test string".to_string());
        store.add_version(make_gd(200, 1, 0, LocaleStore(BTreeMap::new())));
        store.add_version(make_gd(300, 1, 30, LocaleStore(locale)));

        assert!(store
            .local_name_for(1, 100)
            .is_some_and(|s| s == "test string"));
    }

    // ---------------------------------------------------------------------------
    // icon_for tests
    // ---------------------------------------------------------------------------

    fn make_gd_with_icon(version: Version, pbgid: u32, icon: &str) -> GameData {
        let mut gd = GameData::new(version);
        gd.entities.insert(
            pbgid,
            Entity {
                pbgid,
                path: vec!["ebps".into(), "test".into()],
                loc_id: 0,
                icon_name: icon.to_string(),
                spawns: vec![],
                upgrades: vec![],
                resource: None,
                capture: None,
            },
        );
        gd
    }

    #[test]
    fn icon_for_version_match() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd_with_icon(100, 1, "icons/tank"));
        store.add_version(make_gd_with_icon(200, 1, "icons/tank_v2"));
        store.add_version(make_gd_with_icon(300, 1, "icons/tank_v3"));
        assert_eq!(store.icon_for(1, 200), Some("icons/tank_v2"));
    }

    #[test]
    fn icon_for_version_mismatch_skips_empty() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd_with_icon(100, 1, "icons/tank"));
        store.add_version(make_gd_with_icon(300, 1, ""));
        // Version 200 → falls back to 100 (nearest older with non-empty icon), skipping 300
        assert_eq!(store.icon_for(1, 200), Some("icons/tank"));
    }

    #[test]
    fn icon_for_version_does_not_exist() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd_with_icon(200, 1, ""));
        store.add_version(make_gd_with_icon(300, 1, "icons/tank"));
        // Version 100 → no older, 200 is empty so skipped, falls forward to 300
        assert_eq!(store.icon_for(1, 100), Some("icons/tank"));
    }

    #[test]
    fn icon_for_returns_none_when_all_empty() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd_with_icon(100, 1, ""));
        store.add_version(make_gd_with_icon(200, 1, ""));
        assert_eq!(store.icon_for(1, 200), None);
    }

    // ---------------------------------------------------------------------------
    // semver_string_for tests
    // ---------------------------------------------------------------------------

    fn make_gd_with_semver(version: Version, semver: Semver) -> GameData {
        let mut gd = GameData::new(version);
        gd.semver = Some(semver);
        gd
    }

    #[test]
    fn semver_string_for_missing_semver_returns_none() {
        let mut store = VersionedStore::new();
        store.add_version(GameData::new(100));
        assert_eq!(store.semver_string_for(100), None);
    }

    #[test]
    fn semver_string_for_unique_semver_returns_plain_string() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd_with_semver(
            100,
            Semver {
                major: 2,
                minor: 5,
                patch: 0,
            },
        ));
        assert_eq!(store.semver_string_for(100).as_deref(), Some("2.5.0"));
    }

    #[test]
    fn semver_string_for_hotfix_collision_appends_build_number() {
        let mut store = VersionedStore::new();
        let semver = Semver {
            major: 2,
            minor: 5,
            patch: 0,
        };
        store.add_version(make_gd_with_semver(48652, semver));
        store.add_version(make_gd_with_semver(48791, semver));

        // Earliest build keeps the plain marketing semver.
        assert_eq!(store.semver_string_for(48652).as_deref(), Some("2.5.0"));
        // Later build sharing the same semver disambiguates with its build number.
        assert_eq!(
            store.semver_string_for(48791).as_deref(),
            Some("2.5.0.48791")
        );
    }

    #[test]
    fn semver_string_for_second_hotfix_appends_its_own_build_number() {
        let mut store = VersionedStore::new();
        let semver = Semver {
            major: 2,
            minor: 5,
            patch: 0,
        };
        store.add_version(make_gd_with_semver(48652, semver));
        store.add_version(make_gd_with_semver(48791, semver));
        store.add_version(make_gd_with_semver(48999, semver));

        assert_eq!(store.semver_string_for(48652).as_deref(), Some("2.5.0"));
        assert_eq!(
            store.semver_string_for(48791).as_deref(),
            Some("2.5.0.48791")
        );
        assert_eq!(
            store.semver_string_for(48999).as_deref(),
            Some("2.5.0.48999")
        );
    }

    // ---------------------------------------------------------------------------
    // ScreenNameFormatter / local_name_for_formatted tests
    // ---------------------------------------------------------------------------

    fn make_gd_with_upgrade_formatter(
        version: Version,
        pbgid: u32,
        template_loc_id: u32,
        arg_loc_ids: Vec<u32>,
        locale: LocaleStore,
    ) -> GameData {
        let mut gd = GameData::new(version);
        gd.locale = locale;
        gd.upgrades.insert(
            pbgid,
            Upgrade {
                pbgid,
                path: vec!["upgrade".into(), "test".into()],
                loc_id: 0,
                icon_name: String::new(),
                screen_name_formatter: Some(ScreenNameFormatter {
                    template_loc_id,
                    arg_loc_ids,
                }),
            },
        );
        gd
    }

    #[test]
    fn local_name_for_formatted_uses_formatter_when_loc_id_zero() {
        let mut store = VersionedStore::new();
        let mut locale = BTreeMap::new();
        locale.insert(100, "Hello %1%".to_string());
        locale.insert(200, "World".to_string());
        store.add_version(make_gd_with_upgrade_formatter(
            300,
            42,
            100,
            vec![200],
            LocaleStore(locale),
        ));
        assert_eq!(
            store.local_name_for_formatted(42, 300),
            Some("Hello World".to_string())
        );
    }

    #[test]
    fn local_name_for_formatted_prefers_direct_loc_id() {
        let mut store = VersionedStore::new();
        let mut locale = BTreeMap::new();
        locale.insert(10, "Direct Name".to_string());
        locale.insert(100, "Formatter Template %1%".to_string());
        locale.insert(200, "Arg".to_string());
        let mut gd = GameData::new(300);
        gd.locale = LocaleStore(locale);
        // Upgrade with both a loc_id and a formatter — loc_id should win.
        gd.upgrades.insert(
            42,
            Upgrade {
                pbgid: 42,
                path: vec!["upgrade".into(), "test".into()],
                loc_id: 10,
                icon_name: String::new(),
                screen_name_formatter: Some(ScreenNameFormatter {
                    template_loc_id: 100,
                    arg_loc_ids: vec![200],
                }),
            },
        );
        store.add_version(gd);
        assert_eq!(
            store.local_name_for_formatted(42, 300),
            Some("Direct Name".to_string())
        );
    }

    #[test]
    fn local_name_for_formatted_returns_none_if_neither() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd(300, 42, 0, LocaleStore(BTreeMap::new())));
        assert_eq!(store.local_name_for_formatted(42, 300), None);
    }

    #[test]
    fn apply_formatter_substitutes_single_arg() {
        let mut store = VersionedStore::new();
        let mut locale = BTreeMap::new();
        locale.insert(1, "Unlock %1% Production".to_string());
        locale.insert(2, "Sherman Easy Eight".to_string());
        store.add_version(make_gd_with_upgrade_formatter(
            100,
            9,
            1,
            vec![2],
            LocaleStore(locale),
        ));
        assert_eq!(
            store.local_name_for_formatted(9, 100),
            Some("Unlock Sherman Easy Eight Production".to_string())
        );
    }

    #[test]
    fn apply_formatter_substitutes_multiple_args() {
        let mut store = VersionedStore::new();
        let mut locale = BTreeMap::new();
        locale.insert(1, "Allows the %1% to be produced from the %2%.".to_string());
        locale.insert(2, "Sherman Easy Eight".to_string());
        locale.insert(3, "Tank Depot".to_string());
        store.add_version(make_gd_with_upgrade_formatter(
            100,
            9,
            1,
            vec![2, 3],
            LocaleStore(locale),
        ));
        assert_eq!(
            store.local_name_for_formatted(9, 100),
            Some("Allows the Sherman Easy Eight to be produced from the Tank Depot.".to_string())
        );
    }

    #[test]
    fn apply_formatter_returns_none_if_template_missing() {
        let mut store = VersionedStore::new();
        // Locale has arg but NOT the template.
        let mut locale = BTreeMap::new();
        locale.insert(2, "Arg Value".to_string());
        store.add_version(make_gd_with_upgrade_formatter(
            100,
            9,
            999, // template_loc_id not in locale
            vec![2],
            LocaleStore(locale),
        ));
        assert_eq!(store.local_name_for_formatted(9, 100), None);
    }

    #[test]
    fn local_name_for_formatted_regular_data_works() {
        let store = VersionedStore::bundled();
        // Version 44736 should have read game data (panzergrenadier squad entity)
        assert!(store.local_name_for_formatted(188642, 44736).is_some());
        assert_eq!(
            store.local_name_for_formatted(188642, 44736).unwrap(),
            "Panzergrenadier Squad"
        );
    }

    #[test]
    fn load_name_for_regular_data_works() {
        let store = VersionedStore::bundled();
        // Version 44736 should have read game data (panzergrenadier squad entity)
        assert!(store.local_name_for(188642, 44736).is_some());
        assert_eq!(
            store.local_name_for(188642, 44736).unwrap(),
            "Panzergrenadier Squad"
        );
    }
}
