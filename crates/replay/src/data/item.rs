use crate::data::{ParserResult, Span};
use nom::bytes::complete::take;
use nom::combinator::{cut, map, recognize};
use nom::multi::length_data;
use nom::number::complete::le_u32;
use nom::sequence::tuple;
use nom_tracable::tracable_parser;

/// One inventory item record, kept byte-for-byte as it appears in the file.
#[derive(Debug, Clone)]
pub struct Item {
    pub raw: Vec<u8>,
}

impl Item {
    /// A human player's item: a 24-byte tagged prefix, a length-prefixed payload, then 4 bytes.
    #[tracable_parser]
    pub fn parse_player_item(input: Span) -> ParserResult<Item> {
        cut(map(
            recognize(tuple((take(24u32), length_data(le_u32), take(4u32)))),
            |raw: Span| Item { raw: raw.to_vec() },
        ))(input)
    }

    /// An AI player's item: 12 bytes in a format of its own.
    #[tracable_parser]
    pub fn parse_cpu_item(input: Span) -> ParserResult<Item> {
        cut(map(recognize(take(12u32)), |raw: Span| Item {
            raw: raw.to_vec(),
        }))(input)
    }

    /// The item definition id, the second tagged u64 of a human item's prefix
    /// (`0a 01 <kind> | u64 | 0a 01 <kind> | u64 | u16`). `None` for any other layout.
    pub fn definition_id(&self) -> Option<u64> {
        let raw = &self.raw;
        if raw.len() < 24 || raw[0..2] != [0x0a, 0x01] || raw[11..13] != [0x0a, 0x01] {
            return None;
        }

        Some(u64::from_le_bytes(raw[14..22].try_into().unwrap()))
    }
}
