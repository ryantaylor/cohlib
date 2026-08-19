//! Guards the `DataTypeFunctions::size` implementations on `Player`/`Replay` against regressing
//! back to the default `size_of_val`-based behavior, which only sees each struct's own stack
//! layout and is blind to the commands/camera_tracks/camera_counts a real replay holds -- the gap
//! that let Ruby's GC go unaware of megabytes of native memory per `Replay::players()` call.

#![cfg(feature = "magnus")]

use cohlib::Replay;
use magnus::DataTypeFunctions;

#[test]
fn player_size_reflects_its_commands_and_camera_data() {
    let data = include_bytes!("../replays/USvDAK_v10612.rec");
    let replay = Replay::from_bytes(data).unwrap();
    let player = replay
        .players()
        .into_iter()
        .max_by_key(|player| player.commands().len() + player.camera_tracks().len())
        .unwrap();

    // A player with real commands/camera data must report far more than its own stack size --
    // std::mem::size_of::<Player>() alone is a few hundred bytes.
    assert!(
        player.size() > 10 * std::mem::size_of_val(&player),
        "size() only reflects the struct's own layout, not its owned Vec contents: {}",
        player.size()
    );
}

#[test]
fn replay_size_is_at_least_the_sum_of_its_players() {
    let data = include_bytes!("../replays/USvDAK_v10612.rec");
    let replay = Replay::from_bytes(data).unwrap();
    let players_total: usize = replay.players().iter().map(DataTypeFunctions::size).sum();

    assert!(
        replay.size() >= players_total,
        "replay.size() ({}) should be at least the sum of its players' size() ({})",
        replay.size(),
        players_total
    );
}
