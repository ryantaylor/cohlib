//! cohlib Ruby bindings.
//!
//! Exposes the CohLib module with classes for replay parsing, build order
//! extraction, and versioned game data access.

mod camera;
mod command;

use cohlib::{
    extract_build_order, parse_replay, BuildAction, BuildActionKind, BuildOrder, Command, MapPoint,
    MapSize, Message, Player, Rect, Replay, Scenario, ScenarioPoint, Sector, Semver,
    StartingPosition, VersionedStore,
};
use magnus::{function, method, prelude::*, Error, RArray, RHash, Ruby};

// ---------------------------------------------------------------------------
// CohLib::Replay
// ---------------------------------------------------------------------------

fn replay_game_type(rb_self: &Replay) -> String {
    rb_self.game_type().to_string()
}

fn replay_mod_uuid(rb_self: &Replay) -> String {
    rb_self.mod_uuid().to_string()
}

fn replay_from_bytes(ruby: &Ruby, bytes: Vec<u8>) -> Result<Replay, Error> {
    parse_replay(&bytes).map_err(|e| Error::new(ruby.exception_runtime_error(), e.to_string()))
}

fn replay_players(ruby: &Ruby, rb_self: &Replay) -> RArray {
    let arr = ruby.ary_new();
    for player in rb_self.players() {
        arr.push(ruby.obj_wrap(player)).unwrap();
    }
    arr
}

fn replay_starting_positions(ruby: &Ruby, rb_self: &Replay) -> RArray {
    let arr = ruby.ary_new();
    for position in rb_self.starting_positions().iter().cloned() {
        arr.push(ruby.obj_wrap(position)).unwrap();
    }
    arr
}

fn replay_territory_points(ruby: &Ruby, rb_self: &Replay) -> RArray {
    let arr = ruby.ary_new();
    for point in rb_self.territory_points().iter().cloned() {
        arr.push(ruby.obj_wrap(point)).unwrap();
    }
    arr
}

fn replay_victory_points(ruby: &Ruby, rb_self: &Replay) -> RArray {
    let arr = ruby.ary_new();
    for point in rb_self.victory_points().iter().cloned() {
        arr.push(ruby.obj_wrap(point)).unwrap();
    }
    arr
}

// ---------------------------------------------------------------------------
// CohLib::Player
// ---------------------------------------------------------------------------

fn player_messages(ruby: &Ruby, rb_self: &Player) -> RArray {
    let arr = ruby.ary_new();
    for msg in rb_self.messages() {
        arr.push(ruby.obj_wrap(msg)).unwrap();
    }
    arr
}

fn player_faction(rb_self: &Player) -> String {
    rb_self.faction().to_string()
}

fn player_team(rb_self: &Player) -> usize {
    rb_self.team().value()
}

fn player_starting_position(rb_self: &Player) -> Option<StartingPosition> {
    rb_self.starting_position().cloned()
}

fn wrap_commands(ruby: &Ruby, commands: Vec<Command>) -> RArray {
    let arr = ruby.ary_new_capa(commands.len());
    for command in commands {
        arr.push(ruby.obj_wrap(command)).unwrap();
    }
    arr
}

fn player_commands(ruby: &Ruby, rb_self: &Player) -> RArray {
    wrap_commands(ruby, rb_self.commands())
}

fn player_build_commands(ruby: &Ruby, rb_self: &Player) -> RArray {
    wrap_commands(ruby, rb_self.build_commands())
}

fn player_battlegroup_commands(ruby: &Ruby, rb_self: &Player) -> RArray {
    wrap_commands(ruby, rb_self.battlegroup_commands())
}

// ---------------------------------------------------------------------------
// CohLib::Message
// ---------------------------------------------------------------------------

fn message_to_h(ruby: &Ruby, rb_self: &Message) -> RHash {
    let hash = ruby.hash_new();
    hash.aset(ruby.to_symbol("tick"), rb_self.tick()).unwrap();
    hash.aset(ruby.to_symbol("message"), rb_self.message())
        .unwrap();
    hash
}

// ---------------------------------------------------------------------------
// CohLib::StartingPosition
// ---------------------------------------------------------------------------

fn starting_position_to_h(ruby: &Ruby, rb_self: &StartingPosition) -> RHash {
    let hash = ruby.hash_new();
    hash.aset(ruby.to_symbol("index"), rb_self.index()).unwrap();
    hash.aset(ruby.to_symbol("name"), rb_self.name()).unwrap();
    hash.aset(ruby.to_symbol("x"), rb_self.x()).unwrap();
    hash.aset(ruby.to_symbol("y"), rb_self.y()).unwrap();
    hash
}

