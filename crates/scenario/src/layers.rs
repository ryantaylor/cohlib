//! Scans `.layer` files for placed resource/victory point entities.
//!
//! `.info`'s `point_positions` is a snapshot taken when the scenario was last
//! saved from the map editor and can go stale after later edits — credit to
//! [cohstats/coh3-data `layer_parser.py`](https://github.com/cohstats/coh3-data/blob/master/scripts/mp-maps/layer_parser.py)
//! for identifying this (their documented example: `twin_beach_2p_mkii`'s
//! `.info` claims five `territory_fuel_point_medium`, but the map actually
//! has two medium + three lower-tier points). The actually-placed entities
//! live in `DATA ENTI` chunks inside the map's `.layer` files (and
//! `.scenario`, unused here — MP scenario files are 50+ MB and every map's
//! resource/victory points are placed in a `.layer`, in every scenario this
//! crate has seen).
//!
//! Rather than walk the full Relic Chunky tree (`.layer` files also contain an
//! elaborate, format-version-sensitive blueprint-registry section this crate
//! has no need to understand — see chunky.rs's doc comment for what *is*
//! walked structurally), this scans raw bytes directly for the `DATA`+`ENTI`
//! chunk header, which is what cohstats' parser does too. `DATA ENTI` payload
//! (verified against `castello_8p`'s `ressource and vp.layer`):
//!
//! ```text
//! u32 guid_len(16), [16 bytes guid],
//! u32 name_len, [name_len bytes ebp name],
//! f32[9] rotation matrix, f32 x, f32 height, f32 z, f32 scale
//! ```
//!
//! Note the `.info` file's `y` axis is this payload's `z` — the world is
//! stored ground-plane-as-XZ here, but CoH3's `.info`/replay data flattens it
//! to XY.

use sga::ArchiveEntry;

const TAG: &[u8; 8] = b"DATAENTI";

#[derive(Debug, Clone, PartialEq)]
pub struct PlacedEntity {
    pub ebp: String,
    pub x: f32,
    pub y: f32,
}

/// A placed entity is considered a match for an `.info` point within this
/// world-unit radius — matches cohstats' documented threshold.
const MATCH_RADIUS: f32 = 10.0;

/// Scans all given `.layer` files for placed point entities.
pub fn scan_entities(files: &[&ArchiveEntry]) -> Vec<PlacedEntity> {
    files.iter().flat_map(|f| scan_bytes(&f.bytes)).collect()
}

fn scan_bytes(bytes: &[u8]) -> Vec<PlacedEntity> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(rel) = find_subslice(&bytes[pos..], TAG) {
        let tag_start = pos + rel;
        let header_start = tag_start + TAG.len();
        if header_start + 12 > bytes.len() {
            break;
        }
        // header_start..+4 is the chunk version field — skip it, we don't need it.
        let data_len = u32::from_le_bytes(
            bytes[header_start + 4..header_start + 8]
                .try_into()
                .unwrap(),
        ) as usize;
        let name_len = u32::from_le_bytes(
            bytes[header_start + 8..header_start + 12]
                .try_into()
                .unwrap(),
        ) as usize;
        let payload_start = header_start + 12 + name_len;
        let payload_end = payload_start + data_len;
        if payload_end > bytes.len() {
            pos = tag_start + 1;
            continue;
        }
        if let Some(entity) = parse_entity_payload(&bytes[payload_start..payload_end]) {
            out.push(entity);
        }
        pos = payload_end;
    }
    out
}

fn parse_entity_payload(payload: &[u8]) -> Option<PlacedEntity> {
    let guid_len = u32::from_le_bytes(payload.get(0..4)?.try_into().ok()?) as usize;
    let mut offset = 4 + guid_len;
    let name_len = u32::from_le_bytes(payload.get(offset..offset + 4)?.try_into().ok()?) as usize;
    offset += 4;
    let ebp = std::str::from_utf8(payload.get(offset..offset + name_len)?)
        .ok()?
        .to_string();
    offset += name_len;

    let remaining = payload.get(offset..)?;
    // 9 rotation floats + x, height, z, scale = 13 floats.
    if remaining.len() < 13 * 4 {
        return None;
    }
    let float_at = |i: usize| -> Option<f32> {
        let b = remaining.get(i * 4..i * 4 + 4)?;
        Some(f32::from_le_bytes(b.try_into().ok()?))
    };
    let x = float_at(9)?;
    // float_at(10) is the height/elevation component — unused here.
    let z = float_at(11)?;

    Some(PlacedEntity { ebp, x, y: z })
}

