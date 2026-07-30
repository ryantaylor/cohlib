use crate::{
    command_data::Source,
    command_type::CommandType,
    data::ticks::payload::{self, ParamBlock},
    data::ticks::value::Value,
    data::{ParserResult, Span},
};
use nom::{
    combinator::{flat_map, map, peek, rest},
    multi::length_value,
    number::complete::{le_u16, le_u32, le_u8},
    sequence::tuple,
};

#[derive(Debug, Clone)]
pub enum CommandData {
    Empty,
    Pbgid(u32),
    SourcedPbgid(u32, u16),
    Sourced(u16),
    SourcedIndex(u16, u32),
    Unknown,
}

impl CommandData {
    pub fn parse_empty(input: Span) -> ParserResult<CommandData> {
        map(rest, |_| CommandData::Empty)(input)
    }

    /// Header, source, then a parameter block whose data is usually a single pbgid
    /// value — anything else the block might carry (target position, orientation, ...)
    /// is intentionally left unread for now. If the block's first value isn't a pbgid,
    /// this decodes as `Unknown` rather than a guessed-at pbgid — see
    /// [`Self::parse_sourced_pbgid`] on why (no occurrence of this has been observed
    /// for the specific types routed here, but the same graceful handling applies).
    pub fn parse_pbgid(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, _source, block)| match Value::try_pbgid(expect_block(block).data) {
                Some(pbgid) => CommandData::Pbgid(pbgid),
                None => CommandData::Unknown,
            },
        )(input)
    }

    /// Header, source, then a parameter block whose data is usually a single pbgid
    /// value. A small number of commands routed here carry a different, non-pbgid
    /// first value instead (observed for `CMD_BuildSquad` and `CMD_Upgrade`, always the
    /// same 17-byte blob, always in the same replay — see
    /// `crates/cohlib/tests/command_payload.rs`); those decode as `Unknown` rather than
    /// a guessed-at pbgid.
    pub fn parse_sourced_pbgid(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, source, block)| match Value::try_pbgid(expect_block(block).data) {
                Some(pbgid) => CommandData::SourcedPbgid(pbgid, source.legacy_identifier()),
                None => CommandData::Unknown,
            },
        )(input)
    }

    /// Header and source only; these commands never carry parameters.
    pub fn parse_sourced(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, source, block)| {
                if let Some(block) = block {
                    panic!(
                        "expected no parameters for a sourced-only command, found block kind 0x{:02x}",
                        block.kind
                    );
                }
                CommandData::Sourced(source.legacy_identifier())
            },
        )(input)
    }

    /// Header, source, then a parameter block (kind `0x05`) whose data is a single raw
    /// `u32` queue index — not a tagged `Value`, unlike most other block kinds.
    pub fn parse_sourced_index(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, source, block)| {
                let block = expect_block(block);
                if block.kind != 0x05 {
                    panic!(
                        "expected a queue-index parameter block (kind 0x05), found kind 0x{:02x}",
                        block.kind
                    );
                }
                let queue_index = payload::expect_u32(block.data);
                CommandData::SourcedIndex(source.legacy_identifier(), queue_index)
            },
        )(input)
    }

    pub fn parse_unknown(input: Span) -> ParserResult<CommandData> {
        map(rest, |_| CommandData::Unknown)(input)
    }

    pub fn parser_for_type(
        command_type: CommandType,
    ) -> impl FnMut(Span) -> ParserResult<CommandData> {
        match command_type {
            CommandType::PCMD_AIPlayer => Self::parse_empty,
            CommandType::PCMD_Ability
            | CommandType::PCMD_InstantUpgrade
            | CommandType::PCMD_TentativeUpgrade
            | CommandType::PCMD_PlaceAndConstructEntities => Self::parse_pbgid,
            CommandType::CMD_BuildSquad | CommandType::CMD_Ability | CommandType::CMD_Upgrade => {
                Self::parse_sourced_pbgid
            }
            CommandType::CMD_CancelConstruction => Self::parse_sourced,
            CommandType::CMD_CancelProduction => Self::parse_sourced_index,
            _ => Self::parse_unknown,
        }
    }
}

/// Unwraps a parsed parameter block, panicking if the command had none. Used by
/// decoders for commands known to always carry parameters.
fn expect_block(block: Option<ParamBlock>) -> ParamBlock {
    block.unwrap_or_else(|| panic!("expected a parameter block, found none"))
}

#[derive(Debug, Clone)]
pub struct Command {
    pub action_type: CommandType,
    pub player_id: u8,
    pub index: u32,
    pub data: CommandData,
}

impl Command {
    pub fn parse(input: Span) -> ParserResult<Command> {
        map(
            length_value(
                peek(le_u16),
                tuple((le_u16, flat_map(CommandType::parse, Self::parse_type))),
            ),
            |(_length, command)| command,
        )(input)
    }

    fn parse_type(action_type: CommandType) -> impl FnMut(Span) -> ParserResult<Command> {
        move |input: Span| {
            map(
                tuple((le_u8, le_u32, CommandData::parser_for_type(action_type))),
                |(player_id, index, data)| Command {
                    action_type,
                    player_id: player_id & 0b0111_1111, // bit mask to turn eg 0x87 into 0x7
                    index,
                    data,
                },
            )(input)
        }
    }
}
