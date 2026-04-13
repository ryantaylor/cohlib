//! Integration tests for VersionedStore::bundled().
//!
//! Verifies that the compiled-in data bundle loads all historical versions
//! and that entity counts for known versions are correct.

use cohlib::VersionedStore;

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
