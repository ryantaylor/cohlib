use crate::{
    command_data::Source,
    command_type::CommandType,
    data::ticks::payload::{self, ParamBlock},
    data::ticks::value::{TargetValues, Value},
    data::{ParserResult, Span},
};
use nom::{
    bytes::complete::{tag, take},
    combinator::{flat_map, map, peek, rest},
    multi::{length_count, length_data, length_value},
    number::complete::{be_u32, le_f32, le_u16, le_u32, le_u8},
    sequence::{preceded, tuple},
};
use nom_tracable::tracable_parser;

/// The three positions and spawned entity ids parsed by [`CommandData::parse_construction`].
type ConstructionFields = (([f32; 3], [f32; 3], [f32; 3]), Vec<u32>);

#[derive(Debug, Clone)]
pub enum CommandData {
    Empty,
    Pbgid(u32),
    SourcedPbgid(u32, u16),
    Sourced(u16),
    SourcedIndex(u16, u32),
    /// Header and source only, with the full `Source` preserved (unlike `Sourced`,
    /// which keeps only the legacy truncated identifier). Used for commands whose
    /// source can legitimately be a multi-squad selection.
    SourceOnly(Source),
    /// Header, source and a pbgid, with the full `Source` preserved (unlike
    /// `SourcedPbgid`, which keeps only the legacy truncated identifier).
    SourcePbgid(Source, u32),
    /// Header, source, and zero or more targeting values (position, facing,
    /// orientation, target entity).
    Targeted(Source, TargetValues),
    /// A pbgid and zero or more targeting values, with no source (used by
    /// `PCMD_Ability`, whose source is always the issuing player).
    PbgidTargeted(u32, TargetValues),
    /// A pbgid, legacy source identifier, and zero or more targeting values (used by
    /// `CMD_Ability`).
    SourcedPbgidTargeted(u32, u16, TargetValues),
    /// `SCMD_Ability`'s payload: a source, an optional pbgid (absent when this command
    /// continues/updates an already-active ability's target rather than starting a new
    /// one), and zero or more targeting values.
    Ability(Source, Option<u32>, TargetValues),
    /// `PCMD_PlaceAndConstructEntities`'s payload: a pbgid, three raw (non-tag-prefixed)
    /// positions, and the spawned entity ids.
    Construction(u32, [f32; 3], [f32; 3], [f32; 3], Vec<u32>),
    /// `DCMD_CameraTrack`/`DCMD_COUNT`'s payload: everything after the 5-byte `0xFF`
    /// marker, captured verbatim. Unlike every other command type, these have no
    /// header or source field at all — see [`CommandData::parse_camera_track`].
    CameraTrack(Vec<u8>),
    /// `PCMD_AIPlayer_ResourceBonus`'s payload: a count-prefixed map from
    /// length-prefixed resource name to `f32` multiplier.
    ResourceBonus(Vec<(String, f32)>),
    /// `PCMD_BroadcastMessage`'s payload: 4 not-yet-understood bytes, then a
    /// length-prefixed UTF-8 JSON message.
    BroadcastMessage(String),
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

