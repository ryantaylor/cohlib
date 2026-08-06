use crate::command_data::Position;
use crate::command_type::CommandType;
use crate::data::ticks::value::Blueprint;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A command format used by `PCMD_PlaceAndConstructEntities`: an entity pbgid, three
/// positions, and the internal engine IDs of the entities the game spawned as a result.
///
/// The three positions consistently differ only slightly from one another (position and
/// snapped position share very similar coordinates; final position sometimes differs
/// more) — read as the raw cursor position, the position snapped to the placement grid,
/// and the final validated placement, though the exact semantic distinction between
/// them hasn't been independently confirmed against an authoritative source.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Construction {
    action_type: CommandType,
    tick: u32,
    index: u32,
    pbgid: u32,
    mod_uuid: Option<Uuid>,
    position: Position,
    snapped_position: Position,
    final_position: Position,
    entities: Vec<u32>,
}

impl Construction {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        action_type: CommandType,
        tick: u32,
        index: u32,
        blueprint: Blueprint,
        position: Position,
        snapped_position: Position,
        final_position: Position,
        entities: Vec<u32>,
    ) -> Self {
        Self {
            action_type,
            tick,
            index,
            pbgid: blueprint.pbgid,
            mod_uuid: blueprint.mod_uuid,
            position,
            snapped_position,
            final_position,
            entities,
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
    /// The raw cursor position where placement was requested. See the type-level docs
    /// on the uncertainty around the exact distinction between the three positions.
    pub fn position(&self) -> Position {
        self.position
    }
    /// The position snapped to the placement grid.
    pub fn snapped_position(&self) -> Position {
        self.snapped_position
    }
    /// The final, validated placement position.
    pub fn final_position(&self) -> Position {
        self.final_position
    }
    /// The internal engine IDs of the entities spawned as a result of this command
    /// (e.g. the constructed building, and possibly other associated entities).
    pub fn entities(&self) -> &[u32] {
        &self.entities
    }
}
