//! Wrapper for Company of Heroes 3 player commands.

use crate::{
    command_data::{
        Ability, BroadcastMessage, CommandPayload, Construction, Empty, Orientation, Pbgid,
        Position, ResourceBonus, Source, SourcePbgid, Sourced, SourcedAbility, SourcedIndex,
        SourcedPbgid, Targeted, Unknown,
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
#[cfg_attr(
    feature = "magnus",
    magnus::wrap(class = "CohLib::Command", free_immediately, size)
)]
pub enum Command {
    AITakeover(Empty),
    /// A player's AI issuing itself a resource bonus, `PCMD_AIPlayer_ResourceBonus`.
    AIResourceBonus(ResourceBonus),
    /// A squad attacking a target, `SCMD_Attack`.
    Attack(Targeted),
    /// A squad moving to a position while engaging targets along the way,
    /// `SCMD_AttackMove`.
    AttackMove(Targeted),
    /// An entity attacking from within a garrisoned building, `CMD_AttackFromHold`.
    AttackFromHold(Targeted),
    /// A player broadcasting a UI/scripted action, `PCMD_BroadcastMessage` — see
    /// [`BroadcastMessage`] on why this isn't chat (that's `crate::Message`).
    Broadcast(BroadcastMessage),
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
    ConstructEntity(Construction),
    /// A player clearing all pending (not-yet-purchased) battlegroup ability
    /// selections, e.g. `PCMD_TentativeUpgradeRemoveAll`.
    DeselectAllBattlegroupAbilities(Targeted),
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
    /// One or more squads retreating to base. Newer game builds let a retreat carry a
    /// facing, so the targeting fields are not always empty.
    Retreat(Targeted),
    /// An entity or squad setting a rally point, `CMD_RallyPoint`.
    RallyPoint(Targeted),
    SelectBattlegroup(Pbgid),
    SelectBattlegroupAbility(Pbgid),
    /// One or more squads halting their current action.
    Stop(Targeted),
    /// A squad or entity stopping its currently active ability, `SCMD_StopAbility` or
    /// `CMD_StopAbility` — both produce this variant since the effect is the same.
    StopAbility(SourcePbgid),
    /// A player surrendering the match.
    Surrender(Targeted),
    /// A squad disembarking from a transport, `SCMD_Unload`.
    Unload(Targeted),
    /// A transport unloading all of its passengers, whether issued by the transport
    /// entity (`CMD_UnloadSquads`) or by the passenger squads themselves
    /// (`SCMD_UnloadSquads`) — both produce this variant since the effect is the same.
    UnloadSquads(Targeted),
    /// A squad researching an upgrade, `SCMD_Upgrade` — the squad-level equivalent of
    /// `BuildGlobalUpgrade`.
    UpgradeSquad(SourcePbgid),
    /// An entity or squad using an ability, `CMD_Ability` — see [`SourcedAbility`] on
    /// why its pbgid is optional, the same as [`Self::UseAbilitySquad`]. Differs from
    /// that variant only in how the source is modeled (a legacy `u16` identifier here,
    /// a full [`Source`] there).
    UseAbility(SourcedAbility),
    /// A squad using an ability, `SCMD_Ability` — see [`Ability`] on why its pbgid is
    /// optional, the same as [`Self::UseAbility`].
    UseAbilitySquad(Ability),
    UseBattlegroupAbility(Pbgid),
    Unknown(Unknown),
}

