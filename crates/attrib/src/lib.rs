//! Parser for CoH3 attribute XML files from ReferenceAttributes.sga.
//!
//! Each file's archive path encodes the entity type and path:
//! - `instances\sbps\races\american\infantry\riflemen_us.xml` → path `["sbps","races","american","infantry","riflemen_us"]`
//!
//! XML structure:
//! ```xml
//! <instance template="abilities">
//!   <variant name="default">
//!     <group name="ability_bag">
//!       <uniqueid name="pbgid" value="174094" />
//!       <group name="ui_info">
//!         <locstring name="screen_name" value="11156544" />
//!         <file name="icon_name" value="races\american\..." />
//!       </group>
//!       <instance_reference name="cursor_ghost_ebp" value="ebps\..." />
//!     </group>
//!   </variant>
//! </instance>
//! ```

mod error;
pub use error::Error;

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use data::{Ability, Entity, GameData, LocaleStore, Squad, Upgrade, Version};
use sga::ArchiveEntry;

/// Raw parsed entity from a single attribute XML file.
#[derive(Debug, Clone)]
pub struct RawEntity {
    /// Hierarchical path derived from the archive file path, without "instances" prefix.
    /// E.g. `["abilities", "races", "american", "auto_build", "auto_build_barracks"]`
    pub path: Vec<String>,
    /// Template name from the `<instance>` element: "abilities", "sbps", "ebps", etc.
    pub template: String,
    /// The pbgid of this entity.
    pub pbgid: u32,
    /// Named attribute values collected from the XML (name → value string).
    pub fields: HashMap<String, String>,
    /// Squad paths this entity can spawn (from spawn_items list). Ebps only.
    pub spawns: Vec<String>,
    /// Upgrade paths this entity supports (from standard_upgrades list). Ebps only.
    pub upgrades: Vec<String>,
    /// Parsed `screen_name_formatter` block, if present and non-empty.
    pub screen_name_formatter: Option<data::ScreenNameFormatter>,
}

/// Parse a single entity XML file, deriving path from `file_path`.
///
/// `file_path` is the archive-relative path, e.g.
/// `"instances/abilities/races/american/auto_build/auto_build_barracks.xml"`.
pub fn parse_entity_xml(bytes: &[u8], file_path: &str) -> Result<RawEntity, Error> {
    let path = derive_path(file_path);
    let (template, pbgid, fields, spawns, upgrades, screen_name_formatter) = parse_xml(bytes)?;
    Ok(RawEntity {
        path,
        template,
        pbgid,
        fields,
        spawns,
        upgrades,
        screen_name_formatter,
    })
}

