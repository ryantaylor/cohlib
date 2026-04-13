//! End-to-end build order integration test.
//!
//! Parses `USvDAK_v10612.rec`, extracts build orders for both players using
//! bundled game data, and compares the result tick-by-tick against known
//! output from reinforce — the Ruby reference implementation.

use cohlib::{extract_build_order, BuildActionKind, VersionedStore};

fn store() -> VersionedStore {
    VersionedStore::bundled()
}

/// (tick, index, kind, pbgid, suspect)
type ActionRow = (u32, u32, BuildActionKind, u32, bool);

fn run(player_index: usize) -> Vec<ActionRow> {
    let data = include_bytes!("../replays/USvDAK_v10612.rec");
    let replay = cohlib::parse_replay(data).expect("parse_replay failed");
    let store = store();
    let bo =
        extract_build_order(&replay, player_index, &store).expect("extract_build_order failed");
    bo.actions
        .iter()
        .map(|a| (a.tick, a.index, a.kind.clone(), a.pbgid, a.suspect))
        .collect()
}

#[test]
fn player0_madhax_matches_reinforce() {
    let got = run(0);

    // Known-good output from reinforce for player 0 (madhax, Americans, v10612).
    // AITakeover has no index/pbgid in reinforce; cohlib uses 0 for both.
    let expected: &[ActionRow] = &[
        (28, 1, BuildActionKind::TrainUnit, 198340, false),
        (322, 8, BuildActionKind::TrainUnit, 198341, false),
        (746, 15, BuildActionKind::TrainUnit, 198340, false),
        (1321, 50, BuildActionKind::ConstructBuilding, 198425, false),
        (1524, 57, BuildActionKind::TrainUnit, 2072237, false),
        (
            2309,
            102,
            BuildActionKind::SelectBattlegroup,
            2072430,
            false,
        ),
        (2646, 124, BuildActionKind::ConstructBuilding, 198427, false),
        (3049, 180, BuildActionKind::TrainUnit, 226760, false),
        (
            3061,
            181,
            BuildActionKind::SelectBattlegroupAbility,
            2072407,
            false,
        ),
        (
            3187,
            194,
            BuildActionKind::UseBattlegroupAbility,
            2072379,
            false,
        ),
        (4210, 326, BuildActionKind::ResearchUpgrade, 2072102, false),
        (4481, 348, BuildActionKind::TrainUnit, 2033664, false),
        (5244, 470, BuildActionKind::ResearchUpgrade, 2084221, false),
        (5851, 542, BuildActionKind::TrainUnit, 198340, false),
        (
            6914,
            673,
            BuildActionKind::SelectBattlegroupAbility,
            2082028,
            false,
        ),
        (
            6992,
            674,
            BuildActionKind::SelectBattlegroupAbility,
            2082034,
            false,
        ),
        (7036, 676, BuildActionKind::TrainUnit, 2033664, false),
        (7857, 760, BuildActionKind::ResearchUpgrade, 226774, false),
        (8841, 839, BuildActionKind::TrainUnit, 198357, false),
        (
            10080,
            1033,
            BuildActionKind::SelectBattlegroupAbility,
            2082035,
            false,
        ),
        (
            10643,
            1079,
            BuildActionKind::SelectBattlegroupAbility,
            2082030,
            false,
        ),
        (
            11164,
            1142,
            BuildActionKind::ResearchUpgrade,
            2084250,
            false,
        ),
        (11837, 1225, BuildActionKind::TrainUnit, 2072237, false),
        (
            11899,
            1229,
            BuildActionKind::SelectBattlegroupAbility,
            2082031,
            false,
        ),
        (12419, 0, BuildActionKind::AITakeover, 0, false),
    ];

    assert_eq!(
        got.len(),
        expected.len(),
        "action count mismatch: got {}, expected {}",
        got.len(),
        expected.len()
    );

    for (i, (got_row, exp_row)) in got.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            got_row, exp_row,
            "action[{i}] mismatch:\n  got      {:?}\n  expected {:?}",
            got_row, exp_row
        );
    }
}

#[test]
fn player1_quixalotl_matches_reinforce() {
    let got = run(1);

    // Known-good output from reinforce for player 1 (Quixalotl, AfrikaKorps, v10612).
    let expected: &[ActionRow] = &[
        (110, 4, BuildActionKind::SelectBattlegroup, 196934, false),
        (
            115,
            5,
            BuildActionKind::SelectBattlegroupAbility,
            196935,
            false,
        ),
        (144, 8, BuildActionKind::TrainUnit, 2064019, false),
        (151, 9, BuildActionKind::TrainUnit, 2064019, false),
        (535, 17, BuildActionKind::ConstructBuilding, 174231, false),
        (917, 30, BuildActionKind::TrainUnit, 170004, false),
        (1683, 84, BuildActionKind::ResearchUpgrade, 170560, false),
        (1878, 90, BuildActionKind::TrainUnit, 167433, false),
        (2316, 116, BuildActionKind::TrainUnit, 1535223, false),
        (2897, 161, BuildActionKind::ConstructBuilding, 177883, false),
        (3652, 224, BuildActionKind::TrainUnit, 169994, false),
        (4412, 270, BuildActionKind::TrainUnit, 169994, false),
        (4430, 271, BuildActionKind::TrainUnit, 167433, false),
        (4952, 348, BuildActionKind::TrainUnit, 170154, false),
        (
            5144,
            362,
            BuildActionKind::SelectBattlegroupAbility,
            201145,
            false,
        ),
        (
            5193,
            363,
            BuildActionKind::SelectBattlegroupAbility,
            187595,
            false,
        ),
        (6480, 475, BuildActionKind::ConstructBuilding, 174236, false),
        (
            7475,
            539,
            BuildActionKind::SelectBattlegroupAbility,
            196941,
            false,
        ),
        (7489, 541, BuildActionKind::TrainUnit, 137306, false),
        (7558, 543, BuildActionKind::TrainUnit, 169994, false),
        (
            8429,
            609,
            BuildActionKind::SelectBattlegroupAbility,
            187596,
            false,
        ),
        (8762, 658, BuildActionKind::TrainUnit, 168624, false),
        (9180, 681, BuildActionKind::TrainUnit, 137306, false),
        (9254, 683, BuildActionKind::TrainUnit, 2064019, false),
        (10422, 790, BuildActionKind::TrainUnit, 137306, false),
        (10925, 856, BuildActionKind::TrainUnit, 2064019, false),
        (
            11462,
            898,
            BuildActionKind::SelectBattlegroupAbility,
            196942,
            false,
        ),
        (11920, 929, BuildActionKind::TrainUnit, 169994, false),
        (12240, 943, BuildActionKind::TrainUnit, 137306, false),
    ];

    assert_eq!(
        got.len(),
        expected.len(),
        "action count mismatch: got {}, expected {}",
        got.len(),
        expected.len()
    );

    for (i, (got_row, exp_row)) in got.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            got_row, exp_row,
            "action[{i}] mismatch:\n  got      {:?}\n  expected {:?}",
            got_row, exp_row
        );
    }
}
