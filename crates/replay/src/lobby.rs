//! Lobby setup recorded in a replay: each player's inventory items and the lobby's settings.

use crate::data::chunks::LobbyOption as LobbyOptionData;
use crate::data::{CpuItem as CpuItemData, Item, PlayerItem as PlayerItemData};
use serde::{Deserialize, Serialize};

/// One item a player brought into the match: a battlegroup or a cosmetic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InventoryItem {
    /// A human player's item.
    Player(PlayerItem),
    /// An AI player's item, which uses a different record format.
    Cpu(CpuItem),
}

/// A human player's item, as Relic's inventory stores it. The named fields match the
/// `matchhistoryitems` entries Relic's match history API returns; the `unknown_*` fields have no
/// API counterpart yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerItem {
    instance_id: u64,
    definition_id: u64,
    profile_id: u64,
    durability: u64,
    durability_type: u8,
    metadata: String,
    slot: Option<u32>,
    acquired_at: u64,
    location_id: u64,
    unknown_1: u64,
    unknown_2: i64,
    unknown_3: i64,
    unknown_4: i64,
    extra: Vec<u8>,
}

impl PlayerItem {
    /// This copy of the item in the player's inventory.
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }
    /// Which item this is (e.g. which battlegroup), in Relic's inventory id space.
    pub fn definition_id(&self) -> u64 {
        self.definition_id
    }
    /// The owning player's profile id.
    pub fn profile_id(&self) -> u64 {
        self.profile_id
    }
    pub fn durability(&self) -> u64 {
        self.durability
    }
    pub fn durability_type(&self) -> u8 {
        self.durability_type
    }
    /// The item's metadata JSON, verbatim (e.g. `{"epos":"0","eslot":"0"}`).
    pub fn metadata(&self) -> &str {
        &self.metadata
    }
    /// The loadout slot the item is equipped in: the metadata's `eslot`. Items are not stored in
    /// slot order.
    pub fn slot(&self) -> Option<u32> {
        self.slot
    }
    /// When the player acquired the item, in Unix seconds. Inferred: constant per instance, and
    /// the earliest is the game's launch day.
    pub fn acquired_at(&self) -> u64 {
        self.acquired_at
    }
    /// The instance id of the container the item sits in: the player's faction loadout.
    pub fn location_id(&self) -> u64 {
        self.location_id
    }
    /// A small counter that mostly grows between a given item's appearances.
    pub fn unknown_1(&self) -> u64 {
        self.unknown_1
    }
    /// Always -1 so far.
    pub fn unknown_2(&self) -> i64 {
        self.unknown_2
    }
    /// 19 or 51 so far.
    pub fn unknown_3(&self) -> i64 {
        self.unknown_3
    }
    /// -1 or `i32::MAX` so far.
    pub fn unknown_4(&self) -> i64 {
        self.unknown_4
    }
    /// Record bytes past the known fields; empty unless Relic extends the format.
    pub fn extra(&self) -> &[u8] {
        &self.extra
    }
}

/// An AI player's item. Its id is not in the human items' definition id space.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuItem {
    id: u32,
}

impl CpuItem {
    pub fn id(&self) -> u32 {
        self.id
    }
}

impl InventoryItem {
    /// Bytes the item holds outside its own struct.
    #[cfg(feature = "magnus")]
    pub(crate) fn heap_size(&self) -> usize {
        match self {
            InventoryItem::Player(item) => item.metadata.capacity() + item.extra.capacity(),
            InventoryItem::Cpu(_) => 0,
        }
    }
}

impl From<&Item> for InventoryItem {
    fn from(item: &Item) -> Self {
        match item {
            Item::Player(item) => InventoryItem::Player(PlayerItem::from(item)),
            Item::Cpu(CpuItemData { id }) => InventoryItem::Cpu(CpuItem { id: *id }),
        }
    }
}

impl From<&PlayerItemData> for PlayerItem {
    fn from(item: &PlayerItemData) -> Self {
        PlayerItem {
            instance_id: item.instance_id,
            definition_id: item.definition_id,
            profile_id: item.profile_id,
            durability: item.durability,
            durability_type: item.durability_type,
            metadata: item.metadata.clone(),
            slot: metadata_slot(&item.metadata),
            acquired_at: item.acquired_at,
            location_id: item.location_id,
            unknown_1: item.unknown_1,
            unknown_2: item.unknown_2,
            unknown_3: item.unknown_3,
            unknown_4: item.unknown_4,
            extra: item.extra.clone(),
        }
    }
}

// `eslot` is a decimal string.
fn metadata_slot(metadata: &str) -> Option<u32> {
    let value: serde_json::Value = serde_json::from_str(metadata).ok()?;
    value.get("eslot")?.as_str()?.parse().ok()
}

/// One lobby setting as a name/value pair. Values are usually option indices rather than literal
/// amounts. Lobbies list settings in order and can repeat a name (custom lobbies write a setting's
/// name once with `1` and again with its value), so read them as a sequence, not a map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyOption {
    name: String,
    value: u32,
}

impl LobbyOption {
    /// The setting's name, e.g. `option_tickets`.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// The setting's value.
    pub fn value(&self) -> u32 {
        self.value
    }
}

impl From<&LobbyOptionData> for LobbyOption {
    fn from(option: &LobbyOptionData) -> Self {
        LobbyOption {
            name: option.name.clone(),
            value: option.value,
        }
    }
}
