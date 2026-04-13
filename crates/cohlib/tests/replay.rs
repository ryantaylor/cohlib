//! Integration tests for the replay module, ported from vault's test suite.

use cohlib::{Faction, GameType, Replay, Team};
use uuid::{uuid, Uuid};

#[test]
fn parse_success() {
    let data = include_bytes!("../replays/USvDAK_v10612.rec");
    let replay = Replay::from_bytes(data);
    assert!(replay.is_ok());
    let unwrapped = replay.unwrap();
    assert_eq!(unwrapped.version(), 10612);
    assert_eq!(
        unwrapped
            .players()
            .iter()
            .map(|player| player.name())
            .collect::<Vec<&str>>(),
        vec!["madhax", "Quixalotl"]
    );
    assert_eq!(unwrapped.mod_uuid(), Uuid::nil());
    assert_eq!(unwrapped.game_type(), GameType::Multiplayer);
    assert_eq!(unwrapped.matchhistory_id(), Some(5569487));
}

#[test]
fn parse_failure() {
    let data = [1, 2, 3];
    let replay = Replay::from_bytes(&data);
    assert!(replay.is_err());
}

#[test]
fn parse_success_ai() {
    let data = include_bytes!("../replays/vs_ai.rec");
    let replay = Replay::from_bytes(data);
    assert!(replay.is_ok());
    let unwrapped = replay.unwrap();
    assert_eq!(unwrapped.version(), 21283);
    assert_eq!(
        unwrapped
            .players()
            .iter()
            .map(|player| player.name())
            .collect::<Vec<&str>>(),
        vec!["Janne252", "CPU - Standard"]
    );
    assert_eq!(
        unwrapped.mod_uuid(),
        uuid!("385d9810-96ba-4ece-9040-8281db65174e")
    );
    assert_eq!(unwrapped.game_type(), GameType::Skirmish);
    assert_eq!(unwrapped.matchhistory_id(), None);
}

#[test]
fn parse_weird_description() {
    let data = include_bytes!("../replays/weird_description.rec");
    let replay = Replay::from_bytes(data);
    assert!(replay.is_ok());
    let unwrapped = replay.unwrap();
    assert_eq!(unwrapped.map().localized_name_id(), "Twin Beaches ML");
    assert_eq!(unwrapped.map().localized_description_id(), "TB ML");
    assert_eq!(unwrapped.game_type(), GameType::Multiplayer);
    assert_eq!(unwrapped.matchhistory_id(), Some(11782009));
}

#[test]
fn parse_battlegroup() {
    let data = include_bytes!("../replays/USvDAK_v10612.rec");
    let replay = Replay::from_bytes(data).unwrap();
    assert_eq!(
        replay
            .players()
            .iter()
            .map(|player| player.battlegroup())
            .collect::<Vec<Option<u32>>>(),
        vec![Some(2072430), Some(196934)]
    );
}

#[test]
fn parse_automatch() {
    let data = include_bytes!("../replays/automatch.rec");
    let replay = Replay::from_bytes(data).unwrap();
    assert_eq!(replay.game_type(), GameType::Automatch);
    assert_eq!(replay.matchhistory_id(), Some(18837622));
}

#[test]
fn parse_custom() {
    let data = include_bytes!("../replays/custom.rec");
    let replay = Replay::from_bytes(data).unwrap();
    assert_eq!(replay.game_type(), GameType::Custom);
    assert_eq!(replay.matchhistory_id(), Some(18838931));
}

#[test]
fn parse_skirmish() {
    let data = include_bytes!("../replays/skirmish.rec");
    let replay = Replay::from_bytes(data).unwrap();
    assert_eq!(replay.game_type(), GameType::Skirmish);
    assert_eq!(replay.matchhistory_id(), None);
}

#[test]
fn parse_new_map_chunk() {
    let data = include_bytes!("../replays/one_seven_zero.rec");
    let replay = Replay::from_bytes(data).unwrap();
    assert_eq!(
        replay.map_filename(),
        "data:scenarios\\multiplayer\\desert_airfield_6p_mkii\\desert_airfield_6p_mkii"
    );
    assert_eq!(replay.map_localized_name_id(), "$11233954");
    assert_eq!(replay.map_localized_description_id(), "$11233955");
}

#[test]
fn parse_ai_takeover() {
    let data = include_bytes!("../replays/ai_takeover.rec");
    let replay = Replay::from_bytes(data);
    assert!(replay.is_ok());
}

#[test]
fn parse_zero_item_player() {
    let data = include_bytes!("../replays/zero_items.rec");
    let replay = Replay::from_bytes(data);
    assert!(replay.is_ok());
}

#[test]
fn parse_unusual_items_player() {
    let data = include_bytes!("../replays/unusual_items.rec");
    let replay = Replay::from_bytes(data);
    assert!(replay.is_ok());
}

#[test]
fn parse_unusual_options() {
    let data = include_bytes!("../replays/unusual_options.rec");
    let replay = Replay::from_bytes(data);
    assert!(replay.is_ok());
}

#[test]
fn parse_one_delimited_options() {
    let data = include_bytes!("../replays/one_delimited_options.rec");
    let replay = Replay::from_bytes(data);
    assert!(replay.is_ok());
}

#[test]
fn parse_unusual_cpu_items() {
    let data = include_bytes!("../replays/unusual_cpu_items.rec");
    let replay = Replay::from_bytes(data);
    assert!(replay.is_ok());
}

#[test]
fn parse_230() {
    let data = include_bytes!("../replays/230.rec");
    let replay = Replay::from_bytes(data);
    assert!(replay.is_ok());
}

#[test]
fn parse_unusual_brit_faction() {
    let data = include_bytes!("../replays/unusual_brit_faction.rec");
    let replay = Replay::from_bytes(data);
    assert!(replay.is_ok());
    let unwrapped = replay.unwrap();
    assert_eq!(
        unwrapped
            .players()
            .iter()
            .map(|player| player.faction())
            .collect::<Vec<Faction>>(),
        vec![
            Faction::British,
            Faction::Americans,
            Faction::Wehrmacht,
            Faction::Wehrmacht,
            Faction::AfrikaKorps,
            Faction::Americans
        ]
    );
}

#[test]
fn parse_one_char_options() {
    let data = include_bytes!("../replays/one_char_options.rec");
    let replay = Replay::from_bytes(data);
    assert!(replay.is_ok());
}

#[test]
fn parse_unusual_team_id() {
    let data = include_bytes!("../replays/unusual_team_id.rec");
    let replay = Replay::from_bytes(data);
    assert!(replay.is_ok());
    let unwrapped = replay.unwrap();
    assert_eq!(
        unwrapped
            .players()
            .iter()
            .map(|player| player.team())
            .collect::<Vec<Team>>(),
        vec![
            Team::First,
            Team::Second,
            Team::First,
            Team::Second,
            Team::First,
            Team::Second,
            Team::First,
            Team::Second
        ]
    );
}
