//! Wrapper for Company of Heroes 3 player commands.

use crate::{
    command_data::{
        Empty, Pbgid, SourcePbgid, Sourced, SourcedIndex, SourcedPbgid, Squads, Unknown,
    },
    command_type::CommandType,
    data::ticks,
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
    BuildGlobalUpgrade(SourcedPbgid),
    BuildSquad(SourcedPbgid),
    CancelConstruction(Sourced),
    CancelProduction(SourcedIndex),
    ConstructEntity(Pbgid),
    /// A player clearing all pending (not-yet-purchased) battlegroup ability
    /// selections, e.g. `PCMD_TentativeUpgradeRemoveAll`.
    DeselectAllBattlegroupAbilities(Squads),
    /// A squad reinforcing (adding models back to a depleted squad),
    /// `SCMD_ReinforceUnit`.
    Reinforce(SourcePbgid),
    /// One or more squads retreating to base.
    Retreat(Squads),
    SelectBattlegroup(Pbgid),
    SelectBattlegroupAbility(Pbgid),
    /// One or more squads halting their current action.
    Stop(Squads),
    /// A player surrendering the match.
    Surrender(Squads),
    /// A transport unloading all of its passengers, whether issued by the transport
    /// entity (`CMD_UnloadSquads`) or by the passenger squads themselves
    /// (`SCMD_UnloadSquads`) — both produce this variant since the effect is the same.
    UnloadSquads(Squads),
    /// A squad researching an upgrade, `SCMD_Upgrade` — the squad-level equivalent of
    /// `BuildGlobalUpgrade`.
    UpgradeSquad(SourcePbgid),
    UseAbility(SourcedPbgid),
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
                CommandType::PCMD_Ability => {
                    Self::UseBattlegroupAbility(Pbgid::new(tick, command.index, pbgid))
                }
                CommandType::PCMD_InstantUpgrade => {
                    Self::SelectBattlegroup(Pbgid::new(tick, command.index, pbgid))
                }
                CommandType::PCMD_PlaceAndConstructEntities => {
                    Self::ConstructEntity(Pbgid::new(tick, command.index, pbgid))
                }
                CommandType::PCMD_TentativeUpgrade => {
                    Self::SelectBattlegroupAbility(Pbgid::new(tick, command.index, pbgid))
                }
                _ => panic!(
                    "a pbgid command isn't being handled here! command type {:?}",
                    command.action_type
                ),
            },
            ticks::CommandData::SourcedPbgid(pbgid, source_identifier) => match command.action_type
            {
                CommandType::CMD_Ability => Self::UseAbility(SourcedPbgid::new(
                    tick,
                    command.index,
                    pbgid,
                    source_identifier,
                )),
                CommandType::CMD_BuildSquad => Self::BuildSquad(SourcedPbgid::new(
                    tick,
                    command.index,
                    pbgid,
                    source_identifier,
                )),
                CommandType::CMD_Upgrade => Self::BuildGlobalUpgrade(SourcedPbgid::new(
                    tick,
                    command.index,
                    pbgid,
                    source_identifier,
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
                _ => panic!(
                    "a source pbgid command isn't being handled here! command type {:?}",
                    command.action_type
                ),
            },
            ticks::CommandData::Unknown => {
                Self::Unknown(Unknown::new(tick, command.index, command.action_type))
            }
        }
    }
}
