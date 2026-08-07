//! Parser for CoH3 `.info` scenario metadata files.
//!
//! `.info` files are a Lua-like table literal (`HeaderInfo = { key = value, ... }`)
//! — not valid Lua (no `return`, trailing commas everywhere), so this is a small
//! targeted extractor for the specific keys CoH3 writes, rather than a general
//! Lua parser. Confirmed against `castello_8p.info` from the local game depot:
//!
//! ```text
//! HeaderInfo = {
//!     ScenarioDescription = "$11274015",
//!     map_author = "TheSphinx",
//!     map_origin = 2,
//!     mapsize = { 544, 608 },
//!     point_positions = { { ebp_name = "...", owner_id = 1000, x = 1.0, y = 2.0 }, ... },
//!     scenario_type = 1,
//!     scenarioname = "$11274014",
//!     slots = { { flags = ..., status = 0, team = 0 }, ... },  -- status 0 = enabled
//!     visible_in_lobby = true,
//!     ...
//! }
//! ```

use data::MapSize;

#[derive(Debug, Clone, PartialEq)]
pub struct InfoPoint {
    pub ebp: String,
    pub owner: u32,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub size: MapSize,
    pub points: Vec<InfoPoint>,
    pub max_players: u32,
    pub teams: [u32; 2],
    pub author: String,
    pub name_loc_id: u32,
    pub description_loc_id: u32,
    pub scenario_type: u32,
    pub map_origin: u32,
    pub visible_in_lobby: bool,
}

/// Parses a `.info` file's bytes. Returns `None` if the file doesn't even have a
/// `mapsize` — every real scenario `.info` does, so that's treated as a hard
/// requirement; all other fields degrade gracefully to defaults if missing.
pub fn parse_info(bytes: &[u8]) -> Option<Info> {
    let text = std::str::from_utf8(bytes).ok()?;
    let size = parse_mapsize(text)?;
    let (max_players, teams) = parse_slots(text);
    Some(Info {
        size,
        points: parse_point_positions(text),
        max_players,
        teams,
        author: parse_string_field(text, "map_author").unwrap_or_default(),
        name_loc_id: parse_loc_field(text, "scenarioname"),
        description_loc_id: parse_loc_field(text, "ScenarioDescription"),
        scenario_type: parse_int_field(text, "scenario_type").unwrap_or(0) as u32,
        map_origin: parse_int_field(text, "map_origin").unwrap_or(0) as u32,
        visible_in_lobby: parse_bool_field(text, "visible_in_lobby").unwrap_or(false),
    })
}

/// Finds the start index of `key`'s value (just after its `=`), requiring `key`
/// to appear as a whole identifier (not a prefix/suffix of a longer one) — e.g.
/// searching for `ScenarioDescription` must not match `ScenarioDescriptionlong`.
fn find_value_start(text: &str, key: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut search_from = 0;
    while let Some(rel) = text[search_from..].find(key) {
        let idx = search_from + rel;
        let before_ok = idx == 0 || !is_ident(bytes[idx - 1]);
        let after = idx + key.len();
        let after_ok = after >= bytes.len() || !is_ident(bytes[after]);
        if before_ok && after_ok {
            let eq = text[after..].find('=')?;
            return Some(after + eq + 1);
        }
        search_from = idx + 1;
    }
    None
}

/// Finds the matching closing brace for the first `{` at or after `start`,
/// handling nesting. Returns `(open_index, close_index)`, both inclusive of
/// the braces themselves.
fn matching_braces(text: &str, start: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let open = start + text[start..].find('{')?;
    let mut depth = 0i32;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((open, i));
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_mapsize(text: &str) -> Option<MapSize> {
    let value_start = find_value_start(text, "mapsize")?;
    let (open, close) = matching_braces(text, value_start)?;
    let body = &text[open + 1..close];
    let mut numbers = body.split(',').filter_map(|s| s.trim().parse::<f32>().ok());
    let width = numbers.next()?;
    let height = numbers.next()?;
    Some(MapSize { width, height })
}

fn parse_string_field(text: &str, key: &str) -> Option<String> {
    let value_start = find_value_start(text, key)?;
    let rest = &text[value_start..];
    let open = rest.find('"')?;
    let close = rest[open + 1..].find('"')?;
    Some(rest[open + 1..open + 1 + close].to_string())
}

/// Parses a `"$<loc_id>"`-shaped field into its numeric loc id, or `0` if absent
/// or unset (CoH3 uses literal `"$0"` for "no localization").
fn parse_loc_field(text: &str, key: &str) -> u32 {
    parse_string_field(text, key)
        .and_then(|s| s.trim_start_matches('$').parse::<u32>().ok())
        .unwrap_or(0)
}

fn parse_int_field(text: &str, key: &str) -> Option<i64> {
    let value_start = find_value_start(text, key)?;
    let rest = text[value_start..].trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '-'))
        .unwrap_or(rest.len());
    rest[..end].parse::<i64>().ok()
}

