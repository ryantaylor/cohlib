//! Bulk camera telemetry marshalling.
//!
//! Camera records are roughly three quarters of the records in a replay, so they're
//! returned as plain hashes rather than wrapped objects: one Ruby object per record
//! instead of a wrapped Rust allocation plus a method call per field. `Player`'s camera
//! accessors deep-clone their whole `Vec` on every call — assign the result to a local
//! rather than calling twice.

use cohlib::Player;
use magnus::{RArray, Ruby};

pub(crate) fn tracks(ruby: &Ruby, rb_self: &Player) -> RArray {
    let tracks = rb_self.camera_tracks();
    let arr = ruby.ary_new_capa(tracks.len());

    // Interning symbols once per call rather than once per record removes eight
    // lookups from each of potentially tens of thousands of rows.
    let k_tick = ruby.to_symbol("tick");
    let k_sequence = ruby.to_symbol("sequence");
    let k_x = ruby.to_symbol("x");
    let k_y = ruby.to_symbol("y");
    let k_z = ruby.to_symbol("z");
    let k_orientation = ruby.to_symbol("orientation");
    let k_pitch = ruby.to_symbol("pitch");
    let k_yaw = ruby.to_symbol("yaw");

    for track in &tracks {
        let hash = ruby.hash_new();
        let position = track.position();
        hash.aset(k_tick, track.tick()).unwrap();
        hash.aset(k_sequence, track.sequence()).unwrap();
        hash.aset(k_x, position.x()).unwrap();
        // `y` is altitude, not the ground plane — the ground pair is (x, z).
        hash.aset(k_y, position.y()).unwrap();
        hash.aset(k_z, position.z()).unwrap();
        hash.aset(k_orientation, track.orientation().to_vec())
            .unwrap();
        hash.aset(k_pitch, track.pitch()).unwrap();
        hash.aset(k_yaw, track.yaw()).unwrap();
        arr.push(hash).unwrap();
    }
    arr
}

pub(crate) fn counts(ruby: &Ruby, rb_self: &Player) -> RArray {
    let counts = rb_self.camera_counts();
    let arr = ruby.ary_new_capa(counts.len());

    let k_tick = ruby.to_symbol("tick");
    let k_sequence = ruby.to_symbol("sequence");
    let k_counts = ruby.to_symbol("counts");

    for entry in &counts {
        let hash = ruby.hash_new();
        hash.aset(k_tick, entry.tick()).unwrap();
        hash.aset(k_sequence, entry.sequence()).unwrap();
        hash.aset(k_counts, entry.counts().to_vec()).unwrap();
        arr.push(hash).unwrap();
    }
    arr
}
