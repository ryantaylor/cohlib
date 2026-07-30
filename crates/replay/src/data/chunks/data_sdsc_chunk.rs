use crate::data::chunks::{Chunk, Chunk::DataSdsc, Header};
use crate::data::parser::{parse_utf16_variable, parse_utf8_variable, verify_le_u32};
use crate::data::{ParserResult, Span};
use nom::branch::alt;
use nom::bytes::complete::take;
use nom::combinator::{cond, cut, map, map_parser, success};
use nom::multi::length_count;
use nom::number::complete::{le_f32, le_u32, le_u8};
use nom::sequence::tuple;
use nom::Slice;
use nom_tracable::tracable_parser;

/// A labeled point on the map, as found in the tail of the DATA/SDSC chunk. Used for
/// territory points, victory points, and player starting positions alike — see
/// [`DataSdscChunk::parse_tail_lists`].
#[derive(Debug, Clone)]
pub struct MapPointData {
    pub x: f32,
    pub y: f32,
    pub icon: String,
    pub tags: String,
    pub owner: u32,
    pub flag: u8,
}

#[derive(Debug)]
pub struct DataSdscChunk {
    _header: Header,
    pub map_file: String,
    pub map_name: String,
    pub map_description: String,
    pub territory_points: Vec<MapPointData>,
    pub victory_points: Vec<MapPointData>,
    pub starting_positions: Vec<MapPointData>,
}

impl DataSdscChunk {
    #[tracable_parser]
    pub fn parse(input: Span, header: Header) -> ParserResult<Chunk> {
        cut(map_parser(
            take(header.length),
            map(
                tuple((
                    take(121u32),
                    cond(header.version > 3026, take(8u32)),
                    Self::parse_map_file,
                    Self::parse_map_identifier,
                    alt((verify_le_u32(0u32), success(0u32))),
                    Self::parse_map_identifier,
                    Self::parse_tail_lists,
                )),
                |(
                    _,
                    _,
                    map_file,
                    map_name,
                    _,
                    map_description,
                    (territory_points, victory_points, starting_positions),
                )| {
                    DataSdsc(DataSdscChunk {
                        _header: header.clone(),
                        map_name,
                        map_file,
                        map_description,
                        territory_points,
                        victory_points,
                        starting_positions,
                    })
                },
            ),
        ))(input)
    }

    fn parse_map_file(input: Span) -> ParserResult<String> {
        let (input, (_, section_resources)) = parse_utf8_variable(le_u32)(input)?;
        Ok((input, section_resources))
    }

    fn parse_map_identifier(input: Span) -> ParserResult<String> {
        let (input, (_, section_resources)) = parse_utf16_variable(le_u32)(input)?;
        Ok((input, section_resources))
    }

    fn parse_map_point(input: Span) -> ParserResult<MapPointData> {
        map(
            tuple((
                le_f32,
                le_f32,
                parse_utf8_variable(le_u32),
                parse_utf8_variable(le_u32),
                le_u32,
                le_u8,
            )),
            |(x, y, (_, icon), (_, tags), owner, flag)| MapPointData {
                x,
                y,
                icon,
                tags,
                owner,
                flag,
            },
        )(input)
    }

    fn parse_map_point_list(input: Span) -> ParserResult<Vec<MapPointData>> {
        length_count(le_u32, Self::parse_map_point)(input)
    }

    /// The DATA/SDSC chunk ends with three lists that share one record shape: territory
    /// points, victory points, and player starting positions. Everything between the map
    /// description and this tail (tileset, weather, splat strings, and more) differs across
    /// map/patch versions and isn't otherwise parsed here, so rather than parse through it,
    /// this scans for the offset at which three point-lists exactly consume the remainder of
    /// the chunk, identifying the tail by its content instead of a fixed position.
    ///
    /// Replays recorded before the starting-position table existed (DATA/SDSC version <=
    /// 3026) have no such tail; in that case all three lists come back empty.
    fn parse_tail_lists(
        input: Span,
    ) -> ParserResult<(Vec<MapPointData>, Vec<MapPointData>, Vec<MapPointData>)> {
        let end = input.fragment().len();

        for start in 0..end {
            let candidate = input.slice(start..);
            let attempt = tuple((
                Self::parse_map_point_list,
                Self::parse_map_point_list,
                Self::parse_map_point_list,
            ))(candidate);

            if let Ok((remaining, (territory_points, victory_points, starting_positions))) = attempt
            {
                let looks_like_starting_positions = !starting_positions.is_empty()
                    && starting_positions
                        .iter()
                        .all(|point| point.tags.is_empty() && point.flag == 1 && point.owner < 32);

                if remaining.fragment().is_empty() && looks_like_starting_positions {
                    return Ok((
                        input.slice(end..),
                        (territory_points, victory_points, starting_positions),
                    ));
                }
            }
        }

        Ok((input.slice(end..), (Vec::new(), Vec::new(), Vec::new())))
    }
}
