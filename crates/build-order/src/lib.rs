//! Build order extraction from CoH3 replays.
//!
//! Ports reinforce's `Factory` logic: given a parsed [`Replay`], a player index,
//! and a [`VersionedStore`], classifies commands into a chronological build order.

mod error;
pub use error::Error;

use std::collections::HashMap;

use data::{Entity, Version, VersionedStore};
use replay::command_data::Source;
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
    /// The tick at which this action was marked suspect (cancellation command tick), if suspect.
    /// A building is suspect if it may have been cancelled before first use.
    pub suspect_since: Option<u32>,
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
///
/// If `include_cancelled` is `false` (the default), cancelled actions are excluded from the
/// returned build order.
pub fn extract_build_order(
    replay: &Replay,
    player_index: usize,
    store: &VersionedStore,
    include_cancelled: bool,
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

    if !include_cancelled {
        actions.retain(|a| !a.cancelled);
    }

    Ok(BuildOrder { actions })
}

// ── Internal types ────────────────────────────────────────────────────────────

struct PendingAction {
    tick: u32,
    index: u32,
    kind: BuildActionKind,
    pbgid: u32,
    suspect_since: Option<u32>,
    cancelled: bool,
    /// The building's own entity/instance id, in the truncated 16-bit form
    /// `Source::legacy_identifier` produces -- learned the first time any later
    /// command reveals it (a production it starts, or an entity-sourced command like
    /// a rally point), so it can be matched against a `CMD_CancelConstruction`'s
    /// source with certainty instead of by pbgid/chronology alone. Only ever set on
    /// entries in `Factory::buildings` -- see `Factory::bind_building_identity`.
    entity: Option<u16>,
}

impl PendingAction {
    fn into_build_action(self) -> BuildAction {
        BuildAction {
            tick: self.tick,
            index: self.index,
            kind: self.kind,
            pbgid: self.pbgid,
            suspect_since: self.suspect_since,
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
    /// Confirmed building identities: entity id (legacy 16-bit form) -> index into
    /// `buildings`. See `bind_building_identity`.
    building_entities: HashMap<u16, usize>,
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
            building_entities: HashMap::new(),
        }
    }

    fn classify(&mut self, command: &Command) -> bool {
        match command {
            Command::UseAbility(data) => self.classify_use_ability(
                data.tick(),
                data.index(),
                data.pbgid(),
                data.source_identifier(),
            ),
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
            Command::CancelConstruction(data) => {
                self.cancel_construction(data.tick(), data.source())
            }
            Command::CancelProduction(data) => {
                self.cancel_production(data.source_identifier(), data.queue_index())
            }
            Command::AITakeover(data) => self.process_takeover(data.tick()),
            // Entity-sourced commands that never represent a build-order action
            // themselves, but reveal an existing building's identity -- see
            // `bind_building_identity`. Not exhaustive over every entity-sourced
            // command type; these are the ones a building issues in practice
            // (validated against real replay data).
            Command::RallyPoint(data) => {
                self.bind_from_source(data.source());
                true
            }
            Command::AttackFromHold(data) => {
                self.bind_from_source(data.source());
                true
            }
            Command::UnloadSquads(data) => {
                self.bind_from_source(data.source());
                true
            }
            Command::Move(data) => {
                self.bind_from_source(data.source());
                true
            }
            _ => true,
        }
    }

    /// `bind_building_identity` restricted to `Source::Entity` -- the other `Source`
    /// kinds (squad(s), player) never name a building.
    fn bind_from_source(&mut self, source: &Source) {
        if let Source::Entity(_) = source {
            self.bind_building_identity(source.legacy_identifier(), None);
        }
    }

