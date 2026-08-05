use serde::{Deserialize, Serialize};

/// Identifies who or what issued a command. Wire-level parsing lives alongside the rest
/// of the payload format in `crate::data::ticks::payload`.

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// A player-issued command (surrender, resource donation, ...).
    Player(u8),
    /// A command issued by, or targeting, a single non-squad entity (building,
    /// transport, ...).
    Entity(u32),
    /// A command issued by a single squad.
    Squad(u32),
    /// A command issued by multiple squads at once, e.g. a multi-unit selection giving
    /// a group order.
    Squads(Vec<u32>),
}
