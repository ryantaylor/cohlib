mod error;
pub use error::Error;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde_json::Value;

use data::{Ability, Entity, GameData, LocaleStore, Squad, Upgrade, Version};

/// Ability paths that are autobuilds even if name doesn't contain "auto_build"/"autobuild".
const AUTOBUILDS: &[&str] =
    &["abilities/races/american/battlegroups/infantry/infantry_left_2a_medical_tent"];

/// Locale entry types that are included in the locale store.
const LOCALE_TYPES: &[&str] = &[
    "abilities",
    "map_pool",
    "ebps",
    "racebps",
    "sbps",
    "upgrade",
    "weapon",
];

pub fn import_version(data_dir: &Path, version: Version) -> Result<GameData, Error> {
    let mut game_data = GameData::new(version);

    // abilities.json
    let abilities_path = data_dir.join("abilities.json");
    if abilities_path.exists() {
        let text = std::fs::read_to_string(&abilities_path)
            .map_err(|e| Error::JsonImport(e.to_string()))?;
        let value: Value =
            serde_json::from_str(&text).map_err(|e| Error::JsonImport(e.to_string()))?;
        parse_abilities(&value, &mut game_data.abilities)?;
    }

    // ebps.json
    let ebps_path = data_dir.join("ebps.json");
    if ebps_path.exists() {
        let text =
            std::fs::read_to_string(&ebps_path).map_err(|e| Error::JsonImport(e.to_string()))?;
        let value: Value =
            serde_json::from_str(&text).map_err(|e| Error::JsonImport(e.to_string()))?;
        parse_ebps(&value, &mut game_data.entities)?;
    }

    // sbps.json
    let sbps_path = data_dir.join("sbps.json");
    if sbps_path.exists() {
        let text =
            std::fs::read_to_string(&sbps_path).map_err(|e| Error::JsonImport(e.to_string()))?;
        let value: Value =
            serde_json::from_str(&text).map_err(|e| Error::JsonImport(e.to_string()))?;
        parse_sbps(&value, &mut game_data.squads)?;
    }

    // upgrade.json
    let upgrade_path = data_dir.join("upgrade.json");
    if upgrade_path.exists() {
        let text =
            std::fs::read_to_string(&upgrade_path).map_err(|e| Error::JsonImport(e.to_string()))?;
        let value: Value =
            serde_json::from_str(&text).map_err(|e| Error::JsonImport(e.to_string()))?;
        parse_upgrades(&value, &mut game_data.upgrades)?;
    }

    // locale: try locale.json first, then locale.txt
    let locale_json_path = data_dir.join("locale.json");
    let locale_txt_path = data_dir.join("locale.txt");
    if locale_json_path.exists() {
        let text = std::fs::read_to_string(&locale_json_path)
            .map_err(|e| Error::JsonImport(e.to_string()))?;
        let value: Value =
            serde_json::from_str(&text).map_err(|e| Error::JsonImport(e.to_string()))?;
        game_data.locale = parse_locale_json(&value)?;
    } else if locale_txt_path.exists() {
        let text = std::fs::read_to_string(&locale_txt_path)
            .map_err(|e| Error::JsonImport(e.to_string()))?;
        game_data.locale = parse_locale_txt(&text)?;
    }

    Ok(game_data)
}

// ---------------------------------------------------------------------------
// abilities.json
// ---------------------------------------------------------------------------

fn parse_abilities(value: &Value, out: &mut HashMap<u32, Ability>) -> Result<(), Error> {
    let autobuilds_set: HashSet<&str> = AUTOBUILDS.iter().copied().collect();
    parse_abilities_subtree(value, &["abilities"], out, &autobuilds_set)
}

