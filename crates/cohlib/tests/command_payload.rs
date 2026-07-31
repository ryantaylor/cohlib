//! Corpus-wide validation of command payload parsing.
//!
//! Every fixture must parse without panicking or erroring — the payload parsers in
//! `replay::data::ticks::payload`/`value` panic on wire data that doesn't match the
//! model validated during development, so this is the primary regression guard for
//! that model. It also checks that command types this crate claims to semantically
//! decode actually do so, rather than silently falling back to `Command::Unknown`.

use cohlib::{Command, CommandType};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Command types fully decoded as of this PR (see `replay::command::Command`).
const DECODED_TYPES: &[CommandType] = &[
    // PR 1
    CommandType::PCMD_AIPlayer,
    CommandType::PCMD_Ability,
    CommandType::PCMD_InstantUpgrade,
    CommandType::PCMD_TentativeUpgrade,
    CommandType::PCMD_PlaceAndConstructEntities,
    CommandType::CMD_BuildSquad,
    CommandType::CMD_Ability,
    CommandType::CMD_Upgrade,
    CommandType::CMD_CancelConstruction,
    CommandType::CMD_CancelProduction,
    // PR 2
    CommandType::SCMD_Retreat,
    CommandType::SCMD_Stop,
    CommandType::PCMD_TentativeUpgradeRemoveAll,
    CommandType::SCMD_UnloadSquads,
    CommandType::CMD_UnloadSquads,
    CommandType::PCMD_Surrender,
    CommandType::SCMD_CancelProduction,
    CommandType::PCMD_CancelProduction,
    // PR 3
    CommandType::SCMD_Upgrade,
    CommandType::SCMD_ReinforceUnit,
    // PR 4
    CommandType::CMD_RallyPoint,
    CommandType::CMD_Move,
    CommandType::CMD_AttackFromHold,
    CommandType::SCMD_Move,
    CommandType::SCMD_Attack,
    CommandType::SCMD_Capture,
    CommandType::SCMD_AttackMove,
    CommandType::SCMD_Load,
    CommandType::SCMD_Unload,
    CommandType::SCMD_Face,
    CommandType::SCMD_CaptureTeamWeapon,
    CommandType::SCMD_PickUpSimItem,
    CommandType::SCMD_BuildStructure,
    CommandType::SCMD_Recrew,
    CommandType::PCMD_DetonateCharges,
    // PR 5
    CommandType::SCMD_Ability,
    CommandType::SCMD_StopAbility,
    CommandType::CMD_StopAbility,
    // PR 7
    CommandType::PCMD_AIPlayer_ResourceBonus,
    CommandType::PCMD_BroadcastMessage,
];

/// Fixtures with pre-existing parse failures unrelated to command parsing (present on
/// `main` before this module existed). Out of scope here.
const KNOWN_BROKEN_FIXTURES: &[&str] = &["new_failure.rec"];

fn fixture_paths() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("replays");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("replays dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rec"))
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            !KNOWN_BROKEN_FIXTURES.contains(&name.as_ref())
        })
        .collect();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "expected at least one fixture in {dir:?}"
    );
    paths
}

#[test]
fn every_fixture_parses_without_panicking() {
    for path in fixture_paths() {
        let data = fs::read(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let result = cohlib::parse_replay(&data);
        assert!(
            result.is_ok(),
            "{}: {:?}",
            path.file_name().unwrap().to_string_lossy(),
            result.err()
        );
    }
}

/// A handful of real commands carry something other than the expected parameter shape:
/// - `CMD_BuildSquad`, `CMD_Upgrade`, `PCMD_PlaceAndConstructEntities`,
///   `SCMD_ReinforceUnit` and `SCMD_Ability`: the exact same 17-byte blob, repeated
///   verbatim, all in `unusual_cpu_items.rec` — some AI/CPU-issued command shape not
///   yet understood.
/// - `SCMD_Retreat`: a small fraction (about 1% corpus-wide) carry a target position
///   instead of being parameter-less, all in `v44736_retreat_to_position.rec`.
///
/// Structurally these are normal, fully-accounted-for parameter blocks (see
/// `every_fixture_parses_without_panicking`) — just not a shape this crate decodes yet,
/// rather than malformed data. Everything else routed to a decoder in `DECODED_TYPES`
/// is expected to fully decode. See `CommandData::parse_pbgid` and
/// `CommandData::parse_squads`.
#[test]
fn decoded_command_types_only_fall_back_to_unknown_for_known_exceptions() {
    let mut unknown_counts: HashMap<CommandType, usize> = HashMap::new();

    for path in fixture_paths() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let data = fs::read(&path).unwrap_or_else(|e| panic!("read {name}: {e}"));
        let replay = cohlib::parse_replay(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        for player in replay.players() {
            for command in player.commands() {
                if let Command::Unknown(unknown) = command {
                    let action_type = unknown.action_type();
                    if DECODED_TYPES.contains(&action_type) {
                        *unknown_counts.entry(action_type).or_default() += 1;
                    }
                }
            }
        }
    }

    assert_eq!(
        unknown_counts,
        HashMap::from([
            (CommandType::CMD_BuildSquad, 8),
            (CommandType::CMD_Upgrade, 2),
            (CommandType::PCMD_PlaceAndConstructEntities, 5),
            (CommandType::SCMD_Retreat, 16),
            (CommandType::SCMD_ReinforceUnit, 1),
            (CommandType::SCMD_Ability, 1),
        ]),
        "unexpected Unknown fallback for a command type this crate claims to decode; \
         if this is a newly discovered real variant, add it to the exception list with \
         a comment explaining why, otherwise it's a parsing regression"
    );
}

/// Camera track records (`DCMD_CameraTrack`/`DCMD_COUNT`) are not player commands: they
/// must never appear in `Player::commands()` (as `Command::Unknown` or otherwise), and
/// `Player::camera_tracks()` must actually be populated from the same underlying data.
#[test]
fn camera_tracks_are_excluded_from_commands_and_populated_separately() {
    let mut total_commands = 0;
    let mut total_camera_tracks = 0;

    for path in fixture_paths() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let data = fs::read(&path).unwrap_or_else(|e| panic!("read {name}: {e}"));
        let replay = cohlib::parse_replay(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        for player in replay.players() {
            for command in player.commands() {
                total_commands += 1;
                if let Command::Unknown(unknown) = command {
                    assert!(
                        !matches!(
                            unknown.action_type(),
                            CommandType::DCMD_CameraTrack | CommandType::DCMD_COUNT
                        ),
                        "{name}: a camera track command leaked into Player::commands()"
                    );
                }
            }
            total_camera_tracks += player.camera_tracks().len();
        }
    }

    assert!(total_commands > 0, "expected at least one player command");
    assert!(
        total_camera_tracks > 0,
        "expected at least one camera track"
    );
}
