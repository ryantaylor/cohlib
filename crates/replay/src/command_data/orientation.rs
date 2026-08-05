use serde::{Deserialize, Serialize};

/// A 3D orientation. The wire format encodes this as four `f32`s; the values observed
/// (a unit-length-like `x, y, z, w` tuple) strongly suggest a quaternion, but this
/// hasn't been confirmed against an authoritative source.

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
