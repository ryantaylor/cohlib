//! Reads a multiplayer `.scenario` file's terrain heightfield and world extent.
//!
//! Ground truth verified directly against real depot `.scenario` files (not the
//! prose description alone): `SDSC` is a **top-level** `DATA` chunk (sibling of
//! `SCEN`, not nested under it), and the heightfield lives at
//! `SCEN/GEWD/TERR/HITE/HFLD`, also a `DATA` leaf.

use crate::Error;
use scenario::chunky::{find, find_path, parse_chunky};

/// Plane 1 (terrain height, world units) of a map's `HFLD` chunk, plus its grid
/// dimensions. Planes 2 and 3 are not read — see the module extraction notes.
pub struct Heightfield {
    pub w: i32,
    pub h: i32,
    /// Row-major, `height[j * w + i]`, plane 1 only.
    pub height: Vec<f32>,
}

/// World-space size of the map, in world units, from `SDSC` dwords 22/23
/// (24.8 fixed point). A handful of maps store `0` there; when one axis is
/// zero, it's derived from the other assuming square cells, matching the
/// research script's fallback exactly.
#[derive(Debug, Clone, Copy)]
pub struct WorldExtent {
    pub x: f64,
    pub z: f64,
}

const HFLD_PATH: &[(&[u8; 4], &[u8; 4])] = &[
    (b"FOLD", b"SCEN"),
    (b"FOLD", b"GEWD"),
    (b"FOLD", b"TERR"),
    (b"FOLD", b"HITE"),
    (b"DATA", b"HFLD"),
];

pub fn parse_heightfield(bytes: &[u8]) -> Result<Heightfield, Error> {
    let payload = find_path(bytes, HFLD_PATH)?;
    if payload.len() < 8 {
        return Err(Error::Parse(
            "HFLD payload shorter than its w/h header".into(),
        ));
    }
    let w = i32::from_le_bytes(payload[0..4].try_into().unwrap());
    let h = i32::from_le_bytes(payload[4..8].try_into().unwrap());
    if w <= 0 || h <= 0 {
        return Err(Error::Parse(format!(
            "HFLD reports non-positive dims {w}x{h}"
        )));
    }
    let count = w as usize * h as usize;
    let plane1_len = count * 4;
    if payload.len() < 8 + plane1_len {
        return Err(Error::Parse(format!(
            "HFLD payload too short for {w}x{h} plane 1: have {} bytes, need {}",
            payload.len(),
            8 + plane1_len
        )));
    }
    let mut height = Vec::with_capacity(count);
    for chunk in payload[8..8 + plane1_len].as_chunks::<4>().0 {
        height.push(f32::from_le_bytes(*chunk));
    }
    Ok(Heightfield { w, h, height })
}

/// Reads world extent from the top-level `SDSC` chunk's dwords 22/23, with the
/// research script's zero-axis fallback (assume square cells from the other
/// axis) applied using `w`/`h` from the matching [`Heightfield`].
pub fn parse_world_extent(bytes: &[u8], w: i32, h: i32) -> Result<WorldExtent, Error> {
    let chunks = parse_chunky(bytes)?;
    let sdsc = find(&chunks, b"DATA", b"SDSC")
        .ok_or_else(|| Error::Parse("no top-level SDSC chunk".into()))?;
    let offset = 22 * 4;
    if sdsc.data.len() < offset + 8 {
        return Err(Error::Parse(
            "SDSC payload too short for dwords 22/23".into(),
        ));
    }
    let raw_x = i32::from_le_bytes(sdsc.data[offset..offset + 4].try_into().unwrap());
    let raw_z = i32::from_le_bytes(sdsc.data[offset + 4..offset + 8].try_into().unwrap());
    let mut x = raw_x as f64 / 256.0;
    let mut z = raw_z as f64 / 256.0;
    if x <= 0.0 {
        x = if z > 0.0 && h > 1 {
            (w - 1) as f64 * (z / (h - 1) as f64)
        } else {
            (w - 1) as f64 * 0.5
        };
    }
    if z <= 0.0 {
        z = (h - 1) as f64 * (x / (w - 1) as f64);
    }
    Ok(WorldExtent { x, z })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_chunk(kind: &[u8; 4], id: &[u8; 4], version: u32, data: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(kind);
        buf.extend_from_slice(id);
        buf.extend_from_slice(&version.to_le_bytes());
        buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(data);
        buf
    }

    fn build_chunky(top_level: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"Relic Chunky\r\n\x1a\0");
        buf.extend_from_slice(&4u32.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(top_level);
        buf
    }

    fn hfld_payload(w: i32, h: i32, height: &[f32]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&w.to_le_bytes());
        data.extend_from_slice(&h.to_le_bytes());
        for v in height {
            data.extend_from_slice(&v.to_le_bytes());
        }
        // plane 2 + plane 3, unread but present in real files.
        for _ in 0..(2 * height.len()) {
            data.extend_from_slice(&0f32.to_le_bytes());
        }
        data
    }

    fn sdsc_payload(world_x: i32, world_z: i32) -> Vec<u8> {
        let mut data = vec![0u8; 22 * 4];
        data.extend_from_slice(&world_x.to_le_bytes());
        data.extend_from_slice(&world_z.to_le_bytes());
        data
    }

    fn sample_scenario(w: i32, h: i32, height: &[f32], world_x: i32, world_z: i32) -> Vec<u8> {
        let hfld = build_chunk(b"DATA", b"HFLD", 3001, &hfld_payload(w, h, height));
        let hite = build_chunk(b"FOLD", b"HITE", 3000, &hfld);
        let terr = build_chunk(b"FOLD", b"TERR", 3001, &hite);
        let gewd = build_chunk(b"FOLD", b"GEWD", 3005, &terr);
        let scen = build_chunk(b"FOLD", b"SCEN", 3000, &gewd);
        let sdsc = build_chunk(b"DATA", b"SDSC", 3029, &sdsc_payload(world_x, world_z));
        let mut top = sdsc;
        top.extend_from_slice(&scen);
        build_chunky(&top)
    }

    #[test]
    fn parses_heightfield_plane_1() {
        let height = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let bytes = sample_scenario(3, 2, &height, 90112, 98304);
        let hf = parse_heightfield(&bytes).unwrap();
        assert_eq!(hf.w, 3);
        assert_eq!(hf.h, 2);
        assert_eq!(hf.height, height);
    }

    #[test]
    fn parses_world_extent_matching_cliff_crossing_2p() {
        // Real values from the depot: cliff_crossing_2p is 352 x 384 world units.
        let height = vec![0.0; 6];
        let bytes = sample_scenario(3, 2, &height, 90112, 98304);
        let extent = parse_world_extent(&bytes, 3, 2).unwrap();
        assert!((extent.x - 352.0).abs() < 1e-9);
        assert!((extent.z - 384.0).abs() < 1e-9);
    }

    #[test]
    fn falls_back_to_square_cells_when_one_axis_is_zero() {
        let height = vec![0.0; 6];
        let bytes = sample_scenario(3, 2, &height, 0, 98304);
        let extent = parse_world_extent(&bytes, 3, 2).unwrap();
        // x <= 0 -> x = (w-1) * (z / (h-1)) = 2 * (384/1) = 768
        assert!((extent.x - 768.0).abs() < 1e-9);
        assert!((extent.z - 384.0).abs() < 1e-9);
    }
}
