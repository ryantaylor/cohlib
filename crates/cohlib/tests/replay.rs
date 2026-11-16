//! Integration tests for the replay module, ported from vault's test suite.

use cohlib::{Command, Faction, GameType, InventoryItem, PlayerItem, Replay, Team};
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
fn parse_starting_positions_8p() {
    let data = include_bytes!("../replays/startpos_8p.rec");
    let replay = Replay::from_bytes(data).unwrap();

    let mut positions: Vec<_> = replay.starting_positions().to_vec();
    positions.sort_by_key(|position| position.index());

    let expected = vec![
        (0u32, "1", -288.60, -118.42),
        (1, "2", 245.42, -154.79),
        (2, "3", -288.91, -61.49),
        (3, "4", 264.54, -99.90),
        (4, "5", -283.29, 31.30),
        (5, "6", 282.34, -16.26),
        (6, "7", -280.56, 95.12),
        (7, "8", 287.64, 43.63),
    ];
    assert_eq!(positions.len(), expected.len());
    for (position, (index, name, x, y)) in positions.iter().zip(expected) {
        assert_eq!(position.index(), index);
        assert_eq!(position.name(), name);
        assert!(
            (position.x() - x).abs() < 0.05,
            "x: {} vs {}",
            position.x(),
            x
        );
        assert!(
            (position.y() - y).abs() < 0.05,
            "y: {} vs {}",
            position.y(),
            y
        );
    }

    assert_eq!(replay.territory_points().len(), 30);
    assert_eq!(replay.victory_points().len(), 3);
}

#[test]
fn parse_player_starting_positions() {
    let data = include_bytes!("../replays/startpos_8p.rec");
    let replay = Replay::from_bytes(data).unwrap();
    let players = replay.players();

    assert!(players
        .iter()
        .all(|player| player.starting_position().is_some()));

    let cx: f32 = players
        .iter()
        .map(|player| player.starting_position().unwrap().x())
        .sum::<f32>()
        / players.len() as f32;
    let cy: f32 = players
        .iter()
        .map(|player| player.starting_position().unwrap().y())
        .sum::<f32>()
        / players.len() as f32;

    let angle_degrees = |player: &cohlib::Player| {
        let position = player.starting_position().unwrap();
        (position.y() - cy).atan2(position.x() - cx).to_degrees()
    };
    let start_angle = angle_degrees(
        players
            .iter()
            .find(|player| player.name() == "Marssan")
            .unwrap(),
    );

    let mut clockwise = players.clone();
    clockwise.sort_by(|a, b| {
        // increasing angular distance clockwise (i.e. decreasing angle) from the bottom-left
        // starting position
        let key = |player: &cohlib::Player| (start_angle - angle_degrees(player)).rem_euclid(360.0);
        key(a).partial_cmp(&key(b)).unwrap()
    });

    assert_eq!(
        clockwise
            .iter()
            .map(|player| player.name())
            .collect::<Vec<&str>>(),
        vec![
            "Marssan",
            "Kung Pao Panda",
            "xd",
            "Frying Pan",
            "Barack Obungus",
            "dlwlsdn435",
            "ArtRam",
            "1213229801",
        ]
    );
}

#[test]
fn parse_player_id() {
    let data = include_bytes!("../replays/startpos_8p.rec");
    let replay = Replay::from_bytes(data).unwrap();
    let players = replay.players();

    // Distinct per player, and matches the invariant player_from_data relies on internally to
    // attach each player's own starting position (see StartingPosition::index's doc comment).
    let mut ids: Vec<u32> = players.iter().map(|player| player.id()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), players.len());

    assert!(players
        .iter()
        .all(|player| player.starting_position().unwrap().index() == player.id()));
}

#[test]
fn parse_player_id_ai() {
    let data = include_bytes!("../replays/vs_ai.rec");
    let replay = Replay::from_bytes(data).unwrap();
    let players = replay.players();

    // AI have no profile_id, but id is present regardless of human/AI status.
    assert_eq!(players.len(), 2);
    let mut ids: Vec<u32> = players.iter().map(|player| player.id()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), players.len());
}

