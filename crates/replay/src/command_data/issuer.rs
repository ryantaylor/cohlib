use serde::{Deserialize, Serialize};

/// Who issued a command, read from its payload header.

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Issuer {
    /// The player in the seat.
    Human,
    /// The AI. `takeover_tick` is the tick it took the seat over from a player who left,
    /// or `None` for an AI that held the seat from the start.
    Ai { takeover_tick: Option<u32> },
    /// The game itself rather than anyone in the seat, e.g. a surrender after a player
    /// leaves or a battlegroup granted on the player's behalf.
    Engine,
}
