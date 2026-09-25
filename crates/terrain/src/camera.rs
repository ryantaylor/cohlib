//! Reads the isometric camera module's `tuning` group from
//! `instances/camera/default_multiplayer.xml` (`ReferenceAttributes.sga`).
//!
//! There are two `tuning` groups in this file — one under `camera_bag` (input
//! sensitivity, irrelevant here) and the real one nested inside the
//! `cam_module_isometric` `template_reference`. There's also a `defaults` group
//! on that same module with a *different* `distance` field (35, not a bound) —
//! this is the exact 89/43 bug the research report documents (spec §2.2):
//! reading floats positionally instead of by name+ancestor picks up the wrong
//! group. This reader matches by element name, `name` attribute and ancestry,
//! never by position.

use crate::Error;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

const ISOMETRIC_MODULE: &str = "camera_modules\\cam_module_isometric";

#[derive(Debug, Clone, Copy, Default)]
pub struct CameraTuning {
    pub distance_min: Option<f32>,
    pub distance_max: Option<f32>,
    pub pitch_min: Option<f32>,
    pub pitch_max: Option<f32>,
}

impl CameraTuning {
    /// Errors if any of the four fields the grid command needs was never found.
    pub fn require_complete(self) -> Result<(f32, f32, f32, f32), Error> {
        match (
            self.distance_min,
            self.distance_max,
            self.pitch_min,
            self.pitch_max,
        ) {
            (Some(dmin), Some(dmax), Some(pmin), Some(pmax)) => Ok((dmin, dmax, pmin, pmax)),
            _ => Err(Error::Parse(
                "isometric camera module's tuning group is missing one or more of \
                 distance_min/distance_max/pitch_min/pitch_max"
                    .into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marker {
    Other,
    IsometricModule,
    IsometricTuning,
}

/// Parses `instances/camera/default_multiplayer.xml`'s bytes.
pub fn parse_camera_tuning(xml: &[u8]) -> Result<CameraTuning, Error> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);

    let mut stack: Vec<Marker> = Vec::new();
    let mut tuning = CameraTuning::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Start(e) => {
                let marker = classify(&e, &stack);
                if marker == Marker::IsometricTuning {
                    read_tuning_floats(&mut reader, &mut tuning)?;
                    // read_tuning_floats consumes through the matching End itself.
                    continue;
                }
                stack.push(marker);
            }
            Event::Empty(e) => {
                // A self-closing element can't itself be the tuning group (it has
                // children in every real file), but classify it anyway so an
                // empty `<group name="tuning" />` doesn't silently do nothing.
                let _ = classify(&e, &stack);
            }
            Event::End(_) => {
                stack.pop();
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(tuning)
}

fn get_attr(e: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if a.key.as_ref() == name {
            a.unescape_value().ok().map(|v| v.into_owned())
        } else {
            None
        }
    })
}

fn classify(e: &BytesStart<'_>, stack: &[Marker]) -> Marker {
    let name = e.name();
    let tag = std::str::from_utf8(name.as_ref()).unwrap_or("");
    let name_attr = get_attr(e, b"name");
    let value_attr = get_attr(e, b"value");
    let parent_is_isometric = stack.last() == Some(&Marker::IsometricModule);

    if tag == "template_reference"
        && name_attr.as_deref() == Some("camera_modules")
        && value_attr.as_deref() == Some(ISOMETRIC_MODULE)
    {
        return Marker::IsometricModule;
    }
    if tag == "group" && name_attr.as_deref() == Some("tuning") && parent_is_isometric {
        return Marker::IsometricTuning;
    }
    Marker::Other
}

/// Reads `<float name="..." value="..." />` children of the isometric tuning
/// group until its matching `</group>`, tracking nesting depth so a nested
/// group (e.g. `smoothing`) doesn't end the scan early.
fn read_tuning_floats(reader: &mut Reader<&[u8]>, tuning: &mut CameraTuning) -> Result<(), Error> {
    let mut depth = 0u32;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Start(_) => {
                depth += 1;
            }
            Event::Empty(e) => {
                if depth == 0 && e.name().as_ref() == b"float" {
                    apply_float(&e, tuning);
                }
            }
            Event::End(_) => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

fn apply_float(e: &BytesStart<'_>, tuning: &mut CameraTuning) {
    let name_attr = get_attr(e, b"name");
    let value_attr = get_attr(e, b"value").and_then(|v| v.parse::<f32>().ok());
    match (name_attr.as_deref(), value_attr) {
        (Some("distance_min"), Some(v)) => tuning.distance_min = Some(v),
        (Some("distance_max"), Some(v)) => tuning.distance_max = Some(v),
        (Some("pitch_min"), Some(v)) => tuning.pitch_min = Some(v),
        (Some("pitch_max"), Some(v)) => tuning.pitch_max = Some(v),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real bytes from ReferenceAttributes.sga's instances/camera/default_multiplayer.xml
    // (current build), trimmed to the relevant modules. Includes the decoy
    // `camera_bag > tuning` group and the isometric module's `defaults` group
    // (distance=35) to guard against the 89/43 positional-read bug regressing.
    const REAL_XML: &str = r#"<instance version="5" description="" template="camera">
	<variant name="default">
		<uniqueid name="pbgid" value="1524808" />
		<group name="camera_bag">
			<group name="defaults">
				<float name="fov" value="50" />
			</group>
			<group name="tuning">
				<group name="input">
					<float name="distance_rate_wheel" value="1.5" />
				</group>
			</group>
			<list name="modules">
				<template_reference name="camera_modules" value="camera_modules\cam_module_zoom" List.ItemID="-119907165">
				</template_reference>
				<template_reference name="camera_modules" value="camera_modules\cam_module_isometric" List.ItemID="-1793659976">
					<group name="defaults">
						<float name="pitch" value="43" />
						<float name="distance" value="35" />
						<float name="yaw" value="89" />
					</group>
					<group name="tuning">
						<float name="distance_min" value="15" />
						<float name="distance_max" value="43" />
						<float name="pitch_min" value="25" />
						<float name="pitch_max" value="75" />
						<bool name="enable_pitch_input" value="True" />
						<group name="smoothing">
							<template_reference name="spring_distance" value="options\camera\smoothing_option">
								<float name="spring_strength" value="5" />
							</template_reference>
						</group>
						<float name="fov" value="50" />
					</group>
				</template_reference>
			</list>
		</group>
	</variant>
</instance>"#;

    #[test]
    fn reads_isometric_tuning_not_the_defaults_group_or_camera_bag_tuning() {
        let tuning = parse_camera_tuning(REAL_XML.as_bytes()).unwrap();
        let (dmin, dmax, pmin, pmax) = tuning.require_complete().unwrap();
        assert_eq!(dmin, 15.0);
        assert_eq!(dmax, 43.0); // not 89 (the defaults group's yaw) or 35 (its distance)
        assert_eq!(pmin, 25.0);
        assert_eq!(pmax, 75.0);
    }

    #[test]
    fn missing_isometric_module_errors() {
        let xml = br#"<instance><variant><group name="tuning"><float name="distance_max" value="1" /></group></variant></instance>"#;
        let tuning = parse_camera_tuning(xml).unwrap();
        assert!(tuning.require_complete().is_err());
    }
}
