//! Extracts rich multiplayer scenario (map) metadata from `ScenariosMP.sga`:
//! dimensions, resource point placement/tiers/income, sector boundaries, and
//! playable area.
//!
//! Formats were reverse-engineered directly from game files pulled from the
//! local Steam depot (see each module's doc comment for details). The
//! `.layer`/`.scenario` entity-scan approach in `layers.rs` and the sector
//! boundary tracing in `territory.rs`/`geometry.rs` are informed by
//! [cohstats/coh3-data](https://github.com/cohstats/coh3-data)'s
//! `scripts/mp-maps/` — the only other public CoH3 scenario parser, and the
//! source of that format documentation; see the credit in each module.

mod chunky;
mod error;
mod geometry;
mod info;
mod layers;
mod playable;
mod territory;

pub use error::Error;

use std::collections::BTreeMap;

use data::{GameData, PointKind, PointTier, Scenario, ScenarioPoint};
use sga::ArchiveEntry;

/// Extracts a [`Scenario`] record for every multiplayer scenario found in
/// `entries` (the contents of `ScenariosMP.sga`), keyed the same way
/// `data::normalize_scenario` would key a replay's `map_filename()` — the
/// archive path with the `.info` extension stripped, forward slashes.
///
/// `game_data` supplies resource income (via [`GameData::entities`], joined by
/// ebp path) for the points found in each scenario's `.info` file. Pass the
/// `GameData` from the same import pass (i.e. from `ReferenceAttributes.sga`
/// extracted at the same time as this `ScenariosMP.sga`).
pub fn extract_scenarios(
    entries: &[ArchiveEntry],
    game_data: &GameData,
) -> BTreeMap<String, Scenario> {
    // Every file belonging to a scenario — the top-level `<name>.info` /
    // `<name>_*.override` files and the nested `<name>/<name>/*.layer` files —
    // shares the same containing folder, so group by that rather than by any
    // single scenario file's own name (sibling override/layer files don't
    // share the `.info`'s exact stem, only its parent directory).
    let mut scenarios = BTreeMap::new();
    for entry in entries {
        let Some(dir) = map_dir(&entry.path) else {
            continue;
        };
        let Some(prefix) = parent_folder(&dir) else {
            continue;
        };
        let files: Vec<&ArchiveEntry> = entries
            .iter()
            .filter(|e| e.path.starts_with(prefix.as_str()))
            .collect();
        let Some(scenario) = extract_one(&dir, &files, game_data) else {
            continue;
        };
        scenarios.insert(dir, scenario);
    }
    scenarios
}

/// Returns the scenario's key (archive path, no extension) if `path` is a
/// scenario's `.info` file, e.g.
/// `scenarios/multiplayer/community/castello_8p/castello_8p.info` →
/// `Some("scenarios/multiplayer/community/castello_8p/castello_8p")`.
fn map_dir(path: &str) -> Option<String> {
    let stem = path.strip_suffix(".info")?;
    Some(stem.to_string())
}

/// Returns `dir`'s containing folder (with trailing `/`), used to find every
/// other file belonging to the same scenario.
fn parent_folder(dir: &str) -> Option<String> {
    let idx = dir.rfind('/')?;
    Some(dir[..=idx].to_string())
}

