//! Parser for `<map>_territory.override` — the per-cell sector grid and
//! per-sector metadata (bounding box, adjacency, base/HQ flag).
//!
//! Format credit to
//! [cohstats/coh3-data `territory_parser.py`](https://github.com/cohstats/coh3-data/blob/master/scripts/mp-maps/territory_parser.py) —
//! the Relic Chunky layout below was independently confirmed against
//! `castello_8p_territory.override` from the local game depot, but this is
//! the only other public documentation of it.
//!
//! Chunk path: `FOLDOLYR/FOLDODAT/FOLDDATA/FOLDTRTY`, which has two children:
//!
//! - `FOLDTCEL/DATADATA` (v3001): the cell grid.
//!   ```text
//!   u32 width, u32 height, u32[width*height] sector_id,   -- 0 = unassigned
//!   u32 width, u32 height, u8[width*height]               -- second plane, unused here
//!   ```
//!   1 grid cell = 1 world unit, grid centered on the map origin like
//!   [`data::ScenarioPoint`] coordinates (world = cell - size/2).
//! - `FOLDSECT/DATADATA` (header) + repeated `FOLDSECT`(v3000)/`DATADATA`(v3004)
//!   (one per sector):
//!   ```text
//!   header: u32 sector_count, u32 (unused)
//!   per sector: u16 min_x, max_x, min_y, max_y  -- grid cell coords
//!               u32 neighbor_count, u32[neighbor_count] neighbor sector ids
//!               u16 is_base                     -- nonzero for player base/HQ sectors
//!   ```
//!   Sectors are emitted in id order starting at 1 (confirmed: recomputing
//!   each sector's bbox from the cell grid matches the declared one, on every
//!   sector of every scenario checked).

use crate::{chunky, geometry, Error};
use data::{Rect, Scenario, ScenarioPoint, Sector};

pub struct Territory {
    pub sectors: Vec<Sector>,
    grid: Vec<u32>,
    width: u32,
    height: u32,
}

pub fn parse_territory(bytes: &[u8]) -> Result<Territory, Error> {
    let trty_data = chunky::find_path(
        bytes,
        &[
            (b"FOLD", b"OLYR"),
            (b"FOLD", b"ODAT"),
            (b"FOLD", b"DATA"),
            (b"FOLD", b"TRTY"),
        ],
    )?;
    let trty_children = chunky::parse_chunks(trty_data)?;

    let tcel_fold = chunky::find(&trty_children, b"FOLD", b"TCEL")
        .ok_or_else(|| Error::Scenario("territory override missing TCEL".into()))?;
    let tcel_children = chunky::parse_chunks(tcel_fold.data)?;
    let tcel_payload = chunky::find(&tcel_children, b"DATA", b"DATA")
        .ok_or_else(|| Error::Scenario("TCEL missing DATA payload".into()))?
        .data;
    let (width, height, grid) = parse_cell_grid(tcel_payload)?;

    let sect_fold = chunky::find(&trty_children, b"FOLD", b"SECT")
        .ok_or_else(|| Error::Scenario("territory override missing SECT".into()))?;
    let sect_children = chunky::parse_chunks(sect_fold.data)?;

    let mut sectors = Vec::new();
    for child in &sect_children {
        if child.is_fold(b"SECT") {
            let inner = chunky::parse_chunks(child.data)?;
            let payload = chunky::find(&inner, b"DATA", b"DATA")
                .ok_or_else(|| Error::Scenario("SECT missing DATA payload".into()))?
                .data;
            let id = sectors.len() as u32 + 1;
            sectors.push(parse_sector_record(payload, id, width, height)?);
        }
    }

    let rings_by_id = geometry::trace_rings(&grid, width, height);
    for sector in &mut sectors {
        sector.rings = rings_by_id.get(&sector.id).cloned().unwrap_or_default();
    }

    Ok(Territory {
        sectors,
        grid,
        width,
        height,
    })
}

fn parse_cell_grid(data: &[u8]) -> Result<(u32, u32, Vec<u32>), Error> {
    if data.len() < 8 {
        return Err(Error::Scenario("TCEL payload too short".into()));
    }
    let width = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let height = u32::from_le_bytes(data[4..8].try_into().unwrap());
    let cell_count = width as usize * height as usize;
    let plane_end = 8 + cell_count * 4;
    if data.len() < plane_end {
        return Err(Error::Scenario("TCEL cell plane truncated".into()));
    }
    let grid = data[8..plane_end]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    Ok((width, height, grid))
}