fn parse_abilities_subtree(
    value: &Value,
    path: &[&str],
    out: &mut HashMap<u32, Ability>,
    autobuilds_set: &HashSet<&str>,
) -> Result<(), Error> {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return Ok(()),
    };

    if obj.contains_key("ability_bag") {
        // Leaf node
        parse_ability_bag(value, path, out, autobuilds_set)?;
    } else {
        for (k, v) in obj {
            let mut new_path: Vec<&str> = path.to_vec();
            // We need owned strings; use a temporary approach
            let k_str: &str = k.as_str();
            new_path.push(k_str);
            // Recurse — we need to pass the new path as a slice.
            // Since we need lifetime extension, collect into owned vec and call recursively.
            let owned: Vec<String> = new_path.iter().map(|s| s.to_string()).collect();
            let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
            parse_abilities_subtree(v, &refs, out, autobuilds_set)?;
        }
    }

    Ok(())
}

fn parse_ability_bag(
    data: &Value,
    path: &[&str],
    out: &mut HashMap<u32, Ability>,
    autobuilds_set: &HashSet<&str>,
) -> Result<(), Error> {
    let pbgid = parse_pbgid(data)?;

    let bag = &data["ability_bag"];
    let loc_id = parse_loc_id(bag.pointer("/ui_info/screen_name/locstring/value"));
    let icon_name = bag
        .pointer("/ui_info/icon_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let builds_raw = bag
        .pointer("/cursor_ghost_ebp/instance_reference")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let builds = if builds_raw.is_empty() {
        None
    } else {
        Some(builds_raw.to_string())
    };

    let path_str = path.join("/");
    let autobuild = path.contains(&"auto_build")
        || path.contains(&"autobuild")
        || autobuilds_set.contains(path_str.as_str());

    let ability = Ability {
        pbgid,
        path: path.iter().map(|s| s.to_string()).collect(),
        loc_id,
        icon_name,
        autobuild,
        builds,
    };

    out.insert(pbgid, ability);
    Ok(())
}

// ---------------------------------------------------------------------------
// ebps.json
// ---------------------------------------------------------------------------

fn parse_ebps(value: &Value, out: &mut HashMap<u32, Entity>) -> Result<(), Error> {
    parse_ebps_subtree(value, &["ebps"], out)
}

fn parse_ebps_subtree(
    value: &Value,
    path: &[&str],
    out: &mut HashMap<u32, Entity>,
) -> Result<(), Error> {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return Ok(()),
    };

    if obj.contains_key("extensions") {
        parse_entity_leaf(value, path, out)?;
    } else {
        for (k, v) in obj {
            let mut owned: Vec<String> = path.iter().map(|s| s.to_string()).collect();
            owned.push(k.clone());
            let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
            parse_ebps_subtree(v, &refs, out)?;
        }
    }

    Ok(())
}

fn parse_entity_leaf(
    data: &Value,
    path: &[&str],
    out: &mut HashMap<u32, Entity>,
) -> Result<(), Error> {
    let pbgid = parse_pbgid(data)?;

    let extensions = match data["extensions"].as_array() {
        Some(a) => a,
        None => return Ok(()),
    };

    let mut loc_id = 0u32;
    let mut icon_name = String::new();
    let mut spawns: Vec<String> = Vec::new();
    let mut upgrades: Vec<String> = Vec::new();

    for ext in extensions {
        let exts = match ext.get("exts") {
            Some(e) => e,
            None => continue,
        };

        if exts.get("screen_name").is_some() {
            loc_id = parse_loc_id(exts.pointer("/screen_name/locstring/value"));
            icon_name = exts
                .get("icon_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
        }

        if let Some(spawn_items) = exts.get("spawn_items").and_then(|v| v.as_array()) {
            for item in spawn_items {
                if let Some(r) = item
                    .pointer("/spawn_item/squad/instance_reference")
                    .and_then(|v| v.as_str())
                {
                    if !r.is_empty() {
                        spawns.push(r.to_string());
                    }
                }
            }
        }

        if let Some(std_upgrades) = exts.get("standard_upgrades").and_then(|v| v.as_array()) {
            for item in std_upgrades {
                if let Some(r) = item
                    .pointer("/upgrade/instance_reference")
                    .and_then(|v| v.as_str())
                {
                    if !r.is_empty() {
                        upgrades.push(r.to_string());
                    }
                }
            }
        }
    }

    let entity = Entity {
        pbgid,
        path: path.iter().map(|s| s.to_string()).collect(),
        loc_id,
        icon_name,
        spawns,
        upgrades,
    };

    out.insert(pbgid, entity);
    Ok(())
}

// ---------------------------------------------------------------------------
// sbps.json
// ---------------------------------------------------------------------------

fn parse_sbps(value: &Value, out: &mut HashMap<u32, Squad>) -> Result<(), Error> {
    parse_sbps_subtree(value, &["sbps"], out)
}

fn parse_sbps_subtree(
    value: &Value,
    path: &[&str],
    out: &mut HashMap<u32, Squad>,
) -> Result<(), Error> {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return Ok(()),
    };

    if obj.contains_key("extensions") {
        parse_squad_leaf(value, path, out)?;
    } else {
        for (k, v) in obj {
            let mut owned: Vec<String> = path.iter().map(|s| s.to_string()).collect();
            owned.push(k.clone());
            let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
            parse_sbps_subtree(v, &refs, out)?;
        }
    }

    Ok(())
}