/// Extract a `GameData` from a set of SGA archive entries (from ReferenceAttributes.sga).
///
/// Processes all `.xml` files under `instances/` and classifies them by template type
/// (abilities, ebps, sbps, upgrade). Entries that fail to parse or lack a pbgid are skipped.
pub fn extract_game_data(
    entries: &[ArchiveEntry],
    locale: LocaleStore,
    version: Version,
    on_entry: impl Fn(),
) -> Result<GameData, Error> {
    let mut game_data = GameData::new(version);
    game_data.locale = locale;

    let raws: Vec<RawEntity> = entries
        .iter()
        .filter(|entry| entry.path.starts_with("instances/") && entry.extension() == Some("xml"))
        .filter_map(|entry| {
            on_entry();
            parse_entity_xml(&entry.bytes, &entry.path).ok()
        })
        .collect();

    let bg_activators = raws.iter().filter(|raw| raw.template == "tech_tree").fold(
        HashMap::new(),
        |mut acc, raw| {
            if let Some(activation_upgrade) = raw.fields.get("activation_upgrade") {
                if let Some(loc_id) = raw.fields.get("name") {
                    acc.insert(normalize_sep(activation_upgrade), loc_id.clone());
                }
            }

            acc
        },
    );

    // Build a lookup from normalized path → icon_name for all parsed entities that have an icon.
    // Used to inherit icons from parent_pbg when an entity has no icon of its own.
    let parent_icons: HashMap<String, String> = raws
        .iter()
        .filter_map(|raw| {
            let icon = raw.fields.get("icon_name")?;
            if icon.is_empty() {
                return None;
            }
            Some((raw.path.join("/"), normalize_sep(icon)))
        })
        .collect();

    for raw in raws {
        on_entry();

        let path_str = raw.path.join("/");

        match raw.template.as_str() {
            "abilities" => {
                let builds = raw
                    .fields
                    .get("cursor_ghost_ebp")
                    .filter(|s| !s.is_empty())
                    .map(|s| normalize_sep(s));
                let autobuild = raw
                    .path
                    .iter()
                    .any(|s| s == "auto_build" || s == "autobuild");
                let loc_id = parse_loc_str(raw.fields.get("screen_name"));
                let icon_name = resolve_icon(&raw.fields, &parent_icons);
                let spawns: Vec<String> = raw.spawns.iter().map(|s| normalize_sep(s)).collect();
                let upgrades: Vec<String> = raw.upgrades.iter().map(|s| normalize_sep(s)).collect();
                game_data.abilities.insert(
                    raw.pbgid,
                    Ability {
                        pbgid: raw.pbgid,
                        path: raw.path,
                        loc_id,
                        icon_name,
                        autobuild,
                        builds,
                        spawns,
                        upgrades,
                        screen_name_formatter: raw.screen_name_formatter,
                    },
                );
            }
            "ebps" => {
                let loc_id = parse_loc_str(raw.fields.get("screen_name"));
                let icon_name = resolve_icon(&raw.fields, &parent_icons);
                let spawns: Vec<String> = raw.spawns.iter().map(|s| normalize_sep(s)).collect();
                let upgrades: Vec<String> = raw.upgrades.iter().map(|s| normalize_sep(s)).collect();
                game_data.entities.insert(
                    raw.pbgid,
                    Entity {
                        pbgid: raw.pbgid,
                        path: raw.path,
                        loc_id,
                        icon_name,
                        spawns,
                        upgrades,
                    },
                );
            }
            "sbps" => {
                let loc_id = parse_loc_str(raw.fields.get("screen_name"));
                let icon_name = resolve_icon(&raw.fields, &parent_icons);
                game_data.squads.insert(
                    raw.pbgid,
                    Squad {
                        pbgid: raw.pbgid,
                        path: raw.path,
                        loc_id,
                        icon_name,
                    },
                );
            }
            "upgrade" => {
                let mut loc_id = parse_loc_str(raw.fields.get("screen_name"));
                if loc_id == 0 {
                    // If the upgrade itself doesn't have a locstring associated
                    // with it and it happens to activate a battlegroup, set the
                    // locstring to that of the battlegroup
                    loc_id = parse_loc_str(bg_activators.get(&path_str));
                }
                let icon_name = resolve_icon(&raw.fields, &parent_icons);
                game_data.upgrades.insert(
                    raw.pbgid,
                    Upgrade {
                        pbgid: raw.pbgid,
                        path: raw.path,
                        loc_id,
                        icon_name,
                        screen_name_formatter: raw.screen_name_formatter,
                    },
                );
            }
            _ => {}
        }
    }

    Ok(game_data)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Normalizes Windows backslashes to forward slashes.
fn normalize_sep(s: &str) -> String {
    s.replace('\\', "/")
}

/// Resolves an entity's icon name: uses its own `icon_name` field if non-empty,
/// otherwise falls back to the icon of the entity referenced by `parent_pbg`.
fn resolve_icon(fields: &HashMap<String, String>, parent_icons: &HashMap<String, String>) -> String {
    if let Some(icon) = fields.get("icon_name").filter(|s| !s.is_empty()) {
        return normalize_sep(icon);
    }
    fields
        .get("parent_pbg")
        .map(|p| normalize_sep(p))
        .and_then(|p| parent_icons.get(&p).cloned())
        .unwrap_or_default()
}

/// Parses a loc_id from an optional string value (may have "$" prefix or be "0").
fn parse_loc_str(s: Option<&String>) -> u32 {
    s.map(|v| v.trim_start_matches('$'))
        .filter(|s| !s.is_empty() && *s != "0")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0)
}

