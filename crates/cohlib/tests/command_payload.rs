//! Corpus-wide validation of command payload parsing.
//!
//! Every fixture must parse without panicking or erroring — the payload parsers in
//! `replay::data::ticks::payload`/`value` panic on wire data that doesn't match the
//! model validated during development, so this is the primary regression guard for
//! that model. It also checks that command types this crate claims to semantically
//! decode always do so, rather than silently falling back to `Command::Unknown`.

use cohlib::{Command, CommandType, Source};
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

/// `CMD_Ability` (`Command::UseAbility`) has the same dual shape `SCMD_Ability`
/// (`Command::UseAbilitySquad`) does: most instances carry a blueprint, but a parameter
/// block of kind `0x01` means the command is continuing/updating an already-active
/// ability's target and carries only targeting values, no blueprint at all.
#[test]
fn use_ability_without_a_blueprint_decodes_with_no_pbgid() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("replays")
        .join("cmd_ability_no_blueprint.rec");
    let replay = parse_fixture(&path);

    let use_abilities: Vec<_> = replay
        .players()
        .iter()
        .flat_map(|player| player.commands())
        .filter_map(|command| match command {
            Command::UseAbility(data) => Some(data),
            _ => None,
        })
        .collect();

    assert_eq!(use_abilities.len(), 29, "expected 29 UseAbility commands");
    assert_eq!(
        use_abilities
            .iter()
            .filter(|data| data.pbgid().is_none())
            .count(),
        1,
        "expected exactly one UseAbility command with no pbgid"
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

/// `CMD_CancelConstruction`'s source was assumed scalar until a build 48837 replay
/// showed it can be a multi-squad selection (cancelling construction on several
/// selected buildings in one command), so `Sourced` preserves the full `Source`
/// rather than truncating it to a legacy `u16`.
#[test]
fn cancel_construction_decodes_a_multi_squad_source() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("replays")
        .join("v48837_multi_select_cancel_construction.rec");
    let replay = parse_fixture(&path);

    let has_multi_squad_cancel = replay
        .players()
        .iter()
        .flat_map(|player| player.commands())
        .any(|command| match command {
            Command::CancelConstruction(data) => matches!(data.source(), Source::Squads(_)),
            _ => false,
        });

    assert!(
        has_multi_squad_cancel,
        "expected at least one CancelConstruction with a multi-squad source"
    );
}

/// Camera telemetry records (`DCMD_CameraTrack`/`DCMD_COUNT`) are not player commands:
/// they must never appear in `Player::commands()` (as `Command::Unknown` or otherwise),
/// and `Player::camera_tracks()`/`Player::camera_counts()` must actually be populated
/// from the same underlying data.
#[test]
fn camera_tracks_are_excluded_from_commands_and_populated_separately() {
    let mut total_commands = 0;
    let mut total_camera_tracks = 0;
    let mut total_camera_counts = 0;

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
            total_camera_counts += player.camera_counts().len();
        }
    }

    assert!(total_commands > 0, "expected at least one player command");
    assert!(
        total_camera_tracks > 0,
        "expected at least one camera track"
    );
    assert!(
        total_camera_counts > 0,
        "expected at least one camera counts record"
    );
}

