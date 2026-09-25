//! cohlib CLI — maintainer tooling for managing the bundled game data.

use std::{
    path::{Path, PathBuf},
    process,
};

use cohlib::{extract_build_order, Replay, VersionedStore};
use indicatif::ProgressStyle;

mod backfill;
mod checksums;
mod depot;
mod grid;
mod images;
mod import;
mod semver;

fn spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.cyan} {msg}")
        .unwrap()
        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", ""])
}

fn bar_style() -> ProgressStyle {
    ProgressStyle::with_template("{msg} [{bar:40.cyan/blue}] {pos}/{len}")
        .unwrap()
        .progress_chars("##-")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("populate") => cmd_populate(&args[2..]),
        Some("import") => cmd_import(&args[2..]),
        Some("sort-data") => cmd_sort_data(&args[2..]),
        Some("build-order") => cmd_build_order(&args[2..]),
        Some("grid") => cmd_grid(&args[2..]),
        Some("backfill") => cmd_backfill(&args[2..]),
        _ => {
            eprintln!("Usage:");
            eprintln!("  cohlib populate <source_dir>... --output <data_dir>");
            eprintln!("  cohlib import <depot_path> [--version <build_number>] --output <data_dir> [--images <dir>] [--icons-sga <path>] [--scenarios-sga <path>]");
            eprintln!("  cohlib sort-data <data_dir>");
            eprintln!("  cohlib build-order <replay_path>");
            eprintln!("  cohlib grid <depot_path> --output <dir>");
            eprintln!(
                "  cohlib backfill <build_number> --manifest <id> --output <data_dir> \
                 [--module-manifest <id>] [--workdir <dir>] [--images <dir>] \
                 [--depotdownloader <path>] [--username <user>] [--app <id>] \
                 [--depot <id>] [--module-depot <id>]"
            );
            process::exit(1);
        }
    }
}

/// Populate data/ from one or more cohdata/reinforce source directories.
///
/// Each source directory is expected to contain per-version subdirectories
/// (e.g. `10612/`) with abilities.json, ebps.json, sbps.json, upgrade.json,
/// and optionally locale.txt or locale.json.
///
/// Usage: cohlib populate <source_dir>... --output <data_dir>
fn cmd_populate(args: &[String]) {
    let (source_dirs, output_dir) = parse_populate_args(args);

    std::fs::create_dir_all(&output_dir).unwrap_or_else(|e| {
        eprintln!("Cannot create output dir {}: {e}", output_dir.display());
        process::exit(1);
    });

    let mut imported = 0usize;

    for source_dir in &source_dirs {
        let entries = match std::fs::read_dir(source_dir) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Cannot read {}: {e}", source_dir.display());
                continue;
            }
        };

        for entry in entries.flatten() {
            let version_dir = entry.path();
            if !version_dir.is_dir() {
                continue;
            }
            let version_str = version_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            let version: u32 = match version_str.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };

            let out_version_dir = output_dir.join(version_str);
            let out_path = out_version_dir.join("game_data.json");

            // Skip if already exists (first source wins per version).
            if out_path.exists() {
                continue;
            }

            match json_import::import_version(&version_dir, version) {
                Ok(gd) => {
                    std::fs::create_dir_all(&out_version_dir).unwrap_or_else(|e| {
                        eprintln!("Cannot create {}: {e}", out_version_dir.display());
                    });
                    let json = serde_json::to_string_pretty(&gd).expect("serialize failed");
                    std::fs::write(&out_path, json).unwrap_or_else(|e| {
                        eprintln!("Cannot write {}: {e}", out_path.display());
                    });
                    println!("  imported version {version}");
                    imported += 1;
                }
                Err(e) => {
                    eprintln!("  error importing version {version}: {e}");
                }
            }
        }
    }

    println!("Done. Imported {imported} versions.");
}

fn parse_populate_args(args: &[String]) -> (Vec<PathBuf>, PathBuf) {
    let mut source_dirs = Vec::new();
    let mut output_dir = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--output" {
            i += 1;
            output_dir = args.get(i).map(PathBuf::from);
        } else {
            source_dirs.push(PathBuf::from(&args[i]));
        }
        i += 1;
    }
    let output_dir = output_dir.unwrap_or_else(|| {
        eprintln!("--output <data_dir> is required");
        process::exit(1);
    });
    if source_dirs.is_empty() {
        eprintln!("At least one source directory is required");
        process::exit(1);
    }
    (source_dirs, output_dir)
}

/// Import a single game version from an SGA depot.
///
/// Extracts entity data from `anvil/archives/ReferenceAttributes.sga` and writes
/// a `game_data.json` file to `<output_dir>/<version>/game_data.json`.
///
/// Locale strings are not extracted (LocaleEnglish.sga uses AES-128 encryption
/// whose key is not available statically). Use `cohlib populate` to include locale
/// from pre-processed JSON files.
///
/// Usage: cohlib import <depot_path> [--version <build_number>] --output <data_dir>
fn cmd_import(args: &[String]) {
    let (depot_path, version, output_dir, images_config, scenarios_sga_path) =
        parse_import_args(args);

    if let Err(e) = import::run_import(
        &depot_path,
        version,
        &output_dir,
        images_config.as_ref(),
        &scenarios_sga_path,
    ) {
        eprintln!("error: {e}");
        process::exit(1);
    }
}

