//! Corpus-wide validation of command payload parsing.
//!
//! Every fixture must parse without panicking or erroring — the payload parsers in
//! `replay::data::ticks::payload`/`value` panic on wire data that doesn't match the
//! model validated during development, so this is the primary regression guard for
//! that model. It also checks that command types this crate claims to semantically
//! decode always do so, rather than silently falling back to `Command::Unknown`.

use cohlib::{Command, CommandType};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Command types this crate semantically decodes (see `replay::command::Command`).
const DECODED_TYPES: &[CommandType] = &[
    CommandType::PCMD_AIPlayer,
    CommandType::PCMD_AIPlayer_ResourceBonus,
    CommandType::PCMD_Ability,
    CommandType::PCMD_BroadcastMessage,
    CommandType::PCMD_CancelProduction,
    CommandType::PCMD_DetonateCharges,
    CommandType::PCMD_InstantUpgrade,
    CommandType::PCMD_PlaceAndConstructEntities,
    CommandType::PCMD_Surrender,
    CommandType::PCMD_TentativeUpgrade,
    CommandType::PCMD_TentativeUpgradeRemoveAll,
    CommandType::CMD_Ability,
    CommandType::CMD_AttackFromHold,
    CommandType::CMD_BuildSquad,
    CommandType::CMD_CancelConstruction,
    CommandType::CMD_CancelProduction,
    CommandType::CMD_Move,
    CommandType::CMD_RallyPoint,
    CommandType::CMD_StopAbility,
    CommandType::CMD_UnloadSquads,
    CommandType::CMD_Upgrade,
    CommandType::SCMD_Ability,
    CommandType::SCMD_Attack,
    CommandType::SCMD_AttackMove,
    CommandType::SCMD_BuildStructure,
    CommandType::SCMD_CancelProduction,
    CommandType::SCMD_Capture,
    CommandType::SCMD_CaptureTeamWeapon,
    CommandType::SCMD_Face,
    CommandType::SCMD_Load,
    CommandType::SCMD_Move,
    CommandType::SCMD_PickUpSimItem,
    CommandType::SCMD_Recrew,
    CommandType::SCMD_ReinforceUnit,
    CommandType::SCMD_Retreat,
    CommandType::SCMD_Stop,
    CommandType::SCMD_StopAbility,
    CommandType::SCMD_Unload,
    CommandType::SCMD_UnloadSquads,
    CommandType::SCMD_Upgrade,
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

fn parse_fixture(path: &Path) -> cohlib::Replay {
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    let data = fs::read(path).unwrap_or_else(|e| panic!("read {name}: {e}"));
    cohlib::parse_replay(&data).unwrap_or_else(|e| panic!("{name}: {e}"))
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

/// A command type this crate claims to decode must decode in *every* instance. Falling
/// back to `Command::Unknown` for a payload shape the decoder didn't expect would leave
/// callers unable to tell a complete set of (say) retreat commands from a partial one,
/// so those payloads panic instead — this test is the guard that none of them slip
/// through as `Unknown` either.
#[test]
fn decoded_command_types_never_fall_back_to_unknown() {
    let mut unknown_counts: HashMap<CommandType, usize> = HashMap::new();

    for path in fixture_paths() {
        let replay = parse_fixture(&path);
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
        HashMap::new(),
        "a command type this crate claims to decode fell back to Unknown"
    );
}

/// Blueprints added by a mod are referenced by a content pack UUID plus an ID scoped to
/// that pack, rather than by a globally unique pbgid. `unusual_cpu_items.rec` was played
/// under a mod that adds its own squads, upgrades, abilities and buildings, so it
/// exercises every command shape that carries a blueprint reference.
#[test]
fn mod_scoped_blueprints_decode_with_their_content_pack_uuid() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("replays")
        .join("unusual_cpu_items.rec");
    let replay = parse_fixture(&path);

    let mut mod_scoped = 0;
    for player in replay.players() {
        for command in player.commands() {
            let mod_uuid = match &command {
                Command::BuildSquad(data) | Command::BuildGlobalUpgrade(data) => data.mod_uuid(),
                Command::Reinforce(data) => data.mod_uuid(),
                Command::UseAbilitySquad(data) => data.mod_uuid(),
                Command::ConstructEntity(data) => data.mod_uuid(),
                _ => None,
            };
            if let Some(uuid) = mod_uuid {
                assert_eq!(
                    uuid.to_string(),
                    "bcf7f10c-0a63-4196-a62d-d96e3134068e",
                    "unexpected content pack UUID"
                );
                mod_scoped += 1;
            }
        }
    }

    assert_eq!(
        mod_scoped, 17,
        "expected every mod-scoped blueprint reference in this fixture to decode"
    );
}

/// Retreats were parameter-less until a later game build let them carry a facing, so
/// they decode through the same targeting machinery as movement commands rather than as
/// a source-only command.
#[test]
fn retreats_carrying_a_facing_decode_as_retreats() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("replays")
        .join("v44736_retreat_to_position.rec");
    let replay = parse_fixture(&path);

    let retreats: Vec<_> = replay
        .players()
        .iter()
        .flat_map(|player| player.commands())
        .filter_map(|command| match command {
            Command::Retreat(data) => Some(data),
            _ => None,
        })
        .collect();

    assert!(!retreats.is_empty(), "expected retreat commands");
    assert_eq!(
        retreats
            .iter()
            .filter(|data| data.facing().is_some())
            .count(),
        16,
        "expected every retreat carrying a facing to decode it"
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
        let replay = parse_fixture(&path);
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
