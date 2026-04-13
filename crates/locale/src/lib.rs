//! Locale string parsing for CoH3.
//!
//! Supports three formats:
//! - **SGA archive** (`LocaleEnglish.sga`): AES-128-CBC encrypted `.ucs` plaintext file
//! - **Tab-separated `.txt`** (cohdata format): `type\tsubject\tlabel\t$id\tstring\n`
//! - **JSON** (`{"id": "string"}`): already-parsed locale maps
//!
//! Only entries with types in [`LOCALE_TYPES`] are included (txt format only;
//! SGA and JSON formats include all entries since they lack type metadata).

mod error;
pub use error::Error;

use data::LocaleStore;
use std::{collections::HashMap, path::Path};

/// AES-128 master key for CoH3 locale SGA archives.
///
/// Source: COH3-SGA-Extraction.exe (cohstats/coh3-data), `ArchiveDecryptionMasterKey` field.
const MASTER_KEY: [u8; 16] = [
    79, 154, 72, 16, 166, 101, 78, 231, 177, 57, 94, 33, 50, 206, 163, 163,
];

/// Derive the per-archive AES key by XOR-ing the master key against the archive NiceName,
/// cycling through the NiceName bytes. Mirrors `Essence.Core.IO.Archive.Archive.RandomizeKey`.
fn randomize_key(nice_name: &str) -> [u8; 16] {
    let name_bytes = nice_name.as_bytes();
    let mut key = MASTER_KEY;
    for (i, byte) in key.iter_mut().enumerate() {
        *byte ^= name_bytes[i % name_bytes.len()];
    }
    key
}

/// Derive the AES IV for a file as MD5(UTF-8(file_name)).
/// Mirrors `Essence.Core.IO.Archive.Archive.GetIV`.
fn compute_iv(file_name: &str) -> [u8; 16] {
    use md5::{Digest, Md5};
    Md5::digest(file_name.as_bytes()).into()
}

/// A single encrypted+compressed file extracted from a locale SGA archive.
struct LocFileEntry {
    /// Bare filename (e.g. `"anvil.en.ucs"`), used to compute the AES IV.
    name: String,
    /// Ciphertext bytes, padded to a multiple of 16 for AES-CBC.
    ciphertext: Vec<u8>,
    /// Expected decompressed size in bytes.
    uncomp_len: usize,
}

/// Read all file entries from a locale SGA archive directly from raw bytes.
///
/// The SGA `compressed_length` field stores the pre-encryption (compressed)
/// size, not the ciphertext size.  The actual ciphertext is
/// `ceil(compressed_length / 16) * 16` bytes.  We read the aligned length so
/// that AES-CBC decryption can find the correct PKCS7 padding block.
///
/// SGA v10 header layout (all fields little-endian):
/// ```text
///   0x00  8 B  magic "_ARCHIVE"
///   0x08  2 B  version (u16)
///   0x0A  2 B  product (u16)
///   0x0C  128 B NiceName (64 × UTF-16 LE, null-padded)
///   0x8C  8 B  header_blob_offset (u64)
///   0x94  4 B  header_blob_length (u32)
///   0x98  8 B  data_offset (u64)
///   ...
/// ```
fn read_locale_file_entries(path: &Path) -> Result<Vec<LocFileEntry>, Error> {
    let raw = std::fs::read(path)
        .map_err(|e| Error::Locale(format!("cannot read {}: {e}", path.display())))?;

    if raw.len() < 172 {
        return Err(Error::Locale("SGA file too short".into()));
    }

    let read_u32 = |offset: usize| u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap());
    let read_u64 = |offset: usize| u64::from_le_bytes(raw[offset..offset + 8].try_into().unwrap());

    let header_blob_offset = read_u64(140) as usize;
    let data_offset = read_u64(152) as usize;

    // Header blob fields (11 × u32):
    //   [0] toc_data_offset   [1] toc_data_count
    //   [2] folder_data_offset [3] folder_data_count
    //   [4] file_data_offset  [5] file_data_count
    //   [6] string_offset     [7] string_length
    //   [8] file_hash_offset  [9] file_hash_length
    //  [10] block_size
    let hb = header_blob_offset;
    let file_data_offset = read_u32(hb + 16) as usize;
    let file_data_count = read_u32(hb + 20) as usize;
    let string_offset = read_u32(hb + 24) as usize;

    let mut results = Vec::with_capacity(file_data_count);
    let fe_base = hb + file_data_offset;

    for i in 0..file_data_count {
        // Each file entry is 30 bytes:
        //   name_offset  u32  hash_offset  u32  data_offset  u64
        //   comp_len     u32  uncomp_len   u32  verify_type  u8
        //   store_type   u8   crc          u32
        let base = fe_base + i * 30;
        let name_off = read_u32(base) as usize;
        let file_data_off = read_u64(base + 8) as usize;
        let comp_len = read_u32(base + 16) as usize;
        let uncomp_len = read_u32(base + 20) as usize;

        // Null-terminated filename from the string table.
        let str_start = hb + string_offset + name_off;
        let null_rel = raw[str_start..]
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| Error::Locale("unterminated file name in SGA".into()))?;
        let name = String::from_utf8(raw[str_start..str_start + null_rel].to_vec())
            .map_err(|e| Error::Locale(format!("invalid SGA file name: {e}")))?;

        // Read ciphertext at the AES-aligned length.
        // comp_len is the pre-encryption size; the ciphertext is comp_len
        // rounded up to the next 16-byte boundary.
        let aligned_len = comp_len.div_ceil(16) * 16;
        let data_start = data_offset + file_data_off;
        let ciphertext = raw
            .get(data_start..data_start + aligned_len)
            .ok_or_else(|| {
                Error::Locale(format!(
                    "file '{name}' data out of bounds (offset {data_start}, len {aligned_len})"
                ))
            })?
            .to_vec();

        results.push(LocFileEntry {
            name,
            ciphertext,
            uncomp_len,
        });
    }

    Ok(results)
}

