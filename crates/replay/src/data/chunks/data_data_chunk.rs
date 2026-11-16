use crate::data::chunks::{Chunk, Chunk::DataData, Header, TrashDataChunk};
use crate::data::parser::parse_utf8_variable;
use crate::data::{ParserResult, Player, Span};
use nom::bytes::complete::{tag, take, take_while};
use nom::character::{is_digit, is_hex_digit};
use nom::combinator::{cut, map, map_parser, peek, verify};
use nom::multi::{length_count, length_data, length_value, many_m_n, many_till};
use nom::number::complete::{le_u32, le_u64};
use nom::sequence::{separated_pair, tuple};
use nom_tracable::tracable_parser;
use uuid::Uuid;

/// One lobby setting. Lobbies list them in order and may repeat a name.
#[derive(Debug, Clone)]
pub struct LobbyOption {
    pub name: String,
    pub value: u32,
}

impl LobbyOption {
    #[tracable_parser]
    pub fn parse_option(input: Span) -> ParserResult<LobbyOption> {
        map(
            tuple((parse_utf8_variable(le_u32), le_u32)),
            |((_, name), value)| LobbyOption { name, value },
        )(input)
    }
}

#[derive(Debug)]
pub struct DataDataChunk {
    _header: Header,
    _opponent_type: u32,
    pub players: Vec<Player>,
    pub skirmish: bool,
    pub matchhistory_id: u64,
    pub options: Vec<LobbyOption>,
    pub mod_uuid: Uuid,
    /// The number after the `:` in the mod string; meaning unconfirmed.
    pub mod_version: Option<u64>,
}

impl DataDataChunk {
    #[tracable_parser]
    pub fn parse(input: Span, header: Header) -> ParserResult<Chunk> {
        if header.version == 1 {
            return TrashDataChunk::parse(input, header);
        }

        cut(map_parser(
            take(header.length),
            map(
                tuple((
                    Self::parse_opponent_type,
                    take(6u32),
                    Self::parse_players(header.clone()),
                    length_data(le_u32),
                    Self::parse_skirmish_flag,
                    le_u64,
                    take(24u32),
                    Self::parse_options,
                    take(12u32),
                    Self::parse_mod_info,
                )),
                |(
                    opponent_type,
                    _,
                    players,
                    _,
                    skirmish,
                    matchhistory_id,
                    _,
                    options,
                    _,
                    (mod_uuid, mod_version),
                )| {
                    DataData(DataDataChunk {
                        _header: header.clone(),
                        _opponent_type: opponent_type,
                        players,
                        skirmish,
                        matchhistory_id,
                        options,
                        mod_uuid,
                        mod_version,
                    })
                },
            ),
        ))(input)
    }

    #[tracable_parser]
    fn parse_opponent_type(input: Span) -> ParserResult<u32> {
        le_u32(input)
    }

    fn parse_players(header: Header) -> impl FnMut(Span) -> ParserResult<Vec<Player>> {
        move |input: Span| length_count(le_u32, Player::parse_player(header.clone()))(input)
    }

    #[tracable_parser]
    fn parse_options(input: Span) -> ParserResult<Vec<LobbyOption>> {
        map(
            many_till(
                LobbyOption::parse_option,
                map_parser(
                    peek(take(12u32)),
                    many_m_n(3, 3, verify(le_u32, |n| *n == 0 || *n == 1)),
                ),
            ),
            |(options, _)| options,
        )(input)
    }

    #[tracable_parser]
    fn parse_skirmish_flag(input: Span) -> ParserResult<bool> {
        map(parse_utf8_variable(le_u32), |(_, id)| !id.is_empty())(input)
    }

    #[tracable_parser]
    fn parse_mod_info(input: Span) -> ParserResult<(Uuid, Option<u64>)> {
        length_value(
            le_u32,
            map(
                separated_pair(take_while(is_hex_digit), tag(":"), take_while(is_digit)),
                |(mod_uuid, mod_version): (Span, Span)| {
                    (
                        Uuid::try_parse_ascii(&mod_uuid).unwrap(),
                        std::str::from_utf8(mod_version.fragment())
                            .ok()
                            .and_then(|digits| digits.parse().ok()),
                    )
                },
            ),
        )(input)
    }
}
