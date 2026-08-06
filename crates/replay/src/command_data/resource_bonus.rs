use crate::command_type::CommandType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// `PCMD_AIPlayer_ResourceBonus`'s payload: a map from resource name (e.g.
/// `"manpower"`, `"fuel"`, `"requisition"`) to a bonus multiplier (`1.0` is no bonus).

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceBonus {
    action_type: CommandType,
    tick: u32,
    index: u32,
    values: HashMap<String, f32>,
}

impl ResourceBonus {
    pub(crate) fn new(
        action_type: CommandType,
        tick: u32,
        index: u32,
        values: HashMap<String, f32>,
    ) -> Self {
        Self {
            action_type,
            tick,
            index,
            values,
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
    /// The resource bonus multipliers, keyed by resource name.
    pub fn values(&self) -> &HashMap<String, f32> {
        &self.values
    }
}