/// `CameraTrack::position` is exactly `raw_position() / 100` — this crate doesn't
/// attempt to reconstruct coordinates beyond the wire format's ±327.67 unit range (see
/// the type's docs on why). Checked corpus-wide, including the largest maps, since
/// every fixture examined during development stays within that range regardless of map
/// size.
#[test]
fn camera_track_position_is_raw_position_scaled() {
    let mut checked = 0;
    for path in fixture_paths() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let replay = parse_fixture(&path);
        for player in replay.players() {
            for track in player.camera_tracks() {
                let raw = track.raw_position();
                let expected = [
                    raw[0] as f32 / 100.0,
                    raw[1] as f32 / 100.0,
                    raw[2] as f32 / 100.0,
                ];
                let resolved = track.position();
                assert_eq!(
                    [resolved.x(), resolved.y(), resolved.z()],
                    expected,
                    "{name}: expected position() to be raw_position() scaled by 1/100"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "expected at least one camera track");
}

/// `startpos_8p.rec` has a known-good `StartingPosition` for every one of its 8 players
/// (see `parse_starting_positions_8p` in `replay.rs`), which doubles as ground truth for
/// `CameraTrack::position`: every player's very first camera sample should be centred on
/// their starting position, since that's where the match camera begins. `x` should match
/// almost exactly; `z` (see `CameraTrack::position` on the axis convention) is offset by
/// a small, constant amount — the camera sits slightly behind the point it's centred on.
#[test]
fn camera_track_position_matches_known_starting_positions() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("replays")
        .join("startpos_8p.rec");
    let replay = parse_fixture(&path);

    let mut z_offsets = Vec::new();
    for player in replay.players() {
        let starting_position = player
            .starting_position()
            .unwrap_or_else(|| panic!("{}: expected a starting position", player.name()));
        let first = player
            .camera_tracks()
            .into_iter()
            .min_by_key(|track| track.tick())
            .unwrap_or_else(|| panic!("{}: expected at least one camera track", player.name()));

        let dx = (first.position().x() - starting_position.x()).abs();
        assert!(
            dx < 0.05,
            "{}: camera x {} too far from starting position x {}",
            player.name(),
            first.position().x(),
            starting_position.x()
        );
        z_offsets.push(starting_position.y() - first.position().z());
    }

    let min = z_offsets.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = z_offsets.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    assert!(
        max - min < 0.1,
        "expected the camera's z offset from the starting position to be the same \
         constant for every player, got offsets {z_offsets:?}"
    );
}

/// The camera's orientation is a unit quaternion, and the game's fixed RTS camera never
/// banks — so it should always be roll-free, i.e. `w*z == -x*y` (see
/// `CameraTrack::pitch`/`yaw`, which assume this).
#[test]
fn camera_track_orientation_is_a_roll_free_unit_quaternion() {
    for path in fixture_paths() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let replay = parse_fixture(&path);
        for player in replay.players() {
            for track in player.camera_tracks() {
                let [w, x, y, z] = track.orientation();
                let norm = (w * w + x * x + y * y + z * z).sqrt();
                assert!(
                    (norm - 1.0).abs() < 0.01,
                    "{name}: camera orientation {:?} isn't a unit quaternion (norm {norm})",
                    track.orientation()
                );
                assert!(
                    (w * z - (-x * y)).abs() < 0.01,
                    "{name}: camera orientation {:?} isn't roll-free",
                    track.orientation()
                );
            }
        }
    }
}

/// Large single-step jumps in camera position are common and legitimate — e.g. a player
/// clicking the minimap teleports the camera there instantly — and every fixture's raw
/// camera coordinates stay within the wire format's ±327.67 unit range regardless (see
/// `CameraTrack::position`), so `unusual_options.rec`'s large apparent jumps are real
/// camera movement, not a wraparound artifact to correct.
#[test]
fn camera_track_large_jumps_are_real_movement_not_wraparound() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("replays")
        .join("unusual_options.rec");
    let replay = parse_fixture(&path);

    let mut large_jumps = 0;
    for player in replay.players() {
        for track in player.camera_tracks() {
            let raw = track.raw_position();
            assert!(
                (-32768..=32767).contains(&raw[0]) && (-32768..=32767).contains(&raw[2]),
                "raw position should always be in i16 range by construction"
            );
        }
        let mut tracks = player.camera_tracks();
        tracks.sort_by_key(|track| track.tick());
        for pair in tracks.windows(2) {
            let [a, b] = pair else { unreachable!() };
            if (a.position().x() - b.position().x()).abs() > 500.0 {
                large_jumps += 1;
            }
        }
    }

    assert!(
        large_jumps > 100,
        "expected this fixture to demonstrate large single-step camera movement, found {large_jumps}"
    );
}

/// `CameraTrack::sequence` is a per-player sample counter distinct from `tick` (the
/// position in the replay's overall tick stream) — it should increase monotonically
/// within a player's own samples even though it isn't derivable from `tick` (see the
/// type's docs).
#[test]
fn camera_track_sequence_is_monotonic_per_player() {
    for path in fixture_paths() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let replay = parse_fixture(&path);
        for player in replay.players() {
            let mut tracks = player.camera_tracks();
            tracks.sort_by_key(|track| track.tick());
            for pair in tracks.windows(2) {
                let [a, b] = pair else { unreachable!() };
                assert!(
                    a.sequence() <= b.sequence(),
                    "{name}: expected non-decreasing sequence, got {} then {}",
                    a.sequence(),
                    b.sequence()
                );
            }
        }
    }
}