// ---------------------------------------------------------------------------
// CohLib::MapPoint
// ---------------------------------------------------------------------------

fn map_point_to_h(ruby: &Ruby, rb_self: &MapPoint) -> RHash {
    let hash = ruby.hash_new();
    hash.aset(ruby.to_symbol("icon"), rb_self.icon()).unwrap();
    hash.aset(ruby.to_symbol("tags"), rb_self.tags()).unwrap();
    hash.aset(ruby.to_symbol("x"), rb_self.x()).unwrap();
    hash.aset(ruby.to_symbol("y"), rb_self.y()).unwrap();
    hash
}

// ---------------------------------------------------------------------------
// CohLib::BuildAction
// ---------------------------------------------------------------------------

fn build_action_action_type(rb_self: &BuildAction) -> String {
    match rb_self.kind {
        BuildActionKind::ConstructBuilding => "ConstructBuilding",
        BuildActionKind::TrainUnit => "TrainUnit",
        BuildActionKind::ResearchUpgrade => "ResearchUpgrade",
        BuildActionKind::SelectBattlegroup => "SelectBattlegroup",
        BuildActionKind::SelectBattlegroupAbility => "SelectBattlegroupAbility",
        BuildActionKind::UseBattlegroupAbility => "UseBattlegroupAbility",
        BuildActionKind::AITakeover => "AITakeover",
    }
    .to_owned()
}

fn build_action_to_h(ruby: &Ruby, rb_self: &BuildAction) -> RHash {
    let hash = ruby.hash_new();
    hash.aset(ruby.to_symbol("tick"), rb_self.tick).unwrap();
    hash.aset(
        ruby.to_symbol("action_type"),
        build_action_action_type(rb_self),
    )
    .unwrap();
    hash.aset(ruby.to_symbol("pbgid"), rb_self.pbgid).unwrap();
    hash.aset(ruby.to_symbol("suspect_since"), rb_self.suspect_since)
        .unwrap();
    hash.aset(ruby.to_symbol("cancelled"), rb_self.cancelled)
        .unwrap();
    hash
}

// ---------------------------------------------------------------------------
// CohLib::BuildOrder
// ---------------------------------------------------------------------------

fn build_order_actions(ruby: &Ruby, rb_self: &BuildOrder) -> RArray {
    let arr = ruby.ary_new();
    for action in rb_self.actions.iter().cloned() {
        arr.push(ruby.obj_wrap(action)).unwrap();
    }
    arr
}

// ---------------------------------------------------------------------------
// CohLib::VersionedStore
// ---------------------------------------------------------------------------

fn versioned_store_bundled(ruby: &Ruby) -> Result<VersionedStore, Error> {
    let _ = ruby;
    Ok(VersionedStore::bundled())
}

fn versioned_store_extract_build_order(
    ruby: &Ruby,
    rb_self: &VersionedStore,
    rb_replay: &Replay,
    player_index: usize,
    include_cancelled: bool,
) -> Result<BuildOrder, Error> {
    extract_build_order(rb_replay, player_index, rb_self, include_cancelled)
        .map_err(|e| Error::new(ruby.exception_runtime_error(), e.to_string()))
}

fn versioned_store_t(rb_self: &VersionedStore, build: u32, pbgid: u32) -> Option<String> {
    rb_self
        .local_name_for_formatted(pbgid, build)
        .map(|s| s.to_owned())
}

fn versioned_store_localize(rb_self: &VersionedStore, loc_id: u32, build: u32) -> Option<String> {
    rb_self.localize(loc_id, build).map(|s| s.to_owned())
}

fn versioned_store_icon_for(rb_self: &VersionedStore, pbgid: u32, build: u32) -> Option<String> {
    rb_self.icon_for(pbgid, build).map(|s| s.to_owned())
}

fn map_size_to_array(ruby: &Ruby, size: &MapSize) -> RArray {
    let arr = ruby.ary_new();
    arr.push(size.width).unwrap();
    arr.push(size.height).unwrap();
    arr
}

fn rect_to_hash(ruby: &Ruby, rect: &Rect) -> RHash {
    let h = ruby.hash_new();
    h.aset(ruby.to_symbol("min_x"), rect.min_x).unwrap();
    h.aset(ruby.to_symbol("min_y"), rect.min_y).unwrap();
    h.aset(ruby.to_symbol("max_x"), rect.max_x).unwrap();
    h.aset(ruby.to_symbol("max_y"), rect.max_y).unwrap();
    h
}