/// Decrypt a locale file's AES-128-CBC ciphertext.
///
/// The input must already be padded to a multiple of 16 bytes.
fn decrypt_file_data(data: &[u8], key: &[u8; 16], iv: &[u8; 16]) -> Result<Vec<u8>, Error> {
    use aes::Aes128;
    use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};

    type Aes128CbcDec = cbc::Decryptor<Aes128>;

    let mut buf = data.to_vec();
    let decrypted = Aes128CbcDec::new(key.into(), iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| Error::Locale(format!("AES decryption failed: {e}")))?;
    Ok(decrypted.to_vec())
}

/// Decompress a raw DEFLATE stream (skipping a 2-byte zlib header) into a
/// buffer of `expected_size` bytes.
fn decompress_deflate(data: &[u8], expected_size: usize) -> Result<Vec<u8>, Error> {
    use flate2::read::DeflateDecoder;
    use std::io::Read;

    // The SGA StreamCompress format prepends a 2-byte zlib CMF+FLG header
    // before the raw DEFLATE bitstream; skip it.
    let deflate_data = data
        .get(2..)
        .ok_or_else(|| Error::Locale("compressed data too short to contain zlib header".into()))?;

    let mut out = Vec::with_capacity(expected_size);
    DeflateDecoder::new(deflate_data)
        .read_to_end(&mut out)
        .map_err(|e| Error::Locale(format!("deflate decompression failed: {e}")))?;
    Ok(out)
}

/// Open a locale SGA archive (e.g. `LocaleEnglish.sga`), decrypt and
/// decompress its `.ucs` file, and parse the result into a [`LocaleStore`].
///
/// The archive files are encrypted with AES-128-CBC (key derived from the
/// archive NiceName via `RandomizeKey`; IV = MD5(filename)) and compressed
/// with zlib/DEFLATE.
pub fn parse_locale_sga(path: &Path) -> Result<LocaleStore, Error> {
    let nice_name = sga::read_archive_name(path)
        .map_err(|e| Error::Locale(format!("cannot read archive name: {e}")))?;
    let key = randomize_key(&nice_name);

    let entries = read_locale_file_entries(path)?;

    // Use the largest .ucs file — that's the main locale data.
    let entry = entries
        .iter()
        .filter(|e| e.name.ends_with(".ucs"))
        .max_by_key(|e| e.ciphertext.len())
        .ok_or_else(|| Error::Locale("no .ucs file found in locale archive".into()))?;

    let iv = compute_iv(&entry.name);
    let compressed = decrypt_file_data(&entry.ciphertext, &key, &iv)?;
    let plaintext = decompress_deflate(&compressed, entry.uncomp_len)?;

    let text = decode_ucs_bytes(&plaintext)?;
    parse_locale_ucs(&text)
}

/// Decode UCS file bytes to a Rust `String`.
///
/// UCS files from CoH3 are UTF-16 LE with a leading BOM (`0xFF 0xFE`).
/// Falls back to UTF-8 if no BOM is present.
fn decode_ucs_bytes(bytes: &[u8]) -> Result<String, Error> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        // UTF-16 LE: skip BOM, decode pairs
        let utf16: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16(&utf16)
            .map_err(|e| Error::Locale(format!("invalid UTF-16 in UCS file: {e}")))
    } else {
        String::from_utf8(bytes.to_vec())
            .map_err(|_| Error::Locale("UCS content is neither UTF-16 LE nor UTF-8".into()))
    }
}

