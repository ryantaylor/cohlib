//! Lobby setup recorded in a replay: each player's inventory items and the lobby's settings.

use crate::data::chunks::LobbyOption as LobbyOptionData;
use crate::data::Item;
use serde::{Deserialize, Serialize};

/// One item a player brought into the match: a battlegroup or a cosmetic. Only the definition id
/// is decoded; `raw` holds the item's whole record exactly as the replay file stores it, so fields
/// that aren't decoded yet can still be recovered from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryItem {
    definition_id: Option<u64>,
    raw: Vec<u8>,
}

impl InventoryItem {
    /// Which item this is (e.g. which battlegroup), in Relic's inventory id space. `None` for AI
    /// players' items, which use a different record format.
    pub fn definition_id(&self) -> Option<u64> {
        self.definition_id
    }
    /// The item's full record as stored in the replay file.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
}

impl From<&Item> for InventoryItem {
    fn from(item: &Item) -> Self {
        InventoryItem {
            definition_id: item.definition_id(),
            raw: item.raw.clone(),
        }
    }
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
