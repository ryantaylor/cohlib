//! Wrapper for Company of Heroes 3 player commands.

use crate::{
    command_data::{
        Ability, Empty, Orientation, Pbgid, Position, SourcePbgid, Sourced, SourcedIndex,
        SourcedPbgid, Squads, Targeted, Unknown,
    },
    command_type::CommandType,
    data::ticks::{self, value::TargetValues},
};
use serde::{Deserialize, Serialize};

/// Wrapper for one of many Company of Heroes 3 player commands parsed from a replay file. For
/// details on the specifics of a given command, see the specific enum variants.
///
/// Commands are collected during tick parsing and then associated with the `Player` instance that
/// sent them. To access, see `Player::commands`.

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    AITakeover(Empty),
    /// A squad attacking a target, `SCMD_Attack`.
    Attack(Targeted),
    /// A squad moving to a position while engaging targets along the way,
    /// `SCMD_AttackMove`.
    AttackMove(Targeted),
    /// An entity attacking from within a garrisoned building, `CMD_AttackFromHold`.
    AttackFromHold(Targeted),
    /// A squad constructing a structure, `SCMD_BuildStructure`.
    BuildStructure(Targeted),
    BuildGlobalUpgrade(SourcedPbgid),
    BuildSquad(SourcedPbgid),
    CancelConstruction(Sourced),
    CancelProduction(SourcedIndex),
    /// A squad capturing a strategic point, `SCMD_Capture`.
    Capture(Targeted),
    /// A squad capturing an abandoned team weapon, `SCMD_CaptureTeamWeapon`.
    CaptureTeamWeapon(Targeted),
    ConstructEntity(Pbgid),
    /// A player clearing all pending (not-yet-purchased) battlegroup ability
    /// selections, e.g. `PCMD_TentativeUpgradeRemoveAll`.
    DeselectAllBattlegroupAbilities(Squads),
    /// A player detonating previously-placed demolition charges,
    /// `PCMD_DetonateCharges`.
    DetonateCharges(Targeted),
    /// A squad facing a direction, `SCMD_Face`.
    Face(Targeted),
    /// A squad loading into a transport, `SCMD_Load`.
    Load(Targeted),
    /// An entity moving, `CMD_Move`.
    Move(Targeted),
    /// A squad moving, `SCMD_Move`.
    MoveSquad(Targeted),
    /// A squad picking up a dropped item, `SCMD_PickUpSimItem`.
    PickUpSimItem(Targeted),
    /// A squad recrewing an abandoned team weapon, `SCMD_Recrew`.
    Recrew(Targeted),
    /// A squad reinforcing (adding models back to a depleted squad),
    /// `SCMD_ReinforceUnit`.
    Reinforce(SourcePbgid),
    /// One or more squads retreating to base.
    Retreat(Squads),
    /// An entity or squad setting a rally point, `CMD_RallyPoint`.
    RallyPoint(Targeted),
    SelectBattlegroup(Pbgid),
    SelectBattlegroupAbility(Pbgid),
    /// One or more squads halting their current action.
    Stop(Squads),
    /// A squad or entity stopping its currently active ability, `SCMD_StopAbility` or
    /// `CMD_StopAbility` — both produce this variant since the effect is the same.
    StopAbility(SourcePbgid),
    /// A player surrendering the match.
    Surrender(Squads),
    /// A squad disembarking from a transport, `SCMD_Unload`.
    Unload(Targeted),
    /// A transport unloading all of its passengers, whether issued by the transport
    /// entity (`CMD_UnloadSquads`) or by the passenger squads themselves
    /// (`SCMD_UnloadSquads`) — both produce this variant since the effect is the same.
    UnloadSquads(Squads),
    /// A squad researching an upgrade, `SCMD_Upgrade` — the squad-level equivalent of
    /// `BuildGlobalUpgrade`.
    UpgradeSquad(SourcePbgid),
    UseAbility(SourcedPbgid),
    /// A squad using an ability, `SCMD_Ability` — see [`Ability`] on why its pbgid is
    /// optional, unlike [`Self::UseAbility`].
    UseAbilitySquad(Ability),
    UseBattlegroupAbility(Pbgid),
    Unknown(Unknown),
}

