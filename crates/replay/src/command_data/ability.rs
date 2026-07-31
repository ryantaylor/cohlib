use crate::command_data::{Orientation, Position, Source};
use serde::{Deserialize, Serialize};

/// A command format used only by `SCMD_Ability`, which — uniquely among decoded command
/// types — sometimes carries an ability pbgid (with optional targeting fields) and
/// sometimes carries only targeting fields with no pbgid at all (best understood as
/// continuing or updating an already-active ability's target).

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ability {
    tick: u32,
    index: u32,
    source: Source,
    pbgid: Option<u32>,
    position: Option<Position>,
    facing: Option<f32>,
    orientation: Option<Orientation>,
    entity: Option<u32>,
}

impl Ability {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        tick: u32,
        index: u32,
        source: Source,
        pbgid: Option<u32>,
        position: Option<Position>,
        facing: Option<f32>,
        orientation: Option<Orientation>,
        entity: Option<u32>,
    ) -> Self {
        Self {
            tick,
            index,
            source,
            pbgid,
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
    /// Who or what issued this command.
    pub fn source(&self) -> &Source {
        &self.source
    }
    /// Internal ID identifying the ability, if this command carried one — absent when
    /// this command is continuing or updating the target of an already-active ability.
    pub fn pbgid(&self) -> Option<u32> {
        self.pbgid
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