    fn classify_use_ability(
        &mut self,
        tick: u32,
        index: u32,
        pbgid: Option<u32>,
        source_identifier: u16,
    ) -> bool {
        // `None` means this command is continuing/updating an already-active ability's
        // target rather than starting a new one — nothing new to classify.
        let Some(pbgid) = pbgid else {
            return true;
        };
        if let Some(ability) = self.store.get_ability(pbgid, self.version) {
            if ability.autobuild {
                self.buildings.push(PendingAction {
                    tick,
                    index,
                    kind: BuildActionKind::ConstructBuilding,
                    pbgid,
                    suspect_since: None,
                    cancelled: false,
                    entity: None,
                });
            } else if !ability.spawns.is_empty() {
                self.buildings.push(PendingAction {
                    tick,
                    index,
                    kind: BuildActionKind::TrainUnit,
                    pbgid,
                    suspect_since: None,
                    cancelled: false,
                    entity: None,
                });
                // A non-autobuild ability call (e.g. a call-in/paradrop) is issued
                // from an existing building the same way BuildSquad is -- what it
                // produces can identify its source just as reliably.
                self.bind_building_identity(source_identifier, Some(pbgid));
            } else if !ability.upgrades.is_empty() {
                self.buildings.push(PendingAction {
                    tick,
                    index,
                    kind: BuildActionKind::ResearchUpgrade,
                    pbgid,
                    suspect_since: None,
                    cancelled: false,
                    entity: None,
                });
                self.bind_building_identity(source_identifier, Some(pbgid));
            }
        }
        true
    }

