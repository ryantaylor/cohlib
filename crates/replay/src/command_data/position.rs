use serde::{Deserialize, Serialize};

/// A 3D world-space position.

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    x: f32,
    y: f32,
    z: f32,
}

impl Position {
    pub(crate) fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
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
}
