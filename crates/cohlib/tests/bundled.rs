//! Integration tests for VersionedStore::bundled().
//!
//! Verifies that the compiled-in data bundle loads all historical versions
//! and that entity counts for known versions are correct.

use cohlib::{PointKind, VersionedStore};

#[test]
fn bundled_loads_all_versions() {
    let store = VersionedStore::bundled();
    // data/ currently contains 32 version directories.
    assert!(
        store.version_count() >= 30,
        "expected at least 30 bundled versions, got {}",
        store.version_count()
    );
}

#[test]
fn bundled_v10612_entity_lookup() {
    let store = VersionedStore::bundled();

    // barracks_us pbgid=169963 exists in v10612
    let entity = store.get_entity(169963, 10612);
    assert!(entity.is_some(), "barracks_us (169963) not found at v10612");
    let entity = entity.unwrap();
    assert_eq!(entity.pbgid, 169963);
    assert!(!entity.spawns.is_empty(), "barracks_us should have spawns");
}

#[test]
fn bundled_version_fallback() {
    let store = VersionedStore::bundled();

    // Query at a build number between two known versions; should fall back
    // to the nearest older version.
    let at_exact = store.get_entity(169963, 10612);
    let at_between = store.get_entity(169963, 10700); // between 10612 and 10907
    assert_eq!(
        at_exact.map(|e| e.pbgid),
        at_between.map(|e| e.pbgid),
        "fallback should resolve to same entity as exact version"
    );
}

#[test]
fn bundled_locale_resolution() {
    let store = VersionedStore::bundled();

    // loc_id 11156544 is "Construct Barracks" in English
    let name = store.localize(11156544, 10612);
    assert!(
        name.is_some(),
        "locale id 11156544 should be present in bundled data"
    );
}

#[test]
fn bundled_squad_lookup() {
    let store = VersionedStore::bundled();

    // riflemen_us pbgid=159619
    let squad = store.get_squad(159619, 10612);
    assert!(squad.is_some(), "riflemen_us (159619) not found at v10612");
}

#[test]
fn bundled_upgrade_lookup() {
    let store = VersionedStore::bundled();

    // A known upgrade pbgid present in v10612 data
    let upgrade = store.get_upgrade(170560, 10612);
    assert!(upgrade.is_some(), "upgrade 170560 not found at v10612");
}

// ---------------------------------------------------------------------------
// local_name_for_formatted — simple (direct loc_id) path
// ---------------------------------------------------------------------------

#[test]
fn local_name_for_formatted_simple_entity() {
    let store = VersionedStore::bundled();

    // barracks_us pbgid=169963, loc_id=11153231 → "Barracks"
    let name = store.local_name_for_formatted(169963, 10612);
    assert!(
        name.is_some(),
        "barracks_us (169963) should have a formatted name at v10612"
    );
    assert_eq!(name.as_deref(), Some("Barracks"));
}

#[test]
fn local_name_for_formatted_simple_squad() {
    let store = VersionedStore::bundled();

    // riflemen_us pbgid=159619, loc_id=11241668 → "Riflemen Squad"
    let name = store.local_name_for_formatted(159619, 10612);
    assert!(
        name.is_some(),
        "riflemen_us (159619) should have a formatted name at v10612"
    );
    assert_eq!(name.as_deref(), Some("Riflemen Squad"));
}

#[test]
fn local_name_for_formatted_returns_none_for_unknown_pbgid() {
    let store = VersionedStore::bundled();

    assert_eq!(
        store.local_name_for_formatted(0xDEADBEEF, 10612),
        None,
        "unknown pbgid should return None"
    );
}

#[test]
fn local_name_for_formatted_matches_local_name_for_for_plain_entities() {
    let store = VersionedStore::bundled();

    // For entities with a plain loc_id, both APIs should agree.
    let plain = store.local_name_for(169963, 10612).map(|s| s.to_owned());
    let formatted = store.local_name_for_formatted(169963, 10612);
    assert_eq!(plain, formatted);
}

// ---------------------------------------------------------------------------
// get_scenario / get_map_size — golden test against castello_8p
//
// castello_8p (build 46121) is a real community 8-player map, extracted from
// the actual game depot. Expected values below are read directly off its
// published CoH3Stats page (coh3stats.com/explorer/maps/castello_8p) for
// everything that page and this crate derive from the same authored source
// (point counts, tiers, income, sector/base counts) — verified independently
// here, not copied from their number. Playable area is a deliberate
// exception: CoH3Stats estimates it from the point bounding box (368x423);
// this crate reads the map's authored soft-edge mask instead, which is
// larger because it isn't clipped to the outermost capture points — see
// Scenario::playable_area's doc comment.
// ---------------------------------------------------------------------------

