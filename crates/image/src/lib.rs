//! Icon extraction from UI.sga archives.
//!
//! Converts RRTEX (Relic's proprietary texture format) files to WebP.
//!
//! # Format overview
//!
//! RRTEX files are wrapped in the Relic Chunky container. The relevant chunks
//! are located by searching for their magic byte sequences in the raw file:
//!
//! - `DATATMAN`: texture metadata (11 × u32 LE: uk1, width, height, uk2, uk3,
//!   compression_format, mip_count, uk4, mip_tex_count, uncomp_size, comp_size)
//! - `DATATDAT`: zlib-compressed BCn texture data
//!
//! DATATDAT payload layout (after skipping 16-byte chunk continuation header):
//!
//! Non-mipped: one or more sequential zlib streams whose concatenated decompressed
//! output is `[16-byte mip header] + BCn blocks`. Small textures (icons) fit in
//! one stream; large ones (map minimaps) are split into many 64 KiB streams.
//! The 16-byte header is discarded.
//!
//! Compressed mipped: N sequential zlib streams (smallest mip first). Each stream
//! decompresses to `[mip_level: u32, width: u32, height: u32, u32] + BCn blocks`.
//! We locate and return the stream where `mip_level == 0` (full resolution).
//!
//! Uncompressed mipped: raw concatenated entries starting at offset 0 (relative to
//! the start of the data after the 16-byte header):
//! `[mip_level: u32, width: u32, height: u32, block_size: u32] + BCn blocks`.
//! Same search for `mip_level == 0`.
//!
//! Supported compression formats:
//! - 18 / 19 → BC1 (DXT1): 8 bytes per 4×4 block, RGB with optional 1-bit alpha
//! - 22       → BC3 (DXT5): 16 bytes per 4×4 block, RGB + interpolated alpha
//! - 28       → BC7 (BPTC): 16 bytes per 4×4 block, full-color with alpha

mod error;
pub use error::Error;

use std::io::Read;

const MAGIC_TMAN: &[u8] = b"DATATMAN";
const MAGIC_TDAT: &[u8] = b"DATATDAT";

/// Convert the raw bytes of an RRTEX file to WebP bytes.
///
/// Returns an error for unsupported compression formats or malformed input.
pub fn extract_icon(rrtex_bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let (width, height, compression_format, mip_count, tdat_payload) = parse_rrtex(rrtex_bytes)?;
    let rgba = decode_bcn(tdat_payload, width, height, compression_format, mip_count)?;
    encode_webp(&rgba, width, height)
}

// ---------------------------------------------------------------------------
// RRTEX / Relic Chunky parser
// ---------------------------------------------------------------------------

