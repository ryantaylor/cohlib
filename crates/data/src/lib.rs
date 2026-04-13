mod error;
pub use error::Error;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub type Version = u32;

trait Localizable {
    fn loc_id(&self) -> u32;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub pbgid: u32,
    pub path: Vec<String>,
    pub loc_id: u32,
    pub icon_name: String,
    pub spawns: Vec<String>,
    pub upgrades: Vec<String>,
}

impl Localizable for &Entity {
    fn loc_id(&self) -> u32 {
        self.loc_id
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upgrade {
    pub pbgid: u32,
    pub path: Vec<String>,
    pub loc_id: u32,
    pub icon_name: String,
}

impl Localizable for &Upgrade {
    fn loc_id(&self) -> u32 {
        self.loc_id
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
}

impl Localizable for &Ability {
    fn loc_id(&self) -> u32 {
        self.loc_id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleStore(pub HashMap<u32, String>);

impl LocaleStore {
    pub fn get(&self, id: u32) -> Option<&str> {
        self.0.get(&id).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameData {
    pub version: Version,
    pub entities: HashMap<u32, Entity>,
    pub squads: HashMap<u32, Squad>,
    pub upgrades: HashMap<u32, Upgrade>,
    pub abilities: HashMap<u32, Ability>,
    pub locale: LocaleStore,
}

impl GameData {
    pub fn new(version: Version) -> Self {
        Self {
            version,
            entities: HashMap::new(),
            squads: HashMap::new(),
            upgrades: HashMap::new(),
            abilities: HashMap::new(),
            locale: LocaleStore(HashMap::new()),
        }
    }
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
}

impl VersionedStore {
    /// Creates an empty store. Use [`add_version`] to populate.
    pub fn new() -> Self {
        Self {
            versions: Vec::new(),
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
        let versions: Vec<GameData> =
            serde_json::from_slice(&json).expect("bundled game data is corrupt");
        let mut store = Self { versions };
        store.versions.sort_by_key(|g| g.version);
        store
    }

    /// Load all `game_data.json` files from a directory tree organised as `{dir}/{version}/game_data.json`.
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

    /// Returns the localized string for `loc_id` at `build`, with version fallback.
    pub fn localize(&self, loc_id: u32, build: Version) -> Option<&str> {
        self.resolve(build, |gd| gd.locale.get(loc_id))
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
}

impl Default for VersionedStore {
    fn default() -> Self {
        Self::new()
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
            },
        );
        gd
    }

    #[test]
    fn exact_version_match() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd(100, 1, 10, LocaleStore(HashMap::new())));
        store.add_version(make_gd(200, 1, 20, LocaleStore(HashMap::new())));
        store.add_version(make_gd(300, 1, 30, LocaleStore(HashMap::new())));
        assert_eq!(store.get_entity(1, 200).map(|e| e.loc_id), Some(20));
    }

    #[test]
    fn fallback_to_older_version() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd(100, 1, 10, LocaleStore(HashMap::new())));
        store.add_version(make_gd(300, 1, 30, LocaleStore(HashMap::new())));
        // Version 200 → falls back to 100 (nearest older)
        assert_eq!(store.get_entity(1, 200).map(|e| e.loc_id), Some(10));
    }

    #[test]
    fn fallback_to_newer_version() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd(200, 1, 20, LocaleStore(HashMap::new())));
        store.add_version(make_gd(300, 1, 30, LocaleStore(HashMap::new())));
        // Version 50 → no older, falls forward to 200
        assert_eq!(store.get_entity(1, 50).map(|e| e.loc_id), Some(20));
    }

    #[test]
    fn missing_pbgid_returns_none() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd(100, 1, 10, LocaleStore(HashMap::new())));
        assert_eq!(store.get_entity(999, 100), None);
    }

    #[test]
    fn add_version_replaces_existing() {
        let mut store = VersionedStore::new();
        store.add_version(make_gd(100, 1, 10, LocaleStore(HashMap::new())));
        store.add_version(make_gd(100, 1, 99, LocaleStore(HashMap::new())));
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
        let gd = make_gd(10612, 42, 7, LocaleStore(HashMap::new()));
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
        let mut locale: HashMap<u32, String> = HashMap::new();
        locale.insert(30, "test string".to_string());
        store.add_version(make_gd(200, 1, 0, LocaleStore(HashMap::new())));
        store.add_version(make_gd(300, 1, 30, LocaleStore(locale)));

        assert!(store
            .local_name_for(1, 300)
            .is_some_and(|s| s == "test string"));
    }

    #[test]
    fn local_name_for_version_mismatch() {
        let mut store = VersionedStore::new();
        let mut locale: HashMap<u32, String> = HashMap::new();
        locale.insert(30, "test string".to_string());
        store.add_version(make_gd(200, 1, 0, LocaleStore(HashMap::new())));
        store.add_version(make_gd(300, 1, 30, LocaleStore(locale)));

        assert!(store
            .local_name_for(1, 200)
            .is_some_and(|s| s == "test string"));
    }

    #[test]
    fn local_name_for_version_does_not_exist() {
        let mut store = VersionedStore::new();
        let mut locale: HashMap<u32, String> = HashMap::new();
        locale.insert(30, "test string".to_string());
        store.add_version(make_gd(200, 1, 0, LocaleStore(HashMap::new())));
        store.add_version(make_gd(300, 1, 30, LocaleStore(locale)));

        assert!(store
            .local_name_for(1, 100)
            .is_some_and(|s| s == "test string"));
    }
}
