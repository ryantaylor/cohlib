//! `cohlib backfill` — pulls a single historical CoH3 build's depot files via
//! `DepotDownloader` and runs the full extraction pipeline (entities, locale,
//! scenarios, terrain grids, images) against them, the same way `cohlib
//! import` does against an already-installed depot.
//!
//! This is the "depot history job" from the hack detection design doc §8.3,
//! extended per its follow-up request: rather than pulling only the two
//! archives the terrain grid command needs, it discovers and pulls every file
//! cohlib's extraction pipeline reads today, so the whole bundled dataset —
//! not just terrain — can eventually be backfilled across CoH3's build
//! history.
//!
//! # What's genuinely untested here
//!
//! Built and unit-tested (argument/filelist construction — see `depot.rs`)
//! without ever running against a real historical manifest: no
//! `DepotDownloader` login was available in the session this was written in,
//! and Steam doesn't publish a simple list of "every manifest id CoH3 has ever
//! shipped" the way it does the *current* one. Known open questions to
//! resolve before trusting real output:
//!
//! - Whether `RelicGame.module` (depot 1677282) actually sits at the fetched
//!   tree's root, as the filelist pattern here assumes.
//! - Whether a (depot 1677281 manifest, depot 1677282 manifest) pair for the
//!   same build needs to be looked up together, or whether they drift
//!   independently — this command takes them as two separate, both-optional
//!   inputs and does not try to correlate them.
//! - Where the list of "which manifest id is which build" itself comes from;
//!   this command takes one manifest id per invocation and has no opinion on
//!   how the caller found it (SteamDB history, `-manifest-only`, etc.).
//!
//! None of these can be resolved without a real, authenticated
//! `DepotDownloader` run, which is intentionally out of scope for this PR.

use std::path::PathBuf;
use std::process;

use crate::depot::{self, DepotDownloaderConfig};
use crate::{checksums, grid, import};

pub struct BackfillArgs {
    pub build: u32,
    pub manifest: u64,
    pub module_manifest: Option<u64>,
    pub output_dir: PathBuf,
    pub workdir: Option<PathBuf>,
    pub images_dir: Option<PathBuf>,
    pub depotdownloader: PathBuf,
    pub app: u32,
    pub depot: u32,
    pub module_depot: u32,
    pub username: Option<String>,
}

pub fn parse_backfill_args(args: &[String]) -> BackfillArgs {
    let mut build = None;
    let mut manifest = None;
    let mut module_manifest = None;
    let mut output_dir = None;
    let mut workdir = None;
    let mut images_dir = None;
    let mut depotdownloader = PathBuf::from("DepotDownloader");
    let mut app = 1677280u32;
    let mut depot = 1677281u32;
    let mut module_depot = 1677282u32;
    let mut username = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--manifest" => {
                i += 1;
                manifest = args.get(i).and_then(|s| s.parse().ok());
            }
            "--module-manifest" => {
                i += 1;
                module_manifest = args.get(i).and_then(|s| s.parse().ok());
            }
            "--output" => {
                i += 1;
                output_dir = args.get(i).map(PathBuf::from);
            }
            "--workdir" => {
                i += 1;
                workdir = args.get(i).map(PathBuf::from);
            }
            "--images" => {
                i += 1;
                images_dir = args.get(i).map(PathBuf::from);
            }
            "--depotdownloader" => {
                i += 1;
                if let Some(p) = args.get(i) {
                    depotdownloader = PathBuf::from(p);
                }
            }
            "--app" => {
                i += 1;
                if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                    app = v;
                }
            }
            "--depot" => {
                i += 1;
                if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                    depot = v;
                }
            }
            "--module-depot" => {
                i += 1;
                if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                    module_depot = v;
                }
            }
            "--username" => {
                i += 1;
                username = args.get(i).cloned();
            }
            _ if build.is_none() => {
                build = args[i].parse().ok();
            }
            _ => {}
        }
        i += 1;
    }

    let build = build.unwrap_or_else(|| {
        eprintln!("<build_number> is required");
        process::exit(1);
    });
    let manifest = manifest.unwrap_or_else(|| {
        eprintln!("--manifest <id> is required (depot {depot}'s manifest for this build)");
        process::exit(1);
    });
    let output_dir = output_dir.unwrap_or_else(|| {
        eprintln!("--output <data_dir> is required");
        process::exit(1);
    });

    BackfillArgs {
        build,
        manifest,
        module_manifest,
        output_dir,
        workdir,
        images_dir,
        depotdownloader,
        app,
        depot,
        module_depot,
        username,
    }
}