fn parse_squad_leaf(
    data: &Value,
    path: &[&str],
    out: &mut HashMap<u32, Squad>,
) -> Result<(), Error> {
    let pbgid = parse_pbgid(data)?;

    let extensions = match data["extensions"].as_array() {
        Some(a) => a,
        None => return Ok(()),
    };

    let mut loc_id = 0u32;
    let mut icon_name = String::new();

    'outer: for ext in extensions {
        let squadexts = match ext.get("squadexts") {
            Some(e) => e,
            None => continue,
        };

        if let Some(race_list) = squadexts.get("race_list").and_then(|v| v.as_array()) {
            for race_entry in race_list {
                let screen_name = race_entry.pointer("/race_data/info/screen_name");
                if screen_name.is_some() {
                    loc_id = parse_loc_id(
                        race_entry.pointer("/race_data/info/screen_name/locstring/value"),
                    );
                    icon_name = race_entry
                        .pointer("/race_data/info/icon_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    break 'outer;
                }
            }
        }
    }

    let squad = Squad {
        pbgid,
        path: path.iter().map(|s| s.to_string()).collect(),
        loc_id,
        icon_name,
    };

    out.insert(pbgid, squad);
    Ok(())
}

// ---------------------------------------------------------------------------
// upgrade.json
// ---------------------------------------------------------------------------

fn parse_upgrades(value: &Value, out: &mut HashMap<u32, Upgrade>) -> Result<(), Error> {
    parse_upgrades_subtree(value, &["upgrade"], out)
}

fn parse_upgrades_subtree(
    value: &Value,
    path: &[&str],
    out: &mut HashMap<u32, Upgrade>,
) -> Result<(), Error> {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return Ok(()),
    };

    if obj.contains_key("upgrade_bag") {
        parse_upgrade_leaf(value, path, out)?;
    } else {
        for (k, v) in obj {
            let mut owned: Vec<String> = path.iter().map(|s| s.to_string()).collect();
            owned.push(k.clone());
            let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
            parse_upgrades_subtree(v, &refs, out)?;
        }
    }

    Ok(())
}

fn parse_upgrade_leaf(
    data: &Value,
    path: &[&str],
    out: &mut HashMap<u32, Upgrade>,
) -> Result<(), Error> {
    let pbgid = parse_pbgid(data)?;

    let bag = &data["upgrade_bag"];
    let loc_id = parse_loc_id(bag.pointer("/ui_info/screen_name/locstring/value"));
    let icon_name = bag
        .pointer("/ui_info/icon_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let upgrade = Upgrade {
        pbgid,
        path: path.iter().map(|s| s.to_string()).collect(),
        loc_id,
        icon_name,
    };

    out.insert(pbgid, upgrade);
    Ok(())
}

// ---------------------------------------------------------------------------
// locale.json
// ---------------------------------------------------------------------------

