//! Image extraction, hashing, and release packaging for `cohlib import`.
//!
//! Produces per-version `hashes.json` files (to be committed alongside
//! `data/<version>/game_data.json`) and three release artifacts that are
//! published externally and not committed:
//!
//! - `manifest.json`  — full image list, delta, and removed images with their hashes
//! - `full.tar.gz`    — all current WebP files (for fresh consumer installs)
//! - `delta.tar.gz`   — only new or changed images (for incremental consumer updates)

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Configuration passed from `cmd_import` argument parsing.
pub struct ImagesConfig {
    /// Root directory where `<version>/hashes.json` and release artifacts are written.
    pub images_dir: PathBuf,
    /// Path to the SGA archive containing icon `.rrtex` files (UI.sga).
    pub icons_sga: PathBuf,
    /// Path to the SGA archive containing map minimap `.rrtex` files (ScenariosMP.sga).
    pub scenarios_sga: Option<PathBuf>,
}

/// Content of `<images_dir>/<version>/hashes.json`.
///
/// Committed to the repository. Maps icon_name → SHA-256 hex, or `null` for
/// icons removed in this version. Provides the baseline for the next import run.
type HashesMap = BTreeMap<String, Option<String>>;

/// Release manifest — written as `<images_dir>/<version>/manifest.json`.
#[derive(serde::Serialize)]
struct Manifest {
    version: u32,
    /// All icons present in this version: icon_name → sha256_hex.
    full: BTreeMap<String, String>,
    /// Icons that are new or changed since the previous version: icon_name → sha256_hex.
    delta: BTreeMap<String, String>,
    /// Icons present in the previous version but absent in this one.
    /// Consumers should retain existing files for these names (needed by older replays).
    removed: Vec<String>,
}

