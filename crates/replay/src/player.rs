//! Representation of parsed player information.

use crate::command::Command;
use crate::command_data::{CameraCounts, CameraTrack};
use crate::data::Player as PlayerData;
use crate::map::StartingPosition;
use crate::message::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::fmt::{Display, Formatter};

/// Game-specific player representation. Includes generally immutable information alongside data
/// specific to the replay being parsed.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "magnus", derive(magnus::TypedData))]
#[cfg_attr(
    feature = "magnus",
    magnus(class = "CohLib::Player", size, free_immediately)
)]
pub struct Player {
    id: u32,
    name: String,
    human: bool,
    faction: Faction,
    team: Team,
    battlegroup: Option<u32>,
    battlegroup_selected_at: Option<u32>,
    ai_takeover_at: Option<u32>,
    steam_id: Option<u64>,
    profile_id: Option<u64>,
    messages: Vec<Message>,
    commands: Vec<Command>,
    camera_tracks: Vec<CameraTrack>,
    camera_counts: Vec<CameraCounts>,
    starting_position: Option<StartingPosition>,
}

// Every accessor below that returns an owned Vec (commands, camera_tracks, ...) clones this
// struct's contents on every call, since Magnus's wrapped-value model has no way to hand Ruby a
// borrowed reference into it. The default DataTypeFunctions::size (std::mem::size_of_val) only
// sees this struct's own stack layout -- a few hundred bytes -- and is blind to the megabytes a
// commands/camera_tracks/camera_counts Vec actually holds, so Ruby's GC never learns a Player is
// expensive and won't collect one proactively. A caller that re-fetches players (or their
// commands/tracks/counts) in a loop -- e.g. filtering rows player-by-player instead of grouping
// once -- can push resident memory into the gigabytes before GC catches up. This reports the real
// cost so Ruby collects eagerly under that pattern instead of relying on caller discipline alone.
#[cfg(feature = "magnus")]
impl magnus::DataTypeFunctions for Player {
    fn size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.name.capacity()
            + self.messages.capacity() * std::mem::size_of::<Message>()
            + self.commands.capacity() * std::mem::size_of::<Command>()
            + self.camera_tracks.capacity() * std::mem::size_of::<CameraTrack>()
            + self.camera_counts.capacity() * std::mem::size_of::<CameraCounts>()
    }
}

impl Player {
    /// The player's slot identifier as recorded in the replay file itself. Stable across every
    /// command/camera stream and starting position in this replay, including for AI (unlike
    /// `Player::profile_id`, which AI never have) -- this is the same identifier cohlib uses
    /// internally to attach `Player::commands`, `Player::camera_tracks`, `Player::camera_counts`
    /// and `Player::starting_position` to this player. Not a cross-replay identity: the same
    /// person's `id` can differ between two different replay files.
    pub fn id(&self) -> u32 {
        self.id
    }
    /// Name of the player at the time the replay was recorded. Note that the player may have
    /// changed their name since time of recording. If attempting to uniquely identify players
    /// across replay files, look at `Player::steam_id` and `Player::profile_id` instead. The string
    /// is UTF-16 encoded.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Whether or not the player was a human or an AI/CPU player.
    pub fn human(&self) -> bool {
        self.human
    }
    /// The faction selected by the player in this match.
    pub fn faction(&self) -> Faction {
        self.faction
    }
    /// The team the player was assigned to. Currently only head-to-head matchups are supported
    /// (max two teams).
    pub fn team(&self) -> Team {
        self.team
    }
    /// The pbgid of the battlegroup the player selected, or `None` if no battlegroup was selected.
    /// For details on what this ID represents please see `SelectBattlegroup::pbgid`.
    pub fn battlegroup(&self) -> Option<u32> {
        self.battlegroup
    }
    /// The tick at which the player selected their battlegroup, or `None` if no battlegroup was selected.
    pub fn battlegroup_selected_at(&self) -> Option<u32> {
        self.battlegroup_selected_at
    }
    /// The tick at which the player dropped from the game and AI took over their army, or `None` if the
    /// player never dropped from the game.
    pub fn ai_takeover_at(&self) -> Option<u32> {
        self.ai_takeover_at
    }
    /// The Steam ID of the player, or `None` if the player is AI. This ID can be used to uniquely
    /// identify a player between replays, and connect them to their Steam profile.
    pub fn steam_id(&self) -> Option<u64> {
        self.steam_id
    }
    /// The Relic profile ID of the player, or `None` if the player is AI. This ID can be used to
    /// uniquely identify a player between replays, and can be used to query statistical information
    /// about the player from Relic's stats API.
    pub fn profile_id(&self) -> Option<u64> {
        self.profile_id
    }
    /// A list of all messages sent by the player in the match. Sorted chronologically from first
    /// to last.
    pub fn messages(&self) -> Vec<Message> {
        self.messages.clone()
    }

    /// A list of all commands executed by the player in the match. Sorted chronologically from
    /// first to last.
    pub fn commands(&self) -> Vec<Command> {
        self.commands.clone()
    }

    /// Per-player camera telemetry recorded throughout the match. Not player-issued
    /// commands — see [`CameraTrack`] — so kept separate from [`Self::commands`].
    pub fn camera_tracks(&self) -> Vec<CameraTrack> {
        self.camera_tracks.clone()
    }

