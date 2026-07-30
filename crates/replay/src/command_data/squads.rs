use crate::command_data::Source;
use serde::{Deserialize, Serialize};

/// A command format carrying only a source — no target, entity reference, or upgrade.
/// Despite the name (the most common case: a multi-squad selection issuing a group
/// order), the source can be any [`Source`] variant — e.g. `PCMD_Surrender` and
/// `PCMD_TentativeUpgradeRemoveAll` are always issued by a `Source::Player`.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Squads {
    tick: u32,
    index: u32,
    source: Source,
}

impl Squads {
    pub(crate) fn new(tick: u32, index: u32, source: Source) -> Self {
        Self {
            tick,
            index,
            source,
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
}