#[test]
fn parse_starting_positions_other_shapes() {
    let data = include_bytes!("../replays/one_seven_zero.rec");
    assert_eq!(
        Replay::from_bytes(data).unwrap().starting_positions().len(),
        6
    );

    let data = include_bytes!("../replays/usf_airborne_build.rec");
    assert_eq!(
        Replay::from_bytes(data).unwrap().starting_positions().len(),
        2
    );

    let data = include_bytes!("../replays/zero_items.rec");
    assert_eq!(
        Replay::from_bytes(data).unwrap().starting_positions().len(),
        4
    );

    let data = include_bytes!("../replays/unusual_brit_faction.rec");
    assert_eq!(
        Replay::from_bytes(data).unwrap().starting_positions().len(),
        6
    );

    let data = include_bytes!("../replays/one_delimited_options.rec");
    let replay = Replay::from_bytes(data).unwrap();
    assert_eq!(replay.starting_positions().len(), 8);
    assert!(replay.victory_points().is_empty());
}

#[test]
fn parse_no_starting_positions() {
    let data = include_bytes!("../replays/USvDAK_v10612.rec");
    let replay = Replay::from_bytes(data).unwrap();
    assert!(replay.starting_positions().is_empty());
    assert!(replay
        .players()
        .iter()
        .all(|player| player.starting_position().is_none()));

    let data = include_bytes!("../replays/vs_ai.rec");
    let replay = Replay::from_bytes(data).unwrap();
    assert!(replay.starting_positions().is_empty());

    let data = include_bytes!("../replays/automatch.rec");
    let replay = Replay::from_bytes(data).unwrap();
    assert!(replay.starting_positions().is_empty());
}

