use crate::command_data::Position;
use serde::{Deserialize, Serialize};

/// Per-player camera telemetry (`DCMD_CameraTrack`) recorded throughout the match — not
/// a player-issued command, so it's kept out of [`crate::Player::commands`] and exposed
/// separately via [`crate::Player::camera_tracks`]. These make up the large majority of
/// records in a replay's command stream (roughly three quarters, in replays examined
/// during development).
///
/// A sample carries the camera's eye position and a roll-free orientation (the game's
/// RTS camera never banks). [`Self::position`] is the eye itself — typically well above
/// and behind whatever the player is actually looking at — so a "where is this player's
/// attention" query wants [`Self::focus_at_ground_height`], which projects the view
/// direction down onto an assumed ground plane.
///
/// The wire format encodes `x`/`z` as 16-bit fixed point, representable up to
/// ±327.67 world units — see [`Self::position`] on what happens past that limit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraTrack {
    tick: u32,
    player_id: u8,
    sequence: u32,
    raw_position: [i16; 3],
    position: Position,
    orientation: [f32; 4],
}

impl CameraTrack {
    pub(crate) fn new(
        tick: u32,
        player_id: u8,
        sequence: u32,
        raw_position: [i16; 3],
        raw_orientation: [i16; 4],
    ) -> Self {
        let [x, altitude, ground_y] = raw_position;
        let position = Position::new(
            x as f32 / 100.0,
            altitude as f32 / 100.0,
            ground_y as f32 / 100.0,
        );
        let orientation = raw_orientation.map(|v| v as f32 / 16384.0);
        Self {
            tick,
            player_id,
            sequence,
            raw_position,
            position,
            orientation,
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
    /// A monotonically increasing sample counter, private to how the recording
    /// client scheduled camera capture. It correlates with [`Self::tick`] (both
    /// increase together over the match) but isn't derivable from it: concurrent
    /// players' samples don't necessarily report the same sequence within the same
    /// tick, and consecutive samples' sequence deltas vary. Exposed for callers who
    /// want a finer-grained ordering than [`Self::tick`] provides within the same
    /// tick.
    pub fn sequence(&self) -> u32 {
        self.sequence
    }
    /// The camera's eye position in world space (`x`/`z` are the ground-plane axes, in
    /// the same convention as [`crate::StartingPosition`]; `y` is altitude) —
    /// `raw_position()` scaled by `1/100`.
    ///
    /// The wire format encodes `x`/`z` as signed 16-bit fixed point, representable up
    /// to ±327.67 world units. On a map wide enough for a player's camera to actually
    /// reach past that limit, the value wraps rather than growing further — this crate
    /// doesn't attempt to reconstruct the true coordinate in that case (an earlier
    /// attempt at that, using continuity between samples, proved unreliable: real fast
    /// camera pans — e.g. clicking the minimap to jump across the map — are common
    /// enough, and large enough, to be indistinguishable from a wraparound using
    /// position data alone). No fixture examined during development exercises this:
    /// every replay's camera stays within ±327.67 on both axes, even on the largest
    /// maps, so this is a latent limit rather than an observed problem.
    pub fn position(&self) -> Position {
        self.position
    }
    /// The eye position exactly as recorded on the wire (`[x, altitude, z] × 100`) —
    /// see [`Self::position`] for the scaled value and its wraparound caveat.
    pub fn raw_position(&self) -> [i16; 3] {
        self.raw_position
    }
    /// The camera's orientation as a unit quaternion `[w, x, y, z]`. Always roll-free
    /// (`w·z ≈ -x·y`), which is what a top-down RTS camera should be.
    pub fn orientation(&self) -> [f32; 4] {
        self.orientation
    }
    /// The camera's pitch (rotation about the world X axis) in radians, derived from
    /// [`Self::orientation`]. `0` is level with the horizon; the default camera pitches
    /// down at roughly 43°.
    pub fn pitch(&self) -> f32 {
        let [w, x, _, _] = self.orientation;
        2.0 * x.atan2(w)
    }
    /// The camera's yaw (rotation about the world Y/up axis) in radians, derived from
    /// [`Self::orientation`]. `0` faces `+z`.
    pub fn yaw(&self) -> f32 {
        let [w, _, y, _] = self.orientation;
        2.0 * y.atan2(w)
    }
    /// The unit vector the camera is looking along, derived from [`Self::orientation`].
    pub fn forward(&self) -> [f32; 3] {
        rotate(self.orientation, [0.0, 0.0, 1.0])
    }
    /// The `(x, z)` ground-plane point (in the same coordinates as
    /// [`crate::StartingPosition`]) that the camera's view direction intersects a
    /// horizontal plane at world height `ground` — an approximation of what the player
    /// is actually looking at, since the replay doesn't record terrain elevation.
    /// `ground = 0.0` is a reasonable default for most maps. Returns `None` if the
    /// camera isn't pointed downward (shouldn't happen for the game's fixed-pitch RTS
    /// camera, but the math only makes sense when it does).
    pub fn focus_at_ground_height(&self, ground: f32) -> Option<(f32, f32)> {
        let forward = self.forward();
        if forward[1] >= -1e-6 {
            return None;
        }
        let t = (self.position.y() - ground) / -forward[1];
        Some((
            self.position.x() + forward[0] * t,
            self.position.z() + forward[2] * t,
        ))
    }
}

/// Rotates `v` by unit quaternion `q = [w, x, y, z]`.
fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let [w, qx, qy, qz] = q;
    let [vx, vy, vz] = v;
    let tx = 2.0 * (qy * vz - qz * vy);
    let ty = 2.0 * (qz * vx - qx * vz);
    let tz = 2.0 * (qx * vy - qy * vx);
    [
        vx + w * tx + (qy * tz - qz * ty),
        vy + w * ty + (qz * tx - qx * tz),
        vz + w * tz + (qx * ty - qy * tx),
    ]
}
