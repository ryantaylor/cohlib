use crate::command_data::{Orientation, Position};
use crate::data::ticks::value::Blueprint;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A simple command format that contains an entity pbgid, and optionally targeting
/// fields (position, facing, orientation, target entity) for the handful of command
/// types whose payload carries them — `None` for commands whose payload doesn't.

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Pbgid {
    tick: u32,
    index: u32,
    pbgid: u32,
    mod_uuid: Option<Uuid>,
    position: Option<Position>,
    facing: Option<f32>,
    orientation: Option<Orientation>,
    entity: Option<u32>,
}

impl Pbgid {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        tick: u32,
        index: u32,
        blueprint: Blueprint,
        position: Option<Position>,
        facing: Option<f32>,
        orientation: Option<Orientation>,
        entity: Option<u32>,
    ) -> Self {
        Self {
            tick,
            index,
            pbgid: blueprint.pbgid,
            mod_uuid: blueprint.mod_uuid,
            position,
            facing,
            orientation,
            entity,
        }
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
