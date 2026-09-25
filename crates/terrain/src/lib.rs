//! Terrain grids for the Zoomhack certificate (hack detection design doc §8).
//!
//! Given `ScenariosMP.sga` and `ReferenceAttributes.sga`, produces, per
//! multiplayer map, a coarse grid of conservative terrain-height upper bounds
//! (dilated by the camera's maximum horizontal reach, then block-downsampled)
//! plus the camera tuning constants that reach is derived from. A camera
//! sample's distance from the terrain can then be lower-bounded without ever
//! re-reading the heightfield: `D >= (eye_y - lookup(x, z)) / sin(pitch)`.
//!
//! Ported faithfully from `analysis/zoomhack/terrain/05_coarse_grid.py`
//! (cohdb/next) — see each module's doc comment for the depot format details
//! and the one intentional difference (map keys are emitted as the full
//! normalized scenario path, not the research script's short basename, so
//! they join directly against [`data::GameData::scenarios`]-style keys).

pub mod camera;
mod error;
pub mod grid;
pub mod heightfield;

pub use camera::CameraTuning;
pub use error::Error;
pub use grid::{GridCell, MapMeta, BLOCK};
pub use heightfield::{Heightfield, WorldExtent};

use sga::ArchiveEntry;
use sha2::{Digest, Sha256};

/// One map's terrain grid, keyed the same way [`data::GameData::scenarios`]
/// is: the archive path with the `.scenario` extension stripped, forward
/// slashes (e.g. `scenarios/multiplayer/cliff_crossing_2p/cliff_crossing_2p`).
pub struct TerrainGrid {
    pub map: String,
    pub cells: Vec<GridCell>,
    pub meta: MapMeta,
    /// Short content hash of the raw plane-1 heightfield, so a build whose
    /// grid is byte-identical to a previous build's can be detected without
    /// re-diffing the CSV (used by the patch pipeline's "only commit when
    /// content differs" rule — see design doc §8.2; that rule itself lives in
    /// the patch pipeline, not here).
    pub heightfield_hash: String,
}

/// Extracts every multiplayer map's [`TerrainGrid`] from `ScenariosMP.sga`'s
/// entries, dilating by `distance_max` (the isometric camera's `tuning`
/// group's max reach — the radius is never hard-coded, see [`grid`]).
///
/// A map whose `.scenario` can't be parsed is skipped with its error recorded
/// against its key, rather than failing the whole extraction — one malformed
/// or unexpected file (a non-multiplayer `.scenario`, a future format change)
/// shouldn't block every other map's grid.
pub fn extract_terrain_grids(
    entries: &[ArchiveEntry],
    distance_max: f32,
) -> Vec<(String, Result<TerrainGrid, Error>)> {
    entries
        .iter()
        .filter(|e| e.path.ends_with(".scenario"))
        .map(|e| {
            let map = e.path.strip_suffix(".scenario").unwrap().to_string();
            let result = extract_one(&e.bytes, distance_max);
            (
                map.clone(),
                result.map(|(cells, meta, hash)| TerrainGrid {
                    map: map.clone(),
                    cells,
                    meta,
                    heightfield_hash: hash,
                }),
            )
        })
        .collect()
}

fn extract_one(bytes: &[u8], distance_max: f32) -> Result<(Vec<GridCell>, MapMeta, String), Error> {
    let hf = heightfield::parse_heightfield(bytes)?;
    let extent = heightfield::parse_world_extent(bytes, hf.w, hf.h)?;
    let meta = grid::compute_meta(&hf, extent, BLOCK);
    let cells = grid::dilate_and_downsample(&hf, meta.cx, meta.cz, distance_max, BLOCK);
    let hash = heightfield_hash(&hf.height);
    Ok((cells, meta, hash))
}

/// Finds `instances/camera/default_multiplayer.xml` in `ReferenceAttributes.sga`'s
/// entries and parses the isometric module's tuning group.
pub fn extract_camera_tuning(entries: &[ArchiveEntry]) -> Result<CameraTuning, Error> {
    let entry = entries
        .iter()
        .find(|e| e.path == "instances/camera/default_multiplayer.xml")
        .ok_or_else(|| {
            Error::Parse("instances/camera/default_multiplayer.xml not found in archive".into())
        })?;
    camera::parse_camera_tuning(&entry.bytes)
}

/// 16 hex chars (64 bits) of SHA-256 over plane 1's raw little-endian float
/// bytes — same short-hash convention as [`data::Scenario`]'s content
/// addressing (`crates/cli/src/main.rs`'s `scenario_hash`).
fn heightfield_hash(height: &[f32]) -> String {
    let mut h = Sha256::new();
    for v in height {
        h.update(v.to_le_bytes());
    }
    format!("{:x}", h.finalize())[..16].to_string()
}