/// Files cohlib's own pipeline reads directly, regardless of what
/// `RelicGame.module`'s `syncChecked` sections list (see `depot.rs`'s
/// `build_filelist` doc comment for why these can be a different archive from
/// a `syncChecked` entry with a similar-sounding name).
const DIRECT_FILES: &[&str] = &[
    "anvil/archives/ReferenceAttributes.sga",
    "anvil/archives/ScenariosMP.sga",
    "anvil/archives/LocaleEnglish.sga",
    "anvil/archives/UI.sga",
    "RelicCoH3.exe",
];

pub fn run(args: BackfillArgs) {
    if let Err(e) = run_inner(&args) {
        eprintln!("error: {e}");
        process::exit(1);
    }
}

fn run_inner(args: &BackfillArgs) -> Result<(), String> {
    let workdir = args.workdir.clone().unwrap_or_else(|| {
        args.output_dir
            .join(".backfill-work")
            .join(args.build.to_string())
    });
    std::fs::create_dir_all(&workdir)
        .map_err(|e| format!("cannot create workdir {}: {e}", workdir.display()))?;

    let config = DepotDownloaderConfig {
        binary: args.depotdownloader.clone(),
        app: args.app,
        username: args.username.clone(),
    };

    // Phase 1: RelicGame.module, from the separate module depot, so its
    // syncChecked sections can be read before building the main filelist.
    let sync_sections = if let Some(module_manifest) = args.module_manifest {
        eprintln!(
            "Fetching RelicGame.module (depot {}, manifest {module_manifest})...",
            args.module_depot
        );
        let filelist_path = workdir.join("filelist_module.txt");
        depot::write_filelist(
            &filelist_path,
            &["regex:(?i)^RelicGame\\.module$".to_string()],
        )
        .map_err(|e| format!("cannot write {}: {e}", filelist_path.display()))?;
        depot::fetch(
            &config,
            args.module_depot,
            Some(module_manifest),
            &workdir,
            &filelist_path,
        )?;
        match checksums::parse_module(&workdir.join("RelicGame.module")) {
            Ok(sections) => sections,
            Err(e) => {
                eprintln!("warning: could not parse RelicGame.module: {e}");
                Default::default()
            }
        }
    } else {
        eprintln!(
            "warning: no --module-manifest given; dataChecksum will not be computed for build {}",
            args.build
        );
        Default::default()
    };

    // Phase 2: everything else, from the primary depot — the syncChecked
    // archives (for a reproducible dataChecksum) plus every file cohlib's
    // extraction pipeline reads directly.
    eprintln!(
        "Fetching build {} (depot {}, manifest {})...",
        args.build, args.depot, args.manifest
    );
    let patterns = depot::build_filelist(&sync_sections, DIRECT_FILES);
    let filelist_path = workdir.join("filelist.txt");
    depot::write_filelist(&filelist_path, &patterns)
        .map_err(|e| format!("cannot write {}: {e}", filelist_path.display()))?;
    depot::fetch(
        &config,
        args.depot,
        Some(args.manifest),
        &workdir,
        &filelist_path,
    )?;

    // Phase 3: run the same extraction pipeline `cohlib import` runs against
    // an already-installed depot, now that `workdir` holds one.
    let images_config = args
        .images_dir
        .as_ref()
        .map(|dir| crate::images::ImagesConfig {
            icons_sga: workdir.join("anvil").join("archives").join("UI.sga"),
            scenarios_sga: Some(import::default_scenarios_sga(&workdir)),
            images_dir: dir.clone(),
        });
    import::run_import(
        &workdir,
        args.build,
        &args.output_dir,
        images_config.as_ref(),
        &import::default_scenarios_sga(&workdir),
    )?;

    // Phase 4: terrain grids (design doc §8, and this command's own reason
    // for existing) — same depot snapshot, written alongside game_data.json.
    let terrain_dir = args.output_dir.join(args.build.to_string()).join("terrain");
    if let Err(e) = grid::extract_and_write(&workdir, &terrain_dir) {
        eprintln!("warning: terrain grid extraction failed: {e}");
    }

    eprintln!(
        "Done. Build {} written under {} (depot snapshot kept at {})",
        args.build,
        args.output_dir.display(),
        workdir.display()
    );
    Ok(())
}
