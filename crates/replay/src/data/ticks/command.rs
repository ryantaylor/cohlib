use crate::{
    command_data::Source,
    command_type::CommandType,
    data::ticks::payload::{self, ParamBlock},
    data::ticks::value::{Blueprint, TargetValues, Value},
    data::{ParserResult, Span},
};
use nom::{
    bytes::complete::take,
    combinator::{flat_map, map, peek, rest},
    multi::{length_count, length_data, length_value},
    number::complete::{be_u32, le_f32, le_i16, le_u16, le_u32, le_u8},
    sequence::{preceded, tuple},
};
use nom_tracable::tracable_parser;

/// The three positions and spawned entity ids parsed by [`CommandData::parse_construction`].
type ConstructionFields = (([f32; 3], [f32; 3], [f32; 3]), Vec<u32>);

/// The sequence, eye position and orientation fields parsed by
/// [`CommandData::parse_camera_track`].
type CameraTrackFields = (u32, i16, i16, i16, i16, i16, i16, i16);

#[derive(Debug, Clone)]
pub enum CommandData {
    Empty,
    Pbgid(Blueprint),
    SourcedPbgid(Blueprint, u16),
    /// Header and source only, with the full `Source` preserved (unlike `SourcedPbgid`)
    /// since `CMD_CancelConstruction`'s source can legitimately be a multi-squad
    /// selection.
    Sourced(Source),
    SourcedIndex(u16, u32),
    /// Header, source and a blueprint, with the full `Source` preserved (unlike
    /// `SourcedPbgid`, which keeps only the legacy truncated identifier).
    SourcePbgid(Source, Blueprint),
    /// Header, source, and zero or more targeting values (position, facing,
    /// orientation, target entity).
    Targeted(Source, TargetValues),
    /// A blueprint and zero or more targeting values, with no source (used by
    /// `PCMD_Ability`, whose source is always the issuing player).
    PbgidTargeted(Blueprint, TargetValues),
    /// `CMD_Ability`'s payload: a legacy source identifier, an optional blueprint
    /// (absent when this command continues/updates an already-active ability's target
    /// rather than starting a new one, the same dual shape as `SCMD_Ability` — see
    /// `Ability`), and zero or more targeting values.
    SourcedPbgidTargeted(Option<Blueprint>, u16, TargetValues),
    /// `SCMD_Ability`'s payload: a source, an optional blueprint (absent when this
    /// command continues/updates an already-active ability's target rather than starting
    /// a new one), and zero or more targeting values.
    Ability(Source, Option<Blueprint>, TargetValues),
    /// `PCMD_PlaceAndConstructEntities`'s payload: a blueprint, three raw
    /// (non-tag-prefixed) positions, and the spawned entity ids.
    Construction(Blueprint, [f32; 3], [f32; 3], [f32; 3], Vec<u32>),
    /// `DCMD_CameraTrack`'s payload (parameter block kind `0x2f`): a sample sequence,
    /// the camera's eye position, and its orientation — still in their raw fixed-point
    /// wire encoding, scaled (but not otherwise reinterpreted) into
    /// [`crate::command_data::CameraTrack`] further up.
    CameraTrack {
        sequence: u32,
        position: [i16; 3],
        orientation: [i16; 4],
    },
    /// `DCMD_COUNT`'s payload (parameter block kind `0x30`): a sample sequence and
    /// three not-fully-understood entity counters — see [`crate::command_data::CameraCounts`].
    CameraCounts {
        sequence: u32,
        counts: [u16; 3],
    },
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

