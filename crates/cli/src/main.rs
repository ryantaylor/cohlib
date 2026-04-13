//! cohlib CLI — maintainer tooling for managing the bundled game data.

use std::{path::PathBuf, process};

mod images;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("populate") => cmd_populate(&args[2..]),
        Some("import") => cmd_import(&args[2..]),
        _ => {
            eprintln!("Usage:");
            eprintln!("  cohlib populate <source_dir>... --output <data_dir>");
            eprintln!("  cohlib import <depot_path> --version <build_number> --output <data_dir> [--images <dir>] [--icons-sga <path>] [--scenarios-sga <path>]");
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
/// Usage: cohlib import <depot_path> --version <build_number> --output <data_dir>
fn cmd_import(args: &[String]) {
    let (depot_path, version, output_dir, images_config) = parse_import_args(args);

    let attrib_sga = depot_path
        .join("anvil")
        .join("archives")
        .join("ReferenceAttributes.sga");

    if !attrib_sga.exists() {
        eprintln!(
            "error: ReferenceAttributes.sga not found at {}",
            attrib_sga.display()
        );
        process::exit(1);
    }

    let locale_sga = depot_path
        .join("anvil")
        .join("archives")
        .join("LocaleEnglish.sga");

    let locale = if locale_sga.exists() {
        eprintln!("Extracting locale from {}...", locale_sga.display());
        match locale::parse_locale_sga(&locale_sga) {
            Ok(l) => {
                eprintln!("  {} locale strings loaded", l.0.len());
                l
            }
            Err(e) => {
                eprintln!("  warning: locale extraction failed: {e}");
                data::LocaleStore(std::collections::HashMap::new())
            }
        }
    } else {
        eprintln!("  LocaleEnglish.sga not found, skipping locale");
        data::LocaleStore(std::collections::HashMap::new())
    };

    eprintln!("Extracting from {}...", attrib_sga.display());
    let entries = match sga::open_archive(&attrib_sga) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error reading SGA archive: {e}");
            process::exit(1);
        }
    };
    eprintln!("  {} files extracted", entries.len());

    let gd = match attrib::extract_game_data(&entries, locale, version) {
        Ok(gd) => gd,
        Err(e) => {
            eprintln!("error extracting game data: {e}");
            process::exit(1);
        }
    };

    eprintln!(
        "  entities={} squads={} upgrades={} abilities={}",
        gd.entities.len(),
        gd.squads.len(),
        gd.upgrades.len(),
        gd.abilities.len(),
    );

    let version_str = version.to_string();
    let out_version_dir = output_dir.join(&version_str);
    let out_path = out_version_dir.join("game_data.json");

    std::fs::create_dir_all(&out_version_dir).unwrap_or_else(|e| {
        eprintln!("cannot create {}: {e}", out_version_dir.display());
        process::exit(1);
    });

    let json = serde_json::to_string_pretty(&gd).expect("serialize failed");
    std::fs::write(&out_path, json).unwrap_or_else(|e| {
        eprintln!("cannot write {}: {e}", out_path.display());
        process::exit(1);
    });

    eprintln!("Written to {}", out_path.display());

    if let Some(cfg) = &images_config {
        eprintln!("Extracting icons from {}...", cfg.icons_sga.display());
        match images::extract_images(cfg, version) {
            Ok(()) => eprintln!("Icon extraction complete."),
            Err(e) => eprintln!("warning: icon extraction failed: {e}"),
        }
    }
}

fn parse_import_args(args: &[String]) -> (PathBuf, u32, PathBuf, Option<images::ImagesConfig>) {
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
    let version = version.unwrap_or_else(|| {
        eprintln!("--version <build_number> is required");
        process::exit(1);
    });
    let output_dir = output_dir.unwrap_or_else(|| {
        eprintln!("--output <data_dir> is required");
        process::exit(1);
    });
    let images_config = images_dir.map(|dir| images::ImagesConfig {
        icons_sga: icons_sga
            .unwrap_or_else(|| depot_path.join("anvil").join("archives").join("UI.sga")),
        scenarios_sga: Some(scenarios_sga.unwrap_or_else(|| {
            depot_path
                .join("anvil")
                .join("archives")
                .join("ScenariosMP.sga")
        })),
        images_dir: dir,
    });
    (depot_path, version, output_dir, images_config)
}