fn scenario_point_to_hash(ruby: &Ruby, point: &ScenarioPoint) -> RHash {
    let h = ruby.hash_new();
    h.aset(ruby.to_symbol("ebp"), point.ebp.clone()).unwrap();
    h.aset(ruby.to_symbol("x"), point.x).unwrap();
    h.aset(ruby.to_symbol("y"), point.y).unwrap();
    h.aset(ruby.to_symbol("kind"), format!("{:?}", point.kind))
        .unwrap();
    h.aset(ruby.to_symbol("tier"), point.tier.map(|t| format!("{t:?}")))
        .unwrap();
    h.aset(ruby.to_symbol("owner"), point.owner).unwrap();
    h.aset(ruby.to_symbol("income_per_minute"), point.income_per_minute)
        .unwrap();
    h.aset(ruby.to_symbol("capture_time"), point.capture_time)
        .unwrap();
    h.aset(ruby.to_symbol("sector"), point.sector).unwrap();
    h
}

fn sector_to_hash(ruby: &Ruby, sector: &Sector) -> RHash {
    let h = ruby.hash_new();
    h.aset(ruby.to_symbol("id"), sector.id).unwrap();
    h.aset(ruby.to_symbol("is_base"), sector.is_base).unwrap();
    let neighbors = ruby.ary_new();
    for &n in &sector.neighbors {
        neighbors.push(n).unwrap();
    }
    h.aset(ruby.to_symbol("neighbors"), neighbors).unwrap();
    h.aset(ruby.to_symbol("bounds"), rect_to_hash(ruby, &sector.bounds))
        .unwrap();
    let points = ruby.ary_new();
    for &idx in &sector.points {
        points.push(idx).unwrap();
    }
    h.aset(ruby.to_symbol("points"), points).unwrap();
    let rings = ruby.ary_new();
    for ring in &sector.rings {
        let r = ruby.ary_new();
        for p in ring {
            let coord = ruby.ary_new();
            coord.push(p[0]).unwrap();
            coord.push(p[1]).unwrap();
            r.push(coord).unwrap();
        }
        rings.push(r).unwrap();
    }
    h.aset(ruby.to_symbol("rings"), rings).unwrap();
    h
}

fn scenario_to_hash(ruby: &Ruby, s: &Scenario) -> RHash {
    let h = ruby.hash_new();
    h.aset(ruby.to_symbol("size"), map_size_to_array(ruby, &s.size))
        .unwrap();
    h.aset(
        ruby.to_symbol("playable_area"),
        s.playable_area.map(|r| rect_to_hash(ruby, &r)),
    )
    .unwrap();
    h.aset(ruby.to_symbol("max_players"), s.max_players)
        .unwrap();
    let teams = ruby.ary_new();
    teams.push(s.teams[0]).unwrap();
    teams.push(s.teams[1]).unwrap();
    h.aset(ruby.to_symbol("teams"), teams).unwrap();
    h.aset(ruby.to_symbol("author"), s.author.clone()).unwrap();
    h.aset(ruby.to_symbol("name_loc_id"), s.name_loc_id)
        .unwrap();
    h.aset(ruby.to_symbol("description_loc_id"), s.description_loc_id)
        .unwrap();
    h.aset(ruby.to_symbol("scenario_type"), s.scenario_type)
        .unwrap();
    h.aset(ruby.to_symbol("map_origin"), s.map_origin).unwrap();
    h.aset(ruby.to_symbol("visible_in_lobby"), s.visible_in_lobby)
        .unwrap();

    let points = ruby.ary_new();
    for p in &s.points {
        points.push(scenario_point_to_hash(ruby, p)).unwrap();
    }
    h.aset(ruby.to_symbol("points"), points).unwrap();

    let sectors = ruby.ary_new();
    for sector in &s.sectors {
        sectors.push(sector_to_hash(ruby, sector)).unwrap();
    }
    h.aset(ruby.to_symbol("sectors"), sectors).unwrap();

    h
}

fn versioned_store_scenario(
    ruby: &Ruby,
    rb_self: &VersionedStore,
    scenario: String,
    build: u32,
) -> Option<RHash> {
    let s = rb_self.get_scenario(&scenario, build)?;
    Some(scenario_to_hash(ruby, s))
}