/// The wire types each variant is allowed to have been decoded from. Three variants
/// deliberately merge several wire types (see `Command::StopAbility`,
/// `Command::UnloadSquads`, `Command::CancelProduction`); this is the guard that the
/// originating type is plumbed through faithfully rather than being replaced by a
/// canonical one, since the merged types are the whole reason `action_type` is stored
/// on the payload instead of derived from the variant. `Command::Unknown` is excluded —
/// by construction it covers whichever wire types this crate hasn't decoded, so there's
/// no fixed set to check it against.
const VARIANT_WIRE_TYPES: &[(&str, &[CommandType])] = &[
    ("AITakeover", &[CommandType::PCMD_AIPlayer]),
    (
        "AIResourceBonus",
        &[CommandType::PCMD_AIPlayer_ResourceBonus],
    ),
    ("Attack", &[CommandType::SCMD_Attack]),
    ("AttackMove", &[CommandType::SCMD_AttackMove]),
    ("AttackFromHold", &[CommandType::CMD_AttackFromHold]),
    ("Broadcast", &[CommandType::PCMD_BroadcastMessage]),
    ("BuildStructure", &[CommandType::SCMD_BuildStructure]),
    ("BuildGlobalUpgrade", &[CommandType::CMD_Upgrade]),
    ("BuildSquad", &[CommandType::CMD_BuildSquad]),
    ("CancelConstruction", &[CommandType::CMD_CancelConstruction]),
    (
        "CancelProduction",
        &[
            CommandType::CMD_CancelProduction,
            CommandType::SCMD_CancelProduction,
            CommandType::PCMD_CancelProduction,
        ],
    ),
    ("Capture", &[CommandType::SCMD_Capture]),
    ("CaptureTeamWeapon", &[CommandType::SCMD_CaptureTeamWeapon]),
    (
        "ConstructEntity",
        &[CommandType::PCMD_PlaceAndConstructEntities],
    ),
    (
        "DeselectAllBattlegroupAbilities",
        &[CommandType::PCMD_TentativeUpgradeRemoveAll],
    ),
    ("DetonateCharges", &[CommandType::PCMD_DetonateCharges]),
    ("Face", &[CommandType::SCMD_Face]),
    ("Load", &[CommandType::SCMD_Load]),
    ("Move", &[CommandType::CMD_Move]),
    ("MoveSquad", &[CommandType::SCMD_Move]),
    ("PickUpSimItem", &[CommandType::SCMD_PickUpSimItem]),
    ("Recrew", &[CommandType::SCMD_Recrew]),
    ("Reinforce", &[CommandType::SCMD_ReinforceUnit]),
    ("Retreat", &[CommandType::SCMD_Retreat]),
    ("RallyPoint", &[CommandType::CMD_RallyPoint]),
    ("SelectBattlegroup", &[CommandType::PCMD_InstantUpgrade]),
    (
        "SelectBattlegroupAbility",
        &[CommandType::PCMD_TentativeUpgrade],
    ),
    ("Stop", &[CommandType::SCMD_Stop]),
    (
        "StopAbility",
        &[CommandType::SCMD_StopAbility, CommandType::CMD_StopAbility],
    ),
    ("Surrender", &[CommandType::PCMD_Surrender]),
    ("Unload", &[CommandType::SCMD_Unload]),
    (
        "UnloadSquads",
        &[
            CommandType::SCMD_UnloadSquads,
            CommandType::CMD_UnloadSquads,
        ],
    ),
    ("UpgradeSquad", &[CommandType::SCMD_Upgrade]),
    ("UseAbility", &[CommandType::CMD_Ability]),
    ("UseAbilitySquad", &[CommandType::SCMD_Ability]),
    ("UseBattlegroupAbility", &[CommandType::PCMD_Ability]),
];

/// Variants whose payload legitimately merges more than one wire type — see
/// `VARIANT_WIRE_TYPES`.
const MERGED_VARIANTS: &[&str] = &["StopAbility", "UnloadSquads", "CancelProduction"];

#[test]
fn every_command_action_type_matches_its_variant() {
    for path in fixture_paths() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let replay = parse_fixture(&path);
        for player in replay.players() {
            for command in player.commands() {
                if matches!(command, Command::Unknown(_)) {
                    continue;
                }
                let variant = command.variant_name();
                let (_, allowed) = VARIANT_WIRE_TYPES
                    .iter()
                    .find(|(v, _)| *v == variant)
                    .unwrap_or_else(|| panic!("{name}: no VARIANT_WIRE_TYPES entry for {variant}"));
                assert!(
                    allowed.contains(&command.action_type()),
                    "{name}: {variant} decoded from unexpected wire type {:?}",
                    command.action_type()
                );
            }
        }
    }
}