/// Derives the entity path from the archive file path.
///
/// Strips the leading `instances/` segment and removes the `.xml` extension from the last segment.
/// Normalizes backslashes to forward slashes.
fn derive_path(file_path: &str) -> Vec<String> {
    let normalized = file_path.replace('\\', "/");
    let without_ext = normalized
        .strip_suffix(".xml")
        .unwrap_or(normalized.as_str());

    let segments: Vec<String> = without_ext
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    // Strip leading "instances" if present
    if segments.first().map(|s| s == "instances").unwrap_or(false) {
        segments[1..].to_vec()
    } else {
        segments
    }
}

/// Context labels pushed onto the stack when entering significant XML elements.
#[derive(Debug, PartialEq)]
enum Ctx {
    SpawnItems,
    StandardUpgrades,
    AbilityExtensions,
    ActionList,
    RaceList,
    RaceData,
    TechTreeBag,
    Requirement,
    /// Inside `<template_reference name="screen_name_formatter" value="ui_text_formatter">`.
    ScreenNameFormatter,
    /// Inside `<list name="formatter_arguments">` within a `ScreenNameFormatter`.
    FormatterArguments,
    Other,
}

/// Parse XML bytes, extracting template, pbgid, named leaf values, spawns, upgrades,
/// and an optional screen_name_formatter.
type ParseXmlResult = (
    String,
    u32,
    HashMap<String, String>,
    Vec<String>,
    Vec<String>,
    Option<data::ScreenNameFormatter>,
);

