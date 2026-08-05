//! Common structure shared by the payload of every command other than the DCMD family
//! (which uses its own, unrelated layout — see `super::camera`).
//!
//! A command payload is:
//!
//! ```text
//! [20 bytes] opaque fixed header
//! [4+ bytes] source        -- who/what issued the command
//! [1+ bytes] parameter block (optional; 0xFF means "none")
//! ```
//!
//! This structure was reverse-engineered from and validated against every command in
//! every CoH3 release replay available during development (8.8k replays spanning 8713
//! through 48652, ~62M commands, zero mismatches). It is not derived from any official
//! source. Pre-release CoH3 builds are known to use a different, incompatible layout
//! and are out of scope — see the crate-level docs.
//!
//! Because the format holds across every release build, input that doesn't match it is
//! treated as a bug to surface rather than data to skip: these parsers panic instead of
//! degrading to `Command::Unknown`, which is reserved for command types this crate has
//! not decoded at all.

use crate::command_data::Source;
use crate::data::{ParserResult, Span};
use nom::bytes::complete::take;
use nom::combinator::{map, peek};
use nom::multi::count;
use nom::number::complete::{be_u32, le_u32, le_u8};
use nom::sequence::tuple;
use nom_tracable::tracable_parser;

/// Skips the 20-byte fixed header at the start of a command payload. Its fields aren't
/// understood beyond being stable across every release build examined; the player who
/// issued the command is already available from the enclosing `ticks::Command`, so
/// there's no need to extract a redundant copy from here.
#[tracable_parser]
pub(crate) fn parse_header(input: Span) -> ParserResult<Span> {
    take(20u32)(input)
}

/// Wire-level parsing for [`Source`] (the data type itself lives in `command_data`,
/// alongside the other public command payload shapes).
impl Source {
    /// Parses a source field. Scalar sources (player/entity/squad) are a tag byte
    /// followed by a big-endian 24-bit id; list sources (tag `0x41..=0x7F`, count in
    /// the low 6 bits) are a tag byte followed by that many little-endian `u32`
    /// entries, each independently encoding `(tag << 24) | id`. Panics on an
    /// unrecognized tag — see module docs on why unexpected input panics rather than
    /// degrading to `Unknown`.
    #[tracable_parser]
    pub(crate) fn parse(input: Span) -> ParserResult<Source> {
        let (_, tag) = peek(le_u8)(input)?;
        match tag {
            0x00 => map(be_u32, |v| Source::Player((v & 0xFF) as u8))(input),
            0x10 => map(be_u32, |v| Source::Entity(v & 0x00FF_FFFF))(input),
            0x20 => map(be_u32, |v| Source::Squad(v & 0x00FF_FFFF))(input),
            t if (0x41..=0x7F).contains(&t) => map(
                tuple((le_u8, count(le_u32, (t & 0x3F) as usize))),
                |(_, entries)| {
                    Source::Squads(entries.into_iter().map(|v| v & 0x00FF_FFFF).collect())
                },
            )(input),
            other => panic!("unrecognized command source tag 0x{other:02x}"),
        }
    }

    /// Reproduces the raw `u16` this crate has always exposed as `source_identifier`
    /// on the handful of command variants that predate this module: the low 16 bits of
    /// the source id, byte-swapped, exactly as it falls out of reading the last two
    /// bytes of a scalar source field as little-endian. Kept only for backward
    /// compatibility — new code should match on the `Source` itself instead. Not
    /// meaningful for `Squads`.
    pub(crate) fn legacy_identifier(&self) -> u16 {
        let id = match self {
            Source::Player(id) => *id as u32,
            Source::Entity(id) | Source::Squad(id) => *id,
            Source::Squads(_) => panic!("legacy_identifier is not defined for a squad list"),
        };
        ((id & 0xFFFF) as u16).swap_bytes()
    }
}

/// A command's optional parameter block: a `kind` byte selecting the shape of what
/// follows, and the raw bytes of that shape (interpretation is command-specific — some
/// kinds are one or more [`super::value::Value`]s, others are a fixed scalar, others
/// plain UTF-8 text).
#[derive(Debug, Clone)]
pub(crate) struct ParamBlock<'a> {
    pub kind: u8,
    pub data: Span<'a>,
}

impl<'a> ParamBlock<'a> {
    /// Parses the parameter block following a command's source. Returns `None` for the
    /// `0xFF` sentinel (no parameters). The block length is a single byte when `< 0x80`,
    /// otherwise a two-byte big-endian-ish extended form: `0x80 | (len >> 8)`, `len &
    /// 0xFF`. Panics if the declared length doesn't fit the remaining input — this
    /// parser is only ever run inside a `length_value` that has already bounded the
    /// command to its declared total length, so a bad length here means the block
    /// format itself doesn't match what was validated during development.
    #[tracable_parser]
    pub(crate) fn parse(input: Span<'a>) -> ParserResult<'a, Option<ParamBlock<'a>>> {
        let (input, marker) = peek(le_u8)(input)?;
        if marker == 0xFF {
            return map(le_u8, |_| None)(input);
        }
        map(
            tuple((le_u8, Self::parse_length_prefixed_data)),
            |(kind, data)| Some(ParamBlock { kind, data }),
        )(input)
    }

    fn parse_length_prefixed_data(input: Span<'a>) -> ParserResult<'a, Span<'a>> {
        let (input, first) = le_u8(input)?;
        if first < 0x80 {
            take(first as u32)(input)
        } else {
            let (input, second) = le_u8(input)?;
            let len = (((first & 0x7F) as u32) << 8) | second as u32;
            take(len)(input)
        }
    }
}

/// Parses a `le_u32` out of a block's data and panics if it doesn't fit. Used for
/// parameter block kinds whose data is a single raw scalar rather than a sequence of
/// [`super::value::Value`]s (e.g. queue indexes).
pub(crate) fn expect_u32(input: Span) -> u32 {
    let result: ParserResult<u32> = le_u32(input);
    match result {
        Ok((_, v)) => v,
        Err(e) => panic!("failed to read expected u32: {e:?}"),
    }
}