/// Parse the Relic Chunky RRTEX container.
///
/// Returns `(width, height, compression_format, mip_count, tdat_payload)`.
fn parse_rrtex(raw: &[u8]) -> Result<(u32, u32, u32, u32, &[u8]), Error> {
    let err = |msg: &str| Error::Image(format!("RRTEX: {msg}"));

    let tman_pos = find_bytes(raw, MAGIC_TMAN).ok_or_else(|| err("DATATMAN chunk not found"))?;
    let tdat_pos = find_bytes(raw, MAGIC_TDAT).ok_or_else(|| err("DATATDAT chunk not found"))?;

    // TMAN metadata starts 12 bytes after the end of the "DATATMAN" magic.
    let tman_data_start = tman_pos + MAGIC_TMAN.len() + 12;
    let tman_bytes = raw
        .get(tman_data_start..tdat_pos)
        .filter(|b| b.len() >= 44)
        .ok_or_else(|| err("DATATMAN metadata too short or misaligned"))?;

    // 11 × u32 LE: [0]=uk1, [1]=width, [2]=height, [3]=uk2, [4]=uk3,
    //              [5]=compression_format, [6]=mip_count, [7]=uk4,
    //              [8]=mip_tex_count, [9]=uncomp_size, [10]=comp_size
    let u32_at = |off: usize| u32::from_le_bytes(tman_bytes[off..off + 4].try_into().unwrap());
    let width = u32_at(4);
    let height = u32_at(8);
    let compression_format = u32_at(20);
    let mip_count = u32_at(24);

    let tdat_payload = raw
        .get(tdat_pos + MAGIC_TDAT.len()..)
        .ok_or_else(|| err("DATATDAT truncated"))?;

    Ok((width, height, compression_format, mip_count, tdat_payload))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

// ---------------------------------------------------------------------------
// DATATDAT decompression
// ---------------------------------------------------------------------------

/// Decompress the DATATDAT payload and return the raw BCn block data for the
/// full-resolution mip level.
///
/// `payload` is the raw bytes immediately following the "DATATDAT" magic.
/// The first 16 bytes are a chunk-continuation header that is always skipped.
/// What follows depends on `mip_count`:
///
/// - `mip_count <= 1`: one or more sequential zlib streams whose concatenated
///   decompressed output is `[16-byte header] + BCn`. Small textures (icons) fit
///   in a single stream; large textures (map minimaps) are split into many 64 KiB
///   streams. All streams are read until the expected BCn size is reached.
/// - mipped, compressed: N sequential zlib streams, each decompressing to
///   `[mip_level u32, width u32, height u32, u32] + BCn`. Locate level 0.
/// - mipped, uncompressed: raw entries `[mip_level u32, width u32, height u32,
///   block_size u32] + BCn`, concatenated. Locate level 0.
fn decompress_tdat(
    payload: &[u8],
    width: u32,
    height: u32,
    compression_format: u32,
    mip_count: u32,
) -> Result<Vec<u8>, Error> {
    let err = |msg: &str| Error::Image(format!("RRTEX decompress: {msg}"));

    let data = payload
        .get(16..)
        .ok_or_else(|| err("DATATDAT payload too short (< 16 bytes)"))?;

    if mip_count <= 1 {
        // Compute expected BCn data size so we know when we have everything.
        let block_bytes: usize = match compression_format {
            18 | 19 => 8,
            22 | 28 => 16,
            _ => 0, // unknown format — fall through with whatever we decompress
        };
        let expected_bcn =
            (width as usize).div_ceil(4) * (height as usize).div_ceil(4) * block_bytes;

        // Decompress the first zlib stream. ZlibDecoder::read_to_end correctly
        // handles a stream that is a prefix of `data`; total_in() reports bytes
        // consumed so we can locate the next stream boundary.
        let mut decoder = flate2::read::ZlibDecoder::new(data);
        let mut decompressed: Vec<u8> = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| err(&e.to_string()))?;
        let mut pos = decoder.total_in() as usize;

        if decompressed.len() < 16 {
            return Err(err("decompressed data too short (< 16 bytes)"));
        }

        // If the first stream already contains all the BCn data, we are done.
        // This is the common case for icon textures.
        if expected_bcn == 0 || decompressed.len() >= 16 + expected_bcn {
            return decompressed
                .get(16..)
                .map(|s| s.to_vec())
                .ok_or_else(|| err("decompressed data too short (< 16 bytes)"));
        }

        // Large textures (e.g., map minimaps) split BC7 data across many
        // sequential 64 KiB zlib streams. Read the remaining streams and append
        // until we have the full expected BCn payload.
        while decompressed.len() < 16 + expected_bcn {
            let remaining = match data.get(pos..) {
                Some(r) if !r.is_empty() => r,
                _ => break,
            };
            let offset = match find_next_zlib(remaining) {
                Some(o) => o,
                None => break,
            };
            pos += offset;

            let mut decoder2 = flate2::read::ZlibDecoder::new(&data[pos..]);
            let mut chunk: Vec<u8> = Vec::new();
            if decoder2.read_to_end(&mut chunk).is_err() {
                break;
            }
            let consumed = decoder2.total_in() as usize;
            if consumed == 0 || chunk.is_empty() {
                break;
            }
            decompressed.extend_from_slice(&chunk);
            pos += consumed;
        }

        // When all 64-KiB streams are exact multiples (e.g., 2048×2048 BC7), the
        // last few BCn bytes may be stored as a raw (uncompressed) tail after the
        // final zlib stream rather than in their own stream. Append them directly.
        if decompressed.len() < 16 + expected_bcn {
            let still_need = (16 + expected_bcn) - decompressed.len();
            if let Some(tail) = data.get(pos..) {
                let take = still_need.min(tail.len());
                decompressed.extend_from_slice(&tail[..take]);
            }
        }

        return decompressed
            .get(16..)
            .map(|s| s.to_vec())
            .ok_or_else(|| err("decompressed data too short (< 16 bytes)"));
    }

    let block_bytes: usize = match compression_format {
        18 | 19 => 8,
        22 | 28 => 16,
        other => {
            return Err(Error::Image(format!(
                "unknown compression format {other} in mipped texture"
            )))
        }
    };

    // Distinguish format by whether the data begins with a valid zlib header.
    // Compressed mipped: the first byte after the 16-byte chunk header is 0x78
    // (zlib magic), meaning the first mip is a zlib stream starting at offset 0.
    // Uncompressed mipped: byte 0 is the low byte of `mip_level` (a small integer).
    if is_zlib_start(data) {
        // Compressed mipped: multiple sequential zlib streams starting at offset 0.
        decompress_mipped_zlib(data, width, height, block_bytes)
            .ok_or_else(|| err("mip level 0 not found in compressed mipped texture"))
    } else {
        // Uncompressed mipped: raw concatenated entries.
        decompress_mipped_raw(data, width, height)
            .ok_or_else(|| err("mip level 0 not found in uncompressed mipped texture"))
    }
}

