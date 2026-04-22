//! Build order and suspect parity tests against Reinforce.
//!
//! These tests correspond directly to the RSpec examples in
//! reinforce/spec/reinforce_spec.rb and verify that cohlib produces
//! identical build paths and suspect markings for all three USF fixtures.
//!
//! Path resolution mirrors Reinforce's `command.details.path`:
//!   - Ability-based actions (ConstructBuilding, UseAbility-sourced TrainUnit/ResearchUpgrade,
//!     UseBattlegroupAbility) → ability path via `get_ability`
//!   - Squad-sourced TrainUnit (BuildSquad) → squad path via `get_squad`
//!   - Upgrade-sourced ResearchUpgrade (BuildGlobalUpgrade) and SelectBattlegroup /
//!     SelectBattlegroupAbility → upgrade path via `get_upgrade`
//!
//! The lookup tries ability → squad → upgrade in order; pbgids are unique across
//! all three tables so the first hit is always correct.

use cohlib::{extract_build_order, BuildActionKind, BuildAction, VersionedStore};
use data::Version;

fn store() -> VersionedStore {
    VersionedStore::bundled()
}

/// Resolve the game-data path for a build action.
///
/// Returns an empty string for `AITakeover` (pbgid 0, no data entry).
fn action_path(action: &BuildAction, version: Version, store: &VersionedStore) -> String {
    store
        .get_ability(action.pbgid, version)
        .map(|a| a.path.join("/"))
        .or_else(|| store.get_squad(action.pbgid, version).map(|s| s.path.join("/")))
        .or_else(|| store.get_upgrade(action.pbgid, version).map(|u| u.path.join("/")))
        .unwrap_or_default()
}

fn run(bytes: &[u8]) -> (Vec<String>, Vec<String>) {
    let store = store();
    let replay = cohlib::parse_replay(bytes).expect("parse_replay failed");
    let version = replay.version() as Version;
    let bo = extract_build_order(&replay, 0, &store, false).expect("extract_build_order failed");

    let paths: Vec<String> = bo
        .actions
        .iter()
        .filter(|a| a.kind != BuildActionKind::AITakeover)
        .map(|a| action_path(a, version, &store))
        .collect();

    let mut suspects: Vec<String> = bo
        .actions
        .iter()
        .filter(|a| a.suspect_since.is_some())
        .map(|a| action_path(a, version, &store))
        .collect();
    suspects.sort();

    (paths, suspects)
}

// ── USF Airborne build ────────────────────────────────────────────────────────

#[test]
fn usf_airborne_build_generates_correct_build() {
    let bytes = include_bytes!("../replays/usf_airborne_build.rec");
    let (paths, _) = run(bytes);
    let expected = vec![
        "sbps/races/american/infantry/engineer_us",
        "abilities/races/american/auto_build/auto_build_barracks",
        "abilities/races/american/auto_build/auto_build_barracks",
        "abilities/races/american/auto_build/auto_build_weapon_support_center",
        "upgrade/american/research/infantry_support_center_us",
        "upgrade/american/battlegroups/airborne/airborne",
        "upgrade/american/battlegroups/airborne/airborne_right_1a_pathfinders_us",
        "upgrade/american/battlegroups/airborne/airborne_right_2_paratrooper_us",
        "upgrade/american/battlegroups/airborne/airborne_right_3_paradrop_at_gun_us",
        "abilities/races/american/battlegroups/airborne/airborne_right_2_paratrooper_us",
        "abilities/races/american/battlegroups/airborne/airborne_right_3_paradrop_at_gun_us",
        "upgrade/american/battlegroups/airborne/airborne_left_1b_recon_loiter_us",
        "upgrade/american/battlegroups/airborne/airborne_left_2a_supply_drop_us",
        "upgrade/american/battlegroups/airborne/airborne_left_3b_carpet_bombing_run_us",
        "abilities/races/american/battlegroups/airborne/airborne_left_1b_recon_loiter_us",
        "upgrade/american/research/infantry_support_center/field_support_us",
        "abilities/races/american/battlegroups/airborne/airborne_left_2b_supply_drop_us",
        "abilities/races/american/battlegroups/airborne/airborne_left_3b_carpet_bombing_us",
        "abilities/races/american/auto_build/auto_build_triage_center_us",
        "abilities/races/american/auto_build/auto_build_triage_center_us",
    ];
    assert_eq!(
        paths, expected,
        "build path mismatch\ngot:      {paths:#?}\nexpected: {expected:#?}",
    );
}

#[test]
fn usf_airborne_build_marks_correct_suspects() {
    let bytes = include_bytes!("../replays/usf_airborne_build.rec");
    let (_, suspects) = run(bytes);
    let mut expected = vec![
        "abilities/races/american/auto_build/auto_build_barracks",
        "abilities/races/american/auto_build/auto_build_weapon_support_center",
        "abilities/races/american/auto_build/auto_build_triage_center_us",
    ];
    expected.sort();
    assert_eq!(
        suspects, expected,
        "suspect mismatch\ngot:      {suspects:#?}\nexpected: {expected:#?}",
    );
}

// ── USF Armoured build ────────────────────────────────────────────────────────