    /// Per-player camera diagnostic counters recorded throughout the match. Not
    /// player-issued commands — see [`CameraCounts`] — so kept separate from
    /// [`Self::commands`].
    pub fn camera_counts(&self) -> Vec<CameraCounts> {
        self.camera_counts.clone()
    }

    /// A list of only build-related commands executed by the player in the match. A build command
    /// is any that enqueues the construction of a new unit or upgrade. Sorted chronologically from
    /// first to last.
    pub fn build_commands(&self) -> Vec<Command> {
        self.commands
            .clone()
            .into_iter()
            .filter(|entry| {
                matches!(
                    entry,
                    Command::BuildGlobalUpgrade(_) | Command::BuildSquad(_)
                )
            })
            .collect()
    }

    /// A list of only battlegroup-related commands executed by the player in the match. A
    /// battlegroup command is any that involves the select or use of battlegroups and their
    /// abilities.
    pub fn battlegroup_commands(&self) -> Vec<Command> {
        self.commands
            .clone()
            .into_iter()
            .filter(|entry| {
                matches!(
                    entry,
                    Command::SelectBattlegroup(_)
                        | Command::SelectBattlegroupAbility(_)
                        | Command::UseBattlegroupAbility(_)
                )
            })
            .collect()
    }

    /// The player's starting position on the map, or `None` if the replay was recorded before
    /// this data was written to the replay file. See `Map::starting_positions` for the
    /// coordinate convention.
    pub fn starting_position(&self) -> Option<&StartingPosition> {
        self.starting_position.as_ref()
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn player_from_data(
    player_data: &PlayerData,
    messages: &HashMap<String, Vec<Message>>,
    commands: &HashMap<u32, Vec<Command>>,
    camera_tracks: &HashMap<u32, Vec<CameraTrack>>,
    camera_counts: &HashMap<u32, Vec<CameraCounts>>,
    starting_positions: &[StartingPosition],
) -> Player {
    let mut player = Player {
        id: player_data.id,
        name: player_data.name.clone(),
        human: player_data.human != 0,
        faction: Faction::try_from(player_data.faction.as_ref()).unwrap(),
        team: Team::try_from(player_data.team).unwrap(),
        steam_id: None,
        profile_id: None,
        messages: messages.get(&player_data.name).cloned().unwrap_or_default(),
        commands: commands.get(&player_data.id).cloned().unwrap_or_default(),
        camera_tracks: camera_tracks
            .get(&player_data.id)
            .cloned()
            .unwrap_or_default(),
        camera_counts: camera_counts
            .get(&player_data.id)
            .cloned()
            .unwrap_or_default(),
        battlegroup: None,
        battlegroup_selected_at: None,
        ai_takeover_at: None,
        starting_position: starting_positions
            .iter()
            .find(|position| position.index() == player_data.id)
            .cloned(),
    };

    if player.human {
        player.steam_id = Some(str::parse(&player_data.steam_id).unwrap());
        player.profile_id = Some(player_data.profile_id);
    }

    match player
        .commands
        .iter()
        .find(|&command| matches!(command, Command::SelectBattlegroup(_)))
    {
        Some(Command::SelectBattlegroup(command)) => {
            player.battlegroup = Some(command.pbgid());
            player.battlegroup_selected_at = Some(command.tick());
        }
        Some(_) => panic!(),
        None => {}
    };

    player.ai_takeover_at = match player
        .commands
        .iter()
        .find(|&command| matches!(command, Command::AITakeover(_)))
    {
        Some(Command::AITakeover(command)) => Some(command.tick()),
        Some(_) => panic!(),
        None => None,
    };

    player
}

impl Display for Player {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

// this is safe as Player does not contain any Ruby types

/// Company of Heroes 3 factions.

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Faction {
    Americans,
    British,
    Wehrmacht,
    AfrikaKorps,
}

impl Display for Faction {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Faction::Americans => write!(f, "americans"),
            Faction::British => write!(f, "british_africa"),
            Faction::Wehrmacht => write!(f, "germans"),
            Faction::AfrikaKorps => write!(f, "afrika_korps"),
        }
    }
}

impl TryFrom<&str> for Faction {
    type Error = String;

    fn try_from(input: &str) -> Result<Faction, Self::Error> {
        match input {
            "americans" => Ok(Faction::Americans),
            "british" => Ok(Faction::British),
            "british_africa" => Ok(Faction::British),
            "germans" => Ok(Faction::Wehrmacht),
            "afrika_korps" => Ok(Faction::AfrikaKorps),
            _ => Err(format!("Invalid faction type {}!", input)),
        }
    }
}

/// Representation of a player's team membership.

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Team {
    First = 0,
    Second = 1,
}

impl Team {
    /// Integer representation of the assigned team.
    pub fn value(&self) -> usize {
        *self as usize
    }
}

impl TryFrom<u32> for Team {
    type Error = String;

    fn try_from(input: u32) -> Result<Team, Self::Error> {
        match input {
            0 => Ok(Team::First),
            1 => Ok(Team::Second),
            10000 => Ok(Team::Second),
            _ => Err(format!("Invalid team ID {}!", input)),
        }
    }
}
