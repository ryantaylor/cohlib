use crate::command_data::Source;
use serde::{Deserialize, Serialize};

/// A command format with both a source and an entity pbgid, where the source is
/// preserved in full (unlike [`super::SourcedPbgid`], which truncates it to a legacy
/// `u16`) — needed for commands whose source can legitimately be a multi-squad
/// selection.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePbgid {
    tick: u32,
    index: u32,
    source: Source,
    pbgid: u32,
}

impl SourcePbgid {
    pub(crate) fn new(tick: u32, index: u32, source: Source, pbgid: u32) -> Self {
        Self {
            tick,
            index,
            source,
            pbgid,
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
    /// Who or what issued this command.
    pub fn source(&self) -> &Source {
        &self.source
    }
    /// Internal ID that uniquely identifies entity associated with the command. This value can be
    /// matched to CoH3 attribute files in order to determine the entity in question. Note that,
    /// while rare, it is possible that this value may change between patches for the same entity.
    pub fn pbgid(&self) -> u32 {
        self.pbgid
    }
}