/// Attempt to decode mipped texture as sequential zlib streams.
///
/// Each stream decompresses to `[mip_level: u32, w: u32, h: u32, u32] + BCn`.
/// Returns the BCn data for `mip_level == 0`, or `None` if not found.
///
/// Uses `flate2::Decompress` directly to get an accurate count of consumed input
/// bytes per stream (unlike `ZlibDecoder`, which may read-ahead into the next stream).
fn decompress_mipped_zlib(
    data: &[u8],
    width: u32,
    height: u32,
    block_bytes: usize,
) -> Option<Vec<u8>> {
    let bx = (width as usize).div_ceil(4);
    let by = (height as usize).div_ceil(4);
    let base_mip_size = bx * by * block_bytes;

    // First stream begins at offset 0 (caller verified is_zlib_start).
    let mut pos = 0usize;

    // Limit iterations to avoid looping forever on malformed data.
    for _ in 0..64 {
        if pos >= data.len() {
            break;
        }

        let mut d = flate2::Decompress::new(true); // expect zlib header
        let mut dec = Vec::new();
        match d.decompress_vec(&data[pos..], &mut dec, flate2::FlushDecompress::Finish) {
            Ok(_) => {}
            Err(_) => break,
        }

        let consumed = d.total_in() as usize;
        if consumed == 0 || dec.len() < 16 {
            break;
        }
        pos += consumed;

        let mip_level = u32::from_le_bytes(dec[0..4].try_into().ok()?);
        let chunk_w = u32::from_le_bytes(dec[4..8].try_into().ok()?);
        let chunk_h = u32::from_le_bytes(dec[8..12].try_into().ok()?);
        if mip_level == 0 && chunk_w == width && chunk_h == height {
            return dec.get(16..16 + base_mip_size).map(|s| s.to_vec());
        }

        // Advance to the next zlib stream. Streams are typically contiguous,
        // but scan forward in case of inter-stream padding.
        match find_next_zlib(&data[pos..]) {
            Some(off) => pos += off,
            None => break,
        }
    }
    None
}

