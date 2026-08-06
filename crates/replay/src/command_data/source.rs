use serde::{Deserialize, Serialize};

/// Identifies who or what issued a command. Wire-level parsing lives alongside the rest
/// of the payload format in `crate::data::ticks::payload`.

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// A player-issued command (surrender, resource donation, ...).
    Player(u8),
    /// A command issued by, or targeting, a single non-squad entity (building,
    /// transport, ...).
    Entity(u32),
    /// A command issued by a single squad.
    Squad(u32),
    /// A command issued by multiple squads at once, e.g. a multi-unit selection giving
    /// a group order.
    Squads(Vec<u32>),
}

impl Source {
    /// A stable, lowercase name for this source's kind, e.g. for use as a tag in
    /// serialized output.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Player(_) => "player",
            Self::Entity(_) => "entity",
            Self::Squad(_) => "squad",
            Self::Squads(_) => "squads",
        }
    }

    /// The id(s) carried by this source, always as a list — a single-element list for
    /// the scalar kinds — so callers don't need to branch on `kind` to read it.
    pub fn ids(&self) -> Vec<u32> {
        match self {
            Self::Player(id) => vec![*id as u32],
            Self::Entity(id) | Self::Squad(id) => vec![*id],
            Self::Squads(ids) => ids.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_kind_and_ids() {
        let source = Source::Player(3);
        assert_eq!(source.kind(), "player");
        assert_eq!(source.ids(), vec![3]);
    }

    #[test]
    fn entity_kind_and_ids() {
        let source = Source::Entity(1234);
        assert_eq!(source.kind(), "entity");
        assert_eq!(source.ids(), vec![1234]);
    }

    #[test]
    fn squad_kind_and_ids() {
        let source = Source::Squad(4096);
        assert_eq!(source.kind(), "squad");
        assert_eq!(source.ids(), vec![4096]);
    }

    #[test]
    fn squads_kind_and_ids() {
        let source = Source::Squads(vec![4096, 4097, 4098]);
        assert_eq!(source.kind(), "squads");
        assert_eq!(source.ids(), vec![4096, 4097, 4098]);
    }

    #[test]
    fn empty_squads_still_reports_squads_kind() {
        let source = Source::Squads(vec![]);
        assert_eq!(source.kind(), "squads");
        assert_eq!(source.ids(), Vec::<u32>::new());
    }
}