    /// Header, source, then a parameter block whose data begins with a blueprint
    /// reference — anything else the block might carry (target position, orientation,
    /// ...) is intentionally left unread for now.
    pub fn parse_pbgid(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, _source, block)| CommandData::Pbgid(expect_blueprint(block)),
        )(input)
    }

    /// Header, source, then a parameter block whose data begins with a blueprint
    /// reference, keeping the legacy truncated source identifier.
    pub fn parse_sourced_pbgid(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, source, block)| {
                CommandData::SourcedPbgid(expect_blueprint(block), source.legacy_identifier())
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
                CommandData::Sourced(source)
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

    /// Header, source, then a parameter block whose data begins with a blueprint
    /// reference — like [`Self::parse_sourced_pbgid`], but preserving the full `Source`
    /// (see [`CommandData::SourcePbgid`]) since these commands' source can legitimately
    /// be a multi-squad selection.
    pub fn parse_source_pbgid(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, source, block)| CommandData::SourcePbgid(source, expect_blueprint(block)),
        )(input)
    }

    /// Header, source, then an optional parameter block containing zero or more
    /// targeting values (position, facing, orientation, entity reference). Block kinds
    /// `0x06` and `0x0F` have a fixed-size, not-yet-understood prefix (4 and 8 bytes
    /// respectively) before the value chain; the other kinds these commands use
    /// (`0x01`, `0x03`, `0x1D`) have none. See `Value::parse_targets` on why any
    /// further trailing bytes are intentionally left unread. No block at all (kind
    /// `0xFF`, or absent entirely — the latter seen only for `SCMD_Unload`) yields
    /// all-`None` targeting fields.
    pub fn parse_targeted(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, source, block)| {
                let targets = match block {
                    None => TargetValues::default(),
                    Some(block) => {
                        let skip: u32 = match block.kind {
                            0x01 | 0x03 | 0x1D => 0,
                            0x06 => 4,
                            0x0F => 8,
                            other => {
                                panic!("unrecognized targeting parameter block kind 0x{other:02x}")
                            }
                        };
                        parse_targets(skip_bytes(block.data, skip))
                    }
                };
                CommandData::Targeted(source, targets)
            },
        )(input)
    }

    /// Header, source, then a blueprint-and-targets block (see
    /// `parse_blueprint_and_targets`). `PCMD_Ability`'s source is discarded (always the
    /// issuing player, already available elsewhere) to match [`Self::parse_pbgid`].
    pub fn parse_pbgid_targeted(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, _source, block)| {
                let (blueprint, targets) = parse_blueprint_and_targets(&expect_block(block));
                CommandData::PbgidTargeted(blueprint, targets)
            },
        )(input)
    }

    /// Header, source, then a parameter block, preserving the legacy source identifier
    /// like [`Self::parse_sourced_pbgid`] does. Block kind `0x01` carries only targeting
    /// values with no blueprint at all (continuing/updating an already-active ability's
    /// target, the same shape `SCMD_Ability` uses — see [`Self::parse_ability`]); any
    /// other kind is a blueprint-and-targets block (see `parse_blueprint_and_targets`).
    pub fn parse_sourced_pbgid_targeted(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, source, block)| {
                let block = expect_block(block);
                let (blueprint, targets) = if block.kind == 0x01 {
                    (None, parse_targets(block.data))
                } else {
                    let (blueprint, targets) = parse_blueprint_and_targets(&block);
                    (Some(blueprint), targets)
                };
                CommandData::SourcedPbgidTargeted(blueprint, source.legacy_identifier(), targets)
            },
        )(input)
    }

    /// `SCMD_Ability`'s payload is one of two shapes: block kind `0x01` carries only
    /// targeting values with no blueprint at all (continuing/updating an already-active
    /// ability's target, as best understood); kinds `0x23`/`0x24`/`0x29` carry a
    /// blueprint (see `parse_blueprint_and_targets`). This is the only command type
    /// observed using both.
    pub fn parse_ability(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, source, block)| {
                let (blueprint, targets) = match block {
                    None => (None, TargetValues::default()),
                    Some(block) if block.kind == 0x01 => (None, parse_targets(block.data)),
                    Some(block) => {
                        let (blueprint, targets) = parse_blueprint_and_targets(&block);
                        (Some(blueprint), targets)
                    }
                };
                CommandData::Ability(source, blueprint, targets)
            },
        )(input)
    }

    /// Header, source, then a construction parameter block (kind `0x1A`): a blueprint
    /// reference, three raw (non-tag-prefixed) positions, 4 not-yet-understood bytes,
    /// then a count-prefixed list of spawned entity ids — the count is stored as a
    /// big-endian `u32` whose high three bytes are always zero, i.e. effectively a
    /// one-byte count. Panics if the block isn't kind `0x1A` or doesn't fit this shape.
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
                let (after_blueprint, blueprint) = expect_parsed(Blueprint::parse(block.data));
                let positions = tuple((parse_raw_position, parse_raw_position, parse_raw_position));
                // 4 not-yet-understood bytes precede the count.
                let entities = preceded(take(4u32), length_count(be_u32, le_u32));
                let result: ParserResult<ConstructionFields> =
                    tuple((positions, entities))(after_blueprint);
                match result {
                    Ok((_, ((position, snapped, actual), entities))) => {
                        CommandData::Construction(blueprint, position, snapped, actual, entities)
                    }
                    Err(e) => panic!("failed to parse construction data: {e:?}"),
                }
            },
        )(input)
    }

    /// `DCMD_CameraTrack`'s payload: header, sourceless (empty squad-list) source,
    /// then a parameter block (kind `0x2f`) holding a sample sequence, the camera's
    /// eye position, and its orientation, each still raw fixed-point. Panics if the
    /// block isn't kind `0x2f` or doesn't fit this shape.
    pub fn parse_camera_track(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, _source, block)| {
                let block = expect_block(block);
                if block.kind != 0x2f {
                    panic!(
                        "expected a camera track parameter block (kind 0x2f), found kind 0x{:02x}",
                        block.kind
                    );
                }
                let mut fields = tuple((
                    le_u32, le_i16, le_i16, le_i16, le_i16, le_i16, le_i16, le_i16,
                ));
                let result: ParserResult<CameraTrackFields> = fields(block.data);
                match result {
                    Ok((_, (sequence, x, alt, z, w, qx, qy, qz))) => CommandData::CameraTrack {
                        sequence,
                        position: [x, alt, z],
                        orientation: [w, qx, qy, qz],
                    },
                    Err(e) => panic!("failed to parse camera track data: {e:?}"),
                }
            },
        )(input)
    }

    /// `DCMD_COUNT`'s payload: header, sourceless source, then a parameter block
    /// (kind `0x30`) holding a sample sequence and three entity counters — see
    /// [`crate::command_data::CameraCounts`] on why their exact meaning is unconfirmed.
    /// Panics if the block isn't kind `0x30` or doesn't fit this shape.
    pub fn parse_camera_counts(input: Span) -> ParserResult<CommandData> {
        map(
            tuple((payload::parse_header, Source::parse, ParamBlock::parse)),
            |(_, _source, block)| {
                let block = expect_block(block);
                if block.kind != 0x30 {
                    panic!(
                        "expected a camera counts parameter block (kind 0x30), found kind 0x{:02x}",
                        block.kind
                    );
                }
                let mut fields = tuple((le_u32, le_u16, le_u16, le_u16));
                let result: ParserResult<(u32, u16, u16, u16)> = fields(block.data);
                match result {
                    Ok((_, (sequence, a, b, c))) => CommandData::CameraCounts {
                        sequence,
                        counts: [a, b, c],
                    },
                    Err(e) => panic!("failed to parse camera counts data: {e:?}"),
                }
            },
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
            CommandType::SCMD_Upgrade | CommandType::SCMD_ReinforceUnit => Self::parse_source_pbgid,
            CommandType::SCMD_Retreat
            | CommandType::SCMD_Stop
            | CommandType::PCMD_TentativeUpgradeRemoveAll
            | CommandType::SCMD_UnloadSquads
            | CommandType::CMD_UnloadSquads
            | CommandType::PCMD_Surrender
            | CommandType::CMD_RallyPoint
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
            CommandType::DCMD_CameraTrack => Self::parse_camera_track,
            CommandType::DCMD_COUNT => Self::parse_camera_counts,
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

/// Reads the blueprint reference at the front of a command's parameter block, which
/// every decoder routed here requires.
fn expect_blueprint(block: Option<ParamBlock>) -> Blueprint {
    expect_parsed(Blueprint::parse(expect_block(block).data)).1
}

/// Unwraps a parser result, panicking on failure — for reads whose input has already
/// been bounded by the enclosing block, so a short read means the block doesn't match
/// the wire format this crate models.
fn expect_parsed<T>(result: ParserResult<T>) -> (Span, T) {
    match result {
        Ok(parsed) => parsed,
        Err(e) => panic!("failed to read expected command payload value: {e:?}"),
    }
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

/// Parses `[blueprint reference][kind-specific skip][target value chain]`, the shape
/// shared by every "ability" style block kind (`0x23`, `0x24`, `0x29`): `0x23` has no
/// prefix after the blueprint, `0x24` has 6 not-yet-understood bytes, `0x29` has 1.
fn parse_blueprint_and_targets(block: &ParamBlock) -> (Blueprint, TargetValues) {
    let (after_blueprint, blueprint) = expect_parsed(Blueprint::parse(block.data));
    let skip: u32 = match block.kind {
        0x23 => 0,
        0x24 => 6,
        0x29 => 1,
        other => panic!("unrecognized ability parameter block kind 0x{other:02x}"),
    };
    (blueprint, parse_targets(skip_bytes(after_blueprint, skip)))
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
