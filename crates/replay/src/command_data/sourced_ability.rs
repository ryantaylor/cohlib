use crate::command_data::{Orientation, Position};
use crate::command_type::CommandType;
use crate::data::ticks::value::Blueprint;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A command format used only by `CMD_Ability`, which — like [`super::Ability`] for the
/// squad-issued `SCMD_Ability` — sometimes carries an ability pbgid (with optional
/// targeting fields) and sometimes carries only targeting fields with no pbgid at all
/// (continuing or updating an already-active ability's target). Unlike `Ability`, the
/// source is a legacy `u16` identifier rather than a full [`super::Source`], matching
/// [`super::SourcedPbgid`] (which this would otherwise be identical to, if not for the
/// optional pbgid).

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct SourcedAbility {
    action_type: CommandType,
    tick: u32,
    index: u32,
    pbgid: Option<u32>,
    mod_uuid: Option<Uuid>,
    source_identifier: u16,
    position: Option<Position>,
    facing: Option<f32>,
    orientation: Option<Orientation>,
    entity: Option<u32>,
}

impl SourcedAbility {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        action_type: CommandType,
        tick: u32,
        index: u32,
        blueprint: Option<Blueprint>,
        source_identifier: u16,
        position: Option<Position>,
        facing: Option<f32>,
        orientation: Option<Orientation>,
        entity: Option<u32>,
    ) -> Self {
        Self {
            action_type,
            tick,
            index,
            pbgid: blueprint.map(|b| b.pbgid),
            mod_uuid: blueprint.and_then(|b| b.mod_uuid),
            source_identifier,
            position,
            facing,
            orientation,
            entity,
        }
    }

    /// The Relic wire command type this command was decoded from.
    pub fn action_type(&self) -> CommandType {
        self.action_type
    }
    /// This value is the tick at which the command was found while parsing the replay, which
    /// represents the time in the replay at which it was executed. Because CoH3's engine runs at 8
    /// ticks per second, you can divide this value by 8 to get the number of seconds since the
    /// replay began, which will tell you when this command was executed.
    pub fn tick(&self) -> u32 {
        self.tick
    }
    /// This value is the index of the command relative to the player who issued the command.
    /// Indexes start at 1 and increment on every player-issued command, which means you should be
    /// able to look at the maximum index value of the commands associated with a player to
    /// determine how many commands that player issued in a given game.
    pub fn index(&self) -> u32 {
        self.index
    }
    /// Internal ID identifying the ability, if this command carried one — absent when
    /// this command is continuing or updating the target of an already-active ability.
    ///
    /// This is only globally unique when `Self::mod_uuid` is `None`. Otherwise the ID is
    /// scoped to that mod's content and will not match base game attribute data.
    pub fn pbgid(&self) -> Option<u32> {
        self.pbgid
    }
    /// The UUID of the mod content pack defining the blueprint `Self::pbgid` refers to,
    /// or `None` when it is base game content or this command carried no blueprint at
    /// all. Note that this is not the same value as `Replay::mod_uuid`, which identifies
    /// the mod the match itself was played under.
    pub fn mod_uuid(&self) -> Option<Uuid> {
        self.mod_uuid
    }
    /// This value corresponds to the internal identifier given by the game engine to the entity
    /// that is the source of the command. If you know the identifier for a given entity, you can
    /// use this value to link this command to that entity.
    pub fn source_identifier(&self) -> u16 {
        self.source_identifier
    }
    /// The target world-space position, if this command carried one.
    pub fn position(&self) -> Option<Position> {
        self.position
    }
    /// The target facing angle in radians, if this command carried one.
    pub fn facing(&self) -> Option<f32> {
        self.facing
    }
    /// The target orientation, if this command carried one.
    pub fn orientation(&self) -> Option<Orientation> {
        self.orientation
    }
    /// The internal engine ID of the targeted entity, if this command carried one.
    pub fn entity(&self) -> Option<u32> {
        self.entity
    }
}