impl Command {
    pub(crate) fn from_data_command_at_tick(command: ticks::Command, tick: u32) -> Self {
        match command.data {
            ticks::CommandData::Empty => match command.action_type {
                CommandType::PCMD_AIPlayer => Self::AITakeover(Empty::new(tick)),
                _ => panic!(
                    "an empty command isn't being handled here! command type {:?}",
                    command.action_type
                ),
            },
            ticks::CommandData::Pbgid(pbgid) => match command.action_type {
                CommandType::PCMD_InstantUpgrade => Self::SelectBattlegroup(Pbgid::new(
                    tick,
                    command.index,
                    pbgid,
                    None,
                    None,
                    None,
                    None,
                )),
                CommandType::PCMD_PlaceAndConstructEntities => Self::ConstructEntity(Pbgid::new(
                    tick,
                    command.index,
                    pbgid,
                    None,
                    None,
                    None,
                    None,
                )),
                CommandType::PCMD_TentativeUpgrade => Self::SelectBattlegroupAbility(Pbgid::new(
                    tick,
                    command.index,
                    pbgid,
                    None,
                    None,
                    None,
                    None,
                )),
                _ => panic!(
                    "a pbgid command isn't being handled here! command type {:?}",
                    command.action_type
                ),
            },
            ticks::CommandData::SourcedPbgid(pbgid, source_identifier) => match command.action_type
            {
                CommandType::CMD_BuildSquad => Self::BuildSquad(SourcedPbgid::new(
                    tick,
                    command.index,
                    pbgid,
                    source_identifier,
                    None,
                    None,
                    None,
                    None,
                )),
                CommandType::CMD_Upgrade => Self::BuildGlobalUpgrade(SourcedPbgid::new(
                    tick,
                    command.index,
                    pbgid,
                    source_identifier,
                    None,
                    None,
                    None,
                    None,
                )),
                _ => panic!(
                    "a sourced pbgid command isn't being handled here! command type {:?}",
                    command.action_type
                ),
            },
            ticks::CommandData::Sourced(source_identifier) => match command.action_type {
                CommandType::CMD_CancelConstruction => {
                    Self::CancelConstruction(Sourced::new(tick, command.index, source_identifier))
                }
                _ => panic!(
                    "a sourced command isn't being handled here! command type {:?}",
                    command.action_type
                ),
            },
            ticks::CommandData::SourcedIndex(source_identifier, queue_index) => {
                match command.action_type {
                    // SCMD_/PCMD_CancelProduction cancel a queued production item the
                    // same way CMD_CancelProduction does, just issued from a squad's or
                    // the player's UI instead of a building's — same effect, same
                    // variant.
                    CommandType::CMD_CancelProduction
                    | CommandType::SCMD_CancelProduction
                    | CommandType::PCMD_CancelProduction => Self::CancelProduction(
                        SourcedIndex::new(tick, command.index, source_identifier, queue_index),
                    ),
                    _ => panic!(
                        "a sourced command isn't being handled here! command type {:?}",
                        command.action_type
                    ),
                }
            }
            ticks::CommandData::SourceOnly(source) => match command.action_type {
                CommandType::SCMD_Retreat => {
                    Self::Retreat(Squads::new(tick, command.index, source))
                }
                CommandType::SCMD_Stop => Self::Stop(Squads::new(tick, command.index, source)),
                CommandType::SCMD_UnloadSquads | CommandType::CMD_UnloadSquads => {
                    Self::UnloadSquads(Squads::new(tick, command.index, source))
                }
                CommandType::PCMD_TentativeUpgradeRemoveAll => {
                    Self::DeselectAllBattlegroupAbilities(Squads::new(tick, command.index, source))
                }
                CommandType::PCMD_Surrender => {
                    Self::Surrender(Squads::new(tick, command.index, source))
                }
                _ => panic!(
                    "a source-only command isn't being handled here! command type {:?}",
                    command.action_type
                ),
            },
            ticks::CommandData::SourcePbgid(source, pbgid) => match command.action_type {
                CommandType::SCMD_Upgrade => {
                    Self::UpgradeSquad(SourcePbgid::new(tick, command.index, source, pbgid))
                }
                CommandType::SCMD_ReinforceUnit => {
                    Self::Reinforce(SourcePbgid::new(tick, command.index, source, pbgid))
                }
                // SCMD_/CMD_StopAbility stop the ability referenced by pbgid the same
                // way regardless of whether it was issued by a squad or an entity —
                // same effect, same variant.
                CommandType::SCMD_StopAbility | CommandType::CMD_StopAbility => {
                    Self::StopAbility(SourcePbgid::new(tick, command.index, source, pbgid))
                }
                _ => panic!(
                    "a source pbgid command isn't being handled here! command type {:?}",
                    command.action_type
                ),
            },
            ticks::CommandData::Targeted(source, targets) => {
                let (position, facing, orientation, entity) = split_targets(targets);
                let targeted = Targeted::new(
                    tick,
                    command.index,
                    source,
                    position,
                    facing,
                    orientation,
                    entity,
                );
                match command.action_type {
                    CommandType::CMD_RallyPoint => Self::RallyPoint(targeted),
                    CommandType::CMD_Move => Self::Move(targeted),
                    CommandType::CMD_AttackFromHold => Self::AttackFromHold(targeted),
                    CommandType::SCMD_Move => Self::MoveSquad(targeted),
                    CommandType::SCMD_Attack => Self::Attack(targeted),
                    CommandType::SCMD_Capture => Self::Capture(targeted),
                    CommandType::SCMD_AttackMove => Self::AttackMove(targeted),
                    CommandType::SCMD_Load => Self::Load(targeted),
                    CommandType::SCMD_Unload => Self::Unload(targeted),
                    CommandType::SCMD_Face => Self::Face(targeted),
                    CommandType::SCMD_CaptureTeamWeapon => Self::CaptureTeamWeapon(targeted),
                    CommandType::SCMD_PickUpSimItem => Self::PickUpSimItem(targeted),
                    CommandType::SCMD_BuildStructure => Self::BuildStructure(targeted),
                    CommandType::SCMD_Recrew => Self::Recrew(targeted),
                    CommandType::PCMD_DetonateCharges => Self::DetonateCharges(targeted),
                    _ => panic!(
                        "a targeted command isn't being handled here! command type {:?}",
                        command.action_type
                    ),
                }
            }
            ticks::CommandData::PbgidTargeted(pbgid, targets) => match command.action_type {
                CommandType::PCMD_Ability => {
                    let (position, facing, orientation, entity) = split_targets(targets);
                    Self::UseBattlegroupAbility(Pbgid::new(
                        tick,
                        command.index,
                        pbgid,
                        position,
                        facing,
                        orientation,
                        entity,
                    ))
                }
                _ => panic!(
                    "a pbgid-targeted command isn't being handled here! command type {:?}",
                    command.action_type
                ),
            },
            ticks::CommandData::SourcedPbgidTargeted(pbgid, source_identifier, targets) => {
                match command.action_type {
                    CommandType::CMD_Ability => {
                        let (position, facing, orientation, entity) = split_targets(targets);
                        Self::UseAbility(SourcedPbgid::new(
                            tick,
                            command.index,
                            pbgid,
                            source_identifier,
                            position,
                            facing,
                            orientation,
                            entity,
                        ))
                    }
                    _ => panic!(
                        "a sourced pbgid-targeted command isn't being handled here! command type {:?}",
                        command.action_type
                    ),
                }
            }
            ticks::CommandData::Ability(source, pbgid, targets) => match command.action_type {
                CommandType::SCMD_Ability => {
                    let (position, facing, orientation, entity) = split_targets(targets);
                    Self::UseAbilitySquad(Ability::new(
                        tick,
                        command.index,
                        source,
                        pbgid,
                        position,
                        facing,
                        orientation,
                        entity,
                    ))
                }
                _ => panic!(
                    "an ability command isn't being handled here! command type {:?}",
                    command.action_type
                ),
            },
            ticks::CommandData::Unknown => {
                Self::Unknown(Unknown::new(tick, command.index, command.action_type))
            }
        }
    }
}

/// Splits a wire-level `TargetValues` into the public, typed optional fields shared by
/// every command data shape that can carry targeting information.
fn split_targets(
    targets: TargetValues,
) -> (
    Option<Position>,
    Option<f32>,
    Option<Orientation>,
    Option<u32>,
) {
    let position = targets.position.map(|[x, y, z]| Position::new(x, y, z));
    let orientation = targets
        .orientation
        .map(|[x, y, z, w]| Orientation::new(x, y, z, w));
    (position, targets.facing, orientation, targets.entity)
}