    /// Header and source only, preserving the full `Source` (see
    /// [`CommandData::SourceOnly`]) rather than truncating it to the legacy `u16` the
    /// way [`Self::parse_sourced`] does. These commands are usually parameter-less, but
    /// a small fraction of `SCMD_Retreat` occurrences carry an unexpected parameter
    /// block instead — a real, recognized-but-not-yet-decoded variant rather than
    /// malformed data, so it decodes as `Unknown` rather than panicking. See
    /// `crates/cohlib/tests/command_payload.rs`.
    pub fn parse_squads(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, source, block)| match block {
                None => CommandData::SourceOnly(source),
                Some(_) => CommandData::Unknown,
            },
        )(input)
    }

    /// Header, source, then a parameter block whose data is usually a single pbgid
    /// value — like [`Self::parse_sourced_pbgid`], but preserving the full `Source`
    /// (see [`CommandData::SourcePbgid`]) since these commands' source can legitimately
    /// be a multi-squad selection. See [`Self::parse_pbgid`] on the `Unknown` fallback
    /// for a non-pbgid first value (observed here for `SCMD_ReinforceUnit`, same known
    /// exception as elsewhere).
    pub fn parse_source_pbgid(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, source, block)| match Value::try_pbgid(expect_block(block).data) {
                Some(pbgid) => CommandData::SourcePbgid(source, pbgid),
                None => CommandData::Unknown,
            },
        )(input)
    }

    /// Header, source, then an optional parameter block containing zero or more
    /// targeting values (position, facing, orientation, entity reference). Block kinds
    /// `0x06` and `0x0F` have a fixed-size, not-yet-understood prefix (4 and 8 bytes
    /// respectively) before the value chain; every other kind these commands use
    /// (`0x01`, `0x03`, `0x1D`) has none. Validated against every occurrence of these
    /// block kinds in the corpus examined during development — see
    /// `Value::parse_targets` on why any further trailing bytes are intentionally left
    /// unread. No block at all (kind `0xFF`, or absent entirely — the latter seen only
    /// for `SCMD_Unload`) yields all-`None` targeting fields.
    pub fn parse_targeted(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, source, block)| {
                let targets = match block {
                    None => TargetValues::default(),
                    Some(block) => {
                        let skip: u32 = match block.kind {
                            0x06 => 4,
                            0x0F => 8,
                            _ => 0,
                        };
                        parse_targets(skip_bytes(block.data, skip))
                    }
                };
                CommandData::Targeted(source, targets)
            },
        )(input)
    }

    /// Header, source, then a pbgid-and-targets block (see `parse_pbgid_and_targets`).
    /// `PCMD_Ability`'s source is discarded (always the issuing player, already
    /// available elsewhere) to match [`Self::parse_pbgid`].
    pub fn parse_pbgid_targeted(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, _source, block)| match parse_pbgid_and_targets(&expect_block(block)) {
                Some((pbgid, targets)) => CommandData::PbgidTargeted(pbgid, targets),
                None => CommandData::Unknown,
            },
        )(input)
    }

    /// Header, source, then a pbgid-and-targets block (see `parse_pbgid_and_targets`),
    /// preserving the legacy source identifier like [`Self::parse_sourced_pbgid`] does.
    pub fn parse_sourced_pbgid_targeted(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, source, block)| match parse_pbgid_and_targets(&expect_block(block)) {
                Some((pbgid, targets)) => {
                    CommandData::SourcedPbgidTargeted(pbgid, source.legacy_identifier(), targets)
                }
                None => CommandData::Unknown,
            },
        )(input)
    }

    /// `SCMD_Ability`'s payload is one of two shapes: block kind `0x01` carries only
    /// targeting values with no pbgid at all (continuing/updating an already-active
    /// ability's target, as best understood); kinds `0x23`/`0x24`/`0x29` carry a pbgid
    /// (see `parse_pbgid_and_targets`). This is the only command type observed using
    /// both.
    pub fn parse_ability(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, source, block)| {
                let result = match block {
                    None => Some((None, TargetValues::default())),
                    Some(block) if block.kind == 0x01 => Some((None, parse_targets(block.data))),
                    Some(block) => {
                        parse_pbgid_and_targets(&block).map(|(pbgid, t)| (Some(pbgid), t))
                    }
                };
                match result {
                    Some((pbgid, targets)) => CommandData::Ability(source, pbgid, targets),
                    None => CommandData::Unknown,
                }
            },
        )(input)
    }

    /// Header, source, then a construction parameter block (kind `0x1A`): a pbgid,
    /// three raw (non-tag-prefixed) positions, 4 not-yet-understood bytes, then a
    /// count-prefixed list of spawned entity ids — the count is stored as a big-endian
    /// `u32` whose high three bytes are always zero, i.e. effectively a one-byte count.
    /// Validated against every occurrence in the corpus examined during development.
    /// Panics if the block isn't kind `0x1A` or doesn't fit this shape; gracefully
    /// decodes as `Unknown` (not a panic) if the leading value isn't a pbgid — the same
    /// known AI/CPU-issued anomaly documented on [`Self::parse_sourced_pbgid`] shows up
    /// here too.
    pub fn parse_construction(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, _source, block)| {
                let block = expect_block(block);
                if block.kind != 0x1A {
                    panic!(
                        "expected a construction parameter block (kind 0x1A), found kind 0x{:02x}",
                        block.kind
                    );
                }
                let Some((after_pbgid, pbgid)) = Value::try_pbgid_with_rest(block.data) else {
                    return CommandData::Unknown;
                };
                let positions = tuple((parse_raw_position, parse_raw_position, parse_raw_position));
                // 4 not-yet-understood bytes precede the count.
                let entities = preceded(take(4u32), length_count(be_u32, le_u32));
                let result: ParserResult<ConstructionFields> =
                    tuple((positions, entities))(after_pbgid);
                match result {
                    Ok((_, ((position, snapped, actual), entities))) => {
                        CommandData::Construction(pbgid, position, snapped, actual, entities)
                    }
                    Err(e) => panic!("failed to parse construction data: {e:?}"),
                }
            },
        )(input)
    }

    /// `DCMD_CameraTrack`/`DCMD_COUNT` payload: a fixed 5-byte `0xFF` marker — unlike
    /// every other command type, no header or source field precedes it — followed by
    /// not-yet-decoded telemetry bytes captured verbatim. Panics if the marker isn't
    /// present, since that would mean this payload isn't the validated DCMD shape at
    /// all.
    pub fn parse_camera_track(input: Span) -> ParserResult<CommandData> {
        map(
            preceded(tag([0xffu8, 0xff, 0xff, 0xff, 0xff].as_slice()), rest),
            |data: Span| CommandData::CameraTrack(data.fragment().to_vec()),
        )(input)
    }

    /// Header, source, then a parameter block whose data is a count-prefixed sequence
    /// of `(length-prefixed UTF-8 key, f32 value)` pairs — a resource name to
    /// multiplier map. Validated against every occurrence in the corpus examined during
    /// development.
    pub fn parse_resource_bonus(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, _source, block)| {
                let entry = tuple((
                    map(length_data(le_u32), |s: Span| {
                        String::from_utf8_lossy(s.fragment()).into_owned()
                    }),
                    le_f32,
                ));
                let result: ParserResult<Vec<(String, f32)>> =
                    length_count(le_u32, entry)(expect_block(block).data);
                match result {
                    Ok((_, entries)) => CommandData::ResourceBonus(entries),
                    Err(e) => panic!("failed to parse resource bonus data: {e:?}"),
                }
            },
        )(input)
    }

    /// Header, source, then a parameter block whose data is 4 not-yet-understood
    /// bytes followed by a length-prefixed UTF-8 JSON message. Validated against every
    /// occurrence in the corpus examined during development.
    pub fn parse_broadcast_message(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, _source, block)| {
                let result: ParserResult<Span> =
                    preceded(take(4u32), length_data(le_u32))(expect_block(block).data);
                match result {
                    Ok((_, json)) => CommandData::BroadcastMessage(
                        String::from_utf8_lossy(json.fragment()).into_owned(),
                    ),
                    Err(e) => panic!("failed to parse broadcast message: {e:?}"),
                }
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
            CommandType::PCMD_InstantUpgrade | CommandType::PCMD_TentativeUpgrade => {
                Self::parse_pbgid
            }
            CommandType::PCMD_PlaceAndConstructEntities => Self::parse_construction,
            CommandType::CMD_BuildSquad | CommandType::CMD_Upgrade => Self::parse_sourced_pbgid,
            CommandType::PCMD_Ability => Self::parse_pbgid_targeted,
            CommandType::CMD_Ability => Self::parse_sourced_pbgid_targeted,
            CommandType::SCMD_Ability => Self::parse_ability,
            CommandType::SCMD_StopAbility | CommandType::CMD_StopAbility => {
                Self::parse_source_pbgid
            }
            CommandType::CMD_CancelConstruction => Self::parse_sourced,
            CommandType::CMD_CancelProduction
            | CommandType::SCMD_CancelProduction
            | CommandType::PCMD_CancelProduction => Self::parse_sourced_index,
            CommandType::SCMD_Retreat
            | CommandType::SCMD_Stop
            | CommandType::PCMD_TentativeUpgradeRemoveAll
            | CommandType::SCMD_UnloadSquads
            | CommandType::CMD_UnloadSquads
            | CommandType::PCMD_Surrender => Self::parse_squads,
            CommandType::SCMD_Upgrade | CommandType::SCMD_ReinforceUnit => Self::parse_source_pbgid,
            CommandType::CMD_RallyPoint
            | CommandType::CMD_Move
            | CommandType::CMD_AttackFromHold
            | CommandType::SCMD_Move
            | CommandType::SCMD_Attack
            | CommandType::SCMD_Capture
            | CommandType::SCMD_AttackMove
            | CommandType::SCMD_Load
            | CommandType::SCMD_Unload
            | CommandType::SCMD_Face
            | CommandType::SCMD_CaptureTeamWeapon
            | CommandType::SCMD_PickUpSimItem
            | CommandType::SCMD_BuildStructure
            | CommandType::SCMD_Recrew
            | CommandType::PCMD_DetonateCharges => Self::parse_targeted,
            CommandType::DCMD_CameraTrack | CommandType::DCMD_COUNT => Self::parse_camera_track,
            CommandType::PCMD_AIPlayer_ResourceBonus => Self::parse_resource_bonus,
            CommandType::PCMD_BroadcastMessage => Self::parse_broadcast_message,
            _ => Self::parse_unknown,
        }
    }
}

