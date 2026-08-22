use crate::command_data::{Orientation, Position, Source};
use crate::command_type::CommandType;
use crate::data::ticks::value::Blueprint;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A command format with an entity pbgid and a source, and optionally targeting fields
/// (position, facing, orientation, target entity) for the handful of command types
/// whose payload carries them — `None` for commands whose payload doesn't. Carries both
/// the full `Source` and the legacy truncated `u16` identifier this crate has exposed
/// for these command types since before `Source` existed — new code should prefer
/// `Self::source`.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcedPbgid {
    action_type: CommandType,
    tick: u32,
    index: u32,
    pbgid: u32,
    mod_uuid: Option<Uuid>,
    source: Source,
    source_identifier: u16,
    position: Option<Position>,
    facing: Option<f32>,
    orientation: Option<Orientation>,
    entity: Option<u32>,
}

impl SourcedPbgid {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        action_type: CommandType,
        tick: u32,
        index: u32,
        blueprint: Blueprint,
        source: Source,
        position: Option<Position>,
        facing: Option<f32>,
        orientation: Option<Orientation>,
        entity: Option<u32>,
    ) -> Self {
        let source_identifier = source.legacy_identifier();
        Self {
            action_type,
            tick,
            index,
            pbgid: blueprint.pbgid,
            mod_uuid: blueprint.mod_uuid,
            source,
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
    /// Internal ID that uniquely identifies entity associated with the command. This value can be
    /// matched to CoH3 attribute files in order to determine the entity in question. Note that,
    /// while rare, it is possible that this value may change between patches for the same entity.
    ///
    /// This is only globally unique when `Self::mod_uuid` is `None`. Otherwise the ID is
    /// scoped to that mod's content and will not match base game attribute data.
    pub fn pbgid(&self) -> u32 {
        self.pbgid
    }
    /// The UUID of the mod content pack defining the blueprint `Self::pbgid` refers to,
    /// or `None` when it is base game content. Note that this is not the same value as
    /// `Replay::mod_uuid`, which identifies the mod the match itself was played under.
    pub fn mod_uuid(&self) -> Option<Uuid> {
        self.mod_uuid
    }
    /// Who or what issued this command.
    pub fn source(&self) -> &Source {
        &self.source
    }
    /// This value corresponds to the internal identifier given by the game engine to the entity
    /// that is the source of the command. If you know the identifier for a given entity, you can
    /// use this value to link this command to that entity. Kept for backward compatibility —
    /// new code should prefer `Self::source`.
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