fn find_subslice(haystack: &[u8], needle: &[u8; 8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Finds the placed entity nearest `(x, y)` within [`MATCH_RADIUS`], if any.
pub fn nearest_match(x: f32, y: f32, placed: &[PlacedEntity]) -> Option<&PlacedEntity> {
    placed
        .iter()
        .map(|p| (p, (p.x - x).powi(2) + (p.y - y).powi(2)))
        .filter(|(_, dist_sq)| *dist_sq <= MATCH_RADIUS * MATCH_RADIUS)
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(p, _)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_enti_chunk(ebp: &str, x: f32, height: f32, z: f32, scale: f32) -> Vec<u8> {
        let guid = [0u8; 16];
        let mut payload = Vec::new();
        payload.extend_from_slice(&(guid.len() as u32).to_le_bytes());
        payload.extend_from_slice(&guid);
        payload.extend_from_slice(&(ebp.len() as u32).to_le_bytes());
        payload.extend_from_slice(ebp.as_bytes());
        // identity rotation matrix
        let identity = [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        for f in identity {
            payload.extend_from_slice(&f.to_le_bytes());
        }
        payload.extend_from_slice(&x.to_le_bytes());
        payload.extend_from_slice(&height.to_le_bytes());
        payload.extend_from_slice(&z.to_le_bytes());
        payload.extend_from_slice(&scale.to_le_bytes());

        let mut chunk = Vec::new();
        chunk.extend_from_slice(TAG);
        chunk.extend_from_slice(&3001u32.to_le_bytes()); // version
        chunk.extend_from_slice(&(payload.len() as u32).to_le_bytes()); // data_len
        chunk.extend_from_slice(&0u32.to_le_bytes()); // name_len
        chunk.extend_from_slice(&payload);
        chunk
    }

    #[test]
    fn scans_single_entity() {
        let mut bytes = vec![0xAB; 7]; // junk prefix, as in a real .layer file
        bytes.extend(build_enti_chunk(
            "territory_fuel_point_low",
            -41.5,
            21.6,
            36.5,
            1.0,
        ));
        bytes.extend(vec![0xCD; 5]); // junk suffix

        let entities = scan_bytes(&bytes);
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].ebp, "territory_fuel_point_low");
        assert!((entities[0].x - -41.5).abs() < 1e-4);
        // y comes from the payload's z, not its height.
        assert!((entities[0].y - 36.5).abs() < 1e-4);
    }

    #[test]
    fn scans_multiple_entities_back_to_back() {
        let mut bytes = Vec::new();
        bytes.extend(build_enti_chunk(
            "territory_victory_point",
            1.0,
            2.0,
            3.0,
            1.0,
        ));
        bytes.extend(build_enti_chunk(
            "territory_munitions_point_high",
            4.0,
            5.0,
            6.0,
            1.0,
        ));

        let entities = scan_bytes(&bytes);
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].ebp, "territory_victory_point");
        assert_eq!(entities[1].ebp, "territory_munitions_point_high");
    }

    #[test]
    fn nearest_match_within_radius() {
        let placed = vec![
            PlacedEntity {
                ebp: "territory_fuel_point_low".into(),
                x: 10.0,
                y: 10.0,
            },
            PlacedEntity {
                ebp: "territory_victory_point".into(),
                x: 100.0,
                y: 100.0,
            },
        ];
        let m = nearest_match(12.0, 11.0, &placed).unwrap();
        assert_eq!(m.ebp, "territory_fuel_point_low");
    }

    #[test]
    fn nearest_match_outside_radius_returns_none() {
        let placed = vec![PlacedEntity {
            ebp: "territory_fuel_point_low".into(),
            x: 10.0,
            y: 10.0,
        }];
        assert!(nearest_match(1000.0, 1000.0, &placed).is_none());
    }
}
