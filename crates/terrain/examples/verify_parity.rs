//! Verifies `cohlib grid`'s output against the research reference CSVs
//! (design doc §8.4 acceptance): value-for-value on `map_meta.csv`, cell-for-cell
//! on `coarse_grid.csv`. Not run in CI — it needs the real depot plus the
//! research artifacts from `cohdb/next/analysis/zoomhack/results/`, neither of
//! which belong in this repo.
//!
//! Usage:
//!   cargo run --release -p cli --bin cohlib -- grid <depot_path> --output /tmp/grid_out
//!   cargo run --release -p terrain --example verify_parity -- \
//!       /tmp/grid_out <path-to-cohdb-next>/analysis/zoomhack/results

use std::collections::HashMap;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let grid_out = Path::new(&args[1]);
    let research = Path::new(&args[2]);

    let meta_mismatches = verify_meta(
        &grid_out.join("map_meta.csv"),
        &research.join("map_meta.csv"),
    );
    let (cell_total, cell_mismatches, cell_missing) = verify_grid(
        &grid_out.join("coarse_grid.csv"),
        &research.join("coarse_grid.csv"),
    );

    println!("map_meta.csv mismatches: {meta_mismatches}");
    println!(
        "coarse_grid.csv: {cell_total} cells checked, {cell_mismatches} mismatches, {cell_missing} missing"
    );

    if meta_mismatches > 0 || cell_mismatches > 0 || cell_missing > 0 {
        std::process::exit(1);
    }
    println!("PASS");
}

/// `scenarios/multiplayer/[community/]<name>/<name>` -> the research script's
/// short key, `<name>_<name>` (`community/` stripped).
fn short_key(cohlib_map: &str) -> String {
    let mut parts: Vec<&str> = cohlib_map.split('/').skip(2).collect();
    if parts.first() == Some(&"community") {
        parts.remove(0);
    }
    parts.join("_")
}

fn read_csv(path: &Path) -> Vec<HashMap<String, String>> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().expect("empty csv").split(',').collect();
    lines
        .map(|line| {
            header
                .iter()
                .zip(line.split(','))
                .map(|(h, v)| (h.to_string(), v.to_string()))
                .collect()
        })
        .collect()
}

fn verify_meta(cohlib_path: &Path, research_path: &Path) -> usize {
    let cohlib_rows = read_csv(cohlib_path);
    let research_rows = read_csv(research_path);

    let cohlib_by_key: HashMap<String, &HashMap<String, String>> = cohlib_rows
        .iter()
        .map(|r| (short_key(&r["map"]), r))
        .collect();

    let fields = [
        "w", "h", "cx", "cz", "xhalf", "zhalf", "block_x", "block_z", "hmax",
    ];
    let mut mismatches = 0;
    for r in &research_rows {
        let Some(c) = cohlib_by_key.get(&r["map"]) else {
            println!("MISSING MAP in cohlib output: {}", r["map"]);
            mismatches += 1;
            continue;
        };
        for f in fields {
            let rv: f64 = r[f].parse().unwrap();
            let cv: f64 = c[f].parse().unwrap();
            if (rv - cv).abs() > 0.005 {
                println!("META MISMATCH {} {f}: research={rv} cohlib={cv}", r["map"]);
                mismatches += 1;
            }
        }
    }
    mismatches
}

fn verify_grid(cohlib_path: &Path, research_path: &Path) -> (usize, usize, usize) {
    let cohlib_rows = read_csv(cohlib_path);
    let research_rows = read_csv(research_path);

    let mut cohlib_by_map: HashMap<String, HashMap<(i32, i32), f64>> = HashMap::new();
    for r in &cohlib_rows {
        let key = short_key(&r["map"]);
        let bi: i32 = r["bi"].parse().unwrap();
        let bj: i32 = r["bj"].parse().unwrap();
        let tmax: f64 = r["tmax"].parse().unwrap();
        cohlib_by_map.entry(key).or_default().insert((bi, bj), tmax);
    }

    let mut total = 0;
    let mut mismatches = 0;
    let mut missing = 0;
    for r in &research_rows {
        total += 1;
        let bi: i32 = r["bi"].parse().unwrap();
        let bj: i32 = r["bj"].parse().unwrap();
        let rv: f64 = r["tmax"].parse().unwrap();
        match cohlib_by_map.get(&r["map"]).and_then(|m| m.get(&(bi, bj))) {
            None => {
                missing += 1;
                if missing <= 10 {
                    println!("MISSING CELL {} ({bi},{bj})", r["map"]);
                }
            }
            Some(&cv) if (rv - cv).abs() > 0.005 => {
                mismatches += 1;
                if mismatches <= 10 {
                    println!(
                        "CELL MISMATCH {} ({bi},{bj}): research={rv} cohlib={cv}",
                        r["map"]
                    );
                }
            }
            _ => {}
        }
    }
    (total, mismatches, missing)
}
