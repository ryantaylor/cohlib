use crate::command_data::{Orientation, Position, Source};
use crate::command_type::CommandType;
use serde::{Deserialize, Serialize};

/// A command format with a source and zero or more optional targeting fields (position,
/// facing, orientation, target entity). Which fields are present, if any, depends on
/// both the command type and the specific action — e.g. a plain "stop" carries none,
/// while a "move to position" carries a position. Wire data beyond what's recognized
/// here (movement commands carry additional not-yet-understood trailing bytes in some
/// cases) is intentionally left unread; see `crate::data::ticks::command`.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Targeted {
    action_type: CommandType,
    tick: u32,
    index: u32,
    source: Source,
    position: Option<Position>,
    facing: Option<f32>,
    orientation: Option<Orientation>,
    entity: Option<u32>,
}

impl Targeted {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        action_type: CommandType,
        tick: u32,
        index: u32,
        source: Source,
        position: Option<Position>,
        facing: Option<f32>,
        orientation: Option<Orientation>,
        entity: Option<u32>,
    ) -> Self {
        Self {
            action_type,
            tick,
            index,
            source,
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
    /// Who or what issued this command.
    pub fn source(&self) -> &Source {
        &self.source
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
