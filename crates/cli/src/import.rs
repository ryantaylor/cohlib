//! Shared core of `cohlib import`, factored out so `cohlib backfill` (which
//! assembles a depot snapshot itself rather than reading one already on disk)
//! can run the exact same extraction pipeline against it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use indicatif::ProgressBar;

use crate::images::{self, ImagesConfig};
use crate::{bar_style, checksums, semver, spinner_style, write_scenarios};

/// Extracts entity data, locale, scenarios, the dataChecksum and marketing
/// semver from `depot_path`, and writes `<output_dir>/<version>/game_data.json`.
/// Returns an error (never calls `process::exit`) so callers — `cmd_import`,
/// and `cohlib backfill` running this once per historical build — can decide
/// how to handle a single build's failure.
pub fn run_import(
    depot_path: &Path,
    version: u32,
    output_dir: &Path,
    images_config: Option<&ImagesConfig>,
    scenarios_sga_path: &Path,
) -> Result<(), String> {
    let attrib_sga = depot_path
        .join("anvil")
        .join("archives")
        .join("ReferenceAttributes.sga");

    if !attrib_sga.exists() {
        return Err(format!(
            "ReferenceAttributes.sga not found at {}",
            attrib_sga.display()
        ));
    }

    let locale_sga = depot_path
        .join("anvil")
        .join("archives")
        .join("LocaleEnglish.sga");

    let locale = if locale_sga.exists() {
        let pb = ProgressBar::new_spinner();
        pb.set_style(spinner_style());
        pb.set_message(format!(
            "Extracting locale from {}...",
            locale_sga.display()
        ));
        pb.enable_steady_tick(Duration::from_millis(80));
        match locale::parse_locale_sga(&locale_sga) {
            Ok(l) => {
                pb.finish_with_message(format!("Locale: {} strings", l.0.len()));
                l
            }
            Err(e) => {
                pb.finish_with_message(format!("Locale: extraction failed: {e}"));
                data::LocaleStore(std::collections::BTreeMap::new())
            }
        }
    } else {
        eprintln!("LocaleEnglish.sga not found, skipping locale");
        data::LocaleStore(std::collections::BTreeMap::new())
    };

    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style());
    pb.set_message(format!("Reading {}...", attrib_sga.display()));
    pb.enable_steady_tick(Duration::from_millis(80));
    let entries = match sga::open_archive(&attrib_sga) {
        Ok(e) => {
            pb.finish_with_message(format!("SGA: {} files", e.len()));
            e
        }
        Err(e) => {
            pb.finish_with_message(format!("error reading SGA archive: {e}"));
            return Err(format!("cannot read {}: {e}", attrib_sga.display()));
        }
    };

    let xml_count = entries
        .iter()
        .filter(|e| e.path.starts_with("instances/") && e.extension() == Some("xml"))
        .count() as u64;

    let pb = ProgressBar::new(xml_count);
    pb.set_style(bar_style());
    pb.set_message("Parsing entity XML");
    let mut gd = match attrib::extract_game_data(&entries, locale, version, || pb.inc(1)) {
        Ok(gd) => gd,
        Err(e) => {
            pb.finish_with_message(format!("error extracting game data: {e}"));
            return Err(format!("extracting game data: {e}"));
        }
    };
    pb.finish_with_message(format!(
        "Game data: entities={} squads={} upgrades={} abilities={}",
        gd.entities.len(),
        gd.squads.len(),
        gd.upgrades.len(),
        gd.abilities.len(),
    ));

    match checksums::compute_data_checksum(depot_path) {
        Ok(data_checksum) => gd.data_checksum = Some(data_checksum),
        Err(e) => eprintln!("warning: could not compute dataChecksum: {e}"),
    }

    let exe_path = depot_path.join("RelicCoH3.exe");
    match semver::derive_semver(&exe_path) {
        Ok(s) => {
            eprintln!("Derived marketing semver: {s} (build {version})");
            gd.semver = Some(s);
        }
        Err(e) => {
            eprintln!("warning: could not derive marketing semver: {e}");
        }
    }

    if scenarios_sga_path.exists() {
        match sga::open_archive(scenarios_sga_path) {
            Ok(entries) => {
                let scenarios = scenario::extract_scenarios(&entries, &gd);
                eprintln!("Scenarios: {} extracted", scenarios.len());
                match write_scenarios(output_dir, &scenarios) {
                    Ok(refs) => gd.scenarios = refs,
                    Err(e) => eprintln!("warning: writing scenario records failed: {e}"),
                }
            }
            Err(e) => eprintln!("warning: cannot open {}: {e}", scenarios_sga_path.display()),
        }
    } else {
        eprintln!(
            "ScenariosMP.sga not found at {}, skipping scenario extraction",
            scenarios_sga_path.display()
        );
    }

    let version_str = version.to_string();
    let out_version_dir = output_dir.join(&version_str);
    let out_path = out_version_dir.join("game_data.json");

    std::fs::create_dir_all(&out_version_dir)
        .map_err(|e| format!("cannot create {}: {e}", out_version_dir.display()))?;

    let json = serde_json::to_string_pretty(&gd).expect("serialize failed");
    std::fs::write(&out_path, json)
        .map_err(|e| format!("cannot write {}: {e}", out_path.display()))?;

    eprintln!("Written to {}", out_path.display());

    if let Some(cfg) = images_config {
        if let Err(e) = images::extract_images(cfg, version) {
            eprintln!("warning: icon extraction failed: {e}");
        }
    }

    Ok(())
}

/// Default `ScenariosMP.sga` path under a depot root — extracted so `cohlib
/// backfill` can build the same default without duplicating the join chain.
pub fn default_scenarios_sga(depot_path: &Path) -> PathBuf {
    depot_path
        .join("anvil")
        .join("archives")
        .join("ScenariosMP.sga")
}
