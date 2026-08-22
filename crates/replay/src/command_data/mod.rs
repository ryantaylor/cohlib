//! Representations of replay command data formats.

mod ability;
mod broadcast_message;
mod camera_counts;
mod camera_track;
mod construction;
mod empty;
mod orientation;
mod pbgid;
mod position;
mod resource_bonus;
mod source;
mod source_pbgid;
mod sourced;
mod sourced_ability;
mod sourced_index;
mod sourced_pbgid;
mod targeted;
mod unknown;

pub use crate::command_data::ability::Ability;
pub use crate::command_data::broadcast_message::BroadcastMessage;
pub use crate::command_data::camera_counts::CameraCounts;
pub use crate::command_data::camera_track::CameraTrack;
pub use crate::command_data::construction::Construction;
pub use crate::command_data::empty::Empty;
pub use crate::command_data::orientation::Orientation;
pub use crate::command_data::pbgid::Pbgid;
pub use crate::command_data::position::Position;
pub use crate::command_data::resource_bonus::ResourceBonus;
pub use crate::command_data::source::Source;
pub use crate::command_data::source_pbgid::SourcePbgid;
pub use crate::command_data::sourced::Sourced;
pub use crate::command_data::sourced_ability::SourcedAbility;
pub use crate::command_data::sourced_index::SourcedIndex;
pub use crate::command_data::sourced_pbgid::SourcedPbgid;
pub use crate::command_data::targeted::Targeted;
pub use crate::command_data::unknown::Unknown;

use crate::command_type::CommandType;

/// The fields every decoded command payload carries, plus defaults for the ones only
/// some payload shapes have. Lets `Command` expose its common accessors through a
/// single dispatch instead of one 37-arm match per accessor — see
/// `crate::command::command_variants!`.
pub trait CommandPayload {
    /// The Relic wire command type this command was decoded from.
    fn action_type(&self) -> CommandType;
    fn tick(&self) -> u32;
    fn index(&self) -> u32;
    /// The blueprint this command references, if the payload carries one.
    fn pbgid(&self) -> Option<u32> {
        None
    }
    /// Who issued the command, for the payload shapes that model it as a full
    /// [`Source`] rather than a bare `source_identifier`.
    fn source(&self) -> Option<&Source> {
        None
    }
}

impl CommandPayload for Empty {
    fn action_type(&self) -> CommandType {
        Empty::action_type(self)
    }
    fn tick(&self) -> u32 {
        Empty::tick(self)
    }
    fn index(&self) -> u32 {
        Empty::index(self)
    }
}

impl CommandPayload for Unknown {
    fn action_type(&self) -> CommandType {
        Unknown::action_type(self)
    }
    fn tick(&self) -> u32 {
        Unknown::tick(self)
    }
    fn index(&self) -> u32 {
        Unknown::index(self)
    }
}

impl CommandPayload for Targeted {
    fn action_type(&self) -> CommandType {
        Targeted::action_type(self)
    }
    fn tick(&self) -> u32 {
        Targeted::tick(self)
    }
    fn index(&self) -> u32 {
        Targeted::index(self)
    }
    fn source(&self) -> Option<&Source> {
        Some(Targeted::source(self))
    }
}

impl CommandPayload for Pbgid {
    fn action_type(&self) -> CommandType {
        Pbgid::action_type(self)
    }
    fn tick(&self) -> u32 {
        Pbgid::tick(self)
    }
    fn index(&self) -> u32 {
        Pbgid::index(self)
    }
    fn pbgid(&self) -> Option<u32> {
        Some(Pbgid::pbgid(self))
    }
}

impl CommandPayload for SourcedPbgid {
    fn action_type(&self) -> CommandType {
        SourcedPbgid::action_type(self)
    }
    fn tick(&self) -> u32 {
        SourcedPbgid::tick(self)
    }
    fn index(&self) -> u32 {
        SourcedPbgid::index(self)
    }
    fn pbgid(&self) -> Option<u32> {
        Some(SourcedPbgid::pbgid(self))
    }
    fn source(&self) -> Option<&Source> {
        Some(SourcedPbgid::source(self))
    }
}

impl CommandPayload for SourcePbgid {
    fn action_type(&self) -> CommandType {
        SourcePbgid::action_type(self)
    }
    fn tick(&self) -> u32 {
        SourcePbgid::tick(self)
    }
    fn index(&self) -> u32 {
        SourcePbgid::index(self)
    }
    fn pbgid(&self) -> Option<u32> {
        Some(SourcePbgid::pbgid(self))
    }
    fn source(&self) -> Option<&Source> {
        Some(SourcePbgid::source(self))
    }
}

impl CommandPayload for Sourced {
    fn action_type(&self) -> CommandType {
        Sourced::action_type(self)
    }
    fn tick(&self) -> u32 {
        Sourced::tick(self)
    }
    fn index(&self) -> u32 {
        Sourced::index(self)
    }
    fn source(&self) -> Option<&Source> {
        Some(Sourced::source(self))
    }
}

impl CommandPayload for SourcedIndex {
    fn action_type(&self) -> CommandType {
        SourcedIndex::action_type(self)
    }
    fn tick(&self) -> u32 {
        SourcedIndex::tick(self)
    }
    fn index(&self) -> u32 {
        SourcedIndex::index(self)
    }
    fn source(&self) -> Option<&Source> {
        Some(SourcedIndex::source(self))
    }
}

impl CommandPayload for Ability {
    fn action_type(&self) -> CommandType {
        Ability::action_type(self)
    }
    fn tick(&self) -> u32 {
        Ability::tick(self)
    }
    fn index(&self) -> u32 {
        Ability::index(self)
    }
    fn pbgid(&self) -> Option<u32> {
        Ability::pbgid(self)
    }
    fn source(&self) -> Option<&Source> {
        Some(Ability::source(self))
    }
}

impl CommandPayload for SourcedAbility {
    fn action_type(&self) -> CommandType {
        SourcedAbility::action_type(self)
    }
    fn tick(&self) -> u32 {
        SourcedAbility::tick(self)
    }
    fn index(&self) -> u32 {
        SourcedAbility::index(self)
    }
    fn pbgid(&self) -> Option<u32> {
        SourcedAbility::pbgid(self)
    }
    fn source(&self) -> Option<&Source> {
        Some(SourcedAbility::source(self))
    }
}

impl CommandPayload for Construction {
    fn action_type(&self) -> CommandType {
        Construction::action_type(self)
    }
    fn tick(&self) -> u32 {
        Construction::tick(self)
    }
    fn index(&self) -> u32 {
        Construction::index(self)
    }
    fn pbgid(&self) -> Option<u32> {
        Some(Construction::pbgid(self))
    }
}

impl CommandPayload for ResourceBonus {
    fn action_type(&self) -> CommandType {
        ResourceBonus::action_type(self)
    }
    fn tick(&self) -> u32 {
        ResourceBonus::tick(self)
    }
    fn index(&self) -> u32 {
        ResourceBonus::index(self)
    }
}

impl CommandPayload for BroadcastMessage {
    fn action_type(&self) -> CommandType {
        BroadcastMessage::action_type(self)
    }
    fn tick(&self) -> u32 {
        BroadcastMessage::tick(self)
    }
    fn index(&self) -> u32 {
        BroadcastMessage::index(self)
    }
}