/// Since chunk version 4595383, each player is followed by a count-prefixed list of
/// 6-byte records; it's empty for ordinary human players (so earlier fixtures never
/// exercised it), but this build-48837 automatch has two AI-filled slots that leave it
/// non-empty. The parser used to treat it as a fixed 4-byte skip, which desynced every
/// player after the first one whose list was non-empty.
#[test]
fn parse_ai_filled_player_trailer() {
    let data = include_bytes!("../replays/v48837_ai_filled_player_trailer.rec");
    let replay = Replay::from_bytes(data);
    assert!(replay.is_ok());
    let unwrapped = replay.unwrap();
    assert_eq!(
        unwrapped
            .players()
            .iter()
            .map(|player| (player.name(), player.human()))
            .collect::<Vec<(&str, bool)>>(),
        vec![
            ("A DEAD RABBIT", true),
            ("yue991126", true),
            ("CPU - 专家", false),
            ("CPU - 专家", false),
        ]
    );
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

#[test]
fn parse_battlegroup_ignores_engine_granted_upgrades() {
    let data = include_bytes!("../replays/v50049_engine_granted_battlegroup_upgrades.rec");
    let replay = Replay::from_bytes(data).unwrap();
    let player = &replay.players()[3];

    // This player's Hold Off perk loadout arrives as 40 PCMD_InstantUpgrade commands on tick 150,
    // long before they pick Airborne on tick 448. The pick is the one that counts.
    assert_eq!(player.battlegroup(), Some(182327));
    assert_eq!(player.battlegroup_selected_at(), Some(448));

    // All 45 grants across the match are still reported as commands -- only battlegroup
    // resolution skips them.
    let granted = player
        .commands()
        .iter()
        .filter(|command| matches!(command, Command::SelectBattlegroup(_)) && command.index() == 0)
        .count();
    assert_eq!(granted, 45);

    // Players with no engine-granted upgrades are unaffected.
    assert_eq!(replay.players()[1].battlegroup(), Some(2031370));
    assert_eq!(replay.players()[1].battlegroup_selected_at(), Some(60));
    assert_eq!(replay.players()[2].battlegroup(), None);
}

fn player_item(item: &InventoryItem) -> &PlayerItem {
    match item {
        InventoryItem::Player(item) => item,
        InventoryItem::Cpu(item) => panic!("expected a human item, got {item:?}"),
    }
}

#[test]
fn parse_inventory_items() {
    let data = include_bytes!("../replays/automatch.rec");
    let replay = Replay::from_bytes(data).unwrap();
    let players = replay.players();

    assert_eq!(
        players
            .iter()
            .map(|player| player
                .battlegroup_items()
                .iter()
                .map(|item| player_item(item).definition_id())
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        vec![vec![453294, 453390, 453430], vec![453171, 453172, 453175]]
    );
    assert_eq!(
        players
            .iter()
            .map(|player| player.cosmetic_items().len())
            .collect::<Vec<_>>(),
        vec![25, 19]
    );

    let item = player_item(&players[0].cosmetic_items()[0]);
    assert_eq!(item.instance_id(), 33509337);
    assert_eq!(item.definition_id(), 454134);
    assert_eq!(item.profile_id(), 362985);
    assert_eq!(item.durability(), 1);
    assert_eq!(item.durability_type(), 0);
    assert_eq!(item.metadata(), r#"{"epos":"4","eslot":"4"}"#);
    assert_eq!(item.slot(), Some(4));
    assert_eq!(item.acquired_at(), 1677864554);
    assert_eq!(item.location_id(), 33509267);
    assert_eq!(item.unknown1(), 7);
    assert_eq!(item.unknown2(), -1);
    assert_eq!(item.unknown3(), 19);
    assert_eq!(item.unknown4(), i64::from(i32::MAX));
    assert!(item.extra().is_empty());

    // Every item belongs to its player and sits in that player's loadout container.
    for player in players {
        let items: Vec<_> = player
            .battlegroup_items()
            .iter()
            .chain(player.cosmetic_items())
            .map(player_item)
            .collect();
        assert!(items
            .iter()
            .all(|item| Some(item.profile_id()) == player.profile_id()
                && item.location_id() == items[0].location_id()
                && item.slot().is_some()
                && item.extra().is_empty()));
    }
}

#[test]
fn parse_inventory_items_slot_order() {
    let data = include_bytes!("../replays/unusual_cpu_items.rec");
    let replay = Replay::from_bytes(data).unwrap();

    // Items are stored out of slot order, and the metadata can carry more than the slot.
    let players = replay.players();
    let items = players[2].battlegroup_items();
    assert_eq!(
        items
            .iter()
            .map(|item| player_item(item).slot())
            .collect::<Vec<_>>(),
        vec![Some(0), Some(2), Some(1)]
    );
    assert_eq!(
        player_item(&items[2]).metadata(),
        r#"{"att":{"is_new":{"val":"0"}},"epos":"1","eslot":"1","priority":"1"}"#
    );
}

#[test]
fn parse_inventory_items_ai() {
    let data = include_bytes!("../replays/unusual_cpu_items.rec");
    let replay = Replay::from_bytes(data).unwrap();
    let ai = &replay.players()[1];

    assert!(!ai.human());
    assert!(ai.cosmetic_items().is_empty());
    assert_eq!(
        ai.battlegroup_items()
            .iter()
            .map(|item| match item {
                InventoryItem::Cpu(item) => item.id(),
                InventoryItem::Player(item) => panic!("expected an AI item, got {item:?}"),
            })
            .collect::<Vec<_>>(),
        vec![2044465, 2044467, 2044468]
    );
}

#[test]
fn parse_inventory_items_none() {
    let data = include_bytes!("../replays/zero_items.rec");
    let replay = Replay::from_bytes(data).unwrap();
    let player = &replay.players()[0];

    assert!(player.battlegroup_items().is_empty());
    assert!(player.cosmetic_items().is_empty());
}

#[test]
fn parse_lobby_options() {
    let data = include_bytes!("../replays/automatch.rec");
    let replay = Replay::from_bytes(data).unwrap();

    assert_eq!(
        replay
            .lobby_options()
            .iter()
            .map(|option| (option.name(), option.value()))
            .collect::<Vec<_>>(),
        vec![
            ("section_starting_resources", 1),
            ("option_resources", 0),
            ("section_tickets", 1),
            ("option_tickets", 500)
        ]
    );
}

#[test]
fn parse_lobby_options_repeated_names() {
    let data = include_bytes!("../replays/unusual_cpu_items.rec");
    let replay = Replay::from_bytes(data).unwrap();

    assert_eq!(
        replay
            .lobby_options()
            .iter()
            .take(4)
            .map(|option| (option.name(), option.value()))
            .collect::<Vec<_>>(),
        vec![
            ("bridge_modifier", 1),
            ("bridge_modifier", 0),
            ("ceasefire", 1),
            ("ceasefire", 0)
        ]
    );
}

#[test]
fn parse_mod_version() {
    let automatch = Replay::from_bytes(include_bytes!("../replays/automatch.rec")).unwrap();
    let modded = Replay::from_bytes(include_bytes!("../replays/vs_ai.rec")).unwrap();

    assert_eq!(automatch.mod_version(), Some(3318379781));
    assert_eq!(modded.mod_version(), Some(1111669793));
}