/// Without this, `every_command_action_type_matches_its_variant` would pass just as
/// happily against a canonical-per-variant table (one that dropped the alternate wire
/// types from `VARIANT_WIRE_TYPES`) — it wouldn't be guarding the faithfulness of
/// `action_type` at all. This proves the merged variants really are observed carrying
/// more than one wire type in the corpus.
#[test]
fn merged_variants_are_observed_with_multiple_wire_types() {
    let mut seen: HashMap<&str, std::collections::HashSet<CommandType>> = MERGED_VARIANTS
        .iter()
        .map(|&v| (v, std::collections::HashSet::new()))
        .collect();

    for path in fixture_paths() {
        let replay = parse_fixture(&path);
        for player in replay.players() {
            for command in player.commands() {
                let variant = command.variant_name();
                if let Some(set) = seen.get_mut(variant) {
                    set.insert(command.action_type());
                }
            }
        }
    }

    for variant in MERGED_VARIANTS {
        let set = &seen[variant];
        assert!(
            set.len() > 1,
            "expected {variant} to be observed with more than one wire type in the \
             corpus, got {set:?} — this test isn't guarding anything if it isn't"
        );
    }
}

/// `Source::kind()`/`ids()` are the flattening the Ruby bindings persist; the pair must
/// stay consistent with the source variant and never yield an empty id list.
#[test]
fn source_kind_and_ids_agree_with_the_variant() {
    let mut checked = 0;
    for path in fixture_paths() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let replay = parse_fixture(&path);
        for player in replay.players() {
            for command in player.commands() {
                let Some(source) = command.source() else {
                    continue;
                };
                let ids = source.ids();
                assert!(
                    !ids.is_empty(),
                    "{name}: {} source has empty ids for kind {}",
                    command.variant_name(),
                    source.kind()
                );
                if !matches!(source, Source::Squads(_)) {
                    assert_eq!(
                        ids.len(),
                        1,
                        "{name}: scalar source kind {} should have exactly one id",
                        source.kind()
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "expected at least one command with a source");
}

/// Records which variants the fixture corpus actually reaches, so gaps in Ruby-side
/// `to_h` coverage are visible rather than assumed. Every non-`Unknown` variant is
/// currently reachable; this fails if a previously-reachable variant regresses (e.g. a
/// parse change that silently falls back to `Unknown` for a shape that used to decode),
/// and reports the missing set so it's clear what broke.
#[test]
fn corpus_variant_coverage_does_not_regress() {
    const EXPECTED_REACHABLE: &[&str] = &[
        "AITakeover",
        "AIResourceBonus",
        "Attack",
        "AttackMove",
        "AttackFromHold",
        "Broadcast",
        "BuildStructure",
        "BuildGlobalUpgrade",
        "BuildSquad",
        "CancelConstruction",
        "CancelProduction",
        "Capture",
        "CaptureTeamWeapon",
        "ConstructEntity",
        "DeselectAllBattlegroupAbilities",
        "DetonateCharges",
        "Face",
        "Load",
        "Move",
        "MoveSquad",
        "PickUpSimItem",
        "Recrew",
        "Reinforce",
        "Retreat",
        "RallyPoint",
        "SelectBattlegroup",
        "SelectBattlegroupAbility",
        "Stop",
        "StopAbility",
        "Surrender",
        "Unload",
        "UnloadSquads",
        "UpgradeSquad",
        "UseAbility",
        "UseAbilitySquad",
        "UseBattlegroupAbility",
    ];

    let mut reached: std::collections::HashSet<&'static str> = std::collections::HashSet::new();
    for path in fixture_paths() {
        let replay = parse_fixture(&path);
        for player in replay.players() {
            for command in player.commands() {
                reached.insert(command.variant_name());
            }
        }
    }

    let missing: Vec<_> = EXPECTED_REACHABLE
        .iter()
        .filter(|v| !reached.contains(*v))
        .collect();
    assert!(
        missing.is_empty(),
        "expected these variants to be reachable in the fixture corpus, but they \
         weren't: {missing:?} (reached: {reached:?})"
    );
}