fn extract_one(dir: &str, files: &[&ArchiveEntry], game_data: &GameData) -> Option<Scenario> {
    let info_entry = files.iter().find(|e| e.path == format!("{dir}.info"))?;
    let info = info::parse_info(&info_entry.bytes)?;

    // Not every scenario file carries territory/playable-area overrides (seen on
    // a handful of secondary variants — video-preview loops, defend-mode
    // objective sub-maps); degrade gracefully to empty sectors / no playable
    // area rather than dropping the whole scenario.
    let territory_entry = files
        .iter()
        .find(|e| e.path == format!("{dir}_territory.override"));
    let territory = territory_entry.and_then(|e| territory::parse_territory(&e.bytes).ok());

    let playable_entry = files
        .iter()
        .find(|e| e.path == format!("{dir}_softmapedge.override"));
    let playable_area = playable_entry
        .and_then(|e| playable::parse_playable_area(&e.bytes).ok())
        .and_then(|mask| mask.bounding_rect());

    // Reconcile .info's point_positions (authoritative for shape/tier, since it
    // reflects the scenario author's intent, but can go stale after edits — see
    // layers.rs) against the actual placed entities in the map's .layer/.scenario
    // files, which reflect what's really on the ground.
    let layer_files: Vec<&ArchiveEntry> = files
        .iter()
        .filter(|e| e.path.starts_with(&format!("{dir}/")) && e.path.ends_with(".layer"))
        .copied()
        .collect();
    // Scanning every .layer file (not just the resource-point-specific one,
    // whose name isn't standardized across community maps) also picks up
    // thousands of decorative placements (bushes, splines, vehicles, ...).
    // Only entities whose ebp actually names a territory/start point are
    // candidates for matching — otherwise a nearby decorative prop could
    // spuriously steal a real point's match within MATCH_RADIUS.
    let placed: Vec<layers::PlacedEntity> = layers::scan_entities(&layer_files)
        .into_iter()
        .filter(|p| classify_ebp(&p.ebp).0 != PointKind::Other)
        .collect();

    let mut points: Vec<ScenarioPoint> = info
        .points
        .iter()
        .map(|p| reconcile_point(p, &placed, game_data))
        .collect();

    if let Some(territory) = &territory {
        territory::assign_sectors(&mut points, territory);
    }

    let mut scenario = Scenario {
        size: info.size,
        playable_area,
        max_players: info.max_players,
        teams: info.teams,
        author: info.author,
        name_loc_id: info.name_loc_id,
        description_loc_id: info.description_loc_id,
        scenario_type: info.scenario_type,
        map_origin: info.map_origin,
        visible_in_lobby: info.visible_in_lobby,
        points,
        sectors: territory.map(|t| t.sectors).unwrap_or_default(),
    };
    territory::link_sector_points(&mut scenario);
    Some(scenario)
}

/// Reconciles one `.info` point against the placed-entity scan: prefers the
/// placed entity's position/ebp when a match is found nearby (it reflects
/// in-editor edits the `.info` snapshot may not), falling back to the `.info`
/// value otherwise. Resolves income/capture from `game_data` by ebp path.
fn reconcile_point(
    info_point: &info::InfoPoint,
    placed: &[layers::PlacedEntity],
    game_data: &GameData,
) -> ScenarioPoint {
    let resolved = layers::nearest_match(info_point.x, info_point.y, placed)
        .map(|p| (p.ebp.clone(), p.x, p.y))
        .unwrap_or_else(|| (info_point.ebp.clone(), info_point.x, info_point.y));
    let (ebp, x, y) = resolved;

    let (kind, tier) = classify_ebp(&ebp);
    let owner = matches!(kind, PointKind::Start).then_some(info_point.owner);

    let entity = game_data
        .entities
        .values()
        .find(|e| e.path.last().map(|s| s.as_str()) == Some(strip_shape_suffix(&ebp)));
    let income_per_minute = entity
        .and_then(|e| e.resource)
        .map(|r| r.per_second * 60.0)
        .unwrap_or(0.0);
    let capture_time = entity.and_then(|e| e.capture).map(|c| c.capture_time);

    ScenarioPoint {
        ebp,
        x,
        y,
        kind,
        tier,
        owner,
        income_per_minute,
        capture_time,
        sector: None,
    }
}

/// Strips known shape-variant suffixes from an ebp name so it can be matched
/// against the base ebp's resource/capture data, e.g.
/// `territory_fuel_point_low_smaller` → `territory_fuel_point_low`.
fn strip_shape_suffix(ebp: &str) -> &str {
    const SUFFIXES: &[&str] = &[
        "_smaller",
        "_larger",
        "_rect15x20",
        "_square7x7",
        "_square10x10",
        "_extra_low",
        "_extra_medium",
        "_5m",
        "_coh2",
        "_command",
    ];
    for suffix in SUFFIXES {
        if let Some(stripped) = ebp.strip_suffix(suffix) {
            return stripped;
        }
    }
    ebp
}