/// Attempt to decode mipped texture as raw (uncompressed) concatenated entries.
///
/// Each entry: `[mip_level: u32, width: u32, height: u32, data_size: u32] + BCn`.
///
/// The 4th field is the **total byte count** of BCn data for this mip level
/// (not bytes-per-block). This can be derived from:
///   `ceil(w/4) × ceil(h/4) × bytes_per_block`
/// but is stored directly so no block-size knowledge is needed here.
///
/// Returns the BCn data for the entry where `mip_level == 0` and dimensions match.
fn decompress_mipped_raw(data: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let mut offset = 0usize;
    while offset + 16 <= data.len() {
        let mip_level = u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?);
        let entry_w = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().ok()?);
        let entry_h = u32::from_le_bytes(data[offset + 8..offset + 12].try_into().ok()?);
        let data_size =
            u32::from_le_bytes(data[offset + 12..offset + 16].try_into().ok()?) as usize;
        offset += 16;

        if mip_level == 0 && entry_w == width && entry_h == height {
            return data.get(offset..offset + data_size).map(|s| s.to_vec());
        }

        if data_size == 0 || data_size > data.len() {
            break;
        }
        offset += data_size;
    }
    None
}

/// Returns `true` if `data` begins with a valid zlib header byte pair.
///
/// Zlib streams start with `0x78` followed by `0x01`, `0x5e`, `0x9c`, or `0xda`.
fn is_zlib_start(data: &[u8]) -> bool {
    matches!(data, [0x78, b, ..] if matches!(b, 0x01 | 0x5e | 0x9c | 0xda))
}

/// Find the next zlib stream header anywhere in `data`, returning its offset.
///
/// Used to locate subsequent mip streams after consuming one stream.
fn find_next_zlib(data: &[u8]) -> Option<usize> {
    data.windows(2)
        .position(|w| w[0] == 0x78 && matches!(w[1], 0x01 | 0x5e | 0x9c | 0xda))
}

// ---------------------------------------------------------------------------
// BCn dispatch
// ---------------------------------------------------------------------------