/// Writes each extracted [`data::Scenario`] to `{output_dir}/scenarios/{hash}.json`,
/// content-addressed by a hash of its serialized form, and returns the
/// `scenario_path -> hash` map for [`data::GameData::scenarios`].
///
/// Scenario records rarely change between game versions, so this dedup keeps
/// the bundled data's growth roughly flat as versions accumulate: re-running
/// `import` on an unchanged map writes the same hash and overwrites the same
/// file with identical bytes rather than adding a new one.
fn write_scenarios(
    output_dir: &Path,
    scenarios: &std::collections::BTreeMap<String, data::Scenario>,
) -> std::io::Result<std::collections::BTreeMap<String, String>> {
    let scenarios_dir = output_dir.join("scenarios");
    std::fs::create_dir_all(&scenarios_dir)?;

    let mut refs = std::collections::BTreeMap::new();
    for (path, scenario) in scenarios {
        let json = serde_json::to_vec(scenario).expect("serialize scenario failed");
        let hash = scenario_hash(&json);
        std::fs::write(scenarios_dir.join(format!("{hash}.json")), &json)?;
        refs.insert(path.clone(), hash);
    }
    Ok(refs)
}

/// Short SHA-256 hex digest used as a scenario record's content-addressed key.
/// 16 hex chars (64 bits) is far more than enough to avoid collisions across
/// the low hundreds of unique scenario records this bundle will ever hold.
fn scenario_hash(json: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(json);
    format!("{:x}", h.finalize())[..16].to_string()
}

fn read_exe_version(exe_path: &Path) -> Result<u32, String> {
    let bytes =
        std::fs::read(exe_path).map_err(|e| format!("cannot read {}: {e}", exe_path.display()))?;

    // "VS_VERSION_INFO" as UTF-16 LE
    const MAGIC: &[u8] =
        b"V\x00S\x00_\x00V\x00E\x00R\x00S\x00I\x00O\x00N\x00_\x00I\x00N\x00F\x00O\x00";
    let magic_pos = bytes
        .windows(MAGIC.len())
        .position(|w| w == MAGIC)
        .ok_or_else(|| "VS_VERSION_INFO signature not found in PE file".to_string())?;

    // Skip magic (15 UTF-16 chars = 30 bytes) + null terminator (2 bytes), then align to 4
    let after_magic = magic_pos + MAGIC.len() + 2;
    let aligned = (after_magic + 3) & !3;

    // Locate VS_FIXEDFILEINFO by its signature 0xFEEF04BD
    const FIXEDINFO_SIG: &[u8] = &[0xBD, 0x04, 0xEF, 0xFE];
    let search_end = (aligned + 64).min(bytes.len().saturating_sub(24));
    let sig_pos = bytes[aligned..search_end]
        .windows(FIXEDINFO_SIG.len())
        .position(|w| w == FIXEDINFO_SIG)
        .map(|p| aligned + p)
        .ok_or_else(|| "VS_FIXEDFILEINFO signature not found".to_string())?;

    if sig_pos + 24 > bytes.len() {
        return Err("PE file truncated before end of VS_FIXEDFILEINFO".to_string());
    }

    // VS_FIXEDFILEINFO layout (u32, LE):
    //  +0  dwSignature
    //  +4  dwStrucVersion
    //  +8  dwFileVersionMS
    //  +12 dwFileVersionLS
    //  +16 dwProductVersionMS = (major << 16) | minor
    //  +20 dwProductVersionLS = (build << 16) | revision  <- build number
    let product_version_ls = u32::from_le_bytes(
        bytes[sig_pos + 20..sig_pos + 24]
            .try_into()
            .expect("4 bytes"),
    );
    Ok(product_version_ls >> 16)
}