/// A borrowed view of a command's payload, discriminated by payload *shape* rather than
/// by semantic variant. Thirteen shapes cover all 37 variants, so consumers that only
/// care about the data layout — serializers, hash builders — match on thirteen arms
/// instead of thirty-seven.
#[derive(Debug, Clone, Copy)]
pub enum CommandPayloadRef<'a> {
    Ability(&'a Ability),
    BroadcastMessage(&'a BroadcastMessage),
    Construction(&'a Construction),
    Empty(&'a Empty),
    Pbgid(&'a Pbgid),
    ResourceBonus(&'a ResourceBonus),
    Sourced(&'a Sourced),
    SourcedAbility(&'a SourcedAbility),
    SourcedIndex(&'a SourcedIndex),
    SourcedPbgid(&'a SourcedPbgid),
    SourcePbgid(&'a SourcePbgid),
    Targeted(&'a Targeted),
    Unknown(&'a Unknown),
}

/// Generates `Command`'s variant-wide dispatch from a single copy of the variant list,
/// so adding a variant doesn't mean editing several parallel 37-arm matches by hand.
/// Each generated match is exhaustive, so any drift between this list and the `Command`
/// enum above is a compile error in both directions — don't add a `_ =>` catch-all arm
/// to any of them, since that would silently defeat the guarantee.
macro_rules! command_variants {
    ($($variant:ident($payload:ident)),+ $(,)?) => {
        impl Command {
            /// cohlib's own semantic name for this command, e.g. `"BuildSquad"`. For
            /// the Relic wire name it was decoded from — which is *not* derivable from
            /// this, since several wire types can share a variant — see
            /// [`Self::action_type`].
            pub fn variant_name(&self) -> &'static str {
                match self { $(Self::$variant(_) => stringify!($variant),)+ }
            }

            /// This command's payload, discriminated by shape.
            pub fn payload(&self) -> CommandPayloadRef<'_> {
                match self { $(Self::$variant(data) => CommandPayloadRef::$payload(data),)+ }
            }

            fn common(&self) -> &dyn CommandPayload {
                match self { $(Self::$variant(data) => data,)+ }
            }
        }
    };
}

command_variants! {
    AITakeover(Empty),
    AIResourceBonus(ResourceBonus),
    Attack(Targeted),
    AttackMove(Targeted),
    AttackFromHold(Targeted),
    Broadcast(BroadcastMessage),
    BuildStructure(Targeted),
    BuildGlobalUpgrade(SourcedPbgid),
    BuildSquad(SourcedPbgid),
    CancelConstruction(Sourced),
    CancelProduction(SourcedIndex),
    Capture(Targeted),
    CaptureTeamWeapon(Targeted),
    ConstructEntity(Construction),
    DeselectAllBattlegroupAbilities(Targeted),
    DetonateCharges(Targeted),
    Face(Targeted),
    Load(Targeted),
    Move(Targeted),
    MoveSquad(Targeted),
    PickUpSimItem(Targeted),
    Recrew(Targeted),
    Reinforce(SourcePbgid),
    Retreat(Targeted),
    RallyPoint(Targeted),
    SelectBattlegroup(Pbgid),
    SelectBattlegroupAbility(Pbgid),
    Stop(Targeted),
    StopAbility(SourcePbgid),
    Surrender(Targeted),
    Unload(Targeted),
    UnloadSquads(Targeted),
    UpgradeSquad(SourcePbgid),
    UseAbility(SourcedAbility),
    UseAbilitySquad(Ability),
    UseBattlegroupAbility(Pbgid),
    Unknown(Unknown),
}

impl Command {
    /// The engine tick at which the command was issued. CoH3 runs at 8 ticks per
    /// second, so divide by 8 for seconds since the replay began.
    pub fn tick(&self) -> u32 {
        self.common().tick()
    }

    /// The index of this command relative to the player who issued it.
    pub fn index(&self) -> u32 {
        self.common().index()
    }

    /// The Relic wire command type this command was decoded from. Several wire types
    /// decode to the same variant — `SCMD_StopAbility` and `CMD_StopAbility` both
    /// produce [`Command::StopAbility`] — so this preserves information the variant
    /// alone throws away.
    pub fn action_type(&self) -> CommandType {
        self.common().action_type()
    }

    /// The blueprint this command references, or `None` for commands that reference
    /// none. `Command::UseAbility` and `Command::UseAbilitySquad` can each carry a
    /// payload with no pbgid even though their shape has one, so this being `None`
    /// doesn't imply the shape lacks the field.
    pub fn pbgid(&self) -> Option<u32> {
        self.common().pbgid()
    }

    /// Who issued the command, for the payload shapes that model it as a full
    /// [`Source`]. `None` for shapes that carry only a bare `source_identifier`.
    pub fn source(&self) -> Option<&Source> {
        self.common().source()
    }
}

impl Command {
    pub(crate) fn from_data_command_at_tick(command: ticks::Command, tick: u32) -> Self {
        let action_type = command.action_type;
        let index = command.index;
        match command.data {
            ticks::CommandData::Empty => match action_type {
                CommandType::PCMD_AIPlayer => {
                    Self::AITakeover(Empty::new(action_type, tick, index))
                }
                _ => panic!(
                    "an empty command isn't being handled here! command type {:?}",
                    action_type
                ),
            },
            ticks::CommandData::Pbgid(blueprint) => match action_type {
                CommandType::PCMD_InstantUpgrade => Self::SelectBattlegroup(Pbgid::new(
                    action_type,
                    tick,
                    index,
                    blueprint,
                    None,
                    None,
                    None,
                    None,
                )),
                CommandType::PCMD_TentativeUpgrade => Self::SelectBattlegroupAbility(Pbgid::new(
                    action_type,
                    tick,
                    index,
                    blueprint,
                    None,
                    None,
                    None,
                    None,
                )),
                _ => panic!(
                    "a pbgid command isn't being handled here! command type {:?}",
                    action_type
                ),
            },
            ticks::CommandData::SourcedPbgid(blueprint, source_identifier) => match action_type {
                CommandType::CMD_BuildSquad => Self::BuildSquad(SourcedPbgid::new(
                    action_type,
                    tick,
                    index,
                    blueprint,
                    source_identifier,
                    None,
                    None,
                    None,
                    None,
                )),
                CommandType::CMD_Upgrade => Self::BuildGlobalUpgrade(SourcedPbgid::new(
                    action_type,
                    tick,
                    index,
                    blueprint,
                    source_identifier,
                    None,
                    None,
                    None,
                    None,
                )),
                _ => panic!(
                    "a sourced pbgid command isn't being handled here! command type {:?}",
                    action_type
                ),
            },
            ticks::CommandData::Sourced(source) => match action_type {
                CommandType::CMD_CancelConstruction => Self::CancelConstruction(Sourced::new(
                    action_type,
                    tick,
                    index,
                    source,
                )),
                _ => panic!(
                    "a sourced command isn't being handled here! command type {:?}",
                    action_type
                ),
            },
            ticks::CommandData::SourcedIndex(source_identifier, queue_index) => match action_type
            {
                // SCMD_/PCMD_CancelProduction cancel a queued production item the
                // same way CMD_CancelProduction does, just issued from a squad's or
                // the player's UI instead of a building's — same effect, same
                // variant.
                CommandType::CMD_CancelProduction
                | CommandType::SCMD_CancelProduction
                | CommandType::PCMD_CancelProduction => {
                    Self::CancelProduction(SourcedIndex::new(
                        action_type,
                        tick,
                        index,
                        source_identifier,
                        queue_index,
                    ))
                }
                _ => panic!(
                    "a sourced command isn't being handled here! command type {:?}",
                    action_type
                ),
            },
            ticks::CommandData::SourcePbgid(source, blueprint) => match action_type {
                CommandType::SCMD_Upgrade => Self::UpgradeSquad(SourcePbgid::new(
                    action_type,
                    tick,
                    index,
                    source,
                    blueprint,
                )),
                CommandType::SCMD_ReinforceUnit => Self::Reinforce(SourcePbgid::new(
                    action_type,
                    tick,
                    index,
                    source,
                    blueprint,
                )),
                // SCMD_/CMD_StopAbility stop the ability referenced by pbgid the same
                // way regardless of whether it was issued by a squad or an entity —
                // same effect, same variant.
                CommandType::SCMD_StopAbility | CommandType::CMD_StopAbility => {
                    Self::StopAbility(SourcePbgid::new(action_type, tick, index, source, blueprint))
                }
                _ => panic!(
                    "a source pbgid command isn't being handled here! command type {:?}",
                    action_type
                ),
            },
            ticks::CommandData::Targeted(source, targets) => {
                let (position, facing, orientation, entity) = split_targets(targets);
                let targeted = Targeted::new(
                    action_type,
                    tick,
                    index,
                    source,
                    position,
                    facing,
                    orientation,
                    entity,
                );
                match action_type {
                    CommandType::SCMD_Retreat => Self::Retreat(targeted),
                    CommandType::SCMD_Stop => Self::Stop(targeted),
                    CommandType::SCMD_UnloadSquads | CommandType::CMD_UnloadSquads => {
                        Self::UnloadSquads(targeted)
                    }
                    CommandType::PCMD_TentativeUpgradeRemoveAll => {
                        Self::DeselectAllBattlegroupAbilities(targeted)
                    }
                    CommandType::PCMD_Surrender => Self::Surrender(targeted),
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
                        action_type
                    ),
                }
            }
            ticks::CommandData::PbgidTargeted(blueprint, targets) => match action_type {
                CommandType::PCMD_Ability => {
                    let (position, facing, orientation, entity) = split_targets(targets);
                    Self::UseBattlegroupAbility(Pbgid::new(
                        action_type,
                        tick,
                        index,
                        blueprint,
                        position,
                        facing,
                        orientation,
                        entity,
                    ))
                }
                _ => panic!(
                    "a pbgid-targeted command isn't being handled here! command type {:?}",
                    action_type
                ),
            },
            ticks::CommandData::SourcedPbgidTargeted(blueprint, source_identifier, targets) => {
                match action_type {
                    CommandType::CMD_Ability => {
                        let (position, facing, orientation, entity) = split_targets(targets);
                        Self::UseAbility(SourcedAbility::new(
                            action_type,
                            tick,
                            index,
                            blueprint,
                            source_identifier,
                            position,
                            facing,
                            orientation,
                            entity,
                        ))
                    }
                    _ => panic!(
                        "a sourced pbgid-targeted command isn't being handled here! command type {:?}",
                        action_type
                    ),
                }
            }
            ticks::CommandData::Ability(source, blueprint, targets) => match action_type {
                CommandType::SCMD_Ability => {
                    let (position, facing, orientation, entity) = split_targets(targets);
                    Self::UseAbilitySquad(Ability::new(
                        action_type,
                        tick,
                        index,
                        source,
                        blueprint,
                        position,
                        facing,
                        orientation,
                        entity,
                    ))
                }
                _ => panic!(
                    "an ability command isn't being handled here! command type {:?}",
                    action_type
                ),
            },
            ticks::CommandData::Construction(blueprint, position, snapped, actual, entities) => {
                match action_type {
                    CommandType::PCMD_PlaceAndConstructEntities => {
                        Self::ConstructEntity(Construction::new(
                            action_type,
                            tick,
                            index,
                            blueprint,
                            Position::new(position[0], position[1], position[2]),
                            Position::new(snapped[0], snapped[1], snapped[2]),
                            Position::new(actual[0], actual[1], actual[2]),
                            entities,
                        ))
                    }
                    _ => panic!(
                        "a construction command isn't being handled here! command type {:?}",
                        action_type
                    ),
                }
            }
            ticks::CommandData::CameraTrack { .. } | ticks::CommandData::CameraCounts { .. } => {
                panic!(
                    "camera commands are not player commands and should have been \
                     filtered out before reaching Command conversion — see \
                     Replay::camera_tracks and Replay::camera_counts"
                )
            }
            ticks::CommandData::ResourceBonus(entries) => match action_type {
                CommandType::PCMD_AIPlayer_ResourceBonus => Self::AIResourceBonus(
                    ResourceBonus::new(action_type, tick, index, entries.into_iter().collect()),
                ),
                _ => panic!(
                    "a resource bonus command isn't being handled here! command type {:?}",
                    action_type
                ),
            },
            ticks::CommandData::BroadcastMessage(json) => match action_type {
                CommandType::PCMD_BroadcastMessage => {
                    Self::Broadcast(BroadcastMessage::new(action_type, tick, index, json))
                }
                _ => panic!(
                    "a broadcast message command isn't being handled here! command type {:?}",
                    action_type
                ),
            },
            ticks::CommandData::Unknown => {
                Self::Unknown(Unknown::new(action_type, tick, index))
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
