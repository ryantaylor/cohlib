//! `CohLib::Command` marshalling.
//!
//! Every *decision* here — the variant name, the wire type name, source flattening,
//! payload-shape dispatch — is delegated to `cohlib`, where it's covered by the fixture
//! corpus in `crates/cohlib/tests/`. This module only assembles `RHash`es.

use cohlib::{Command, CommandPayloadRef, Orientation, Position, Source};
use magnus::{RHash, Ruby};

pub(crate) fn variant_name(rb_self: &Command) -> String {
    rb_self.variant_name().to_owned()
}

pub(crate) fn action_type(rb_self: &Command) -> String {
    rb_self.action_type().to_string()
}

/// Flattens a `Source` to `{ kind:, ids: }`. `ids` is always an array — including for
/// the single-id kinds — so callers never need to branch on `kind` to read it.
fn source_to_h(ruby: &Ruby, source: &Source) -> RHash {
    let hash = ruby.hash_new();
    hash.aset(ruby.to_symbol("kind"), source.kind()).unwrap();
    hash.aset(ruby.to_symbol("ids"), source.ids()).unwrap();
    hash
}

fn position_to_h(ruby: &Ruby, position: Position) -> RHash {
    let hash = ruby.hash_new();
    hash.aset(ruby.to_symbol("x"), position.x()).unwrap();
    hash.aset(ruby.to_symbol("y"), position.y()).unwrap();
    hash.aset(ruby.to_symbol("z"), position.z()).unwrap();
    hash
}

fn orientation_to_a(orientation: Orientation) -> Vec<f32> {
    vec![
        orientation.x(),
        orientation.y(),
        orientation.z(),
        orientation.w(),
    ]
}

/// The optional targeting block shared by `Targeted`, `Pbgid`, `SourcedPbgid`,
/// `SourcedAbility` and `Ability`. Keys are always written, `nil` when absent, so the
/// key set for a given `:type` is stable and a columnar schema can be derived from it.
fn aset_targeting(
    ruby: &Ruby,
    hash: RHash,
    position: Option<Position>,
    facing: Option<f32>,
    orientation: Option<Orientation>,
    entity: Option<u32>,
) {
    hash.aset(
        ruby.to_symbol("position"),
        position.map(|p| position_to_h(ruby, p)),
    )
    .unwrap();
    hash.aset(ruby.to_symbol("facing"), facing).unwrap();
    hash.aset(
        ruby.to_symbol("orientation"),
        orientation.map(orientation_to_a),
    )
    .unwrap();
    hash.aset(ruby.to_symbol("entity"), entity).unwrap();
}