/// Extract icons and map images, diff against the previous version's hashes,
/// and write all output files under `config.images_dir/<version>/`.
pub fn extract_images(config: &ImagesConfig, version: u32) -> Result<(), String> {
    if !config.icons_sga.exists() {
        return Err(format!(
            "icons SGA not found at {}",
            config.icons_sga.display()
        ));
    }

    let mut current_icons = extract_webp_icons(&config.icons_sga)?;
    eprintln!("  {} icons converted to WebP", current_icons.len());

    if let Some(scenarios_sga) = &config.scenarios_sga {
        if scenarios_sga.exists() {
            let maps = extract_webp_maps(scenarios_sga)?;
            eprintln!("  {} map images converted to WebP", maps.len());
            current_icons.extend(maps);
        } else {
            eprintln!(
                "  ScenariosMP.sga not found at {}, skipping map images",
                scenarios_sga.display()
            );
        }
    }

    let current_hashes: BTreeMap<String, String> = current_icons
        .keys()
        .map(|k| (k.clone(), sha256_hex(&current_icons[k])))
        .collect();

    let prev_hashes = load_prev_hashes(&config.images_dir, version).unwrap_or_default();

    // Classify each current icon as unchanged, new, or changed.
    let mut hashes: HashesMap = BTreeMap::new();
    let mut full: BTreeMap<String, String> = BTreeMap::new();
    let mut delta_keys: Vec<String> = Vec::new();

    for (name, hash) in &current_hashes {
        full.insert(name.clone(), hash.clone());
        hashes.insert(name.clone(), Some(hash.clone()));
        let prev = prev_hashes.get(name).and_then(|h| h.as_deref());
        if prev != Some(hash.as_str()) {
            delta_keys.push(name.clone());
        }
    }

    // Icons present in the previous version but absent now.
    let mut removed: Vec<String> = prev_hashes
        .iter()
        .filter(|(name, prev_hash)| {
            prev_hash.is_some() && !current_hashes.contains_key(name.as_str())
        })
        .map(|(name, _)| name.clone())
        .collect();
    removed.sort();

    for name in &removed {
        hashes.insert(name.clone(), None);
    }

    let delta: BTreeMap<String, String> = delta_keys
        .iter()
        .filter_map(|n| current_hashes.get(n).map(|h| (n.clone(), h.clone())))
        .collect();

    eprintln!(
        "  {} new/changed, {} removed (vs previous version)",
        delta.len(),
        removed.len()
    );

    let out_dir = config.images_dir.join(version.to_string());
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;

    // hashes.json — committed to repo
    let hashes_path = out_dir.join("hashes.json");
    let hashes_json =
        serde_json::to_string_pretty(&hashes).map_err(|e| format!("serialize hashes: {e}"))?;
    std::fs::write(&hashes_path, hashes_json)
        .map_err(|e| format!("write {}: {e}", hashes_path.display()))?;
    eprintln!("  hashes.json written to {}", hashes_path.display());

    // manifest.json — release artifact
    let manifest = Manifest {
        version,
        full,
        delta,
        removed,
    };
    let manifest_path = out_dir.join("manifest.json");
    let manifest_json =
        serde_json::to_string_pretty(&manifest).map_err(|e| format!("serialize manifest: {e}"))?;
    std::fs::write(&manifest_path, manifest_json)
        .map_err(|e| format!("write {}: {e}", manifest_path.display()))?;
    eprintln!("  manifest.json written");

    // full.tar.gz — release artifact
    let full_path = out_dir.join("full.tar.gz");
    write_tar_gz(&full_path, &current_icons)?;
    eprintln!("  full.tar.gz written ({} files)", current_icons.len());

    // delta.tar.gz — release artifact
    let delta_icons: BTreeMap<String, Vec<u8>> = delta_keys
        .iter()
        .filter_map(|n| current_icons.get(n).map(|v| (n.clone(), v.clone())))
        .collect();
    let delta_path = out_dir.join("delta.tar.gz");
    write_tar_gz(&delta_path, &delta_icons)?;
    eprintln!("  delta.tar.gz written ({} files)", delta_icons.len());

    Ok(())
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Open `sga_path` and convert every `.rrtex` entry to WebP.
///
/// Returns `icon_name → WebP bytes`. Entries that fail conversion are logged
/// to stderr and skipped (non-fatal).
fn extract_webp_icons(sga_path: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let entries = sga::open_archive(sga_path).map_err(|e| format!("cannot open icons SGA: {e}"))?;

    let mut icons = BTreeMap::new();
    let mut skipped = 0usize;

    for entry in &entries {
        let Some(icon_name) = icon_name_from_path(&entry.path) else {
            continue;
        };
        match image::extract_icon(&entry.bytes) {
            Ok(webp) => {
                icons.insert(icon_name.to_string(), webp);
            }
            Err(e) => {
                eprintln!("  warning: skipping {}: {e}", entry.path);
                skipped += 1;
            }
        }
    }

    if skipped > 0 {
        eprintln!("  ({skipped} icons skipped due to conversion errors)");
    }
    Ok(icons)
}

/// Derive an `icon_name` from an SGA archive entry path.
///
/// Strips known leading prefixes (`instances/`, `ui/icons/`) and the `.rrtex` extension.
/// Returns `None` for entries that are not `.rrtex` files.
fn icon_name_from_path(path: &str) -> Option<&str> {
    let s = path
        .strip_prefix("instances/")
        .or_else(|| path.strip_prefix("ui/icons/"))
        .unwrap_or(path);
    s.strip_suffix(".rrtex")
}

/// Open `sga_path` and convert every map minimap `.rrtex` entry to WebP.
///
/// Only `_mm_generated.rrtex` and `_mm_handmade.rrtex` files are included.
/// Large terrain texture variants (`_tmt_*`) and backup copies are skipped.
///
/// Returns `map_name → WebP bytes`. Entries that fail conversion are logged and skipped.
fn extract_webp_maps(sga_path: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let entries =
        sga::open_archive(sga_path).map_err(|e| format!("cannot open scenarios SGA: {e}"))?;

    let mut maps = BTreeMap::new();
    let mut skipped = 0usize;

    for entry in &entries {
        let Some(map_name) = map_name_from_path(&entry.path) else {
            continue;
        };
        match image::extract_icon(&entry.bytes) {
            Ok(webp) => {
                maps.insert(map_name.to_string(), webp);
            }
            Err(e) => {
                eprintln!("  warning: skipping {}: {e}", entry.path);
                skipped += 1;
            }
        }
    }

    if skipped > 0 {
        eprintln!("  ({skipped} map images skipped due to conversion errors)");
    }
    Ok(maps)
}

/// Derive a `map_name` from a ScenariosMP.sga archive entry path.
///
/// Only includes `_mm_generated.rrtex` and `_mm_handmade.rrtex` files — the
/// per-map minimap images. Terrain texture variants (`_tmt_*`) and backup files
/// (`* - backup.rrtex`) are excluded by not matching these suffixes.
///
/// Strips the `scenarios/` leading prefix and the `.rrtex` extension.
/// Returns `None` for non-matching entries.
fn map_name_from_path(path: &str) -> Option<&str> {
    if !path.ends_with("_mm_generated.rrtex") && !path.ends_with("_mm_handmade.rrtex") {
        return None;
    }
    let s = path.strip_prefix("scenarios/").unwrap_or(path);
    s.strip_suffix(".rrtex")
}

/// SHA-256 hash of `data`, returned as a lowercase hex string.
fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    format!("{:x}", h.finalize())
}

/// Load the `hashes.json` from the most recent version in `images_dir` that is
/// older than `current_version`. Returns `None` if no prior version exists.
fn load_prev_hashes(images_dir: &Path, current_version: u32) -> Option<HashesMap> {
    let prev_version = std::fs::read_dir(images_dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
        .filter(|&v| v < current_version)
        .max()?;

    let path = images_dir
        .join(prev_version.to_string())
        .join("hashes.json");
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Write a `.tar.gz` archive to `dest_path`.
///
/// Each entry in `icons` is stored as `<icon_name>.webp`, preserving
/// slash-separated subdirectory structure within the archive.
fn write_tar_gz(dest_path: &Path, icons: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    let file = std::fs::File::create(dest_path)
        .map_err(|e| format!("cannot create {}: {e}", dest_path.display()))?;
    let gz = flate2::write::GzEncoder::new(file, flate2::Compression::best());
    let mut builder = tar::Builder::new(gz);

    for (icon_name, webp) in icons {
        let tar_path = format!("{icon_name}.webp");
        let mut header = tar::Header::new_gnu();
        header.set_size(webp.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0); // deterministic archives
        header.set_cksum();
        builder
            .append_data(&mut header, &tar_path, webp.as_slice())
            .map_err(|e| format!("tar append error for {icon_name}: {e}"))?;
    }

    builder
        .into_inner()
        .map_err(|e| format!("tar finish: {e}"))?
        .finish()
        .map_err(|e| format!("gzip finish: {e}"))?;

    Ok(())
}
