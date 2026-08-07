// build.rs — embeds compiled-in game data from data/{version}/game_data.json files.
//
// Reads all per-version game_data.json files under the workspace-root `data/` directory,
// plus the shared, deduplicated scenario records under `data/scenarios/{hash}.json`
// (see GameData::scenarios doc comment), assembles them into `{"versions": [...],
// "scenarios": {...}}`, gzip-compresses the result, and writes a compressed binary blob
// to OUT_DIR/game_data.bin, which is then embedded via `include_bytes!` in
// VersionedStore::bundled().

use std::{collections::BTreeMap, env, fs, io::Write, path::Path};

fn main() {
    // data/ lives at the workspace root, two levels above this crate (crates/data/).
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let data_dir = Path::new(&manifest_dir).join("../../data");

    println!("cargo:rerun-if-changed={}", data_dir.display());

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("game_data.bin");

    let mut versions: Vec<serde_json::Value> = Vec::new();

    if data_dir.exists() {
        let mut entries: Vec<_> = fs::read_dir(&data_dir)
            .expect("cannot read data/")
            .flatten()
            .filter(|e| e.path().is_dir() && e.file_name() != "scenarios")
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let gd_path = entry.path().join("game_data.json");
            if gd_path.exists() {
                let text = fs::read_to_string(&gd_path)
                    .unwrap_or_else(|e| panic!("cannot read {}: {e}", gd_path.display()));
                let val: serde_json::Value = serde_json::from_str(&text)
                    .unwrap_or_else(|e| panic!("cannot parse {}: {e}", gd_path.display()));
                versions.push(val);
            }
        }
    }

    let mut scenarios: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let scenarios_dir = data_dir.join("scenarios");
    if scenarios_dir.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(&scenarios_dir)
            .expect("cannot read data/scenarios/")
            .flatten()
            .filter(|e| e.path().extension().and_then(|e| e.to_str()) == Some("json"))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            let hash = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_else(|| panic!("bad scenario filename: {}", path.display()))
                .to_string();
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            let val: serde_json::Value = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()));
            scenarios.insert(hash, val);
        }
    }

    let bundle = serde_json::json!({ "versions": versions, "scenarios": scenarios });
    let json = serde_json::to_vec(&bundle).expect("cannot serialize game data");

    // Compress with flate2 (gzip).
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(&json).expect("cannot compress game data");
    let compressed = encoder.finish().expect("cannot finalize compression");

    fs::write(&out_path, &compressed).expect("cannot write game_data.bin");
}