fn parse_import_args(
    args: &[String],
) -> (PathBuf, u32, PathBuf, Option<images::ImagesConfig>, PathBuf) {
    let mut depot_path = None;
    let mut version = None;
    let mut output_dir = None;
    let mut images_dir: Option<PathBuf> = None;
    let mut icons_sga: Option<PathBuf> = None;
    let mut scenarios_sga: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--version" => {
                i += 1;
                version = args.get(i).and_then(|s| s.parse().ok());
            }
            "--output" => {
                i += 1;
                output_dir = args.get(i).map(PathBuf::from);
            }
            "--images" => {
                i += 1;
                images_dir = args.get(i).map(PathBuf::from);
            }
            "--icons-sga" => {
                i += 1;
                icons_sga = args.get(i).map(PathBuf::from);
            }
            "--scenarios-sga" => {
                i += 1;
                scenarios_sga = args.get(i).map(PathBuf::from);
            }
            _ if depot_path.is_none() => {
                depot_path = Some(PathBuf::from(&args[i]));
            }
            _ => {}
        }
        i += 1;
    }
    let depot_path = depot_path.unwrap_or_else(|| {
        eprintln!("<depot_path> is required");
        process::exit(1);
    });
    let version = match version {
        Some(v) => v,
        None => {
            let exe_path = depot_path.join("RelicCoH3.exe");
            match read_exe_version(&exe_path) {
                Ok(v) => {
                    eprintln!("auto-detected version {v} from {}", exe_path.display());
                    v
                }
                Err(e) => {
                    eprintln!(
                        "error: --version not provided and could not auto-detect from {}: {e}",
                        exe_path.display()
                    );
                    eprintln!("hint: pass --version <build_number> explicitly");
                    process::exit(1);
                }
            }
        }
    };
    let output_dir = output_dir.unwrap_or_else(|| {
        eprintln!("--output <data_dir> is required");
        process::exit(1);
    });
    // Resolved unconditionally (not just under --images): scenario dimensions are
    // extracted from the same archive on every import, regardless of whether image
    // extraction was requested.
    let scenarios_sga_path = scenarios_sga.unwrap_or_else(|| {
        depot_path
            .join("anvil")
            .join("archives")
            .join("ScenariosMP.sga")
    });
    let images_config = images_dir.map(|dir| images::ImagesConfig {
        icons_sga: icons_sga
            .unwrap_or_else(|| depot_path.join("anvil").join("archives").join("UI.sga")),
        scenarios_sga: Some(scenarios_sga_path.clone()),
        images_dir: dir,
    });
    (
        depot_path,
        version,
        output_dir,
        images_config,
        scenarios_sga_path,
    )
}

/// Re-serialize all game_data.json files in a data directory through the current
/// data model, producing deterministically-sorted output.
///
/// Usage: cohlib sort-data <data_dir>
fn cmd_sort_data(args: &[String]) {
    let data_dir = match args.first() {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("Usage: cohlib sort-data <data_dir>");
            process::exit(1);
        }
    };

    let entries = match std::fs::read_dir(&data_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Cannot read {}: {e}", data_dir.display());
            process::exit(1);
        }
    };

    let mut sorted = 0usize;
    for entry in entries.flatten() {
        let path = entry.path().join("game_data.json");
        if !path.exists() {
            continue;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Cannot read {}: {e}", path.display());
                continue;
            }
        };
        let gd: data::GameData = match serde_json::from_str(&text) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("Cannot parse {}: {e}", path.display());
                continue;
            }
        };
        let json = serde_json::to_string_pretty(&gd).expect("serialize failed");
        if let Err(e) = std::fs::write(&path, json) {
            eprintln!("Cannot write {}: {e}", path.display());
            continue;
        }
        println!("  sorted {}", path.display());
        sorted += 1;
    }

    println!("Done. Sorted {sorted} versions.");
}

/// Extract terrain grids for the hack detection design's Zoomhack certificate.
///
/// Usage: cohlib grid <depot_path> --output <dir>
fn cmd_grid(args: &[String]) {
    let (depot_path, output_dir) = grid::parse_grid_args(args);
    grid::run(&depot_path, &output_dir);
}

/// Pull one historical build's depot files via DepotDownloader and run the
/// full extraction pipeline against them. See `backfill.rs`'s module doc
/// comment for what's untested about this.
///
/// Usage: cohlib backfill <build_number> --manifest <id> --output <data_dir> [...]
fn cmd_backfill(args: &[String]) {
    let parsed = backfill::parse_backfill_args(args);
    backfill::run(parsed);
}

fn cmd_build_order(args: &[String]) {
    let replay_path = match args.first() {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("Usage: cohlib build-order <replay_path>");
            process::exit(1);
        }
    };

    let data = std::fs::read(&replay_path).unwrap_or_else(|e| {
        eprintln!("Error reading replay file: {e}");
        process::exit(1);
    });

    let replay = Replay::from_bytes(&data).unwrap_or_else(|e| {
        eprintln!("Error parsing replay: {e}");
        process::exit(1);
    });

    let store = VersionedStore::bundled();
    println!("Replay version: {}", replay.version());
    println!("Map: {}", replay.map().filename());
    println!("------------------------------------------------------------");

    let players = replay.players();
    for (idx, player) in players.iter().enumerate() {
        println!("Player {}: {} ({:?})", idx, player.name(), player.faction());

        let build_order = match extract_build_order(&replay, idx, &store, true) {
            Ok(bo) => bo,
            Err(e) => {
                eprintln!("  Error extracting build order: {e}");
                continue;
            }
        };

        for action in build_order.actions {
            let name = store
                .local_name_for_formatted(action.pbgid, replay.version() as u32)
                .unwrap_or_else(|| format!("Unknown ({})", action.pbgid));

            let minutes = action.tick / 8 / 60;
            let seconds = (action.tick / 8) % 60;

            let mut status = String::new();
            if action.cancelled {
                status.push_str(" [CANCELLED]");
            }
            if action.suspect_since.is_some() {
                status.push_str(" [SUSPECT]");
            }

            println!(
                "  {:02}:{:02}  {:<25}  {:<10}  {}{}",
                minutes,
                seconds,
                format!("{:?}", action.kind),
                action.pbgid,
                name,
                status
            );
        }
        println!();
    }
}
