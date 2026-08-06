//! Extracts per-scenario map dimensions from `ScenariosMP.sga` `.info` files.
//!
//! Each multiplayer scenario ships a `<name>.info` file — a Lua-like table —
//! containing `HeaderInfo.mapsize = { width, height }`, the world-space extent of
//! the map. This data is never written into replay files (confirmed by inspecting
//! the `DATA/SDSC` chunk directly), so it can only be recovered from the game's
//! shipped scenario metadata.

use std::collections::BTreeMap;
use std::path::Path;

use data::MapSize;

/// Extracts `scenario_path -> MapSize` for every scenario `.info` file in `sga_path`.
///
/// Keys are the archive's normalized forward-slash path with the `.info` extension
/// stripped (e.g. `scenarios/multiplayer/community/wadi_darnah_4p/wadi_darnah_4p`) —
/// the same form `VersionedStore::get_map_size` normalizes a replay's scenario
/// string to before looking it up.
pub fn extract_map_sizes(sga_path: &Path) -> Result<BTreeMap<String, MapSize>, String> {
    let entries =
        sga::open_archive(sga_path).map_err(|e| format!("cannot open scenarios SGA: {e}"))?;

    let mut sizes = BTreeMap::new();
    for entry in &entries {
        if entry.extension() != Some("info") {
            continue;
        }
        let Ok(text) = std::str::from_utf8(&entry.bytes) else {
            continue;
        };
        let Some(size) = parse_mapsize(text) else {
            continue;
        };
        let scenario = entry.path.trim_end_matches(".info").to_string();
        sizes.insert(scenario, size);
    }
    Ok(sizes)
}

/// Pulls the two numbers out of a `mapsize = { width, height }` field. `.info`
/// files are a Lua-like table format; rather than write a general parser for it,
/// this scans for the `mapsize` key and reads the first two comma-separated
/// numbers inside its braces.
fn parse_mapsize(text: &str) -> Option<MapSize> {
    let start = text.find("mapsize")?;
    let brace_open = text[start..].find('{')? + start;
    let brace_close = text[brace_open..].find('}')? + brace_open;
    let body = &text[brace_open + 1..brace_close];

    let mut numbers = body.split(',').filter_map(|s| s.trim().parse::<f32>().ok());
    let width = numbers.next()?;
    let height = numbers.next()?;
    Some(MapSize { width, height })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mapsize() {
        let text = "HeaderInfo = {\n\tmapsize = \n\t{\n\t\t512,\n\t\t384,\n\t},\n}";
        assert_eq!(
            parse_mapsize(text),
            Some(MapSize {
                width: 512.0,
                height: 384.0
            })
        );
    }

    #[test]
    fn returns_none_without_mapsize() {
        assert_eq!(parse_mapsize("HeaderInfo = { other = 1 }"), None);
    }
}