pub(crate) fn to_h(ruby: &Ruby, rb_self: &Command) -> RHash {
    let hash = ruby.hash_new();
    hash.aset(ruby.to_symbol("type"), rb_self.variant_name())
        .unwrap();
    hash.aset(
        ruby.to_symbol("action_type"),
        rb_self.action_type().to_string(),
    )
    .unwrap();
    hash.aset(ruby.to_symbol("tick"), rb_self.tick()).unwrap();
    hash.aset(ruby.to_symbol("index"), rb_self.index()).unwrap();

    // Thirteen arms, not thirty-seven: `Command::payload` discriminates by payload shape,
    // and the two type tags above already carry the semantic variant.
    match rb_self.payload() {
        CommandPayloadRef::Empty(_) | CommandPayloadRef::Unknown(_) => {}
        CommandPayloadRef::Sourced(data) => {
            hash.aset(ruby.to_symbol("source"), source_to_h(ruby, data.source()))
                .unwrap();
        }
        CommandPayloadRef::SourcedIndex(data) => {
            hash.aset(ruby.to_symbol("source"), source_to_h(ruby, data.source()))
                .unwrap();
            hash.aset(
                ruby.to_symbol("source_identifier"),
                data.source_identifier(),
            )
            .unwrap();
            hash.aset(ruby.to_symbol("queue_index"), data.queue_index())
                .unwrap();
        }
        CommandPayloadRef::Targeted(data) => {
            hash.aset(ruby.to_symbol("source"), source_to_h(ruby, data.source()))
                .unwrap();
            aset_targeting(
                ruby,
                hash,
                data.position(),
                data.facing(),
                data.orientation(),
                data.entity(),
            );
        }
        CommandPayloadRef::SourcePbgid(data) => {
            hash.aset(ruby.to_symbol("source"), source_to_h(ruby, data.source()))
                .unwrap();
            hash.aset(ruby.to_symbol("pbgid"), data.pbgid()).unwrap();
            hash.aset(
                ruby.to_symbol("mod_uuid"),
                data.mod_uuid().map(|u| u.to_string()),
            )
            .unwrap();
        }
        CommandPayloadRef::Ability(data) => {
            hash.aset(ruby.to_symbol("source"), source_to_h(ruby, data.source()))
                .unwrap();
            hash.aset(ruby.to_symbol("pbgid"), data.pbgid()).unwrap();
            hash.aset(
                ruby.to_symbol("mod_uuid"),
                data.mod_uuid().map(|u| u.to_string()),
            )
            .unwrap();
            aset_targeting(
                ruby,
                hash,
                data.position(),
                data.facing(),
                data.orientation(),
                data.entity(),
            );
        }
        CommandPayloadRef::Pbgid(data) => {
            hash.aset(ruby.to_symbol("pbgid"), data.pbgid()).unwrap();
            hash.aset(
                ruby.to_symbol("mod_uuid"),
                data.mod_uuid().map(|u| u.to_string()),
            )
            .unwrap();
            aset_targeting(
                ruby,
                hash,
                data.position(),
                data.facing(),
                data.orientation(),
                data.entity(),
            );
        }
        CommandPayloadRef::SourcedPbgid(data) => {
            hash.aset(ruby.to_symbol("pbgid"), data.pbgid()).unwrap();
            hash.aset(
                ruby.to_symbol("mod_uuid"),
                data.mod_uuid().map(|u| u.to_string()),
            )
            .unwrap();
            hash.aset(ruby.to_symbol("source"), source_to_h(ruby, data.source()))
                .unwrap();
            hash.aset(
                ruby.to_symbol("source_identifier"),
                data.source_identifier(),
            )
            .unwrap();
            aset_targeting(
                ruby,
                hash,
                data.position(),
                data.facing(),
                data.orientation(),
                data.entity(),
            );
        }
        CommandPayloadRef::SourcedAbility(data) => {
            hash.aset(ruby.to_symbol("pbgid"), data.pbgid()).unwrap();
            hash.aset(
                ruby.to_symbol("mod_uuid"),
                data.mod_uuid().map(|u| u.to_string()),
            )
            .unwrap();
            hash.aset(ruby.to_symbol("source"), source_to_h(ruby, data.source()))
                .unwrap();
            hash.aset(
                ruby.to_symbol("source_identifier"),
                data.source_identifier(),
            )
            .unwrap();
            aset_targeting(
                ruby,
                hash,
                data.position(),
                data.facing(),
                data.orientation(),
                data.entity(),
            );
        }
        CommandPayloadRef::Construction(data) => {
            hash.aset(ruby.to_symbol("pbgid"), data.pbgid()).unwrap();
            hash.aset(
                ruby.to_symbol("mod_uuid"),
                data.mod_uuid().map(|u| u.to_string()),
            )
            .unwrap();
            hash.aset(
                ruby.to_symbol("position"),
                position_to_h(ruby, data.position()),
            )
            .unwrap();
            hash.aset(
                ruby.to_symbol("snapped_position"),
                position_to_h(ruby, data.snapped_position()),
            )
            .unwrap();
            hash.aset(
                ruby.to_symbol("final_position"),
                position_to_h(ruby, data.final_position()),
            )
            .unwrap();
            hash.aset(ruby.to_symbol("entities"), data.entities().to_vec())
                .unwrap();
        }
        CommandPayloadRef::ResourceBonus(data) => {
            let values = ruby.hash_new();
            for (name, amount) in data.values() {
                values.aset(name.as_str(), *amount).unwrap();
            }
            hash.aset(ruby.to_symbol("values"), values).unwrap();
        }
        CommandPayloadRef::BroadcastMessage(data) => {
            hash.aset(ruby.to_symbol("json"), data.json()).unwrap();
        }
    }
    hash
}