fn parse_bool_field(text: &str, key: &str) -> Option<bool> {
    let value_start = find_value_start(text, key)?;
    let rest = text[value_start..].trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn parse_point_positions(text: &str) -> Vec<InfoPoint> {
    let Some(value_start) = find_value_start(text, "point_positions") else {
        return Vec::new();
    };
    let Some((open, close)) = matching_braces(text, value_start) else {
        return Vec::new();
    };
    let body = &text[open + 1..close];

    let mut points = Vec::new();
    let mut offset = 0;
    while let Some((sub_open, sub_close)) = matching_braces(body, offset) {
        let entry = &body[sub_open + 1..sub_close];
        if let Some(point) = parse_point_entry(entry) {
            points.push(point);
        }
        offset = sub_close + 1;
    }
    points
}

fn parse_point_entry(entry: &str) -> Option<InfoPoint> {
    let ebp = parse_string_field(entry, "ebp_name")?;
    let owner = parse_int_field(entry, "owner_id").unwrap_or(0) as u32;
    let x = parse_float_field(entry, "x")?;
    let y = parse_float_field(entry, "y")?;
    Some(InfoPoint { ebp, owner, x, y })
}

fn parse_float_field(text: &str, key: &str) -> Option<f32> {
    let value_start = find_value_start(text, key)?;
    let rest = text[value_start..].trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.'))
        .unwrap_or(rest.len());
    rest[..end].parse::<f32>().ok()
}

/// Returns `(enabled_slot_count, [team0_count, team1_count])` from the `slots`
/// array. A slot with `status == 0` is enabled/playable; `status != 0` means the
/// slot doesn't exist for this map's player count (e.g. slots 9-16 on an 8p map).
fn parse_slots(text: &str) -> (u32, [u32; 2]) {
    let Some(value_start) = find_value_start(text, "slots") else {
        return (0, [0, 0]);
    };
    let Some((open, close)) = matching_braces(text, value_start) else {
        return (0, [0, 0]);
    };
    let body = &text[open + 1..close];

    let mut max_players = 0;
    let mut teams = [0u32; 2];
    let mut offset = 0;
    while let Some((sub_open, sub_close)) = matching_braces(body, offset) {
        let entry = &body[sub_open + 1..sub_close];
        let status = parse_int_field(entry, "status").unwrap_or(1);
        if status == 0 {
            max_players += 1;
            if let Some(team) = parse_int_field(entry, "team") {
                if let Ok(idx) = usize::try_from(team) {
                    if idx < teams.len() {
                        teams[idx] += 1;
                    }
                }
            }
        }
        offset = sub_close + 1;
    }
    (max_players, teams)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
HeaderInfo =
{
	ScenarioDescription = "$11274015",
	ScenarioDescriptionlong = "$11274099",
	map_author = "TheSphinx",
	map_origin = 2,
	mapsize =
	{
		544,
		608,
	},
	point_positions =
	{

		{
			ebp_name = "starting_position",
			owner_id = 1003,
			x = 175.47891,
			y = 152.27177,
		},

		{
			ebp_name = "territory_fuel_point_low",
			owner_id = 0,
			x = -64.09598,
			y = 212.23982,
		},
	},
	scenario_type = 1,
	scenarioname = "$11274014",
	slots =
	{

		{
			flags = 4294967231,
			status = 0,
			team = 0,
		},

		{
			flags = 4294967231,
			status = 0,
			team = 1,
		},

		{
			flags = 4294967231,
			status = 1,
			team = 0,
		},
	},
	version = 3001,
	visible_in_lobby = true,
	worldbp = "default",
}
"#;

    #[test]
    fn parses_mapsize() {
        let info = parse_info(SAMPLE.as_bytes()).unwrap();
        assert_eq!(
            info.size,
            MapSize {
                width: 544.0,
                height: 608.0
            }
        );
    }

    #[test]
    fn parses_point_positions() {
        let info = parse_info(SAMPLE.as_bytes()).unwrap();
        assert_eq!(info.points.len(), 2);
        assert_eq!(info.points[0].ebp, "starting_position");
        assert_eq!(info.points[0].owner, 1003);
        assert!((info.points[0].x - 175.47891).abs() < 1e-4);
        assert!((info.points[0].y - 152.27177).abs() < 1e-4);
        assert_eq!(info.points[1].ebp, "territory_fuel_point_low");
        assert!((info.points[1].x - -64.09598).abs() < 1e-4);
    }

    #[test]
    fn parses_string_and_loc_fields_without_prefix_collision() {
        let info = parse_info(SAMPLE.as_bytes()).unwrap();
        assert_eq!(info.author, "TheSphinx");
        // Must not match "ScenarioDescriptionlong"'s value.
        assert_eq!(info.description_loc_id, 11274015);
        assert_eq!(info.name_loc_id, 11274014);
    }

    #[test]
    fn parses_int_and_bool_fields() {
        let info = parse_info(SAMPLE.as_bytes()).unwrap();
        assert_eq!(info.scenario_type, 1);
        assert_eq!(info.map_origin, 2);
        assert!(info.visible_in_lobby);
    }

    #[test]
    fn parses_slots_into_max_players_and_teams() {
        let info = parse_info(SAMPLE.as_bytes()).unwrap();
        assert_eq!(info.max_players, 2);
        assert_eq!(info.teams, [1, 1]);
    }

    #[test]
    fn returns_none_without_mapsize() {
        assert!(parse_info(b"HeaderInfo = { other = 1 }").is_none());
    }
}
