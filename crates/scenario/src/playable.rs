//! Parser for `<map>_softmapedge.override` — the playable-area mask.
//!
//! Unlike `territory.rs`/`layers.rs`, this format and its use here are this
//! crate's own work, not derived from CoH3Stats (which estimates playable area
//! from the bounding box of resource points instead — see `Scenario::playable_area`'s
//! doc comment for why we use the mask instead).
//!
//! Chunk path: `FOLDOLYR/FOLDODAT/FOLDDATA/FOLDIACT/DATADATA` (v3002). Payload,
//! confirmed against `castello_8p_softmapedge.override`:
//!
//! ```text
//! u32 (=1), u32 (=0), u32 width, u32 height, u8[width*height] mask
//! ```
//!
//! Mask semantics were confirmed empirically, not assumed: rendering the mask
//! as an image shows a clean octagonal playable region inset from a uniform
//! border, and every resource/victory/starting point position on every
//! scenario checked falls on a `0` cell — so `0` is playable ground, `1` is the
//! unplayable border/edge margin every map has around its declared world size.

use crate::Error;
use data::Rect;

pub struct Mask {
    pub width: u32,
    pub height: u32,
    /// Row-major, `0` = playable, `1` = unplayable border.
    data: Vec<u8>,
}

impl Mask {
    /// World-space bounding box of all playable (`0`) cells, or `None` if the
    /// mask has no playable cells at all (shouldn't happen for a real scenario).
    pub fn bounding_rect(&self) -> Option<Rect> {
        let (w, h) = (self.width as usize, self.height as usize);
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (w, 0usize, h, 0usize);
        let mut found = false;
        for y in 0..h {
            let row = &self.data[y * w..(y + 1) * w];
            if let (Some(first), Some(last)) = (
                row.iter().position(|&v| v == 0),
                row.iter().rposition(|&v| v == 0),
            ) {
                found = true;
                min_x = min_x.min(first);
                max_x = max_x.max(last);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }
        if !found {
            return None;
        }
        let half_w = self.width as f32 / 2.0;
        let half_h = self.height as f32 / 2.0;
        Some(Rect {
            min_x: min_x as f32 - half_w,
            max_x: (max_x + 1) as f32 - half_w,
            min_y: min_y as f32 - half_h,
            max_y: (max_y + 1) as f32 - half_h,
        })
    }
}

pub fn parse_playable_area(bytes: &[u8]) -> Result<Mask, Error> {
    let payload = crate::chunky::find_path(
        bytes,
        &[
            (b"FOLD", b"OLYR"),
            (b"FOLD", b"ODAT"),
            (b"FOLD", b"DATA"),
            (b"FOLD", b"IACT"),
            (b"DATA", b"DATA"),
        ],
    )?;
    if payload.len() < 16 {
        return Err(Error::Scenario("softmapedge payload too short".into()));
    }
    let width = u32::from_le_bytes(payload[8..12].try_into().unwrap());
    let height = u32::from_le_bytes(payload[12..16].try_into().unwrap());
    let expected = 16 + (width as usize) * (height as usize);
    if payload.len() < expected {
        return Err(Error::Scenario("softmapedge mask truncated".into()));
    }
    let data = payload[16..expected].to_vec();
    Ok(Mask {
        width,
        height,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_softmapedge(width: u32, height: u32, mask: &[u8]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&width.to_le_bytes());
        payload.extend_from_slice(&height.to_le_bytes());
        payload.extend_from_slice(mask);

        fn chunk(kind: &[u8; 4], id: &[u8; 4], version: u32, data: &[u8]) -> Vec<u8> {
            let mut buf = Vec::new();
            buf.extend_from_slice(kind);
            buf.extend_from_slice(id);
            buf.extend_from_slice(&version.to_le_bytes());
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            buf.extend_from_slice(&0u32.to_le_bytes());
            buf.extend_from_slice(data);
            buf
        }

        let data_chunk = chunk(b"DATA", b"DATA", 3002, &payload);
        let iact = chunk(b"FOLD", b"IACT", 3002, &data_chunk);
        let inner_data = chunk(b"FOLD", b"DATA", 3000, &iact);
        let odat = chunk(b"FOLD", b"ODAT", 3000, &inner_data);
        let olyr = chunk(b"FOLD", b"OLYR", 3000, &odat);

        let mut file = Vec::new();
        file.extend_from_slice(b"Relic Chunky\r\n\x1a\0");
        file.extend_from_slice(&4u32.to_le_bytes());
        file.extend_from_slice(&1u32.to_le_bytes());
        file.extend_from_slice(&olyr);
        file
    }

    #[test]
    fn bounding_rect_of_simple_mask() {
        // 4x4 grid, playable (0) 2x2 block in the middle, border (1) elsewhere.
        #[rustfmt::skip]
        let mask = [
            1, 1, 1, 1,
            1, 0, 0, 1,
            1, 0, 0, 1,
            1, 1, 1, 1,
        ];
        let file = build_softmapedge(4, 4, &mask);
        let m = parse_playable_area(&file).unwrap();
        assert_eq!(m.width, 4);
        assert_eq!(m.height, 4);
        let rect = m.bounding_rect().unwrap();
        // Playable cells span grid x=[1,2], y=[1,2] -> world [1-2, 3-2] = [-1, 1] on each axis.
        assert_eq!(rect.min_x, -1.0);
        assert_eq!(rect.max_x, 1.0);
        assert_eq!(rect.min_y, -1.0);
        assert_eq!(rect.max_y, 1.0);
    }

    #[test]
    fn all_unplayable_returns_none() {
        let mask = [1u8; 16];
        let file = build_softmapedge(4, 4, &mask);
        let m = parse_playable_area(&file).unwrap();
        assert!(m.bounding_rect().is_none());
    }
}
