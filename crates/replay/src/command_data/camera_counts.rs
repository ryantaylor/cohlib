use serde::{Deserialize, Serialize};

/// Per-player telemetry (`DCMD_COUNT`) recorded alongside [`crate::command_data::CameraTrack`]
/// — not a player-issued command, so kept out of [`crate::Player::commands`] and exposed
/// separately via [`crate::Player::camera_counts`].
///
/// The three counters' exact meaning is not confirmed — this crate doesn't have an
/// authoritative source for the DCMD wire format — but their observed behavior across
/// every replay examined during development is consistent enough to describe:
/// `counts()[0]` is identical for every player at a given [`Self::sequence`] and rises
/// monotonically over the match; `counts()[1]` is identical within a team but differs
/// between teams; `counts()[2]` is per-player and fluctuates rather than accumulating.
/// The three are always nested `counts()[2] <= counts()[1] <= counts()[0]`, which reads
/// naturally as total / team-visible / this-player-visible entity counts, but that's a
/// guess based on the shape of the data, not a confirmed interpretation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CameraCounts {
    tick: u32,
    player_id: u8,
    sequence: u32,
    counts: [u16; 3],
}

impl CameraCounts {
    pub(crate) fn new(tick: u32, player_id: u8, sequence: u32, counts: [u16; 3]) -> Self {
        Self {
            tick,
            player_id,
            sequence,
            counts,
        }
    }

    /// This value is the tick at which this record was found while parsing the replay,
    /// which represents the time in the replay it occurred. Because CoH3's engine runs
    /// at 8 ticks per second, you can divide this value by 8 to get the number of
    /// seconds since the replay began.
    pub fn tick(&self) -> u32 {
        self.tick
    }
    /// The internal ID of the player this telemetry belongs to.
    pub fn player_id(&self) -> u8 {
        self.player_id
    }
    /// A sample clock shared with [`crate::command_data::CameraTrack::sequence`] —
    /// samples from the same instant carry the same value across players.
    pub fn sequence(&self) -> u32 {
        self.sequence
    }
    /// The three not-fully-understood counters — see the type-level docs for what's
    /// been observed about them.
    pub fn counts(&self) -> [u16; 3] {
        self.counts
    }
}
