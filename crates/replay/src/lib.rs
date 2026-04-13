// replay module — ported from vault (https://github.com/ryantaylor/vault)
// Parses Company of Heroes 3 binary replay files (.rec format).

mod command;
pub mod command_data;
mod command_type;
mod data;
mod errors;
mod map;
mod message;
mod parsed;
mod player;

pub mod error;
pub use error::Error;

pub use command::Command;
pub use command_type::CommandType;
pub use map::Map;
pub use message::Message;
pub use parsed::{GameType, Replay};
pub use player::{Faction, Player, Team};

/// Parse a CoH3 replay from raw bytes.
///
/// Returns a [`Replay`] on success. Any parse failure returns an [`Error`].
pub fn parse_replay(bytes: &[u8]) -> Result<Replay, Error> {
    Replay::from_bytes(bytes).map_err(|e| Error::Replay(e.to_string()))
}
