use crate::data::parser::parse_utf8_variable;
use crate::data::{ParserResult, Span};
use nom::bytes::complete::take;
use nom::combinator::{cut, map, map_parser, rest};
use nom::multi::length_data;
use nom::number::complete::{le_i64, le_u32, le_u64, le_u8};
use nom::sequence::tuple;
use nom_tracable::tracable_parser;

/// One inventory item record.
#[derive(Debug, Clone)]
pub enum Item {
    Player(PlayerItem),
    Cpu(CpuItem),
}

/// A human player's item: Relic's inventory record for it, as its API returns it.
#[derive(Debug, Clone)]
pub struct PlayerItem {
    pub instance_id: u64,
    pub unknown_1: u64,
    pub definition_id: u64,
    pub profile_id: u64,
    pub durability: u64,
    pub durability_type: u8,
    pub metadata: String,
    pub acquired_at: u64,
    pub location_id: u64,
    pub unknown_2: i64,
    pub unknown_3: i64,
    pub unknown_4: i64,
    /// Payload bytes past the known fields; empty in every replay seen so far.
    pub extra: Vec<u8>,
}

/// An AI player's item.
#[derive(Debug, Clone)]
pub struct CpuItem {
    pub id: u32,
}

impl Item {
    /// `0a 01 <kind> | u64 instance | 0a 01 <kind> | u64 definition | u16 payload length + 4`, the
    /// u32-length payload, then 4 zero bytes. The prefix only repeats the payload's ids.
    #[tracable_parser]
    pub fn parse_player_item(input: Span) -> ParserResult<Item> {
        cut(map(
            tuple((
                take(24u32),
                map_parser(length_data(le_u32), Self::parse_player_payload),
                take(4u32),
            )),
            |(_, item, _)| Item::Player(item),
        ))(input)
    }

    #[tracable_parser]
    fn parse_player_payload(input: Span) -> ParserResult<PlayerItem> {
        map(
            tuple((
                tuple((le_u64, le_u64, le_u64, le_u64, le_u64, le_u8)),
                parse_utf8_variable(le_u32),
                tuple((le_u64, le_u64, le_i64, le_i64, le_i64)),
                rest,
            )),
            |(
                (instance_id, unknown_1, definition_id, profile_id, durability, durability_type),
                (_, metadata),
                (acquired_at, location_id, unknown_2, unknown_3, unknown_4),
                extra,
            ): (_, _, _, Span)| PlayerItem {
                instance_id,
                unknown_1,
                definition_id,
                profile_id,
                durability,
                durability_type,
                metadata,
                acquired_at,
                location_id,
                unknown_2,
                unknown_3,
                unknown_4,
                extra: extra.to_vec(),
            },
        )(input)
    }

    /// `07 02 04 01 | u32 id | 4 zero bytes`.
    #[tracable_parser]
    pub fn parse_cpu_item(input: Span) -> ParserResult<Item> {
        cut(map(
            tuple((take(4u32), le_u32, take(4u32))),
            |(_, id, _)| Item::Cpu(CpuItem { id }),
        ))(input)
    }
}
