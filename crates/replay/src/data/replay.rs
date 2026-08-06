use crate::command::Command;
use crate::command_data::{CameraCounts, CameraTrack};
use crate::command_type::CommandType;
use crate::data::chunks::Chunk::{DataAuto, DataData, DataSdsc};
use crate::data::chunks::{Chunk, DataAutoChunk, DataDataChunk, DataSdscChunk};
use crate::data::ticks::{CommandData, CommandTick, Tick};
use crate::data::{Chunky, Header};
use crate::data::{ParserResult, Span};
use crate::message::Message;
use nom::combinator::eof;
use nom::combinator::map;
use nom::multi::many_till;
use nom::sequence::tuple;
use nom_tracable::tracable_parser;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Replay {
    pub header: Header,
    _chunkies: Vec<Chunky>,
    pub chunks: Vec<Chunk>,
    pub ticks: Vec<Tick>,
}

impl Replay {
    #[tracable_parser]
    pub fn from_span(input: Span) -> ParserResult<Replay> {
        let (input, header) = Header::parse_header(input)?;

        let mut parser = map(
            tuple((
                Chunky::parse,
                Chunk::parse(header.version),
                Chunky::parse,
                Chunk::parse(header.version),
                Chunk::parse(header.version),
                many_till(Tick::parse, eof),
            )),
            |(
                first_chunky,
                foldpost_chunk,
                second_chunky,
                foldinfo_chunk,
                datasdsc_chunk,
                (ticks, _),
            )| {
                Replay {
                    header: header.clone(),
                    _chunkies: vec![first_chunky, second_chunky],
                    chunks: vec![foldpost_chunk, foldinfo_chunk, datasdsc_chunk],
                    ticks,
                }
            },
        );

        parser(input)
    }

    pub fn data_chunks(&self) -> Vec<&Chunk> {
        self.chunks
            .iter()
            .flat_map(|chunk| match chunk {
                Chunk::Fold(fold) => fold.chunks.iter().collect(),
                _ => vec![chunk],
            })
            .collect()
    }

    pub fn game_data(&self) -> &DataDataChunk {
        let chunks = self.data_chunks();

        let data_chunk = chunks
            .iter()
            .find(|chunk| matches!(chunk, DataData(_)))
            .unwrap();

        match data_chunk {
            DataData(data) => data,
            _ => panic!(),
        }
    }

    pub fn automatch_data(&self) -> Option<&DataAutoChunk> {
        match self
            .data_chunks()
            .iter()
            .find(|chunk| matches!(chunk, DataAuto(_)))
        {
            Some(DataAuto(chunk)) => Some(chunk),
            None => None,
            _ => panic!(),
        }
    }

    pub fn map_data(&self) -> &DataSdscChunk {
        let chunks = self.data_chunks();

        let map_chunk = chunks
            .iter()
            .find(|chunk| matches!(chunk, DataSdsc(_)))
            .unwrap();

        match map_chunk {
            DataSdsc(map) => map,
            _ => panic!(),
        }
    }

    pub fn command_ticks(&self) -> impl Iterator<Item = &CommandTick> {
        self.ticks.iter().filter_map(|tick| match tick {
            Tick::Command(command) => Some(command),
            _ => None,
        })
    }

    /// Player-issued commands, keyed by player id. Camera telemetry records
    /// (`DCMD_CameraTrack`/`DCMD_COUNT`) are not player commands — see
    /// [`Self::camera_tracks`] and [`Self::camera_counts`] — and are excluded here,
    /// which removes the large majority of records in the underlying command stream.
    pub fn commands(&self) -> HashMap<u32, Vec<Command>> {
        self.command_ticks()
            .enumerate()
            .fold(HashMap::new(), |mut acc, (idx, tick)| {
                for bundle in &tick.bundles {
                    for command in &bundle.commands {
                        if is_camera_telemetry(command.action_type) {
                            continue;
                        }
                        let player_commands = acc.entry(command.player_id as u32).or_default();
                        player_commands.push(Command::from_data_command_at_tick(
                            command.clone(),
                            idx as u32 + 1,
                        ));
                    }
                }
                acc
            })
    }

    /// Per-player camera telemetry, keyed by player id — see [`CameraTrack`] on why
    /// this is kept separate from [`Self::commands`].
    pub fn camera_tracks(&self) -> HashMap<u32, Vec<CameraTrack>> {
        self.command_ticks()
            .enumerate()
            .fold(HashMap::new(), |mut acc, (idx, tick)| {
                for bundle in &tick.bundles {
                    for command in &bundle.commands {
                        let CommandData::CameraTrack {
                            sequence,
                            position,
                            orientation,
                        } = &command.data
                        else {
                            continue;
                        };
                        let tracks = acc.entry(command.player_id as u32).or_default();
                        tracks.push(CameraTrack::new(
                            idx as u32 + 1,
                            command.player_id,
                            *sequence,
                            *position,
                            *orientation,
                        ));
                    }
                }
                acc
            })
    }

    /// Per-player camera diagnostic counters, keyed by player id — see [`CameraCounts`]
    /// on why this is kept separate from [`Self::commands`].
    pub fn camera_counts(&self) -> HashMap<u32, Vec<CameraCounts>> {
        self.command_ticks()
            .enumerate()
            .fold(HashMap::new(), |mut acc, (idx, tick)| {
                for bundle in &tick.bundles {
                    for command in &bundle.commands {
                        let CommandData::CameraCounts { sequence, counts } = &command.data else {
                            continue;
                        };
                        let entries = acc.entry(command.player_id as u32).or_default();
                        entries.push(CameraCounts::new(
                            idx as u32 + 1,
                            command.player_id,
                            *sequence,
                            *counts,
                        ));
                    }
                }
                acc
            })
    }

    pub fn messages(&self) -> HashMap<String, Vec<Message>> {
        self.ticks
            .iter()
            .enumerate()
            .filter_map(|(idx, tick)| match tick {
                Tick::Message(message) => Some((idx + 1, message.messages.clone())),
                _ => None,
            })
            .fold(HashMap::new(), |mut acc, (tick, messages)| {
                for message in messages.iter() {
                    let msgs = acc.entry(message.name.clone()).or_default();
                    msgs.push(Message::new(tick as u32, message.message.clone()));
                }
                acc
            })
    }
}

fn is_camera_telemetry(action_type: CommandType) -> bool {
    matches!(
        action_type,
        CommandType::DCMD_CameraTrack | CommandType::DCMD_COUNT
    )
}