    fn classify_use_battlegroup_ability(&mut self, tick: u32, index: u32, pbgid: u32) -> bool {
        if let Some(ability) = self.store.get_ability(pbgid, self.version) {
            if ability.autobuild || ability.builds.is_some() {
                return self.push_battlegroup(
                    tick,
                    index,
                    pbgid,
                    BuildActionKind::ConstructBuilding,
                );
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
                suspect_since: None,
                cancelled: false,
                entity: None,
            });
        // What this building just produced identifies it, the same way a rally point
        // or another entity-sourced command would -- see `bind_building_identity`.
        self.bind_building_identity(source, Some(pbgid));
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
            suspect_since: None,
            cancelled: false,
            entity: None,
        });
        true
    }

    /// Binds `id` (a building's entity/instance id, truncated to the legacy 16-bit
    /// form) to a specific pending action in `self.buildings`, if it isn't already
    /// known and exactly one candidate matches. `produced_pbgid`, when given,
    /// restricts candidates to ones whose ability could plausibly have produced it
    /// (via `produces`) -- without that, two different building types placed close
    /// together would count as ambiguous with each other, which the pbgid-based
    /// suspect rectification below never had to worry about. When `produced_pbgid` is
    /// `None` (an entity-sourced command that doesn't reveal what it produced, or a
    /// `CancelProduction` that only reveals a queue slot), this narrows purely by
    /// elimination against whatever else is already known.
    ///
    /// Deliberately conservative: if more than one candidate remains, nothing is
    /// bound rather than guessing -- the caller falls back to today's
    /// chronology/pbgid-based handling for anything this can't resolve with
    /// certainty (validated against real replay data; see cohdb's cancellation-
    /// detection investigation).
    fn bind_building_identity(&mut self, id: u16, produced_pbgid: Option<u32>) {
        if self.building_entities.contains_key(&id) {
            return;
        }
        let mut candidates = self
            .buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| b.entity.is_none())
            .filter(|(_, b)| match produced_pbgid {
                Some(produced) => resolve_building_entity(b.pbgid, self.version, self.store)
                    .is_some_and(|entity| produces(&entity, produced, self.version, self.store)),
                None => true,
            })
            .map(|(i, _)| i);
        let Some(only) = candidates.next() else {
            return;
        };
        if candidates.next().is_some() {
            return;
        }
        self.buildings[only].entity = Some(id);
        self.building_entities.insert(id, only);
    }

    /// Resolves a `CMD_CancelConstruction`'s source against `self.buildings` with
    /// certainty when possible: cancels exactly the one identified pending building
    /// and leaves every other pending building in `self.buildings` untouched, without
    /// blanket-marking anything suspect. Falls back to today's blanket-suspect
    /// behavior for `self.buildings` only when identity can't determine which one was
    /// cancelled -- see `bind_building_identity` for why that's a deliberate,
    /// validated choice rather than a gap. `self.battlegroup` is untouched here
    /// either way; see `cancel_construction`, which always blanket-suspects it
    /// exactly as before, independent of how this resolves.
    fn cancel_buildings(&mut self, tick: u32, source: &Source) {
        // `Source::legacy_identifier` panics on `Squads` as a whole value (it's not
        // meaningful to truncate a *list* to one id), but the same per-element
        // transform is well-defined -- apply it to each id individually via a
        // single-element `Source::Entity` rather than duplicating the bit-twiddle.
        let ids: Vec<u16> = match source {
            Source::Entity(_) | Source::Squad(_) => vec![source.legacy_identifier()],
            Source::Squads(ids) => ids
                .iter()
                .map(|id| Source::Entity(*id).legacy_identifier())
                .collect(),
            Source::Player(_) => Vec::new(),
        };

        for id in &ids {
            if let Some(&idx) = self.building_entities.get(id) {
                self.buildings[idx].cancelled = true;
                return;
            }
        }

        // Id unseen: if exactly one still-unidentified building is pending, it must
        // be the one -- bind and cancel it with the same certainty as an
        // already-known id. Otherwise (zero candidates -- likely a squad-built
        // structure or other construction this classifier doesn't track at all -- or
        // two-plus genuinely simultaneous candidates) fall through to blanket suspect.
        let mut candidates = self
            .buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| b.entity.is_none() && b.suspect_since.is_none() && !b.cancelled)
            .map(|(i, _)| i);
        if let Some(only) = candidates.next() {
            if candidates.next().is_none() {
                self.buildings[only].cancelled = true;
                if let Some(&id) = ids.first() {
                    self.buildings[only].entity = Some(id);
                    self.building_entities.insert(id, only);
                }
                return;
            }
        }

        for building in &mut self.buildings {
            if building.entity.is_none() && building.suspect_since.is_none() && !building.cancelled
            {
                building.suspect_since = Some(tick);
            }
        }
    }

    fn cancel_construction(&mut self, tick: u32, source: &Source) -> bool {
        self.cancel_buildings(tick, source);
        // Unconditional, exactly as before this change: a battlegroup-built
        // structure has no identity signal to resolve against at all (see the module
        // doc on `cancel_buildings`), so it stays on the original blanket-suspect
        // path regardless of how `self.buildings` resolved.
        for building in &mut self.battlegroup {
            if building.kind == BuildActionKind::ConstructBuilding
                && building.suspect_since.is_none()
            {
                building.suspect_since = Some(tick);
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
        // A cancelled queue slot still proves its building is an active producer --
        // narrow by elimination the same way a successful production would.
        self.bind_building_identity(source, None);
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
            suspect_since: None,
            cancelled: false,
            entity: None,
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

/// Resolves the pbgid of an autobuild/battlegroup ability that constructs a building
/// (`ability.builds`) to that building's own `Entity` -- the thing whose `spawns`/
/// `upgrades` lists say what it can produce. Shared by `rectify_suspects` (checking
/// what a *suspect* building could have produced) and `bind_building_identity`
/// (checking what an *unresolved, not-yet-suspect* one could have).
fn resolve_building_entity(
    ability_pbgid: u32,
    version: Version,
    store: &VersionedStore,
) -> Option<Entity> {
    store
        .get_ability(ability_pbgid, version)
        .and_then(|a| a.builds.as_ref())
        .and_then(|builds_path| {
            let target = builds_path.replace('\\', "/");
            store.get_entity_by_path(&target, version).cloned()
        })
}

fn rectify_suspects(actions: &mut [BuildAction], version: Version, store: &VersionedStore) {
    let n = actions.len();
    for i in 0..n {
        if actions[i].suspect_since.is_none() {
            continue;
        }
        let suspect_pbgid = actions[i].pbgid;

        let building_entity = resolve_building_entity(suspect_pbgid, version, store);

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
            actions[i].suspect_since = None;
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
            suspect_since: Some(50),
            cancelled: false,
        };
        assert_eq!(action.tick, 100);
        assert_eq!(action.pbgid, 42);
        assert_eq!(action.suspect_since, Some(50));
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

    // Two pending buildings with no way to identify either -- the cancel's source
    // can't resolve to one specific building, so both fall back to today's blanket
    // suspicion, unchanged.
    #[test]
    fn cancel_construction_marks_buildings_as_suspect_when_ambiguous() {
        let store = VersionedStore::new();
        let mut factory = Factory::new(true, 10612, &store);
        for _ in 0..2 {
            factory.buildings.push(PendingAction {
                tick: 10,
                index: 0,
                kind: BuildActionKind::ConstructBuilding,
                pbgid: 42,
                suspect_since: None,
                cancelled: false,
                entity: None,
            });
        }
        factory.cancel_construction(20, &Source::Entity(999));
        assert_eq!(factory.buildings[0].suspect_since, Some(20));
        assert_eq!(factory.buildings[1].suspect_since, Some(20));
        assert!(!factory.buildings[0].cancelled);
        assert!(!factory.buildings[1].cancelled);
    }

    // Exactly one pending building and no other information -- it must be the one,
    // so it's cancelled with certainty rather than merely marked suspect.
    #[test]
    fn cancel_construction_cancels_the_sole_pending_building() {
        let store = VersionedStore::new();
        let mut factory = Factory::new(true, 10612, &store);
        factory.buildings.push(PendingAction {
            tick: 10,
            index: 0,
            kind: BuildActionKind::ConstructBuilding,
            pbgid: 42,
            suspect_since: None,
            cancelled: false,
            entity: None,
        });
        factory.cancel_construction(20, &Source::Entity(999));
        assert!(factory.buildings[0].cancelled);
        assert_eq!(factory.buildings[0].suspect_since, None);
    }

    // The cancelled building already revealed its identity (e.g. it had produced
    // something) before being cancelled -- resolves to exactly that one even with a
    // second, unrelated building still pending.
    #[test]
    fn cancel_construction_cancels_only_the_identified_building() {
        let store = VersionedStore::new();
        let mut factory = Factory::new(true, 10612, &store);
        factory.buildings.push(PendingAction {
            tick: 10,
            index: 0,
            kind: BuildActionKind::ConstructBuilding,
            pbgid: 42,
            suspect_since: None,
            cancelled: false,
            entity: None,
        });
        factory.buildings.push(PendingAction {
            tick: 15,
            index: 1,
            kind: BuildActionKind::ConstructBuilding,
            pbgid: 43,
            suspect_since: None,
            cancelled: false,
            entity: None,
        });
        factory
            .building_entities
            .insert(Source::Entity(999).legacy_identifier(), 0);
        factory.buildings[0].entity = Some(Source::Entity(999).legacy_identifier());

        factory.cancel_construction(20, &Source::Entity(999));

        assert!(factory.buildings[0].cancelled);
        assert!(!factory.buildings[1].cancelled);
        assert_eq!(factory.buildings[1].suspect_since, None);
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
        factory.classify_use_ability(10, 0, Some(100), 0);
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
    fn classify_use_battlegroup_ability_with_builds_as_construct_building() {
        let mut gd = data::GameData::new(10612);
        gd.abilities.insert(
            300,
            data::Ability {
                pbgid: 300,
                path: vec!["abilities".into(), "medical_tent".into()],
                loc_id: 0,
                icon_name: String::new(),
                autobuild: false,
                builds: Some("ebps/races/american/buildings/medical_tent".into()),
                spawns: vec![],
                upgrades: vec![],
                screen_name_formatter: None,
            },
        );
        let mut store = VersionedStore::new();
        store.add_version(gd);
        let mut factory = Factory::new(true, 10612, &store);
        factory.classify_use_battlegroup_ability(10, 0, 300);
        let actions = factory.consolidate();
        assert_eq!(actions[0].kind, BuildActionKind::ConstructBuilding);
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
                spawns: vec![
                    "ai/ai_ability_intents/spawns/air_and_sea_commandos_ability_intent".into(),
                ],
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
        let result = extract_build_order(&replay, 99, &store, false);
        assert!(result.is_err());
    }

    #[test]
    fn extract_build_order_returns_actions() {
        let store = VersionedStore::bundled();
        let data = include_bytes!("../../cohlib/replays/USvDAK_v10612.rec");
        let replay = Replay::from_bytes(data).unwrap();
        let build_order = extract_build_order(&replay, 0, &store, false).unwrap();
        // Should have at least some actions
        assert!(!build_order.actions.is_empty());
    }
}
