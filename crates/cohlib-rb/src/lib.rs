//! cohlib Ruby bindings.
//!
//! Exposes the CohLib module with classes for replay parsing, build order
//! extraction, and versioned game data access.

use cohlib::{
    extract_build_order, parse_replay, BuildAction, BuildActionKind, BuildOrder, Message, Player,
    Replay, VersionedStore,
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

// ---------------------------------------------------------------------------
// CohLib::Message
// ---------------------------------------------------------------------------

fn message_to_h(ruby: &Ruby, rb_self: &Message) -> RHash {
    let hash = ruby.hash_new();
    hash.aset(ruby.to_symbol("tick"), rb_self.tick()).unwrap();
    hash.aset(ruby.to_symbol("message"), rb_self.message()).unwrap();
    hash
}

// ---------------------------------------------------------------------------
// CohLib::BuildAction
// ---------------------------------------------------------------------------

fn build_action_action_type(rb_self: &BuildAction) -> String {
    match rb_self.kind {
        BuildActionKind::ConstructBuilding => "construct_building",
        BuildActionKind::TrainUnit => "train_unit",
        BuildActionKind::ResearchUpgrade => "research_upgrade",
        BuildActionKind::SelectBattlegroup => "select_battlegroup",
        BuildActionKind::SelectBattlegroupAbility => "select_battlegroup_ability",
        BuildActionKind::UseBattlegroupAbility => "use_battlegroup_ability",
        BuildActionKind::AITakeover => "ai_takeover",
    }
    .to_owned()
}

fn build_action_to_h(ruby: &Ruby, rb_self: &BuildAction) -> RHash {
    let hash = ruby.hash_new();
    hash.aset(ruby.to_symbol("tick"), rb_self.tick).unwrap();
    hash.aset(ruby.to_symbol("action_type"), build_action_action_type(rb_self))
        .unwrap();
    hash.aset(ruby.to_symbol("pbgid"), rb_self.pbgid).unwrap();
    hash.aset(ruby.to_symbol("suspect"), rb_self.suspect).unwrap();
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
) -> Result<BuildOrder, Error> {
    extract_build_order(rb_replay, player_index, rb_self)
        .map_err(|e| Error::new(ruby.exception_runtime_error(), e.to_string()))
}

fn versioned_store_t(rb_self: &VersionedStore, build: u32, pbgid: u32) -> Option<String> {
    rb_self.local_name_for_formatted(pbgid, build).map(|s| s.to_owned())
}

fn versioned_store_localize(rb_self: &VersionedStore, loc_id: u32, build: u32) -> Option<String> {
    rb_self.localize(loc_id, build).map(|s| s.to_owned())
}

fn versioned_store_icon_for(rb_self: &VersionedStore, pbgid: u32, build: u32) -> Option<String> {
    rb_self
        .get_entity(pbgid, build)
        .map(|e| e.icon_name.to_owned())
        .or_else(|| rb_self.get_squad(pbgid, build).map(|s| s.icon_name.to_owned()))
        .or_else(|| rb_self.get_upgrade(pbgid, build).map(|u| u.icon_name.to_owned()))
        .or_else(|| rb_self.get_ability(pbgid, build).map(|a| a.icon_name.to_owned()))
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

    // CohLib::Message
    let message_class = module.define_class("Message", ruby.class_object())?;
    message_class.define_method("tick", method!(Message::tick, 0))?;
    message_class.define_method("message", method!(Message::message, 0))?;
    message_class.define_method("to_h", method!(message_to_h, 0))?;

    // CohLib::BuildAction
    let build_action_class = module.define_class("BuildAction", ruby.class_object())?;
    build_action_class.define_method("tick", method!(|a: &BuildAction| a.tick, 0))?;
    build_action_class.define_method("index", method!(|a: &BuildAction| a.index, 0))?;
    build_action_class.define_method("action_type", method!(build_action_action_type, 0))?;
    build_action_class.define_method("pbgid", method!(|a: &BuildAction| a.pbgid, 0))?;
    build_action_class.define_method("suspect", method!(|a: &BuildAction| a.suspect, 0))?;
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
        method!(versioned_store_extract_build_order, 2),
    )?;
    store_class.define_method("t", method!(versioned_store_t, 2))?;
    store_class.define_method("localize", method!(versioned_store_localize, 2))?;
    store_class.define_method("icon_for", method!(versioned_store_icon_for, 2))?;

    Ok(())
}