fn parse_locale_json(value: &Value) -> Result<LocaleStore, Error> {
    let obj = value
        .as_object()
        .ok_or_else(|| Error::JsonImport("locale.json root is not an object".into()))?;

    let mut map = HashMap::new();
    for (k, v) in obj {
        if let Ok(id) = k.parse::<u32>() {
            if let Some(s) = v.as_str() {
                map.insert(id, s.to_string());
            }
        }
    }

    Ok(LocaleStore(map))
}

// ---------------------------------------------------------------------------
// locale.txt
// ---------------------------------------------------------------------------

fn parse_locale_txt(text: &str) -> Result<LocaleStore, Error> {
    let valid_types: HashSet<&str> = LOCALE_TYPES.iter().copied().collect();
    let mut map = HashMap::new();

    for line in text.lines() {
        let parts: Vec<&str> = line.splitn(5, '\t').collect();
        if parts.len() < 5 {
            continue;
        }
        let entry_type = parts[0];
        if !valid_types.contains(entry_type) {
            continue;
        }
        let id_str = parts[3].trim_start_matches('$');
        if let Ok(id) = id_str.parse::<u32>() {
            let string = parts[4].to_string();
            // Last writer wins (same as Ruby which uses a hash)
            map.insert(id, string);
        }
    }

    Ok(LocaleStore(map))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_pbgid(data: &Value) -> Result<u32, Error> {
    data.get("pbgid")
        .and_then(|v| v.as_f64())
        .map(|f| f as u32)
        .ok_or_else(|| Error::JsonImport("missing or invalid pbgid".into()))
}

fn parse_loc_id(value: Option<&Value>) -> u32 {
    value
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty() && *s != "0")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn data_dir() -> PathBuf {
        PathBuf::from("/Users/ryantaylor/cohdata/data/10612")
    }

    #[test]
    fn test_parse_abilities() {
        let path = data_dir().join("abilities.json");
        let text = std::fs::read_to_string(&path).expect("abilities.json not found");
        let value: Value = serde_json::from_str(&text).expect("parse JSON");

        let mut abilities: HashMap<u32, Ability> = HashMap::new();
        parse_abilities(&value, &mut abilities).expect("parse abilities");

        assert!(
            !abilities.is_empty(),
            "should have parsed at least one ability"
        );

        // throw_grenade: pbgid=165228, loc_id=11154319, icon='common/upgrades/grenade_riflemen_us'
        let ability = abilities.get(&165228).expect("throw_grenade not found");
        assert_eq!(ability.pbgid, 165228);
        assert_eq!(ability.loc_id, 11154319);
        assert_eq!(ability.icon_name, "common/upgrades/grenade_riflemen_us");
        assert!(!ability.autobuild);
        assert!(ability.builds.is_none());

        // prioritize_vehicles: pbgid=172523, loc_id=11155111
        let pv = abilities
            .get(&172523)
            .expect("prioritize_vehicles not found");
        assert_eq!(pv.pbgid, 172523);
        assert_eq!(pv.loc_id, 11155111);
        assert!(!pv.autobuild);
    }

    #[test]
    fn test_parse_ebps() {
        let path = data_dir().join("ebps.json");
        let text = std::fs::read_to_string(&path).expect("ebps.json not found");
        let value: Value = serde_json::from_str(&text).expect("parse JSON");

        let mut entities: HashMap<u32, Entity> = HashMap::new();
        parse_ebps(&value, &mut entities).expect("parse ebps");

        assert!(
            !entities.is_empty(),
            "should have parsed at least one entity"
        );

        // hq_ak: pbgid=198235, loc_id=11153221
        // spawns includes 'sbps/races/afrika_korps/vehicles/kradschutzen_motorcycle_ak'
        // upgrades includes 'upgrade/afrika_korps/research/hq_armored_assault_tactics_ak'
        let entity = entities.get(&198235).expect("hq_ak not found");
        assert_eq!(entity.pbgid, 198235);
        assert_eq!(entity.loc_id, 11153221);
        assert!(
            entity.spawns.contains(
                &"sbps/races/afrika_korps/vehicles/kradschutzen_motorcycle_ak".to_string()
            ),
            "spawns should contain kradschutzen"
        );
        assert!(
            entity.upgrades.contains(
                &"upgrade/afrika_korps/research/hq_armored_assault_tactics_ak".to_string()
            ),
            "upgrades should contain hq_armored_assault_tactics_ak"
        );
    }

    #[test]
    fn test_parse_sbps() {
        let path = data_dir().join("sbps.json");
        let text = std::fs::read_to_string(&path).expect("sbps.json not found");
        let value: Value = serde_json::from_str(&text).expect("parse JSON");

        let mut squads: HashMap<u32, Squad> = HashMap::new();
        parse_sbps(&value, &mut squads).expect("parse sbps");

        assert!(!squads.is_empty(), "should have parsed at least one squad");

        // assault_panzergrenadier_ak: pbgid=2072107, loc_id=11220485
        let squad = squads
            .get(&2072107)
            .expect("assault_panzergrenadier_ak not found");
        assert_eq!(squad.pbgid, 2072107);
        assert_eq!(squad.loc_id, 11220485);
        assert_eq!(
            squad.icon_name,
            "races/afrika_corps/infantry/assault_panzergrenadier_ak"
        );
    }

    #[test]
    fn test_parse_upgrades() {
        let path = data_dir().join("upgrade.json");
        let text = std::fs::read_to_string(&path).expect("upgrade.json not found");
        let value: Value = serde_json::from_str(&text).expect("parse JSON");

        let mut upgrades: HashMap<u32, Upgrade> = HashMap::new();
        parse_upgrades(&value, &mut upgrades).expect("parse upgrades");

        assert!(
            !upgrades.is_empty(),
            "should have parsed at least one upgrade"
        );

        // armored_support_ak: pbgid=2075339, loc_id=11242440
        let upgrade = upgrades
            .get(&2075339)
            .expect("armored_support_ak not found");
        assert_eq!(upgrade.pbgid, 2075339);
        assert_eq!(upgrade.loc_id, 11242440);
        assert_eq!(
            upgrade.icon_name,
            "races/afrika_corps/battlegroups/armored_ak"
        );
    }

    #[test]
    fn test_parse_locale_txt() {
        let path = data_dir().join("locale.txt");
        let text = std::fs::read_to_string(&path).expect("locale.txt not found");
        let locale = parse_locale_txt(&text).expect("parse locale.txt");

        // abilities entry: id=11216622 -> 'Battleship Bombardment'
        assert_eq!(
            locale.get(11216622),
            Some("Battleship Bombardment"),
            "locale id 11216622 should be 'Battleship Bombardment'"
        );
        // another: id=11205541 -> 'Offensive / Select target position'
        assert_eq!(
            locale.get(11205541),
            Some("Offensive / Select target position"),
        );
    }

    #[test]
    fn test_import_version_round_trip() {
        let dir = data_dir();
        let game_data = import_version(&dir, 10612).expect("import_version failed");

        assert_eq!(game_data.version, 10612);
        assert!(
            !game_data.entities.is_empty(),
            "entities should not be empty"
        );
        assert!(!game_data.squads.is_empty(), "squads should not be empty");
        assert!(
            !game_data.upgrades.is_empty(),
            "upgrades should not be empty"
        );
        assert!(
            !game_data.abilities.is_empty(),
            "abilities should not be empty"
        );
        assert!(!game_data.locale.0.is_empty(), "locale should not be empty");
    }

    #[test]
    fn test_import_version_missing_dir() {
        let dir = PathBuf::from("/nonexistent/path/12345");
        let game_data = import_version(&dir, 99999).expect("should return Ok for missing dir");

        assert_eq!(game_data.version, 99999);
        assert!(game_data.entities.is_empty());
        assert!(game_data.squads.is_empty());
        assert!(game_data.upgrades.is_empty());
        assert!(game_data.abilities.is_empty());
        assert!(game_data.locale.0.is_empty());
    }
}
