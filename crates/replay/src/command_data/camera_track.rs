use serde::{Deserialize, Serialize};

/// Per-player camera telemetry (`DCMD_CameraTrack` and `DCMD_COUNT`) recorded
/// throughout the match — not a player-issued command, so it's kept out of
/// [`crate::Player::commands`] and exposed separately via
/// [`crate::Player::camera_tracks`]. These make up the large majority of records in a
/// replay's command stream (roughly three quarters, in replays examined during
/// development).
///
/// The payload always begins with five `0xFF` bytes followed by a mix of constant and
/// slowly-varying fields (likely camera position/orientation and a timestamp), but the
/// exact internal layout beyond that marker hasn't been decoded — `data()` exposes the
/// raw bytes that follow it.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraTrack {
    tick: u32,
    player_id: u8,
    data: Vec<u8>,
}

impl CameraTrack {
    pub(crate) fn new(tick: u32, player_id: u8, data: Vec<u8>) -> Self {
        Self {
            tick,
            player_id,
            data,
        }
    }

    /// This value is the tick at which this record was found while parsing the replay,
    /// which represents the time in the replay it occurred. Because CoH3's engine runs
    /// at 8 ticks per second, you can divide this value by 8 to get the number of
    /// seconds since the replay began.
    pub fn tick(&self) -> u32 {
        self.tick
    }
    /// The internal ID of the player this camera telemetry belongs to.
    pub fn player_id(&self) -> u8 {
        self.player_id
    }
    /// The raw bytes following the `0xFF` marker. See the type-level docs on why this
    /// crate doesn't decode them further yet.
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}