#[test]
fn usf_armoured_build_generates_correct_build() {
    let bytes = include_bytes!("../replays/usf_armoured_build.rec");
    let (paths, _) = run(bytes);
    let expected = vec![
        "abilities/races/american/auto_build/auto_build_barracks",
        "abilities/races/american/auto_build/auto_build_weapon_support_center",
        "upgrade/american/research/air_support_center_us",
        "upgrade/american/research/weapon_support_center/super_bazookas_us",
        "abilities/races/american/auto_build/auto_build_triage_center_us",
        "abilities/races/american/auto_build/auto_build_triage_center_us",
        "upgrade/american/battlegroups/armored/armored",
        "upgrade/american/battlegroups/armored/armored_left_1a_assault_engineers_us",
        "upgrade/american/battlegroups/armored/armored_left_2b_recovery_vehicle_us",
        "upgrade/american/battlegroups/armored/armored_left_3_war_machine_us",
        "abilities/races/american/battlegroups/armored/armored_left_2b_recovery_vehicle_us",
        "abilities/races/american/auto_build/auto_build_tank_depot",
        "upgrade/american/battlegroups/armored/armored_right_1a_fast_deploy_us",
        "upgrade/american/battlegroups/armored/armored_right_2a_scott_us",
        "upgrade/american/battlegroups/armored/armored_right_3_sherman_easy_8_us",
        "abilities/races/american/battlegroups/armored/armored_right_2a_scott_us",
        "abilities/races/american/battlegroups/armored/armored_right_3_easy_8_task_force_us",
        "sbps/races/american/infantry/assault_engineer_us",
        "upgrade/american/research/air_support_center/advanced_air_recon_us",
        "upgrade/american/research/air_support_center/air_supply_us",
        "upgrade/american/research/air_support_center/double_sortie_us",
    ];
    assert_eq!(
        paths, expected,
        "build path mismatch\ngot:      {paths:#?}\nexpected: {expected:#?}",
    );
}

#[test]
fn usf_armoured_build_marks_correct_suspects() {
    let bytes = include_bytes!("../replays/usf_armoured_build.rec");
    let (_, suspects) = run(bytes);
    let mut expected = vec![
        "abilities/races/american/auto_build/auto_build_barracks",
        "abilities/races/american/auto_build/auto_build_tank_depot",
        "abilities/races/american/auto_build/auto_build_triage_center_us",
        "abilities/races/american/auto_build/auto_build_triage_center_us",
    ];
    expected.sort();
    assert_eq!(
        suspects, expected,
        "suspect mismatch\ngot:      {suspects:#?}\nexpected: {expected:#?}",
    );
}

// ── USF Advanced Infantry build ───────────────────────────────────────────────

#[test]
fn usf_advanced_inf_build_generates_correct_build() {
    let bytes = include_bytes!("../replays/usf_advanced_inf_build.rec");
    let (paths, _) = run(bytes);
    let expected = vec![
        "abilities/races/american/auto_build/auto_build_barracks",
        "upgrade/american/battlegroups/infantry/infantry",
        "upgrade/american/battlegroups/infantry/infantry_left_1_convert_rifleman_to_ranger_us",
        "abilities/races/american/auto_build/auto_build_weapon_support_center",
        "sbps/races/american/infantry/ranger_us",
        "sbps/races/american/infantry/riflemen_us",
        "upgrade/american/battlegroups/infantry/infantry_left_2a_frontline_medical_tent_us",
        "abilities/races/american/battlegroups/infantry/infantry_left_1_rifleman_convert_to_ranger_us",
        "abilities/races/american/battlegroups/infantry/infantry_left_2a_medical_tent",
        "sbps/races/american/infantry/engineer_us",
        "upgrade/american/battlegroups/infantry/infantry_left_3b_infantry_assault_us",
        "abilities/races/american/battlegroups/infantry/infantry_left_3b_infantry_assault_us",
        "upgrade/american/battlegroups/infantry/infantry_right_1a_artillery_observers_us",
        "upgrade/american/battlegroups/infantry/infantry_right_2_howitzer_105mm_us",
        "upgrade/american/battlegroups/infantry/infantry_right_3a_off_map_artillery_us",
        "abilities/races/american/battlegroups/infantry/infantry_right_3a_off_map_artillery_us",
        "abilities/races/american/auto_build/auto_build_triage_center_us",
    ];
    assert_eq!(
        paths, expected,
        "build path mismatch\ngot:      {paths:#?}\nexpected: {expected:#?}",
    );
}

#[test]
fn usf_advanced_inf_build_marks_correct_suspects() {
    let bytes = include_bytes!("../replays/usf_advanced_inf_build.rec");
    let (_, suspects) = run(bytes);
    let mut expected = vec![
        "abilities/races/american/auto_build/auto_build_weapon_support_center",
        "abilities/races/american/battlegroups/infantry/infantry_left_2a_medical_tent",
        "abilities/races/american/auto_build/auto_build_triage_center_us",
    ];
    expected.sort();
    assert_eq!(
        suspects, expected,
        "suspect mismatch\ngot:      {suspects:#?}\nexpected: {expected:#?}",
    );
}