fn decode_bcn(
    tdat_payload: &[u8],
    width: u32,
    height: u32,
    compression_format: u32,
    mip_count: u32,
) -> Result<Vec<u8>, Error> {
    let bcn = decompress_tdat(tdat_payload, width, height, compression_format, mip_count)?;

    match compression_format {
        18 | 19 => Ok(decode_bc1(&bcn, width, height)),
        22 => Ok(decode_bc3(&bcn, width, height)),
        28 => decode_bc7(&bcn, width, height),
        other => Err(Error::Image(format!(
            "unknown BCn compression format: {other}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// BC1 (DXT1) decoder
// ---------------------------------------------------------------------------

fn decode_bc1(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let bx_count = w.div_ceil(4);
    let by_count = h.div_ceil(4);
    let mut out = vec![0u8; w * h * 4];

    for by in 0..by_count {
        for bx in 0..bx_count {
            let off = (by * bx_count + bx) * 8;
            let Some(block) = data.get(off..off + 8) else {
                break;
            };
            bc1_block(block, &mut out, bx, by, w, h);
        }
    }
    out
}

fn bc1_block(block: &[u8], out: &mut [u8], bx: usize, by: usize, w: usize, h: usize) {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let indices = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);

    let (r0, g0, b0) = rgb565(c0);
    let (r1, g1, b1) = rgb565(c1);

    let colors: [(u8, u8, u8, u8); 4] = if c0 > c1 {
        [
            (r0, g0, b0, 255),
            (r1, g1, b1, 255),
            (
                lerp(r0, r1, 1, 3),
                lerp(g0, g1, 1, 3),
                lerp(b0, b1, 1, 3),
                255,
            ),
            (
                lerp(r0, r1, 2, 3),
                lerp(g0, g1, 2, 3),
                lerp(b0, b1, 2, 3),
                255,
            ),
        ]
    } else {
        [
            (r0, g0, b0, 255),
            (r1, g1, b1, 255),
            (
                lerp(r0, r1, 1, 2),
                lerp(g0, g1, 1, 2),
                lerp(b0, b1, 1, 2),
                255,
            ),
            (0, 0, 0, 0), // transparent black (1-bit alpha mode)
        ]
    };

    for py in 0..4usize {
        for px in 0..4usize {
            let x = bx * 4 + px;
            let y = by * 4 + py;
            if x >= w || y >= h {
                continue;
            }
            let (r, g, b, a) = colors[((indices >> (2 * (py * 4 + px))) & 3) as usize];
            let i = (y * w + x) * 4;
            out[i] = r;
            out[i + 1] = g;
            out[i + 2] = b;
            out[i + 3] = a;
        }
    }
}

// ---------------------------------------------------------------------------
// BC7 (BPTC) decoder
// ---------------------------------------------------------------------------

fn decode_bc7(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, Error> {
    let pixel_count = (width * height) as usize;
    let mut pixels = vec![0u32; pixel_count];
    texture2ddecoder::decode_bc7(data, width as usize, height as usize, &mut pixels)
        .map_err(|e| Error::Image(format!("BC7 decode error: {e}")))?;

    // texture2ddecoder packs pixels as BGRA (little-endian bytes: [b, g, r, a]).
    // Swap R and B to produce RGBA for the WebP encoder.
    let mut rgba = Vec::with_capacity(pixel_count * 4);
    for pixel in pixels {
        let [b, g, r, a] = pixel.to_le_bytes();
        rgba.extend_from_slice(&[r, g, b, a]);
    }
    Ok(rgba)
}

// ---------------------------------------------------------------------------
// BC3 (DXT5) decoder
// ---------------------------------------------------------------------------

fn decode_bc3(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let bx_count = w.div_ceil(4);
    let by_count = h.div_ceil(4);
    let mut out = vec![0u8; w * h * 4];

    for by in 0..by_count {
        for bx in 0..bx_count {
            let off = (by * bx_count + bx) * 16;
            let Some(block) = data.get(off..off + 16) else {
                break;
            };
            bc3_block(block, &mut out, bx, by, w, h);
        }
    }
    out
}

fn bc3_block(block: &[u8], out: &mut [u8], bx: usize, by: usize, w: usize, h: usize) {
    // Alpha sub-block: a0, a1, then 48 bits of 3-bit indices for 16 pixels.
    let a0 = block[0];
    let a1 = block[1];
    let mut alpha_bits: u64 = 0;
    for (i, &b) in block[2..8].iter().enumerate() {
        alpha_bits |= (b as u64) << (i * 8);
    }

    // Build 8-entry alpha palette.
    let alphas: [u8; 8] = if a0 > a1 {
        // 8-alpha mode: 6 interpolated values between a0 and a1.
        [
            a0,
            a1,
            lerp(a0, a1, 1, 7),
            lerp(a0, a1, 2, 7),
            lerp(a0, a1, 3, 7),
            lerp(a0, a1, 4, 7),
            lerp(a0, a1, 5, 7),
            lerp(a0, a1, 6, 7),
        ]
    } else {
        // 6-alpha mode: 4 interpolated values, then 0 and 255.
        [
            a0,
            a1,
            lerp(a0, a1, 1, 5),
            lerp(a0, a1, 2, 5),
            lerp(a0, a1, 3, 5),
            lerp(a0, a1, 4, 5),
            0,
            255,
        ]
    };

    // Color sub-block: same layout as BC1, but always decoded in 4-color mode.
    let c0 = u16::from_le_bytes([block[8], block[9]]);
    let c1 = u16::from_le_bytes([block[10], block[11]]);
    let ci = u32::from_le_bytes([block[12], block[13], block[14], block[15]]);

    let (r0, g0, b0) = rgb565(c0);
    let (r1, g1, b1) = rgb565(c1);
    let colors: [(u8, u8, u8); 4] = [
        (r0, g0, b0),
        (r1, g1, b1),
        (lerp(r0, r1, 1, 3), lerp(g0, g1, 1, 3), lerp(b0, b1, 1, 3)),
        (lerp(r0, r1, 2, 3), lerp(g0, g1, 2, 3), lerp(b0, b1, 2, 3)),
    ];

    for py in 0..4usize {
        for px in 0..4usize {
            let x = bx * 4 + px;
            let y = by * 4 + py;
            if x >= w || y >= h {
                continue;
            }
            let pixel = py * 4 + px;
            let alpha = alphas[((alpha_bits >> (pixel * 3)) & 7) as usize];
            let (r, g, b) = colors[((ci >> (pixel * 2)) & 3) as usize];
            let i = (y * w + x) * 4;
            out[i] = r;
            out[i + 1] = g;
            out[i + 2] = b;
            out[i + 3] = alpha;
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Expand an RGB565 value to `(r8, g8, b8)`.
fn rgb565(c: u16) -> (u8, u8, u8) {
    let r = ((c >> 11) & 0x1F) as u8;
    let g = ((c >> 5) & 0x3F) as u8;
    let b = (c & 0x1F) as u8;
    (
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
    )
}

/// Linear interpolation: `a + (b − a) × num / den` in u8.
fn lerp(a: u8, b: u8, num: u32, den: u32) -> u8 {
    ((a as u32 * (den - num) + b as u32 * num) / den) as u8
}

// ---------------------------------------------------------------------------
// WebP output
// ---------------------------------------------------------------------------

fn encode_webp(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, Error> {
    let encoder = webp::Encoder::from_rgba(rgba, width, height);
    Ok(encoder.encode(85.0).to_vec())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb565_white() {
        // 0xFFFF = R=31, G=63, B=31 → (255, 255, 255)
        assert_eq!(rgb565(0xFFFF), (255, 255, 255));
    }

    #[test]
    fn rgb565_black() {
        assert_eq!(rgb565(0x0000), (0, 0, 0));
    }

    #[test]
    fn lerp_endpoints() {
        assert_eq!(lerp(0, 255, 0, 7), 0);
        assert_eq!(lerp(0, 255, 7, 7), 255);
    }

    #[test]
    fn lerp_midpoint() {
        // (200*(3-1) + 100*1) / 3 = (400+100)/3 = 166
        assert_eq!(lerp(200, 100, 1, 3), 166);
    }

    #[test]
    fn bc1_solid_red_block() {
        // Encode a BC1 block with c0=red (0xF800), c1=0, all pixels index 0
        let c0: u16 = 0xF800; // R=31, G=0, B=0 → (255, 0, 0)
        let c1: u16 = 0x0000;
        let mut block = [0u8; 8];
        block[0..2].copy_from_slice(&c0.to_le_bytes());
        block[2..4].copy_from_slice(&c1.to_le_bytes());
        // indices = 0 for all 16 pixels (all select color 0 = red)
        block[4..8].copy_from_slice(&0u32.to_le_bytes());

        let mut out = vec![0u8; 4 * 4 * 4];
        bc1_block(&block, &mut out, 0, 0, 4, 4);

        // Every pixel should be (255, 0, 0, 255)
        for chunk in out.chunks(4) {
            assert_eq!(chunk, [255, 0, 0, 255]);
        }
    }

    #[test]
    fn bc1_transparent_index3() {
        // c0 <= c1 mode: index 3 = transparent black
        let c0: u16 = 0x0000; // black
        let c1: u16 = 0xFFFF; // white — c0 < c1 activates 1-bit alpha mode
        let mut block = [0u8; 8];
        block[0..2].copy_from_slice(&c0.to_le_bytes());
        block[2..4].copy_from_slice(&c1.to_le_bytes());
        // All pixels use index 3 (transparent)
        block[4..8].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());

        let mut out = vec![0u8; 4 * 4 * 4];
        bc1_block(&block, &mut out, 0, 0, 4, 4);

        for chunk in out.chunks(4) {
            assert_eq!(chunk, [0, 0, 0, 0]);
        }
    }

    #[test]
    fn find_bytes_found() {
        let data = b"hello DATATMAN world";
        assert_eq!(find_bytes(data, b"DATATMAN"), Some(6));
    }

    #[test]
    fn find_bytes_not_found() {
        let data = b"no magic here";
        assert_eq!(find_bytes(data, b"DATATMAN"), None);
    }
}
