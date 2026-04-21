//! Build order extraction from CoH3 replays.
//!
//! Ports reinforce's `Factory` logic: given a parsed [`Replay`], a player index,
//! and a [`VersionedStore`], classifies commands into a chronological build order.

mod error;
pub use error::Error;

use std::collections::HashMap;

use data::{Entity, Version, VersionedStore};
use replay::{Command, Replay};

/// A single action in the build order.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "magnus", magnus::wrap(class = "CohLib::BuildAction"))]
pub struct BuildAction {
    /// Game tick at which the action occurred. Divide by 8 for seconds.
    pub tick: u32,
    /// Command index within the tick, used for tie-breaking.
    pub index: u32,
    /// The kind of build action.
    pub kind: BuildActionKind,
    /// The pbgid of the entity/ability/upgrade being built.
    pub pbgid: u32,
    /// Whether this action is a suspect (building may have been cancelled before use).
    pub suspect: bool,
    /// Whether this action was cancelled.
    pub cancelled: bool,
}

/// The classification of a build action.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildActionKind {
    /// A building was placed via an autobuild ability.
    ConstructBuilding,
    /// A squad was trained (BuildSquad or spawner ability).
    TrainUnit,
    /// An upgrade was researched (BuildGlobalUpgrade).
    ResearchUpgrade,
    /// A battlegroup was selected.
    SelectBattlegroup,
    /// A battlegroup ability was selected.
    SelectBattlegroupAbility,
    /// A battlegroup ability was used.
    UseBattlegroupAbility,
    /// The player dropped and AI took over.
    AITakeover,
}

/// The complete build order for a single player.
#[cfg_attr(feature = "magnus", magnus::wrap(class = "CohLib::BuildOrder"))]
pub struct BuildOrder {
    pub actions: Vec<BuildAction>,
}

/// Extract the build order for `player_index` from `replay` using game data from `store`.
pub fn extract_build_order(
    replay: &Replay,
    player_index: usize,
    store: &VersionedStore,
) -> Result<BuildOrder, Error> {
    let version = replay.version() as Version;
    let players = replay.players();
    let player = players
        .get(player_index)
        .ok_or_else(|| Error::BuildOrder(format!("player index {player_index} out of range")))?;

    let mut factory = Factory::new(player.human(), version, store);
    for command in player.commands() {
        if !factory.classify(&command) {
            break;
        }
    }

    let mut actions = factory.consolidate();
    rectify_suspects(&mut actions, version, store);

    Ok(BuildOrder { actions })
}

// ── Internal types ────────────────────────────────────────────────────────────

struct PendingAction {
    tick: u32,
    index: u32,
    kind: BuildActionKind,
    pbgid: u32,
    suspect: bool,
    cancelled: bool,
}

impl PendingAction {
    fn into_build_action(self) -> BuildAction {
        BuildAction {
            tick: self.tick,
            index: self.index,
            kind: self.kind,
            pbgid: self.pbgid,
            suspect: self.suspect,
            cancelled: self.cancelled,
        }
    }
}

struct Factory<'a> {
    human: bool,
    version: Version,
    store: &'a VersionedStore,
    buildings: Vec<PendingAction>,
    productions: HashMap<u16, Vec<PendingAction>>,
    battlegroup: Vec<PendingAction>,
    takeover: Vec<PendingAction>,
}

impl<'a> Factory<'a> {
    fn new(human: bool, version: Version, store: &'a VersionedStore) -> Self {
        Self {
            human,
            version,
            store,
            buildings: Vec::new(),
            productions: HashMap::new(),
            battlegroup: Vec::new(),
            takeover: Vec::new(),
        }
    }

    fn classify(&mut self, command: &Command) -> bool {
        match command {
            Command::UseAbility(data) => {
                self.classify_use_ability(data.tick(), data.index(), data.pbgid())
            }
            Command::BuildSquad(data) => self.push_production(
                data.tick(),
                data.index(),
                data.pbgid(),
                data.source_identifier(),
                BuildActionKind::TrainUnit,
            ),
            Command::BuildGlobalUpgrade(data) => self.push_production(
                data.tick(),
                data.index(),
                data.pbgid(),
                data.source_identifier(),
                BuildActionKind::ResearchUpgrade,
            ),
            Command::SelectBattlegroup(data) => self.push_battlegroup(
                data.tick(),
                data.index(),
                data.pbgid(),
                BuildActionKind::SelectBattlegroup,
            ),
            Command::SelectBattlegroupAbility(data) => self.push_battlegroup(
                data.tick(),
                data.index(),
                data.pbgid(),
                BuildActionKind::SelectBattlegroupAbility,
            ),
            Command::UseBattlegroupAbility(data) => {
                self.classify_use_battlegroup_ability(data.tick(), data.index(), data.pbgid())
            }
            Command::CancelConstruction(_) => self.cancel_construction(),
            Command::CancelProduction(data) => {
                self.cancel_production(data.source_identifier(), data.queue_index())
            }
            Command::AITakeover(data) => self.process_takeover(data.tick()),
            _ => true,
        }
    }

