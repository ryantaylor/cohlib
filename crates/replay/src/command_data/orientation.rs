use serde::{Deserialize, Serialize};

/// A 3D orientation, as carried by a command's targeting parameters (e.g. "face this
/// direction"). The wire format encodes this as four `f32`s; despite the name, this
/// isn't a quaternion — `w` is exactly `1.0` in every occurrence examined during
/// development, while `x, y, z` form a unit vector — so it's a plain direction with a
/// constant trailing component, not a rotation. Contrast with the camera's orientation
/// (`CameraTrack::orientation`), which is a real quaternion with a varying `w`.

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct Orientation {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

impl Orientation {
    pub(crate) fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn x(&self) -> f32 {
        self.x
    }
    pub fn y(&self) -> f32 {
        self.y
    }
    pub fn z(&self) -> f32 {
        self.z
    }
    pub fn w(&self) -> f32 {
        self.w
    }
}