// Like `versioned_store_scenario`, but with no version fallback — `nil` unless `build`
// itself has a scenario record for `scenario`. See `get_scenario_exact`'s doc comment
// for why callers reach for this over the fallback variant.
fn versioned_store_scenario_exact(
    ruby: &Ruby,
    rb_self: &VersionedStore,
    scenario: String,
    build: u32,
) -> Option<RHash> {
    let s = rb_self.get_scenario_exact(&scenario, build)?;
    Some(scenario_to_hash(ruby, s))
}

fn versioned_store_map_size(
    ruby: &Ruby,
    rb_self: &VersionedStore,
    scenario: String,
    build: u32,
) -> Option<RArray> {
    rb_self
        .get_map_size(&scenario, build)
        .map(|size| map_size_to_array(ruby, size))
}

fn versioned_store_checksums_for(
    ruby: &Ruby,
    rb_self: &VersionedStore,
    build: u32,
) -> Option<RHash> {
    rb_self.checksums_for(build).map(|(dc, abc)| {
        let h = ruby.hash_new();
        h.aset(ruby.to_symbol("data_checksum"), dc).unwrap();
        h.aset(ruby.to_symbol("app_binary_checksum"), abc).unwrap();
        h
    })
}

fn semver_to_hash(ruby: &Ruby, s: Semver) -> RHash {
    let h = ruby.hash_new();
    h.aset(ruby.to_symbol("major"), s.major).unwrap();
    h.aset(ruby.to_symbol("minor"), s.minor).unwrap();
    h.aset(ruby.to_symbol("patch"), s.patch).unwrap();
    h
}

fn versioned_store_semver_for(ruby: &Ruby, rb_self: &VersionedStore, build: u32) -> Option<RHash> {
    rb_self.semver_for(build).map(|s| semver_to_hash(ruby, s))
}

fn versioned_store_semver_string_for(rb_self: &VersionedStore, build: u32) -> Option<String> {
    rb_self.semver_string_for(build)
}

fn versioned_store_builds(ruby: &Ruby, rb_self: &VersionedStore) -> RArray {
    let arr = ruby.ary_new();
    for v in rb_self.builds() {
        arr.push(v).unwrap();
    }
    arr
}

// ---------------------------------------------------------------------------
// Extension init
// ---------------------------------------------------------------------------

