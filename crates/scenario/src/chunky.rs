//! Minimal reader for the Relic Chunky binary container format used by CoH3's
//! `.scenario`, `.layer`, and `*.override` scenario files.
//!
//! Format (all integers little-endian), verified against real
//! `<map>_territory.override` / `<map>_softmapedge.override` files from the game
//! depot (not from any external spec):
//!
//! ```text
//! header:  16 bytes magic "Relic Chunky\r\n\x1a\0", u32 version, u32 platform
//! chunk:   4 bytes kind ("FOLD" or "DATA"), 4 bytes id, u32 version,
//!          u32 data_len, u32 name_len, name_len bytes name, data_len bytes data
//! ```
//!
//! A `FOLD` chunk's data is itself a sequence of sibling chunks; a `DATA` chunk's
//! data is a leaf payload specific to its id. This reader only walks the FOLD
//! tree far enough to reach named chunks by path — it does not interpret leaf
//! payloads, which is left to `territory.rs` / `playable.rs`.

use crate::Error;

const MAGIC: &[u8] = b"Relic Chunky\r\n\x1a\0";
const HEADER_LEN: usize = 24; // 16 magic + 4 version + 4 platform

#[derive(Debug, Clone, Copy)]
pub struct Chunk<'a> {
    /// `*b"FOLD"` or `*b"DATA"`.
    pub kind: [u8; 4],
    pub id: [u8; 4],
    /// Kept for debugging/future format-version checks; not currently read —
    /// every chunk version this crate parses has been a fixed, known value.
    #[allow(dead_code)]
    pub version: u32,
    /// For `FOLD` chunks, the nested chunk sequence (parse with [`parse_chunks`]).
    /// For `DATA` chunks, the raw leaf payload.
    pub data: &'a [u8],
}

impl<'a> Chunk<'a> {
    pub fn is_fold(&self, id: &[u8; 4]) -> bool {
        &self.kind == b"FOLD" && &self.id == id
    }
}

/// Parses the outer Relic Chunky header and returns the top-level chunk sequence.
pub fn parse_chunky(bytes: &[u8]) -> Result<Vec<Chunk<'_>>, Error> {
    if bytes.len() < HEADER_LEN || &bytes[..MAGIC.len()] != MAGIC {
        return Err(Error::Scenario("not a Relic Chunky file".into()));
    }
    parse_chunks(&bytes[HEADER_LEN..])
}

/// Parses a byte range into a sequence of sibling chunks (the contents of one
/// `FOLD` chunk, or the top-level sequence after the outer header).
pub fn parse_chunks(mut bytes: &[u8]) -> Result<Vec<Chunk<'_>>, Error> {
    let mut chunks = Vec::new();
    while !bytes.is_empty() {
        if bytes.len() < 20 {
            return Err(Error::Scenario("truncated chunk header".into()));
        }
        let kind: [u8; 4] = bytes[0..4].try_into().unwrap();
        let id: [u8; 4] = bytes[4..8].try_into().unwrap();
        if &kind != b"FOLD" && &kind != b"DATA" {
            return Err(Error::Scenario(format!(
                "unrecognized chunk kind {:?}",
                String::from_utf8_lossy(&kind)
            )));
        }
        let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let data_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let name_len = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;

        let data_start = 20 + name_len;
        let data_end = data_start
            .checked_add(data_len)
            .ok_or_else(|| Error::Scenario("chunk length overflow".into()))?;
        if data_end > bytes.len() {
            return Err(Error::Scenario("chunk data extends past buffer".into()));
        }

        chunks.push(Chunk {
            kind,
            id,
            version,
            data: &bytes[data_start..data_end],
        });
        bytes = &bytes[data_end..];
    }
    Ok(chunks)
}

/// Finds the first direct child chunk matching `kind`/`id` (e.g. `find(chunks, b"FOLD", b"TRTY")`).
pub fn find<'a, 'b>(
    chunks: &'b [Chunk<'a>],
    kind: &[u8; 4],
    id: &[u8; 4],
) -> Option<&'b Chunk<'a>> {
    chunks.iter().find(|c| &c.kind == kind && &c.id == id)
}

/// Walks a path of `(kind, id)` pairs through nested `FOLD` chunks, parsing each
/// level's children as needed, and returns the final chunk's data. All but the
/// last path element must be `FOLD` chunks.
pub fn find_path<'a>(bytes: &'a [u8], path: &[(&[u8; 4], &[u8; 4])]) -> Result<&'a [u8], Error> {
    let mut chunks = parse_chunky(bytes)?;
    let mut data: &[u8] = &[];
    for (i, (kind, id)) in path.iter().enumerate() {
        let chunk = find(&chunks, kind, id).ok_or_else(|| {
            Error::Scenario(format!(
                "chunk path not found: missing {:?} {:?} at depth {i}",
                String::from_utf8_lossy(*kind),
                String::from_utf8_lossy(*id)
            ))
        })?;
        data = chunk.data;
        if i + 1 < path.len() {
            chunks = parse_chunks(data)?;
        }
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal Relic Chunky buffer with one top-level chunk for testing.
    fn build_chunky(chunks: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(MAGIC);
        buf.extend_from_slice(&4u32.to_le_bytes()); // version
        buf.extend_from_slice(&1u32.to_le_bytes()); // platform
        buf.extend_from_slice(chunks);
        buf
    }

    fn build_chunk(kind: &[u8; 4], id: &[u8; 4], version: u32, data: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(kind);
        buf.extend_from_slice(id);
        buf.extend_from_slice(&version.to_le_bytes());
        buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes()); // name_len
        buf.extend_from_slice(data);
        buf
    }

    #[test]
    fn rejects_non_chunky_bytes() {
        assert!(parse_chunky(b"not a chunky file").is_err());
    }

    #[test]
    fn parses_single_data_chunk() {
        let inner = build_chunk(b"DATA", b"HEAD", 3000, &[1, 2, 3, 4]);
        let file = build_chunky(&inner);
        let chunks = parse_chunky(&file).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(&chunks[0].kind, b"DATA");
        assert_eq!(&chunks[0].id, b"HEAD");
        assert_eq!(chunks[0].version, 3000);
        assert_eq!(chunks[0].data, &[1, 2, 3, 4]);
    }

    #[test]
    fn parses_nested_fold() {
        let leaf = build_chunk(b"DATA", b"DATA", 3001, &[9, 9]);
        let fold_data = build_chunk(b"FOLD", b"INNR", 3000, &leaf);
        let file = build_chunky(&fold_data);
        let chunks = parse_chunky(&file).unwrap();
        assert!(chunks[0].is_fold(b"INNR"));

        let inner = parse_chunks(chunks[0].data).unwrap();
        assert_eq!(&inner[0].kind, b"DATA");
        assert_eq!(&inner[0].id, b"DATA");
        assert_eq!(inner[0].data, &[9, 9]);
    }

    #[test]
    fn find_path_walks_nested_folds() {
        let leaf = build_chunk(b"DATA", b"LEAF", 1, b"payload");
        let mid = build_chunk(b"FOLD", b"MID_", 1, &leaf);
        let file = build_chunky(&mid);
        let data = find_path(&file, &[(b"FOLD", b"MID_"), (b"DATA", b"LEAF")]).unwrap();
        assert_eq!(data, b"payload");
    }

    #[test]
    fn find_path_missing_chunk_errors() {
        let file = build_chunky(&[]);
        assert!(find_path(&file, &[(b"FOLD", b"NOPE")]).is_err());
    }
}