/// Unwraps a parsed parameter block, panicking if the command had none. Used by
/// decoders for commands known to always carry parameters.
fn expect_block(block: Option<ParamBlock>) -> ParamBlock {
    block.unwrap_or_else(|| panic!("expected a parameter block, found none"))
}

/// Skips `n` bytes, panicking if the block doesn't have that many left — used for the
/// fixed-size, not-yet-understood prefixes some block kinds have before their value
/// chain.
fn skip_bytes(data: Span, n: u32) -> Span {
    let result: ParserResult<Span> = take(n)(data);
    match result {
        Ok((rest, _)) => rest,
        Err(e) => panic!("block too short to skip {n} bytes: {e:?}"),
    }
}

/// Parses a chain of targeting values. `Value::parse_targets` is itself infallible;
/// this just centralizes unwrapping its `ParserResult`.
fn parse_targets(data: Span) -> TargetValues {
    match Value::parse_targets(data) {
        Ok((_, targets)) => targets,
        Err(e) => panic!("failed to parse targeting values: {e:?}"),
    }
}

/// Parses a raw `[f32; 3]` position with no leading `Value` tag — used by construction
/// commands, whose positions aren't tagged the way `Value::Position` normally is.
fn parse_raw_position(input: Span) -> ParserResult<[f32; 3]> {
    map(tuple((le_f32, le_f32, le_f32)), |(x, y, z)| [x, y, z])(input)
}

/// Parses `[pbgid value][kind-specific skip][target value chain]`, the shape shared by
/// every "ability" style block kind (`0x23`, `0x24`, `0x29`): `0x23` has no prefix
/// after the pbgid, `0x24` has 6 not-yet-understood bytes, `0x29` has 1. Validated
/// against every occurrence of these block kinds in the corpus examined during
/// development. Returns `None` (rather than panicking) if the first value isn't a
/// pbgid — the same known AI/CPU-issued anomaly documented on
/// [`CommandData::parse_pbgid`] shows up here too.
fn parse_pbgid_and_targets(block: &ParamBlock) -> Option<(u32, TargetValues)> {
    let (after_pbgid, pbgid) = Value::try_pbgid_with_rest(block.data)?;
    let skip: u32 = match block.kind {
        0x24 => 6,
        0x29 => 1,
        _ => 0,
    };
    Some((pbgid, parse_targets(skip_bytes(after_pbgid, skip))))
}

#[derive(Debug, Clone)]
pub struct Command {
    pub action_type: CommandType,
    pub player_id: u8,
    pub index: u32,
    pub data: CommandData,
}

impl Command {
    #[tracable_parser]
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
