//! Static derivation of the `dataChecksum` value required by the Relic
//! advertisement API (`findObservableAdvertisements` / `findAdvertisements`).
//!
//! The algorithm reads `RelicGame.module` to find which SGA archives are
//! `syncChecked`, computes MD5-based identifiers for each, then CRC32s the
//! concatenation in a fixed selector order hardcoded in `RelicCoH3.exe`.
//!
//! Both depot 1677281 (SGA archives) and depot 1677282 (`RelicGame.module`)
//! must be present under `game_dir` for this to work.
//!
//! See `COH3_DATACHECKSUM_INVESTIGATION.md` and `derive_data_checksum.py` for
//! the full reverse-engineering notes and the reference Python implementation.

use md5::Digest as _;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Selector order hardcoded in `RelicCoH3.exe` at static VA `0x148333870`.
/// Drives the iteration order in the dataChecksum hashing pass.
const SELECTOR_ORDER: &[&str] = &[
    "attrib", "audio", "data", "data", "locale", "movies", "reflect",
    "telemetry", "scardocs", "thumbnail", "toolsdata", "data",
];

/// Returns the signed `i32` `dataChecksum` for the game depot at `game_dir`.
///
/// `game_dir` must contain both:
/// - `RelicGame.module` (from Steam depot 1677282)
/// - The SGA archives referenced by `syncChecked = 1` sections (from depot 1677281)
///
/// Errors if `RelicGame.module` is missing or if any required SGA cannot be read.
pub fn compute_data_checksum(game_dir: &Path) -> Result<i32, String> {
    let module_path = game_dir.join("RelicGame.module");
    if !module_path.exists() {
        return Err(format!(
            "RelicGame.module not found at {} \
             — depot 1677282 must be downloaded alongside depot 1677281",
            module_path.display()
        ));
    }

    let sections = parse_module(&module_path)?;

    let mut archives_by_category: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for (category, entries) in &sections {
        for (root, name) in entries {
            // Module uses Windows backslashes; normalize for cross-platform joins.
            let root_normalized = root.replace('\\', "/");
            let sga = resolve_sga(game_dir, &root_normalized, name)?;
            let (id, ver) = compute_archive_id_version(&sga)?;
            archives_by_category
                .entry(category.clone())
                .or_default()
                .push((id, ver));
        }
    }

    let mut parts = String::new();
    for &selector in SELECTOR_ORDER {
        if let Some(archives) = archives_by_category.get(selector) {
            let mut sorted = archives.clone();
            sorted.sort();
            for (id, ver) in sorted {
                parts.push_str(&id);
                parts.push_str(&ver);
            }
        }
    }

    Ok(crc32fast::hash(parts.as_bytes()) as i32)
}

/// Parse `RelicGame.module` (INI-like format). Returns a map of
/// `category -> Vec<(archive_root, archive_name)>` for sections that have
/// `syncChecked = 1`.
fn parse_module(path: &Path) -> Result<HashMap<String, Vec<(String, String)>>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    parse_module_text(&text)
}

fn parse_module_text(text: &str) -> Result<HashMap<String, Vec<(String, String)>>, String> {
    let mut result: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut current_category: Option<String> = None;
    let mut current_root: Option<String> = None;
    let mut current_archives: Vec<String> = Vec::new();
    let mut sync_checked = false;

    let flush = |category: &Option<String>,
                 root: &Option<String>,
                 archives: &[String],
                 checked: bool,
                 result: &mut HashMap<String, Vec<(String, String)>>| {
        if checked {
            if let (Some(cat), Some(r)) = (category, root) {
                let entry = result.entry(cat.clone()).or_default();
                for a in archives {
                    entry.push((r.clone(), a.clone()));
                }
            }
        }
    };

    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix('[') {
            // New section — flush the previous one.
            flush(
                &current_category,
                &current_root,
                &current_archives,
                sync_checked,
                &mut result,
            );
            // Extract the category (everything before the first ':').
            let section_name = rest.trim_end_matches(']');
            current_category = Some(section_name.split(':').next().unwrap_or("").to_string());
            current_root = None;
            current_archives = Vec::new();
            sync_checked = false;
        } else if line.starts_with("syncChecked") {
            if let Some(val) = line.split('=').nth(1) {
                if val.trim() == "1" {
                    sync_checked = true;
                }
            }
        } else if line.starts_with("archiveRoot") {
            if let Some(val) = line.split('=').nth(1) {
                current_root = Some(val.trim().to_string());
            }
        } else if line.starts_with("archive.") {
            if let Some(val) = line.split('=').nth(1) {
                current_archives.push(val.trim().to_string());
            }
        }
    }
    // Flush the final section.
    flush(
        &current_category,
        &current_root,
        &current_archives,
        sync_checked,
        &mut result,
    );

    Ok(result)
}

