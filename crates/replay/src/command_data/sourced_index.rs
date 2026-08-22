use crate::command_data::Source;
use crate::command_type::CommandType;
use serde::{Deserialize, Serialize};

/// A command format with both a source and a queue index. Carries both the full
/// `Source` and the legacy truncated `u16` identifier this crate has exposed for these
/// command types since before `Source` existed — new code should prefer `Self::source`.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcedIndex {
    action_type: CommandType,
    tick: u32,
    index: u32,
    source: Source,
    source_identifier: u16,
    queue_index: u32,
}

impl SourcedIndex {
    pub(crate) fn new(
        action_type: CommandType,
        tick: u32,
        index: u32,
        source: Source,
        queue_index: u32,
    ) -> Self {
        let source_identifier = source.legacy_identifier();
        Self {
            action_type,
            tick,
            index,
            source,
            source_identifier,
            queue_index,
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
    /// This value corresponds to the internal identifier given by the game engine to the entity
    /// that is the source of the command. If you know the identifier for a given entity, you can
    /// use this value to link this command to that entity. Kept for backward compatibility —
    /// new code should prefer `Self::source`.
    pub fn source_identifier(&self) -> u16 {
        self.source_identifier
    }
    /// The index of the position in the source entity's build queue that this command corresponds
    /// to. Usually used with build and cancellation commands, every time a build command is issued,
    /// the command is added to the source structure's build queue and given an index. These indexes
    /// start at 1 and increase by 1 every time a new build command is issued. This value can be used
    /// alongside source identifier to determine which specific build command is being cancelled.
    pub fn queue_index(&self) -> u32 {
        self.queue_index
    }
}