/// Uses a context stack to track list/group nesting and collect multi-valued fields.
fn parse_xml(bytes: &[u8]) -> Result<ParseXmlResult, Error> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);

    let mut template = String::new();
    let mut pbgid: Option<u32> = None;
    let mut fields: HashMap<String, String> = HashMap::new();
    let mut spawns: Vec<String> = Vec::new();
    let mut upgrades: Vec<String> = Vec::new();
    let mut formatter_template_id: u32 = 0;
    let mut formatter_arg_ids: Vec<u32> = Vec::new();

    // Context stack for tracking nested elements
    let mut ctx_stack: Vec<Ctx> = Vec::new();
    // Whether we've found all the sbps ui fields (from the first race_data group)
    let mut race_data_ui_found = false;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();

                match tag.as_str() {
                    "instance" => {
                        if let Some(attr) = get_attr(e, b"template") {
                            template = attr;
                        }
                        ctx_stack.push(Ctx::Other);
                    }
                    "list" => {
                        let name = get_attr(e, b"name").unwrap_or_default();
                        match name.as_str() {
                            "spawn_items" => ctx_stack.push(Ctx::SpawnItems),
                            "standard_upgrades" => ctx_stack.push(Ctx::StandardUpgrades),
                            "race_list" => ctx_stack.push(Ctx::RaceList),
                            "requirement_list" | "requirements" => ctx_stack.push(Ctx::Requirement),
                            "formatter_arguments"
                                if ctx_stack.last() == Some(&Ctx::ScreenNameFormatter) =>
                            {
                                ctx_stack.push(Ctx::FormatterArguments)
                            }
                            n if n.contains("action") || n.contains("custom_pbgid") => {
                                ctx_stack.push(Ctx::ActionList)
                            }
                            _ => ctx_stack.push(Ctx::Other),
                        }
                    }
                    "group" => {
                        let name = get_attr(e, b"name").unwrap_or_default();
                        if name == "techtree_bag" {
                            ctx_stack.push(Ctx::TechTreeBag);
                        } else if name == "race_data" && ctx_stack.last() == Some(&Ctx::RaceList) {
                            ctx_stack.push(Ctx::RaceData);
                        } else if name.contains("requirement") {
                            ctx_stack.push(Ctx::Requirement);
                        } else if name.contains("action") || name.contains("custom_property") {
                            ctx_stack.push(Ctx::ActionList);
                        } else {
                            ctx_stack.push(Ctx::Other);
                        }
                    }
                    "template_reference" => {
                        let name = get_attr(e, b"name").unwrap_or_default();
                        let value = get_attr(e, b"value").unwrap_or_default();
                        if name == "screen_name_formatter" && !value.is_empty() {
                            ctx_stack.push(Ctx::ScreenNameFormatter);
                        } else if value.contains("requirement") {
                            ctx_stack.push(Ctx::Requirement);
                        } else if (name == "ability_extensions" || name == "exts")
                            && value.contains("squad_spawner_ext")
                        {
                            ctx_stack.push(Ctx::AbilityExtensions);
                        } else if name.contains("action")
                            || value.contains("action")
                            || name.contains("custom_property")
                            || value.contains("custom_property")
                        {
                            ctx_stack.push(Ctx::ActionList);
                        } else {
                            ctx_stack.push(Ctx::Other);
                        }
                    }
                    "instance_reference" => {
                        if let (Some(name), Some(value)) =
                            (get_attr(e, b"name"), get_attr(e, b"value"))
                        {
                            let ancestor_ctx =
                                ctx_stack.iter().rev().find(|c| !matches!(c, Ctx::Other));

                            let normalized_val = value.replace('\\', "/");
                            let is_spawn = normalized_val.contains("sbps/")
                                || normalized_val.contains("/spawns/")
                                || normalized_val.contains("call_in_ability");
                            let is_upgrade = normalized_val.contains("upgrade/");

                            match ancestor_ctx {
                                Some(Ctx::Requirement) => {}
                                Some(Ctx::SpawnItems)
                                | Some(Ctx::AbilityExtensions)
                                | Some(Ctx::ActionList) => {
                                    if is_spawn {
                                        spawns.push(value.clone());
                                    } else if is_upgrade {
                                        upgrades.push(value.clone());
                                    }
                                }
                                _ => {
                                    if is_spawn {
                                        spawns.push(value.clone());
                                    } else if is_upgrade && name != "parent_pbg" {
                                        upgrades.push(value.clone());
                                    }

                                    if !name.is_empty() {
                                        fields.entry(name).or_insert(value);
                                    }
                                }
                            }
                        }
                        ctx_stack.push(Ctx::Other);
                    }
                    _ => {
                        ctx_stack.push(Ctx::Other);
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();

                let current_ctx = ctx_stack.last();

                match tag.as_str() {
                    "uniqueid" => {
                        if let (Some(name), Some(value)) =
                            (get_attr(e, b"name"), get_attr(e, b"value"))
                        {
                            if name == "pbgid" && pbgid.is_none() {
                                if let Ok(v) = value.parse::<f64>() {
                                    pbgid = Some(v as u32);
                                }
                            }
                        }
                    }
                    "locstring" => {
                        if let (Some(name), Some(value)) =
                            (get_attr(e, b"name"), get_attr(e, b"value"))
                        {
                            if name == "screen_name" {
                                // For sbps: capture screen_name from inside race_data (authoritative).
                                // race_data must overwrite any earlier value from other ext blocks
                                // (e.g. squad_reinforce_ext also has a screen_name).
                                // For non-sbps: capture the first screen_name encountered.
                                // Check ancestors because screen_name is nested inside
                                // race_data > info > locstring, so the top of the stack is Other.
                                let in_race_data =
                                    ctx_stack.iter().rev().any(|c| c == &Ctx::RaceData);
                                if in_race_data && !race_data_ui_found {
                                    fields.insert("screen_name".to_string(), value);
                                } else if !in_race_data {
                                    fields.entry("screen_name".to_string()).or_insert(value);
                                }
                            } else if name == "name" && current_ctx == Some(&Ctx::TechTreeBag) {
                                // battlegroup attributes use this naming convention, but
                                // we have to make sure we're in the techtree_bag context
                                // and not somewhere else like upgrades.
                                fields.entry("name".to_string()).or_insert(value);
                            } else if name == "formatter"
                                && current_ctx == Some(&Ctx::ScreenNameFormatter)
                            {
                                if let Ok(id) = value.trim_start_matches('$').parse::<u32>() {
                                    formatter_template_id = id;
                                }
                            } else if name == "locstring_value"
                                && current_ctx == Some(&Ctx::FormatterArguments)
                            {
                                if let Ok(id) = value.trim_start_matches('$').parse::<u32>() {
                                    if id != 0 {
                                        formatter_arg_ids.push(id);
                                    }
                                }
                            }
                        }
                    }
                    "file" => {
                        if let (Some(name), Some(value)) =
                            (get_attr(e, b"name"), get_attr(e, b"value"))
                        {
                            if name == "icon_name" {
                                let in_race_data =
                                    ctx_stack.iter().rev().any(|c| c == &Ctx::RaceData);
                                if in_race_data && !race_data_ui_found {
                                    fields.insert("icon_name".to_string(), value);
                                } else if !in_race_data {
                                    fields.entry("icon_name".to_string()).or_insert(value);
                                }
                            }
                        }
                    }
                    "instance_reference" => {
                        if let (Some(name), Some(value)) =
                            (get_attr(e, b"name"), get_attr(e, b"value"))
                        {
                            let ancestor_ctx =
                                ctx_stack.iter().rev().find(|c| !matches!(c, Ctx::Other));

                            let normalized_val = value.replace('\\', "/");
                            let is_spawn = normalized_val.contains("sbps/")
                                || normalized_val.contains("spawns/")
                                || normalized_val.contains("call_in_ability");
                            let is_upgrade = normalized_val.contains("upgrade/");

                            match ancestor_ctx {
                                Some(Ctx::Requirement) => {}
                                Some(Ctx::SpawnItems)
                                | Some(Ctx::AbilityExtensions)
                                | Some(Ctx::ActionList) => {
                                    if is_spawn {
                                        spawns.push(value.clone());
                                    } else if is_upgrade {
                                        upgrades.push(value.clone());
                                    }
                                }
                                _ => {
                                    if is_spawn {
                                        spawns.push(value.clone());
                                    } else if is_upgrade && name != "parent_pbg" {
                                        upgrades.push(value.clone());
                                    }

                                    if !name.is_empty() {
                                        fields.entry(name).or_insert(value);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(_)) => {
                // If leaving a race_data group, mark ui found if we captured any ui fields.
                if ctx_stack.last() == Some(&Ctx::RaceData)
                    && (fields.contains_key("screen_name") || fields.contains_key("icon_name"))
                {
                    race_data_ui_found = true;
                }
                ctx_stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(Error::Attrib(format!("XML parse error: {e}")));
            }
            _ => {}
        }
        buf.clear();
    }

    let pbgid = pbgid.ok_or_else(|| Error::Attrib("missing pbgid".into()))?;

    let screen_name_formatter = if formatter_template_id != 0 {
        Some(data::ScreenNameFormatter {
            template_loc_id: formatter_template_id,
            arg_loc_ids: formatter_arg_ids,
        })
    } else {
        None
    };

    Ok((
        template,
        pbgid,
        fields,
        spawns,
        upgrades,
        screen_name_formatter,
    ))
}

/// Extract an attribute value by name from an XML element.
fn get_attr(e: &quick_xml::events::BytesStart<'_>, name: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if a.key.as_ref() == name {
            a.unescape_value().ok().map(|v| v.into_owned())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABILITY_XML: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<instance version="5" template="abilities">
  <variant name="default">
    <group name="ability_bag">
      <uniqueid name="pbgid" value="174094" />
      <group name="ui_info">
        <locstring name="screen_name" value="11156544" />
        <file name="icon_name" value="races\american\buildings\barracks_us" />
      </group>
      <instance_reference name="cursor_ghost_ebp" value="ebps\races\american\buildings\production\barracks_us" />
    </group>
  </variant>
</instance>"#;

    const EBPS_XML: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<instance version="5" template="ebps">
  <variant name="default">
    <list name="extensions">
      <template_reference name="exts" value="ebpextensions\spawn_ext">
        <list name="spawn_items">
          <group name="spawn_item">
            <instance_reference name="squad" value="sbps\races\american\infantry\riflemen_us" />
          </group>
          <group name="spawn_item">
            <instance_reference name="squad" value="sbps\races\american\team_weapons\mortar_81mm_us" />
          </group>
        </list>
        <file name="icon_name" value="races\american\buildings\barracks_us" />
        <locstring name="screen_name" value="11153231" />
      </template_reference>
      <template_reference name="exts" value="ebpextensions\upgrade_ext">
        <list name="standard_upgrades">
          <instance_reference name="upgrade" value="upgrade\american\research\barracks\bar_riflemen_global_us" />
          <instance_reference name="upgrade" value="upgrade\american\research\barracks\grenade_riflemen_us" />
        </list>
      </template_reference>
    </list>
    <uniqueid name="pbgid" value="169963" />
  </variant>
</instance>"#;

    const SBPS_XML: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<instance version="5" template="sbps">
  <variant name="default">
    <list name="extensions">
      <template_reference name="exts" value="sbpextensions\squad_ui_ext">
        <list name="race_list">
          <group name="race_data">
            <group name="info">
              <file name="icon_name" value="races\american\infantry\riflemen_us" />
              <locstring name="screen_name" value="11241668" />
            </group>
          </group>
        </list>
      </template_reference>
    </list>
    <uniqueid name="pbgid" value="159619" />
  </variant>
</instance>"#;

    /// Reproduces the bug where squad_reinforce_ext's screen_name (e.g. "Reinforce") appears
    /// before squad_ui_ext's race_data screen_name in the XML and was incorrectly captured first.
    const SBPS_REINFORCE_EXT_BEFORE_UI_EXT_XML: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<instance version="5" template="sbps">
  <variant name="default">
    <list name="extensions">
      <template_reference name="squadexts" value="sbpextensions\squad_reinforce_ext">
        <group name="ui_info">
          <locstring name="screen_name" value="11153295" />
        </group>
      </template_reference>
      <template_reference name="squadexts" value="sbpextensions\squad_ui_ext">
        <list name="race_list">
          <group name="race_data">
            <group name="info">
              <file name="icon_name" value="races\german\infantry\panzer_grenadier_ger" />
              <locstring name="screen_name" value="11166831" />
            </group>
          </group>
        </list>
      </template_reference>
    </list>
    <uniqueid name="pbgid" value="188642" />
  </variant>
</instance>"#;

    #[test]
    fn parse_ability_xml_pbgid() {
        let entity = parse_entity_xml(
            ABILITY_XML,
            "instances/abilities/races/american/auto_build/auto_build_barracks.xml",
        )
        .unwrap();
        assert_eq!(entity.pbgid, 174094);
    }

    #[test]
    fn parse_ability_xml_template() {
        let entity = parse_entity_xml(
            ABILITY_XML,
            "instances/abilities/races/american/auto_build/auto_build_barracks.xml",
        )
        .unwrap();
        assert_eq!(entity.template, "abilities");
    }

    #[test]
    fn parse_ability_xml_locstring() {
        let entity = parse_entity_xml(
            ABILITY_XML,
            "instances/abilities/races/american/auto_build/auto_build_barracks.xml",
        )
        .unwrap();
        assert_eq!(
            entity.fields.get("screen_name").map(|s| s.as_str()),
            Some("11156544")
        );
    }

    #[test]
    fn parse_ability_xml_icon_name() {
        let entity = parse_entity_xml(
            ABILITY_XML,
            "instances/abilities/races/american/auto_build/auto_build_barracks.xml",
        )
        .unwrap();
        assert_eq!(
            entity.fields.get("icon_name").map(|s| s.as_str()),
            Some(r"races\american\buildings\barracks_us")
        );
    }

    #[test]
    fn parse_ability_xml_cursor_ghost_ebp() {
        let entity = parse_entity_xml(
            ABILITY_XML,
            "instances/abilities/races/american/auto_build/auto_build_barracks.xml",
        )
        .unwrap();
        assert_eq!(
            entity.fields.get("cursor_ghost_ebp").map(|s| s.as_str()),
            Some(r"ebps\races\american\buildings\production\barracks_us")
        );
    }

    #[test]
    fn parse_ebps_xml_spawns() {
        let entity = parse_entity_xml(
            EBPS_XML,
            "instances/ebps/races/american/buildings/barracks_us.xml",
        )
        .unwrap();
        assert_eq!(entity.pbgid, 169963);
        assert_eq!(entity.spawns.len(), 2);
        assert!(entity
            .spawns
            .contains(&r"sbps\races\american\infantry\riflemen_us".to_string()));
    }

    #[test]
    fn parse_ebps_xml_upgrades() {
        let entity = parse_entity_xml(
            EBPS_XML,
            "instances/ebps/races/american/buildings/barracks_us.xml",
        )
        .unwrap();
        assert_eq!(entity.upgrades.len(), 2);
        assert!(entity
            .upgrades
            .contains(&r"upgrade\american\research\barracks\bar_riflemen_global_us".to_string()));
    }

    #[test]
    fn parse_sbps_xml_screen_name() {
        let entity = parse_entity_xml(
            SBPS_XML,
            "instances/sbps/races/american/infantry/riflemen_us.xml",
        )
        .unwrap();
        assert_eq!(entity.pbgid, 159619);
        assert_eq!(
            entity.fields.get("screen_name").map(|s| s.as_str()),
            Some("11241668")
        );
    }

    /// Regression: squad_reinforce_ext has a screen_name ("Reinforce") that appears before
    /// squad_ui_ext's race_data screen_name in the XML. The race_data value must win.
    #[test]
    fn parse_sbps_xml_race_data_screen_name_beats_reinforce_ext() {
        let entity = parse_entity_xml(
            SBPS_REINFORCE_EXT_BEFORE_UI_EXT_XML,
            "instances/sbps/races/german/infantry/panzergrenadier_ger.xml",
        )
        .unwrap();
        assert_eq!(entity.pbgid, 188642);
        assert_eq!(
            entity.fields.get("screen_name").map(|s| s.as_str()),
            Some("11166831"),
            "race_data screen_name should override the reinforce_ext screen_name"
        );
    }

    #[test]
    fn derive_path_strips_instances_and_ext() {
        let path =
            derive_path("instances/abilities/races/american/auto_build/auto_build_barracks.xml");
        assert_eq!(
            path,
            vec![
                "abilities",
                "races",
                "american",
                "auto_build",
                "auto_build_barracks"
            ]
        );
    }

    #[test]
    fn derive_path_normalizes_backslashes() {
        let path = derive_path(r"instances\sbps\races\american\infantry\riflemen_us.xml");
        assert_eq!(
            path,
            vec!["sbps", "races", "american", "infantry", "riflemen_us"]
        );
    }

    #[test]
    fn derive_path_without_instances_prefix() {
        let path = derive_path("abilities/throw_grenade.xml");
        assert_eq!(path, vec!["abilities", "throw_grenade"]);
    }

    #[test]
    fn missing_pbgid_returns_error() {
        let xml =
            br#"<instance template="abilities"><variant name="default"></variant></instance>"#;
        let result = parse_entity_xml(xml, "instances/abilities/test.xml");
        assert!(result.is_err());
    }

    #[test]
    fn extract_game_data_abilities() {
        let entry = ArchiveEntry {
            path: "instances/abilities/races/american/auto_build/auto_build_barracks.xml"
                .to_string(),
            bytes: ABILITY_XML.to_vec(),
        };
        let gd = extract_game_data(&[entry], LocaleStore(Default::default()), 99, || {}).unwrap();
        assert_eq!(gd.abilities.len(), 1);
        let ab = gd.abilities.get(&174094).unwrap();
        assert!(ab.autobuild);
        assert_eq!(
            ab.builds.as_deref(),
            Some("ebps/races/american/buildings/production/barracks_us")
        );
    }

    #[test]
    fn extract_game_data_ebps() {
        let entry = ArchiveEntry {
            path: "instances/ebps/races/american/buildings/barracks_us.xml".to_string(),
            bytes: EBPS_XML.to_vec(),
        };
        let gd = extract_game_data(&[entry], LocaleStore(Default::default()), 99, || {}).unwrap();
        assert_eq!(gd.entities.len(), 1);
        let e = gd.entities.get(&169963).unwrap();
        assert_eq!(e.spawns.len(), 2);
        assert_eq!(e.upgrades.len(), 2);
        assert!(e
            .spawns
            .contains(&"sbps/races/american/infantry/riflemen_us".to_string()));
    }

    // ---------------------------------------------------------------------------
    // screen_name_formatter tests
    // ---------------------------------------------------------------------------

    const UPGRADE_FORMATTER_XML: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<instance version="5" template="upgrade">
  <variant name="default">
    <group name="upgrade_bag">
      <uniqueid name="pbgid" value="2160077" />
      <group name="ui_info">
        <locstring name="screen_name" value="0" />
        <template_reference name="screen_name_formatter" value="ui_text_formatter">
          <list name="formatter_arguments">
            <locstring name="locstring_value" value="11241678" />
          </list>
          <locstring name="formatter" value="11261319" />
        </template_reference>
      </group>
    </group>
  </variant>
</instance>"#;

    #[test]
    fn parse_upgrade_xml_with_formatter() {
        let entity = parse_entity_xml(
            UPGRADE_FORMATTER_XML,
            "instances/upgrade/american/battlegroups/armored/armored_right_3_sherman_easy_8_production_unlock_us.xml",
        )
        .unwrap();
        assert_eq!(entity.pbgid, 2160077);
        let fmt = entity.screen_name_formatter.unwrap();
        assert_eq!(fmt.template_loc_id, 11261319);
        assert_eq!(fmt.arg_loc_ids, vec![11241678]);
    }

    #[test]
    fn parse_upgrade_xml_without_formatter_has_none() {
        // ABILITY_XML has no screen_name_formatter block.
        let entity = parse_entity_xml(
            ABILITY_XML,
            "instances/abilities/races/american/auto_build/auto_build_barracks.xml",
        )
        .unwrap();
        assert!(entity.screen_name_formatter.is_none());
    }

    /// Ability with no icon_name but a parent_pbg that has an icon.
    /// Mirrors strafing_run_p47_us.xml → strafing_run_generic.xml.
    #[test]
    fn extract_game_data_ability_inherits_parent_icon() {
        const PARENT_XML: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<instance version="5" template="abilities">
  <variant name="default">
    <group name="ability_bag">
      <uniqueid name="pbgid" value="10001" />
      <group name="ui_info">
        <locstring name="screen_name" value="11000001" />
        <file name="icon_name" value="common\abilities\strafing_run_p47_us" />
      </group>
    </group>
  </variant>
</instance>"#;

        const CHILD_XML: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<instance version="5" template="abilities">
  <variant name="default">
    <group name="ability_bag">
      <uniqueid name="pbgid" value="10002" />
      <instance_reference name="parent_pbg" value="abilities\races\common\archetypes\flight\strafing_run_generic" />
      <group name="ui_info">
        <locstring name="screen_name" value="11000002" />
      </group>
    </group>
  </variant>
</instance>"#;

        let entries = vec![
            ArchiveEntry {
                path: "instances/abilities/races/common/archetypes/flight/strafing_run_generic.xml"
                    .to_string(),
                bytes: PARENT_XML.to_vec(),
            },
            ArchiveEntry {
                path: "instances/abilities/races/american/aircraft/strafing_run_p47_us.xml"
                    .to_string(),
                bytes: CHILD_XML.to_vec(),
            },
        ];

        let gd = extract_game_data(&entries, LocaleStore(Default::default()), 99, || {}).unwrap();
        let child = gd.abilities.get(&10002).unwrap();
        assert_eq!(
            child.icon_name,
            "common/abilities/strafing_run_p47_us",
            "child should inherit parent's icon when it has none"
        );
        let parent = gd.abilities.get(&10001).unwrap();
        assert_eq!(
            parent.icon_name,
            "common/abilities/strafing_run_p47_us",
            "parent's own icon should be unchanged"
        );
    }

    #[test]
    fn extract_game_data_upgrade_with_formatter() {
        let entry = ArchiveEntry {
            path: "instances/upgrade/american/battlegroups/armored/armored_right_3_sherman_easy_8_production_unlock_us.xml".to_string(),
            bytes: UPGRADE_FORMATTER_XML.to_vec(),
        };
        let gd = extract_game_data(&[entry], LocaleStore(Default::default()), 99, || {}).unwrap();
        let upgrade = gd.upgrades.get(&2160077).unwrap();
        let fmt = upgrade.screen_name_formatter.as_ref().unwrap();
        assert_eq!(fmt.template_loc_id, 11261319);
        assert_eq!(fmt.arg_loc_ids, vec![11241678]);
    }
}