    fn classify_use_ability(&mut self, tick: u32, index: u32, pbgid: u32) -> bool {
        if let Some(ability) = self.store.get_ability(pbgid, self.version) {
            if ability.autobuild {
                self.buildings.push(PendingAction {
                    tick,
                    index,
                    kind: BuildActionKind::ConstructBuilding,
                    pbgid,
                    suspect: false,
                    cancelled: false,
                });
            } else if !ability.spawns.is_empty() {
                self.buildings.push(PendingAction {
                    tick,
                    index,
                    kind: BuildActionKind::TrainUnit,
                    pbgid,
                    suspect: false,
                    cancelled: false,
                });
            } else if !ability.upgrades.is_empty() {
                self.buildings.push(PendingAction {
                    tick,
                    index,
                    kind: BuildActionKind::ResearchUpgrade,
                    pbgid,
                    suspect: false,
                    cancelled: false,
                });
            }
        }
        true
    }

    fn classify_use_battlegroup_ability(&mut self, tick: u32, index: u32, pbgid: u32) -> bool {
        if let Some(ability) = self.store.get_ability(pbgid, self.version) {
            if ability.autobuild {
                return self.push_battlegroup(tick, index, pbgid, BuildActionKind::ConstructBuilding);
            } else if !ability.spawns.is_empty() {
                return self.push_battlegroup(tick, index, pbgid, BuildActionKind::TrainUnit);
            } else if !ability.upgrades.is_empty() {
                return self.push_battlegroup(tick, index, pbgid, BuildActionKind::ResearchUpgrade);
            }
        }

        self.push_battlegroup(tick, index, pbgid, BuildActionKind::UseBattlegroupAbility)
    }

    fn push_production(
        &mut self,
        tick: u32,
        index: u32,
        pbgid: u32,
        source: u16,
        kind: BuildActionKind,
    ) -> bool {
        self.productions
            .entry(source)
            .or_default()
            .push(PendingAction {
                tick,
                index,
                kind,
                pbgid,
                suspect: false,
                cancelled: false,
            });
        true
    }

    fn push_battlegroup(
        &mut self,
        tick: u32,
        index: u32,
        pbgid: u32,
        kind: BuildActionKind,
    ) -> bool {
        self.battlegroup.push(PendingAction {
            tick,
            index,
            kind,
            pbgid,
            suspect: false,
            cancelled: false,
        });
        true
    }

    fn cancel_construction(&mut self) -> bool {
        for building in &mut self.buildings {
            if !building.suspect {
                building.suspect = true;
            }
        }
        true
    }

    fn cancel_production(&mut self, source: u16, queue_index: u32) -> bool {
        if let Some(queue) = self.productions.get_mut(&source) {
            let idx = (queue_index as usize).saturating_sub(1);
            if let Some(action) = queue.get_mut(idx) {
                action.cancelled = true;
            }
        }
        true
    }

    fn process_takeover(&mut self, tick: u32) -> bool {
        if !self.human {
            return true;
        }
        self.takeover.push(PendingAction {
            tick,
            index: 0,
            kind: BuildActionKind::AITakeover,
            pbgid: 0,
            suspect: false,
            cancelled: false,
        });
        false
    }

    fn consolidate(self) -> Vec<BuildAction> {
        let mut all: Vec<PendingAction> = self
            .buildings
            .into_iter()
            .chain(self.battlegroup)
            .chain(self.takeover)
            .chain(self.productions.into_values().flatten())
            .collect();
        all.sort_by(|a, b| a.tick.cmp(&b.tick).then(a.index.cmp(&b.index)));
        all.into_iter().map(|p| p.into_build_action()).collect()
    }
}

// ── Suspect rectification ─────────────────────────────────────────────────────

