//! `cohlib grid` — terrain grid extraction for the hack detection design's
//! Zoomhack certificate (design doc §8.1). Writes `coarse_grid.csv`,
//! `map_meta.csv` and `tuning.json` to an output directory; the patch
//! pipeline (out of scope for cohlib — see design doc §8.2) is responsible
//! for committing these under `db/hack_detection/terrain/<build>/` and
//! updating `manifest.json` only when content differs from the previous build.

use std::path::{Path, PathBuf};
use std::process;

use terrain::TerrainGrid;

pub fn parse_grid_args(args: &[String]) -> (PathBuf, PathBuf) {
    let mut depot_path = None;
    let mut output_dir = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output" => {
                i += 1;
                output_dir = args.get(i).map(PathBuf::from);
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
    let output_dir = output_dir.unwrap_or_else(|| {
        eprintln!("--output <dir> is required");
        process::exit(1);
    });
    (depot_path, output_dir)
}

pub fn run(depot_path: &Path, output_dir: &Path) {
    let attrib_sga = depot_path
        .join("anvil")
        .join("archives")
        .join("ReferenceAttributes.sga");
    let scenarios_sga = depot_path
        .join("anvil")
        .join("archives")
        .join("ScenariosMP.sga");

    for (label, path) in [
        ("ReferenceAttributes.sga", &attrib_sga),
        ("ScenariosMP.sga", &scenarios_sga),
    ] {
        if !path.exists() {
            eprintln!("error: {label} not found at {}", path.display());
            process::exit(1);
        }
    }

    let attrib_entries = sga::open_archive(&attrib_sga).unwrap_or_else(|e| {
        eprintln!("error reading {}: {e}", attrib_sga.display());
        process::exit(1);
    });
    let tuning = terrain::extract_camera_tuning(&attrib_entries).unwrap_or_else(|e| {
        eprintln!("error extracting camera tuning: {e}");
        process::exit(1);
    });
    let (distance_min, distance_max, pitch_min, pitch_max) =
        tuning.require_complete().unwrap_or_else(|e| {
            eprintln!("error: {e}");
            process::exit(1);
        });
    eprintln!(
        "Camera tuning: distance_min={distance_min} distance_max={distance_max} \
         pitch_min={pitch_min} pitch_max={pitch_max}"
    );

    let scenario_entries = sga::open_archive(&scenarios_sga).unwrap_or_else(|e| {
        eprintln!("error reading {}: {e}", scenarios_sga.display());
        process::exit(1);
    });
    let results = terrain::extract_terrain_grids(&scenario_entries, distance_max);

    let mut grids: Vec<TerrainGrid> = Vec::new();
    let mut failures = 0usize;
    for (map, result) in results {
        match result {
            Ok(grid) => grids.push(grid),
            Err(e) => {
                eprintln!("warning: skipping {map}: {e}");
                failures += 1;
            }
        }
    }
    grids.sort_by(|a, b| a.map.cmp(&b.map));

    eprintln!("Extracted {} map grids ({} failed)", grids.len(), failures);

    std::fs::create_dir_all(output_dir).unwrap_or_else(|e| {
        eprintln!("cannot create {}: {e}", output_dir.display());
        process::exit(1);
    });

    write_coarse_grid_csv(&output_dir.join("coarse_grid.csv"), &grids);
    write_map_meta_csv(&output_dir.join("map_meta.csv"), &grids);
    write_tuning_json(
        &output_dir.join("tuning.json"),
        distance_min,
        distance_max,
        pitch_min,
        pitch_max,
    );

    eprintln!("Written to {}", output_dir.display());
}

fn write_coarse_grid_csv(path: &Path, grids: &[TerrainGrid]) {
    let mut out = String::from("map,bi,bj,tmax\n");
    for g in grids {
        for c in &g.cells {
            out.push_str(&format!("{},{},{},{:.2}\n", g.map, c.bi, c.bj, c.tmax));
        }
    }
    std::fs::write(path, out).unwrap_or_else(|e| {
        eprintln!("cannot write {}: {e}", path.display());
        process::exit(1);
    });
}

fn write_map_meta_csv(path: &Path, grids: &[TerrainGrid]) {
    let mut out = String::from("map,w,h,cx,cz,xhalf,zhalf,block_x,block_z,hmax,heightfield_hash\n");
    for g in grids {
        let m = &g.meta;
        out.push_str(&format!(
            "{},{},{},{:.6},{:.6},{:.3},{:.3},{:.6},{:.6},{:.2},{}\n",
            g.map,
            m.w,
            m.h,
            m.cx,
            m.cz,
            m.xhalf,
            m.zhalf,
            m.block_x,
            m.block_z,
            m.hmax,
            g.heightfield_hash
        ));
    }
    std::fs::write(path, out).unwrap_or_else(|e| {
        eprintln!("cannot write {}: {e}", path.display());
        process::exit(1);
    });
}

fn write_tuning_json(
    path: &Path,
    distance_min: f32,
    distance_max: f32,
    pitch_min: f32,
    pitch_max: f32,
) {
    let value = serde_json::json!({
        "distance_min": distance_min,
        "distance_max": distance_max,
        "pitch_min": pitch_min,
        "pitch_max": pitch_max,
    });
    let json = serde_json::to_string_pretty(&value).expect("serialize failed");
    std::fs::write(path, json).unwrap_or_else(|e| {
        eprintln!("cannot write {}: {e}", path.display());
        process::exit(1);
    });
}