fn parse_sector_record(data: &[u8], id: u32, width: u32, height: u32) -> Result<Sector, Error> {
    if data.len() < 12 {
        return Err(Error::Scenario("sector record too short".into()));
    }
    let min_x = u16::from_le_bytes(data[0..2].try_into().unwrap());
    let max_x = u16::from_le_bytes(data[2..4].try_into().unwrap());
    let min_y = u16::from_le_bytes(data[4..6].try_into().unwrap());
    let max_y = u16::from_le_bytes(data[6..8].try_into().unwrap());
    let adj_count = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;

    let neighbors_end = 12 + adj_count * 4;
    let flag_end = neighbors_end + 2;
    if data.len() < flag_end {
        return Err(Error::Scenario("sector record truncated".into()));
    }
    let neighbors = data[12..neighbors_end]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    let is_base = u16::from_le_bytes(data[neighbors_end..flag_end].try_into().unwrap()) != 0;

    let half_w = width as f32 / 2.0;
    let half_h = height as f32 / 2.0;
    let bounds = Rect {
        min_x: min_x as f32 - half_w,
        max_x: (max_x as f32 + 1.0) - half_w,
        min_y: min_y as f32 - half_h,
        max_y: (max_y as f32 + 1.0) - half_h,
    };

    Ok(Sector {
        id,
        is_base,
        neighbors,
        bounds,
        points: Vec::new(),
        rings: Vec::new(),
    })
}

/// Sets [`ScenarioPoint::sector`] for every point that falls within a cell
/// belonging to a sector. `Scenario::sectors`' `points` field is populated by
/// the caller afterward, once both `points` and `sectors` are in their final
/// owned locations (this only needs a mutable points slice).
pub fn assign_sectors(points: &mut [ScenarioPoint], territory: &Territory) {
    let half_w = territory.width as f32 / 2.0;
    let half_h = territory.height as f32 / 2.0;
    for point in points.iter_mut() {
        let gx = (point.x + half_w).round();
        let gy = (point.y + half_h).round();
        if gx < 0.0 || gy < 0.0 || gx >= territory.width as f32 || gy >= territory.height as f32 {
            continue;
        }
        let idx = gy as usize * territory.width as usize + gx as usize;
        let id = territory.grid[idx];
        if id != 0 {
            point.sector = Some(id);
        }
    }
}

/// Populates each [`Sector::points`] from the final point list's `sector`
/// assignments. Called once both are in their final locations on [`Scenario`].
pub fn link_sector_points(scenario: &mut Scenario) {
    for (idx, point) in scenario.points.iter().enumerate() {
        let Some(sector_id) = point.sector else {
            continue;
        };
        if let Some(sector) = scenario.sectors.iter_mut().find(|s| s.id == sector_id) {
            sector.points.push(idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cell_grid_dimensions() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&2u32.to_le_bytes());
        payload.extend_from_slice(&2u32.to_le_bytes());
        for id in [1u32, 1, 0, 0] {
            payload.extend_from_slice(&id.to_le_bytes());
        }
        let (w, h, grid) = parse_cell_grid(&payload).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(grid, vec![1, 1, 0, 0]);
    }

    #[test]
    fn parses_sector_record_with_neighbors() {
        let mut data = Vec::new();
        data.extend_from_slice(&0u16.to_le_bytes()); // min_x
        data.extend_from_slice(&1u16.to_le_bytes()); // max_x
        data.extend_from_slice(&0u16.to_le_bytes()); // min_y
        data.extend_from_slice(&1u16.to_le_bytes()); // max_y
        data.extend_from_slice(&2u32.to_le_bytes()); // adj_count
        data.extend_from_slice(&5u32.to_le_bytes());
        data.extend_from_slice(&7u32.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes()); // is_base

        let sector = parse_sector_record(&data, 3, 4, 4).unwrap();
        assert_eq!(sector.id, 3);
        assert!(sector.is_base);
        assert_eq!(sector.neighbors, vec![5, 7]);
        // width=height=4, so half=2; cells [0,1] -> world [-2, 0].
        assert_eq!(sector.bounds.min_x, -2.0);
        assert_eq!(sector.bounds.max_x, 0.0);
    }

    #[test]
    fn assign_sectors_sets_point_sector_from_grid() {
        let territory = Territory {
            sectors: vec![],
            grid: vec![0, 1, 1, 2, 2, 0],
            width: 3,
            height: 2,
        };
        // width=3 -> half=1.5; grid x=1 -> world x = 1 - 1.5 = -0.5
        let mut points = vec![ScenarioPoint {
            ebp: "territory_fuel_point_low".into(),
            x: -0.5,
            y: -0.5, // height=2 -> half=1; grid y=0 -> world y = 0 - 1 = -1... use 0.0 instead
            kind: data::PointKind::Fuel,
            tier: None,
            owner: None,
            income_per_minute: 0.0,
            capture_time: None,
            sector: None,
        }];
        // Recompute expected grid cell: gx = round(x + 1.5), gy = round(y + 1.0)
        points[0].x = -0.4; // gx = round(1.1) = 1
        points[0].y = -0.9; // gy = round(0.1) = 0
        assign_sectors(&mut points, &territory);
        assert_eq!(points[0].sector, Some(1));
    }
}