fn rectify_suspects(actions: &mut [BuildAction], version: Version, store: &VersionedStore) {
    let n = actions.len();
    for i in 0..n {
        if !actions[i].suspect {
            continue;
        }
        let suspect_pbgid = actions[i].pbgid;

        let building_entity: Option<Entity> = store
            .get_ability(suspect_pbgid, version)
            .and_then(|a| a.builds.as_ref())
            .and_then(|builds_path| {
                let target = builds_path.replace('\\', "/");
                store.get_entity_by_path(&target, version).cloned()
            });

        let next_same = actions[(i + 1)..]
            .iter()
            .position(|a| a.pbgid == suspect_pbgid)
            .map(|pos| i + 1 + pos)
            .unwrap_or(n);

        let relevant = &actions[(i + 1)..next_same];

        let used = relevant.iter().any(|a| {
            building_entity
                .as_ref()
                .map(|entity| produces(entity, a.pbgid, version, store))
                .unwrap_or(false)
        });

        if used {
            actions[i].suspect = false;
        }
    }
}

fn produces(entity: &Entity, pbgid: u32, version: Version, store: &VersionedStore) -> bool {
    let squad_path = store.get_squad(pbgid, version).map(|s| s.path.join("/"));
    let upgrade_path = store.get_upgrade(pbgid, version).map(|u| u.path.join("/"));

    squad_path
        .as_deref()
        .map(|p| {
            entity.spawns.iter().any(|s| {
                let s = s.replace('\\', "/");
                s.ends_with(p) || p.ends_with(&s)
            })
        })
        .unwrap_or(false)
        || upgrade_path
            .as_deref()
            .map(|p| {
                entity.upgrades.iter().any(|u| {
                    let u = u.replace('\\', "/");
                    u.ends_with(p) || p.ends_with(&u)
                })
            })
            .unwrap_or(false)
}