/// Locate `<game_dir>/<root>/<name>.sga`, trying exact case then lowercase.
/// The module file uses Windows casing (e.g. `Reflect`) but the actual depot
/// file on Linux is lowercase (`reflect.sga`).
fn resolve_sga(game_dir: &Path, root: &str, name: &str) -> Result<PathBuf, String> {
    for candidate in [name.to_string(), name.to_lowercase()] {
        let path = game_dir.join(root).join(format!("{candidate}.sga"));
        if path.exists() {
            return Ok(path);
        }
    }
    Err(format!(
        "SGA archive not found: {root}/{name}.sga (also tried lowercase)"
    ))
}

/// Compute the `(archive_id, archive_version)` pair for a single SGA file.
///
/// - `archive_id`:      MD5 of the UTF-16LE internal name at file offset `0x0C`
///                      (up to the first null-pair), formatted in MS-GUID byte order.
/// - `archive_version`: MD5 of all file bytes from the TOC offset (`u32_le` at `0x8C`)
///                      to EOF, formatted in MS-GUID byte order.
fn compute_archive_id_version(path: &Path) -> Result<(String, String), String> {
    let data =
        std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if data.len() < 0x90 {
        return Err(format!("{}: file too short to be a valid SGA", path.display()));
    }

    // Internal name: UTF-16LE starting at 0x0C, terminated by a \x00\x00 pair.
    let mut end = 0x0C;
    while end + 1 < data.len() && !(data[end] == 0 && data[end + 1] == 0) {
        end += 2;
    }
    let name_bytes = &data[0x0C..end];
    let id = ms_guid_hex(md5::Md5::digest(name_bytes).as_ref());

    // TOC offset stored as u32 LE at 0x8C.
    let toc = u32::from_le_bytes(data[0x8C..0x90].try_into().unwrap()) as usize;
    if toc > data.len() {
        return Err(format!("{}: TOC offset 0x{toc:X} is out of bounds", path.display()));
    }
    let ver = ms_guid_hex(md5::Md5::digest(&data[toc..]).as_ref());

    Ok((id, ver))
}

/// Format a 16-byte MD5 digest as 32 lowercase hex chars in Microsoft GUID
/// field byte-order: Data1 (4 bytes) reversed, Data2 (2 bytes) reversed,
/// Data3 (2 bytes) reversed, Data4 (8 bytes) sequential.
fn ms_guid_hex(raw: &[u8]) -> String {
    assert_eq!(raw.len(), 16);
    format!(
        "{:02x}{:02x}{:02x}{:02x}\
         {:02x}{:02x}\
         {:02x}{:02x}\
         {:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        raw[3], raw[2], raw[1], raw[0],
        raw[5], raw[4],
        raw[7], raw[6],
        raw[8], raw[9], raw[10], raw[11], raw[12], raw[13], raw[14], raw[15],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ms_guid_hex() {
        // Known value from the investigation: MD5("AnvilAttributes" in UTF-16LE)
        // produces raw bytes that format to "cc658689e8327aa213f047ca598ee0c9"
        // in MS-GUID byte order.
        let raw: [u8; 16] = [
            0x89, 0x86, 0x65, 0xcc, // Data1 bytes (stored LE, reversed to cc658689)
            0x32, 0xe8,             // Data2 bytes (reversed to e832)
            0xa2, 0x7a,             // Data3 bytes (reversed to 7aa2)
            0x13, 0xf0, 0x47, 0xca, 0x59, 0x8e, 0xe0, 0xc9, // Data4 sequential
        ];
        assert_eq!(ms_guid_hex(&raw), "cc658689e8327aa213f047ca598ee0c9");
    }

    #[test]
    fn test_parse_module_sync_checked() {
        let module_text = "\
[attrib:common]
required = 1
syncChecked = 1
archiveRoot = anvil\\archives
archive.01 = Attrib

[audio:common]
required = 1
archiveRoot = anvil\\archives
archive.01 = SoundCommon

[data:common]
required = 1
syncChecked = 1
archiveRoot = engine\\archives
archive.01 = Data
archive.02 = UI
";
        let sections = parse_module_text(module_text).unwrap();

        // Only syncChecked sections should appear.
        assert!(sections.contains_key("attrib"));
        assert!(!sections.contains_key("audio"));
        assert!(sections.contains_key("data"));

        let attrib = &sections["attrib"];
        assert_eq!(attrib.len(), 1);
        assert_eq!(attrib[0], ("anvil\\archives".to_string(), "Attrib".to_string()));

        let data = &sections["data"];
        assert_eq!(data.len(), 2);
        assert_eq!(data[0], ("engine\\archives".to_string(), "Data".to_string()));
        assert_eq!(data[1], ("engine\\archives".to_string(), "UI".to_string()));
    }
}
