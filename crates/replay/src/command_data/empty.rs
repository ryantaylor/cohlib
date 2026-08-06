use crate::command_type::CommandType;
use serde::{Deserialize, Serialize};

/// An empty command format with no additional context.

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Empty {
    action_type: CommandType,
    tick: u32,
    index: u32,
}

impl Empty {
    pub(crate) fn new(action_type: CommandType, tick: u32, index: u32) -> Self {
        Self {
            action_type,
            tick,
            index,
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
}