#[magnus::init(name = "cohlib")]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("CohLib")?;

    // CohLib::Replay
    let replay_class = module.define_class("Replay", ruby.class_object())?;
    replay_class.define_singleton_method("from_bytes", function!(replay_from_bytes, 1))?;
    replay_class.define_method("version", method!(Replay::version, 0))?;
    replay_class.define_method("timestamp", method!(Replay::timestamp, 0))?;
    replay_class.define_method("game_type", method!(replay_game_type, 0))?;
    replay_class.define_method("matchhistory_id", method!(Replay::matchhistory_id, 0))?;
    replay_class.define_method("mod_uuid", method!(replay_mod_uuid, 0))?;
    replay_class.define_method("map_filename", method!(Replay::map_filename, 0))?;
    replay_class.define_method(
        "map_localized_name_id",
        method!(Replay::map_localized_name_id, 0),
    )?;
    replay_class.define_method(
        "map_localized_description_id",
        method!(Replay::map_localized_description_id, 0),
    )?;
    replay_class.define_method("length", method!(Replay::length, 0))?;
    replay_class.define_method("players", method!(replay_players, 0))?;
    replay_class.define_method("starting_positions", method!(replay_starting_positions, 0))?;
    replay_class.define_method("territory_points", method!(replay_territory_points, 0))?;
    replay_class.define_method("victory_points", method!(replay_victory_points, 0))?;

    // CohLib::Player
    let player_class = module.define_class("Player", ruby.class_object())?;
    player_class.define_method("name", method!(Player::name, 0))?;
    player_class.define_method("human?", method!(Player::human, 0))?;
    player_class.define_method("faction", method!(player_faction, 0))?;
    player_class.define_method("team", method!(player_team, 0))?;
    player_class.define_method("battlegroup", method!(Player::battlegroup, 0))?;
    player_class.define_method(
        "battlegroup_selected_at",
        method!(Player::battlegroup_selected_at, 0),
    )?;
    player_class.define_method("ai_takeover_at", method!(Player::ai_takeover_at, 0))?;
    player_class.define_method("steam_id", method!(Player::steam_id, 0))?;
    player_class.define_method("profile_id", method!(Player::profile_id, 0))?;
    player_class.define_method("messages", method!(player_messages, 0))?;
    player_class.define_method("starting_position", method!(player_starting_position, 0))?;
    player_class.define_method("commands", method!(player_commands, 0))?;
    player_class.define_method("build_commands", method!(player_build_commands, 0))?;
    player_class.define_method(
        "battlegroup_commands",
        method!(player_battlegroup_commands, 0),
    )?;
    player_class.define_method("camera_tracks", method!(camera::tracks, 0))?;
    player_class.define_method("camera_counts", method!(camera::counts, 0))?;

    // CohLib::Command
    let command_class = module.define_class("Command", ruby.class_object())?;
    command_class.define_method("tick", method!(Command::tick, 0))?;
    command_class.define_method("index", method!(Command::index, 0))?;
    command_class.define_method("type", method!(command::variant_name, 0))?;
    command_class.define_method("action_type", method!(command::action_type, 0))?;
    command_class.define_method("pbgid", method!(Command::pbgid, 0))?;
    command_class.define_method("to_h", method!(command::to_h, 0))?;

    // CohLib::Message
    let message_class = module.define_class("Message", ruby.class_object())?;
    message_class.define_method("tick", method!(Message::tick, 0))?;
    message_class.define_method("message", method!(Message::message, 0))?;
    message_class.define_method("to_h", method!(message_to_h, 0))?;

    // CohLib::StartingPosition
    let starting_position_class = module.define_class("StartingPosition", ruby.class_object())?;
    starting_position_class.define_method("index", method!(StartingPosition::index, 0))?;
    starting_position_class.define_method("name", method!(StartingPosition::name, 0))?;
    starting_position_class.define_method("x", method!(StartingPosition::x, 0))?;
    starting_position_class.define_method("y", method!(StartingPosition::y, 0))?;
    starting_position_class.define_method("to_h", method!(starting_position_to_h, 0))?;

    // CohLib::MapPoint
    let map_point_class = module.define_class("MapPoint", ruby.class_object())?;
    map_point_class.define_method("icon", method!(MapPoint::icon, 0))?;
    map_point_class.define_method("tags", method!(MapPoint::tags, 0))?;
    map_point_class.define_method("x", method!(MapPoint::x, 0))?;
    map_point_class.define_method("y", method!(MapPoint::y, 0))?;
    map_point_class.define_method("to_h", method!(map_point_to_h, 0))?;

    // CohLib::BuildAction
    let build_action_class = module.define_class("BuildAction", ruby.class_object())?;
    build_action_class.define_method("tick", method!(|a: &BuildAction| a.tick, 0))?;
    build_action_class.define_method("index", method!(|a: &BuildAction| a.index, 0))?;
    build_action_class.define_method("action_type", method!(build_action_action_type, 0))?;
    build_action_class.define_method("pbgid", method!(|a: &BuildAction| a.pbgid, 0))?;
    build_action_class.define_method(
        "suspect_since",
        method!(|a: &BuildAction| a.suspect_since, 0),
    )?;
    build_action_class.define_method("cancelled", method!(|a: &BuildAction| a.cancelled, 0))?;
    build_action_class.define_method("to_h", method!(build_action_to_h, 0))?;

    // CohLib::BuildOrder
    let build_order_class = module.define_class("BuildOrder", ruby.class_object())?;
    build_order_class.define_method("actions", method!(build_order_actions, 0))?;

    // CohLib::VersionedStore
    let store_class = module.define_class("VersionedStore", ruby.class_object())?;
    store_class.define_singleton_method("bundled", function!(versioned_store_bundled, 0))?;
    store_class.define_method(
        "extract_build_order",
        method!(versioned_store_extract_build_order, 3),
    )?;
    store_class.define_method("t", method!(versioned_store_t, 2))?;
    store_class.define_method("localize", method!(versioned_store_localize, 2))?;
    store_class.define_method("icon_for", method!(versioned_store_icon_for, 2))?;
    store_class.define_method("map_size", method!(versioned_store_map_size, 2))?;
    store_class.define_method("scenario", method!(versioned_store_scenario, 2))?;
    store_class.define_method("scenario_exact", method!(versioned_store_scenario_exact, 2))?;
    store_class.define_method("checksums_for", method!(versioned_store_checksums_for, 1))?;
    store_class.define_method("semver_for", method!(versioned_store_semver_for, 1))?;
    store_class.define_method(
        "semver_string_for",
        method!(versioned_store_semver_string_for, 1),
    )?;
    store_class.define_method("builds", method!(versioned_store_builds, 0))?;

    Ok(())
}
