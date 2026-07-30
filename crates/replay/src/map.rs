//! Representation of parsed map information.

use crate::data::chunks::{DataSdscChunk, MapPointData};
use serde::{Deserialize, Serialize};

/// Representation of all map-related information that can be parsed from a Company of Heroes 3
/// replay

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Map {
    filename: String,
    localized_name_id: String,
    localized_description_id: String,
    starting_positions: Vec<StartingPosition>,
    territory_points: Vec<MapPoint>,
    victory_points: Vec<MapPoint>,
}

impl Map {
    /// This is a "filename" in the sense that its structure resembles one, but it doesn't actually
    /// point to any file on the file system. The final "token" in this string (if you split by
    /// slash) generally corresponds to the map name returned by the CoH3 stats API. The string is
    /// UTF-8 encoded.
    pub fn filename(&self) -> &str {
        &self.filename
    }
    /// Entity ID that corresponds to a localization string that represents the localized name of
    /// the map. Conventionally these IDs do not change between patches, but that isn't guaranteed.
    /// The string is UTF-16 encoded.
    pub fn localized_name_id(&self) -> &str {
        &self.localized_name_id
    }
    /// Entity ID that corresponds to a localization string that represents the localized
    /// description of the map. Conventionally these IDs do not change between patches, but that
    /// isn't guaranteed. The string is UTF-16 encoded.
    pub fn localized_description_id(&self) -> &str {
        &self.localized_description_id
    }
    /// The map's starting positions, one per player slot. Coordinates are in world units, with
    /// `+x` pointing right and `+y` pointing up (i.e. angle increases counterclockwise); sorting
    /// by angle around their centroid recovers the seating order shown in-game. Empty for
    /// replays recorded before this data was written to the replay file (DATA/SDSC chunk version
    /// 3026 or earlier).
    pub fn starting_positions(&self) -> &[StartingPosition] {
        &self.starting_positions
    }
    /// All territory (capture) points on the map, including resource and victory points. Empty
    /// for replays recorded before this data was written to the replay file (DATA/SDSC chunk
    /// version 3026 or earlier).
    pub fn territory_points(&self) -> &[MapPoint] {
        &self.territory_points
    }
    /// The subset of territory points that award victory points. Empty for replays recorded
    /// before this data was written to the replay file (DATA/SDSC chunk version 3026 or
    /// earlier).
    pub fn victory_points(&self) -> &[MapPoint] {
        &self.victory_points
    }
}

/// A player's starting position on the map.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "magnus", magnus::wrap(class = "CohLib::StartingPosition"))]
pub struct StartingPosition {
    index: u32,
    name: String,
    x: f32,
    y: f32,
}

impl StartingPosition {
    /// The 0-based player slot this starting position belongs to. Corresponds to `Player::id`
    /// as used internally to key commands, and is how `Player::starting_position` resolves its
    /// value.
    pub fn index(&self) -> u32 {
        self.index
    }
    /// The 1-based label shown for this starting position in-game (e.g. `"1"` through `"8"`).
    pub fn name(&self) -> &str {
        &self.name
    }
    /// World X coordinate.
    pub fn x(&self) -> f32 {
        self.x
    }
    /// World Y coordinate.
    pub fn y(&self) -> f32 {
        self.y
    }
}

/// A territory point on the map (a resource or victory point).

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "magnus", magnus::wrap(class = "CohLib::MapPoint"))]
pub struct MapPoint {
    icon: String,
    tags: String,
    x: f32,
    y: f32,
}

impl MapPoint {
    /// Path to the icon representing this point, relative to the game's icon root. May be
    /// empty for points with no minimap icon.
    pub fn icon(&self) -> &str {
        &self.icon
    }
    /// Comma-separated list of tags describing this point (e.g.
    /// `"resource_point,territory_point,resource_point_fuel,fuel_point_low"`).
    pub fn tags(&self) -> &str {
        &self.tags
    }
    /// World X coordinate.
    pub fn x(&self) -> f32 {
        self.x
    }
    /// World Y coordinate.
    pub fn y(&self) -> f32 {
        self.y
    }
}

pub(crate) fn map_from_data(data: &DataSdscChunk) -> Map {
    Map {
        filename: data.map_file.clone(),
        localized_name_id: data.map_name.clone(),
        localized_description_id: data.map_description.clone(),
        starting_positions: data
            .starting_positions
            .iter()
            .map(starting_position_from_data)
            .collect(),
        territory_points: data
            .territory_points
            .iter()
            .map(map_point_from_data)
            .collect(),
        victory_points: data
            .victory_points
            .iter()
            .map(map_point_from_data)
            .collect(),
    }
}

fn starting_position_from_data(data: &MapPointData) -> StartingPosition {
    StartingPosition {
        index: data.owner,
        name: data.icon.clone(),
        x: data.x,
        y: data.y,
    }
}

fn map_point_from_data(data: &MapPointData) -> MapPoint {
    MapPoint {
        icon: data.icon.clone(),
        tags: data.tags.clone(),
        x: data.x,
        y: data.y,
    }
}