/// Parse a decrypted `.ucs` plaintext file into a [`LocaleStore`].
///
/// Each non-empty line is expected to contain a numeric ID and a string value
/// separated by a tab character, with an optional leading `$` on the ID.
/// Lines that don't match this structure are silently skipped.
pub fn parse_locale_ucs(text: &str) -> Result<LocaleStore, Error> {
    let mut map: HashMap<u32, String> = HashMap::new();
    for line in text.lines() {
        let line = line.trim_start_matches('\u{feff}'); // strip UTF-8 BOM on first line
        let Some((id_part, string_part)) = line.split_once('\t') else {
            continue;
        };
        let id_str = id_part.trim_start_matches('$');
        if let Ok(id) = id_str.trim().parse::<u32>() {
            map.entry(id).or_insert_with(|| string_part.to_string());
        }
    }
    Ok(LocaleStore(map))
}

/// Entity type prefixes included in the locale store.
const LOCALE_TYPES: &[&str] = &[
    "abilities",
    "map_pool",
    "ebps",
    "racebps",
    "sbps",
    "upgrade",
    "weapon",
];

/// Parse a locale tab-separated text file into a [`LocaleStore`].
///
/// Each line has the format: `type\tsubject\tlabel\t$id\tstring`
/// Lines with unrecognized type or missing fields are silently skipped.
pub fn parse_locale_txt(text: &str) -> Result<LocaleStore, Error> {
    let mut map: HashMap<u32, String> = HashMap::new();

    for line in text.lines() {
        let parts: Vec<&str> = line.splitn(5, '\t').collect();
        if parts.len() < 5 {
            continue;
        }
        let entry_type = parts[0];
        let id_str = parts[3].trim_start_matches('$');
        let string = parts[4];

        if !LOCALE_TYPES.contains(&entry_type) {
            continue;
        }

        if let Ok(id) = id_str.parse::<u32>() {
            map.entry(id).or_insert_with(|| string.to_string());
        }
    }

    Ok(LocaleStore(map))
}

/// Parse a locale JSON file into a [`LocaleStore`].
///
/// Accepts `{"id_str": "string"}` where id_str may or may not have a `$` prefix.
pub fn parse_locale_json(json: &str) -> Result<LocaleStore, Error> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| Error::Locale(e.to_string()))?;

    let obj = value
        .as_object()
        .ok_or_else(|| Error::Locale("locale JSON must be an object".into()))?;

    let mut map: HashMap<u32, String> = HashMap::new();

    for (key, val) in obj {
        let id_str = key.trim_start_matches('$');
        if let (Ok(id), Some(s)) = (id_str.parse::<u32>(), val.as_str()) {
            map.entry(id).or_insert_with(|| s.to_string());
        }
    }

    Ok(LocaleStore(map))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_txt_basic() {
        let txt = "abilities\tsome_path\tlabel\t$11156544\tBarracks\n\
                   upgrade\tpath\tlabel\t$9001\tMachine Gun\n\
                   unknown_type\tpath\tlabel\t$1\tIgnored\n";
        let store = parse_locale_txt(txt).unwrap();
        assert_eq!(store.get(11156544), Some("Barracks"));
        assert_eq!(store.get(9001), Some("Machine Gun"));
        assert_eq!(store.get(1), None);
    }

    #[test]
    fn parse_txt_skips_short_lines() {
        let txt = "abilities\tpath\t$123\n";
        let store = parse_locale_txt(txt).unwrap();
        assert_eq!(store.0.len(), 0);
    }

    #[test]
    fn parse_txt_deduplicates_ids() {
        let txt = "abilities\tp\tl\t$100\tFirst\n\
                   abilities\tp\tl\t$100\tSecond\n";
        let store = parse_locale_txt(txt).unwrap();
        assert_eq!(store.get(100), Some("First"));
    }

    #[test]
    fn parse_json_basic() {
        let json = r#"{"11156544": "Barracks", "9001": "Machine Gun"}"#;
        let store = parse_locale_json(json).unwrap();
        assert_eq!(store.get(11156544), Some("Barracks"));
        assert_eq!(store.get(9001), Some("Machine Gun"));
    }

    #[test]
    fn parse_json_strips_dollar_prefix() {
        let json = r#"{"$11156544": "Barracks"}"#;
        let store = parse_locale_json(json).unwrap();
        assert_eq!(store.get(11156544), Some("Barracks"));
    }

    #[test]
    fn parse_json_invalid_returns_error() {
        let result = parse_locale_json("not json");
        assert!(result.is_err());
    }

    #[test]
    fn parse_real_locale_txt() {
        let path = "/Users/ryantaylor/cohdata/data/10612/locale.txt";
        if !std::path::Path::new(path).exists() {
            return;
        }
        let text = std::fs::read_to_string(path).unwrap();
        let store = parse_locale_txt(&text).unwrap();
        assert!(!store.0.is_empty());
        assert!(store.get(11156544).is_some());
    }
}
