//! Tagged values found inside a command's [`super::payload::ParamBlock`] data.
//!
//! Several parameter block kinds encode their data as a [`Blueprint`] reference
//! followed by zero or more `tag + value` pairs (e.g. an ability blueprint followed by
//! a target position). Others (queue indexes, JSON text) use a fixed or opaque layout
//! instead and are read directly by the command that needs them, bypassing these types.
//!
//! The two tag namespaces are positional and independent: the leading blueprint
//! reference uses its own tags (`0x01`/`0x02`), and the [`Value`] chain that follows
//! uses `0x02`–`0x05`. The overlap on `0x02` is real — it means "mod-scoped blueprint"
//! in the leading position and "position" in the value chain.

use crate::data::{ParserResult, Span};
use nom::bytes::complete::take;
use nom::combinator::{map, peek};
use nom::number::complete::{le_f32, le_u32, le_u8};
use nom::sequence::tuple;
use nom_tracable::tracable_parser;
use uuid::Uuid;

/// A reference to a game blueprint (a squad, entity, upgrade or ability definition),
/// found at the front of every parameter block kind that identifies "what" a command
/// acts on.
///
/// Base-game blueprints are referenced by a globally unique `pbgid` alone. Blueprints
/// added by a mod are referenced by the UUID of the content pack that defines them plus
/// an id scoped to that pack, so the same `pbgid` can mean different things under
/// different mods.
#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) struct Blueprint {
    pub pbgid: u32,
    pub mod_uuid: Option<Uuid>,
}

impl Blueprint {
    /// Parses a blueprint reference: tag `0x01` followed by a `u32` pbgid for base-game
    /// content, or tag `0x02` followed by a 16-byte content pack GUID and a `u32` id
    /// scoped to that pack. Panics on any other tag, which would mean the block doesn't
    /// begin with a blueprint reference at all and so isn't the shape this parser was
    /// dispatched for.
    #[tracable_parser]
    pub(crate) fn parse(input: Span) -> ParserResult<Blueprint> {
        let (input, tag) = le_u8(input)?;
        match tag {
            0x01 => map(le_u32, |pbgid| Blueprint {
                pbgid,
                mod_uuid: None,
            })(input),
            0x02 => map(
                tuple((take(16u32), le_u32)),
                |(uuid, pbgid): (Span, u32)| Blueprint {
                    pbgid,
                    // GUIDs are stored in the engine's native Windows layout: the first
                    // three fields little-endian, the last eight bytes in order.
                    mod_uuid: Some(
                        Uuid::from_slice_le(uuid.fragment()).expect("16 bytes taken above"),
                    ),
                },
            )(input),
            other => panic!("unrecognized blueprint reference tag 0x{other:02x}"),
        }
    }
}

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
    Position([f32; 3]),
    Facing(f32),
    EntityRef(u32),
    Orientation([f32; 4]),
}

impl Value {
    /// Parses a single tagged value. Panics on an unrecognized tag: every CoH3 release
    /// build examined only ever produces the four tags handled here, so an unknown tag
    /// indicates a wire format this crate doesn't understand yet rather than data that
    /// can be safely skipped.
    #[tracable_parser]
    pub(crate) fn parse(input: Span) -> ParserResult<Value> {
        let (input, tag) = le_u8(input)?;
        match tag {
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

    /// Greedily parses a chain of targeting-relevant values (position, facing, entity
    /// reference, orientation) from the front of `input`, stopping at the first tag
    /// that isn't one of those four. Movement commands in particular carry additional
    /// trailing bytes after this chain that aren't `Value`s at all and aren't understood
    /// yet; stopping cleanly at the first unrecognized byte and leaving the rest unread
    /// is the intended behavior here, not a failure — unlike [`Self::parse`], this never
    /// panics. The command itself always decodes either way, so this only ever leaves
    /// extra detail unread, never a whole command.
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
            }
            input = rest;
        }
        Ok((input, values))
    }
}