// ── Test helpers ──────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_action_fields() {
        let action = BuildAction {
            tick: 100,
            index: 1,
            kind: BuildActionKind::ConstructBuilding,
            pbgid: 42,
            suspect: true,
            cancelled: false,
        };
        assert_eq!(action.tick, 100);
        assert_eq!(action.pbgid, 42);
        assert!(action.suspect);
        assert!(!action.cancelled);
        assert_eq!(action.kind, BuildActionKind::ConstructBuilding);
    }

    #[test]
    fn cancel_production_marks_correct_index() {
        let store = VersionedStore::new();
        let mut factory = Factory::new(true, 10612, &store);
        factory.push_production(10, 0, 100, 1, BuildActionKind::TrainUnit);
        factory.push_production(20, 0, 200, 1, BuildActionKind::TrainUnit);
        factory.cancel_production(1, 1);
        let actions = factory.consolidate();
        assert!(actions[0].cancelled);
        assert!(!actions[1].cancelled);
        assert_eq!(actions.len(), 2);
    }

    #[test]
    fn ai_takeover_stops_processing_for_human() {
        let store = VersionedStore::new();
        let mut factory = Factory::new(true, 10612, &store);
        let result = factory.process_takeover(50);
        assert!(!result);
        assert_eq!(factory.takeover.len(), 1);
    }

    #[test]
    fn ai_takeover_continues_for_cpu() {
        let store = VersionedStore::new();
        let mut factory = Factory::new(false, 10612, &store);
        let result = factory.process_takeover(50);
        assert!(result);
        assert_eq!(factory.takeover.len(), 0);
    }

    #[test]
    fn consolidate_sorts_by_tick_then_index() {
        let store = VersionedStore::new();
        let mut factory = Factory::new(true, 10612, &store);
        factory.push_production(30, 2, 300, 1, BuildActionKind::TrainUnit);
        factory.push_production(10, 1, 100, 1, BuildActionKind::TrainUnit);
        factory.push_production(10, 0, 50, 2, BuildActionKind::TrainUnit);
        let actions = factory.consolidate();
        assert_eq!(actions[0].pbgid, 50);
        assert_eq!(actions[1].pbgid, 100);
        assert_eq!(actions[2].pbgid, 300);
    }

    #[test]
    fn cancel_construction_marks_buildings_as_suspect() {
        let store = VersionedStore::new();
        let mut factory = Factory::new(true, 10612, &store);
        factory.buildings.push(PendingAction {
            tick: 10,
            index: 0,
            kind: BuildActionKind::ConstructBuilding,
            pbgid: 42,
            suspect: false,
            cancelled: false,
        });
        factory.cancel_construction();
        assert!(factory.buildings[0].suspect);
    }

    #[test]
    fn classify_use_ability_as_train_unit() {
        let mut gd = data::GameData::new(10612);
        gd.abilities.insert(
            100,
            data::Ability {
                pbgid: 100,
                path: vec!["abilities".into(), "call_in".into()],
                loc_id: 0,
                icon_name: String::new(),
                autobuild: false,
                builds: None,
                spawns: vec!["sbps/races/german/infantry/coastal_reserves_ger".into()],
                upgrades: vec![],
                screen_name_formatter: None,
            },
        );
        let mut store = VersionedStore::new();
        store.add_version(gd);
        let mut factory = Factory::new(true, 10612, &store);
        factory.classify_use_ability(10, 0, 100);
        let actions = factory.consolidate();
        assert_eq!(actions[0].kind, BuildActionKind::TrainUnit);
    }

    #[test]
    fn classify_use_battlegroup_ability_as_train_unit() {
        let mut gd = data::GameData::new(10612);
        gd.abilities.insert(
            2164165,
            data::Ability {
                pbgid: 2164165,
                path: vec!["abilities".into(), "canadian_shock".into()],
                loc_id: 0,
                icon_name: String::new(),
                autobuild: false,
                builds: None,
                spawns: vec!["sbps/races/british/infantry/heavy_infantry_canadian_uk".into()],
                upgrades: vec![],
                screen_name_formatter: None,
            },
        );
        let mut store = VersionedStore::new();
        store.add_version(gd);
        let mut factory = Factory::new(true, 10612, &store);
        factory.classify_use_battlegroup_ability(10, 0, 2164165);
        let actions = factory.consolidate();
        assert_eq!(actions[0].kind, BuildActionKind::TrainUnit);
        assert_eq!(actions[0].pbgid, 2164165);
    }

    #[test]
    fn classify_use_battlegroup_ability_as_research_upgrade() {
        let mut gd = data::GameData::new(10612);
        gd.abilities.insert(
            200,
            data::Ability {
                pbgid: 200,
                path: vec!["abilities".into(), "upgrade_ability".into()],
                loc_id: 0,
                icon_name: String::new(),
                autobuild: false,
                builds: None,
                spawns: vec![],
                upgrades: vec!["upgrade/german/research/global_upgrade".into()],
                screen_name_formatter: None,
            },
        );
        let mut store = VersionedStore::new();
        store.add_version(gd);
        let mut factory = Factory::new(true, 10612, &store);
        factory.classify_use_battlegroup_ability(10, 0, 200);
        let actions = factory.consolidate();
        assert_eq!(actions[0].kind, BuildActionKind::ResearchUpgrade);
    }

    #[test]
    fn classify_paradrop_as_train_unit() {
        let mut gd = data::GameData::new(10612);
        gd.abilities.insert(
            2029788,
            data::Ability {
                pbgid: 2029788,
                path: vec!["abilities".into(), "paradrop".into()],
                loc_id: 0,
                icon_name: String::new(),
                autobuild: false,
                builds: None,
                spawns: vec!["ai/ai_ability_intents/spawns/air_and_sea_commandos_ability_intent".into()],
                upgrades: vec![],
                screen_name_formatter: None,
            },
        );
        let mut store = VersionedStore::new();
        store.add_version(gd);
        let mut factory = Factory::new(true, 10612, &store);
        factory.classify_use_battlegroup_ability(10, 0, 2029788);
        let actions = factory.consolidate();
        assert_eq!(actions[0].kind, BuildActionKind::TrainUnit);
    }

    #[test]
    fn classify_conversion_as_train_unit() {
        let mut gd = data::GameData::new(10612);
        gd.abilities.insert(
            2166906,
            data::Ability {
                pbgid: 2166906,
                path: vec!["abilities".into(), "conversion".into()],
                loc_id: 0,
                icon_name: String::new(),
                autobuild: false,
                builds: None,
                spawns: vec!["sbps/races/german/infantry/sturmpioneer_ger".into()],
                upgrades: vec![],
                screen_name_formatter: None,
            },
        );
        let mut store = VersionedStore::new();
        store.add_version(gd);
        let mut factory = Factory::new(true, 10612, &store);
        factory.classify_use_battlegroup_ability(10, 0, 2166906);
        let actions = factory.consolidate();
        assert_eq!(actions[0].kind, BuildActionKind::TrainUnit);
    }

    #[test]
    fn extract_build_order_invalid_player_index() {
        let store = VersionedStore::new();
        let data = include_bytes!("../../cohlib/replays/USvDAK_v10612.rec");
        let replay = Replay::from_bytes(data).unwrap();
        let result = extract_build_order(&replay, 99, &store);
        assert!(result.is_err());
    }

    #[test]
    fn extract_build_order_returns_actions() {
        let store = VersionedStore::bundled();
        let data = include_bytes!("../../cohlib/replays/USvDAK_v10612.rec");
        let replay = Replay::from_bytes(data).unwrap();
        let build_order = extract_build_order(&replay, 0, &store).unwrap();
        // Should have at least some actions
        assert!(!build_order.actions.is_empty());
    }
}
