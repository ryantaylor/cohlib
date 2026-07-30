//! Tagged values found inside a command's [`super::payload::ParamBlock`] data.
//!
//! Several parameter block kinds encode their data as one or more of these
//! `tag + value` pairs back to back (e.g. a pbgid followed by a target position).
//! Others (queue indexes, JSON text) use a fixed or opaque layout instead and are
//! read directly by the command that needs them, bypassing this type.

use crate::data::{ParserResult, Span};
use nom::combinator::{map, peek};
use nom::number::complete::{le_f32, le_u32, le_u8};
use nom::sequence::tuple;
use nom_tracable::tracable_parser;

/// The targeting-relevant fields ([`Value`] tags `0x02`–`0x05`) found at the front of a
/// parameter block's data, collected by [`Value::parse_targets`].
#[derive(Debug, Default, Copy, Clone)]
pub(crate) struct TargetValues {
    pub position: Option<[f32; 3]>,
    pub facing: Option<f32>,
    pub orientation: Option<[f32; 4]>,
    pub entity: Option<u32>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum Value {
    Pbgid(u32),
    Position([f32; 3]),
    Facing(f32),
    EntityRef(u32),
    Orientation([f32; 4]),
}

impl Value {
    /// Parses a single tagged value. Panics on an unrecognized tag: every CoH3 release
    /// build examined only ever produces the five tags handled here, so an unknown tag
    /// indicates a wire format this crate doesn't understand yet rather than data that
    /// can be safely skipped.
    #[tracable_parser]
    pub(crate) fn parse(input: Span) -> ParserResult<Value> {
        let (input, tag) = le_u8(input)?;
        match tag {
            0x01 => map(le_u32, Value::Pbgid)(input),
            0x02 => map(tuple((le_f32, le_f32, le_f32)), |(x, y, z)| {
                Value::Position([x, y, z])
            })(input),
            0x03 => map(le_f32, Value::Facing)(input),
            0x04 => map(le_u32, Value::EntityRef)(input),
            0x05 => map(tuple((le_f32, le_f32, le_f32, le_f32)), |(a, b, c, d)| {
                Value::Orientation([a, b, c, d])
            })(input),
            other => panic!("unrecognized command value tag 0x{other:02x}"),
        }
    }

    /// Parses a single value and returns the pbgid if it is one. Returns `None` if the
    /// value is a different, recognized shape — e.g. some rare `CMD_BuildSquad`
    /// occurrences carry something other than a pbgid as their first value, a real
    /// (if not yet understood) variant rather than malformed data. Still panics if the
    /// bytes don't parse as a value at all, since that indicates the block doesn't
    /// match the validated wire format.
    pub(crate) fn try_pbgid(input: Span) -> Option<u32> {
        match Self::parse(input) {
            Ok((_, Value::Pbgid(pbgid))) => Some(pbgid),
            Ok(_) => None,
            Err(e) => panic!("failed to read expected value: {e:?}"),
        }
    }

    /// Greedily parses a chain of targeting-relevant values (position, facing, entity
    /// reference, orientation) from the front of `input`, stopping at the first tag
    /// that isn't one of those four. Movement commands in particular carry additional
    /// trailing bytes after this chain that aren't `Value`s at all and aren't
    /// understood yet (observed but not decoded — see
    /// `crates/cohlib/tests/command_payload.rs`); stopping cleanly at the first
    /// unrecognized byte and leaving the rest unread is the intended behavior here, not
    /// a failure — unlike [`Self::parse`], this never panics.
    pub(crate) fn parse_targets(mut input: Span) -> ParserResult<TargetValues> {
        let mut values = TargetValues::default();
        loop {
            let peeked: ParserResult<u8> = peek(le_u8)(input);
            let Ok((_, tag)) = peeked else {
                break;
            };
            if !matches!(tag, 0x02..=0x05) {
                break;
            }
            let (rest, value) = Self::parse(input)?;
            match value {
                Value::Position(p) => values.position = Some(p),
                Value::Facing(f) => values.facing = Some(f),
                Value::EntityRef(e) => values.entity = Some(e),
                Value::Orientation(o) => values.orientation = Some(o),
                Value::Pbgid(_) => unreachable!("pbgid tag is filtered out above"),
            }
            input = rest;
        }
        Ok((input, values))
    }
}