const CASTELLO_8P: &str = "data:scenarios\\multiplayer\\community\\castello_8p\\castello_8p";
const CASTELLO_BUILD: u32 = 46121;

#[test]
fn get_map_size_matches_get_scenario_size() {
    let store = VersionedStore::bundled();
    let size = store
        .get_map_size(CASTELLO_8P, CASTELLO_BUILD)
        .expect("castello_8p map size");
    let scenario = store
        .get_scenario(CASTELLO_8P, CASTELLO_BUILD)
        .expect("castello_8p scenario");
    assert_eq!(*size, scenario.size);
    assert_eq!((size.width, size.height), (544.0, 608.0));
}

#[test]
fn castello_8p_metadata() {
    let store = VersionedStore::bundled();
    let s = store
        .get_scenario(CASTELLO_8P, CASTELLO_BUILD)
        .expect("castello_8p scenario");

    assert_eq!(s.author, "TheSphinx");
    assert_eq!(s.max_players, 8);
    assert_eq!(s.teams, [4, 4]);
    assert_eq!(s.map_origin, 2, "castello_8p is a community (Workshop) map");
}

#[test]
fn castello_8p_point_counts_and_income_match_published_values() {
    let store = VersionedStore::bundled();
    let s = store
        .get_scenario(CASTELLO_8P, CASTELLO_BUILD)
        .expect("castello_8p scenario");

    let count = |kind: PointKind| s.points.iter().filter(|p| p.kind == kind).count();
    let income = |kind: PointKind| -> f32 {
        s.points
            .iter()
            .filter(|p| p.kind == kind)
            .map(|p| p.income_per_minute)
            .sum()
    };

    assert_eq!(count(PointKind::Fuel), 8);
    assert_eq!(count(PointKind::Munitions), 8);
    assert_eq!(count(PointKind::Manpower), 4);
    assert_eq!(count(PointKind::Victory), 3);
    assert_eq!(count(PointKind::Start), 8);
    // 20 capturable resource points + 3 victory points = 23 capturable, matching
    // the published page's "Capturable points: 23".
    assert_eq!(
        count(PointKind::Fuel)
            + count(PointKind::Munitions)
            + count(PointKind::Manpower)
            + count(PointKind::Victory),
        23
    );

    // Published: fuel 40, munitions 82.1, manpower 32 per minute.
    assert!((income(PointKind::Fuel) - 40.0).abs() < 0.1);
    assert!((income(PointKind::Munitions) - 82.1).abs() < 0.1);
    assert!((income(PointKind::Manpower) - 32.0).abs() < 0.1);
}

#[test]
fn castello_8p_sectors() {
    let store = VersionedStore::bundled();
    let s = store
        .get_scenario(CASTELLO_8P, CASTELLO_BUILD)
        .expect("castello_8p scenario");

    assert_eq!(s.sectors.len(), 24);
    assert_eq!(s.sectors.iter().filter(|sec| sec.is_base).count(), 4);

    // Every point should have resolved to a sector.
    assert!(
        s.points.iter().all(|p| p.sector.is_some()),
        "every point should fall within a sector"
    );

    // Every sector should have a traced, closed outline sharing exact
    // boundary coordinates with its neighbors (see geometry.rs).
    for sector in &s.sectors {
        assert!(
            !sector.rings.is_empty(),
            "sector {} should have a traced outline",
            sector.id
        );
        for ring in &sector.rings {
            assert!(
                ring.len() >= 4,
                "ring should have at least 4 points (a closed loop)"
            );
            assert_eq!(ring.first(), ring.last(), "ring should be closed");
        }
    }
}

#[test]
fn castello_8p_playable_area_is_smaller_than_full_size_but_larger_than_point_bbox_estimate() {
    let store = VersionedStore::bundled();
    let s = store
        .get_scenario(CASTELLO_8P, CASTELLO_BUILD)
        .expect("castello_8p scenario");
    let playable = s.playable_area.expect("castello_8p playable area");

    assert!(playable.width() < s.size.width);
    assert!(playable.height() < s.size.height);
    // CoH3Stats' point-bbox estimate is 368x423; the authored mask is larger
    // because playable ground extends past the outermost capture points.
    assert!(playable.width() > 368.0);
    assert!(playable.height() > 423.0);
}