/// Classifies an ebp name into a [`PointKind`]/[`PointTier`], from the name
/// alone (shape suffixes stripped first).
fn classify_ebp(ebp: &str) -> (PointKind, Option<PointTier>) {
    if ebp == "starting_position" {
        return (PointKind::Start, None);
    }
    let base = strip_shape_suffix(ebp);
    let kind = if base.contains("fuel") {
        PointKind::Fuel
    } else if base.contains("munitions") {
        PointKind::Munitions
    } else if base.contains("strategic") {
        PointKind::Manpower
    } else if base.contains("victory") {
        PointKind::Victory
    } else {
        return (PointKind::Other, None);
    };
    // Check the `_extra_*` variants before their plain counterparts, since
    // `_extra_low` also ends with `_low`.
    let tier = if base.ends_with("_extra_low") {
        Some(PointTier::ExtraLow)
    } else if base.ends_with("_extra_medium") {
        Some(PointTier::ExtraMedium)
    } else if base.ends_with("_low") {
        Some(PointTier::Low)
    } else if base.ends_with("_medium") {
        Some(PointTier::Medium)
    } else if base.ends_with("_high") {
        Some(PointTier::High)
    } else {
        None
    };
    (kind, tier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_fuel_low() {
        assert_eq!(
            classify_ebp("territory_fuel_point_low"),
            (PointKind::Fuel, Some(PointTier::Low))
        );
    }

    #[test]
    fn classify_fuel_low_smaller_strips_suffix() {
        assert_eq!(
            classify_ebp("territory_fuel_point_low_smaller"),
            (PointKind::Fuel, Some(PointTier::Low))
        );
    }

    #[test]
    fn classify_munitions_high() {
        assert_eq!(
            classify_ebp("territory_munitions_point_high"),
            (PointKind::Munitions, Some(PointTier::High))
        );
    }

    #[test]
    fn classify_strategic_is_manpower() {
        assert_eq!(
            classify_ebp("territory_strategic_point_smaller"),
            (PointKind::Manpower, None)
        );
    }

    #[test]
    fn classify_victory() {
        assert_eq!(
            classify_ebp("territory_victory_point_larger"),
            (PointKind::Victory, None)
        );
    }

    #[test]
    fn classify_starting_position() {
        assert_eq!(classify_ebp("starting_position"), (PointKind::Start, None));
    }

    #[test]
    fn map_dir_strips_info_extension() {
        assert_eq!(
            map_dir("scenarios/multiplayer/community/castello_8p/castello_8p.info"),
            Some("scenarios/multiplayer/community/castello_8p/castello_8p".to_string())
        );
    }

    #[test]
    fn map_dir_ignores_non_info_files() {
        assert_eq!(
            map_dir("scenarios/multiplayer/community/castello_8p/castello_8p.scenario"),
            None
        );
    }

    #[test]
    fn parent_folder_of_flat_scenario() {
        assert_eq!(
            parent_folder("scenarios/multiplayer/community/castello_8p/castello_8p"),
            Some("scenarios/multiplayer/community/castello_8p/".to_string())
        );
    }

    #[test]
    fn parent_folder_scopes_sibling_override_files_but_not_other_maps() {
        // Regression: grouping by the .info's own file stem (rather than its
        // parent folder) would never find sibling files like
        // `<name>_territory.override`, which don't share the .info's stem.
        let prefix =
            parent_folder("scenarios/multiplayer/community/castello_8p/castello_8p").unwrap();
        assert!(
            "scenarios/multiplayer/community/castello_8p/castello_8p_territory.override"
                .starts_with(&prefix)
        );
        assert!(
            "scenarios/multiplayer/community/castello_8p/castello_8p/actionmarker.layer"
                .starts_with(&prefix)
        );
        assert!(
            !"scenarios/multiplayer/community/gothic_line_8p/gothic_line_8p.info"
                .starts_with(&prefix)
        );
    }
}
