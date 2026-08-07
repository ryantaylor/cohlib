//! Traces sector outlines directly from `territory.rs`'s per-cell sector-id
//! grid, using the standard raster boundary-tracing technique (as used by e.g.
//! GDAL's polygonize): walk each cell that belongs to a sector, emit a
//! directed unit edge for each side that borders a different sector (or the
//! grid edge), then chain same-sector edges tail-to-head into closed rings.
//!
//! Credit to
//! [cohstats/coh3-data `sector_geometry.py`](https://github.com/cohstats/coh3-data/blob/master/scripts/mp-maps/sector_geometry.py)
//! for the goal of sharing exact boundary coordinates between adjacent
//! sectors so rendered borders have no gaps. This module reaches that
//! property differently: rather than simplify each sector's ring independently
//! with a Douglas-Peucker tolerance and then re-stitch shared arcs, it merges
//! only exactly-collinear runs of unit edges. Two neighboring sectors' shared
//! boundary is a sequence of unit edges at the same grid coordinates by
//! construction, and collinear-run merging is a per-ring operation with no
//! tolerance parameter — so it can never disagree between the two sides about
//! where a line segment begins or ends, which a tolerance-based simplifier run
//! independently per sector could.

use std::collections::{HashMap, HashSet};

/// Grid-corner coordinate (not a cell index — cell `(x, y)` spans corners
/// `(x, y)` to `(x+1, y+1)`).
type Vertex = (i32, i32);

/// Traces one closed ring (as world-space coordinates) per contiguous boundary
/// loop, for every sector id present in `grid`. A sector split into disjoint
/// regions (shouldn't happen for real scenarios, but not assumed impossible)
/// yields multiple rings.
pub fn trace_rings(grid: &[u32], width: u32, height: u32) -> HashMap<u32, Vec<Vec<[f32; 2]>>> {
    let w = width as i32;
    let h = height as i32;
    let id_at = |x: i32, y: i32| -> u32 {
        if x < 0 || y < 0 || x >= w || y >= h {
            0
        } else {
            grid[(y * w + x) as usize]
        }
    };

    let mut edges_by_id: HashMap<u32, Vec<(Vertex, Vertex)>> = HashMap::new();
    for y in 0..h {
        for x in 0..w {
            let id = id_at(x, y);
            if id == 0 {
                continue;
            }
            let edges = edges_by_id.entry(id).or_default();
            // Walk clockwise (screen coords, y increasing downward) around each
            // cell, emitting only sides that border a different sector.
            if id_at(x, y - 1) != id {
                edges.push(((x, y), (x + 1, y))); // top
            }
            if id_at(x + 1, y) != id {
                edges.push(((x + 1, y), (x + 1, y + 1))); // right
            }
            if id_at(x, y + 1) != id {
                edges.push(((x + 1, y + 1), (x, y + 1))); // bottom
            }
            if id_at(x - 1, y) != id {
                edges.push(((x, y + 1), (x, y))); // left
            }
        }
    }

    let half_w = width as f32 / 2.0;
    let half_h = height as f32 / 2.0;
    let to_world = |v: Vertex| [v.0 as f32 - half_w, v.1 as f32 - half_h];

    edges_by_id
        .into_iter()
        .map(|(id, edges)| {
            let rings = build_rings(edges)
                .into_iter()
                .map(|ring| simplify_collinear(&ring, to_world))
                .collect();
            (id, rings)
        })
        .collect()
}

/// Chains directed unit edges into closed rings. Each vertex in a well-formed
/// raster boundary has equal in/out degree, so a walk starting from any unused
/// edge always returns to its own start vertex.
fn build_rings(edges: Vec<(Vertex, Vertex)>) -> Vec<Vec<Vertex>> {
    let mut by_start: HashMap<Vertex, Vec<Vertex>> = HashMap::new();
    for &(a, b) in &edges {
        by_start.entry(a).or_default().push(b);
    }

    let mut rings = Vec::new();
    let mut visited: HashSet<(Vertex, Vertex)> = HashSet::new();
    for &(start, first_next) in &edges {
        if visited.contains(&(start, first_next)) {
            continue;
        }
        let mut ring = vec![start];
        let mut current = start;
        let mut next = first_next;
        loop {
            visited.insert((current, next));
            ring.push(next);
            if next == start {
                break;
            }
            let Some(candidates) = by_start.get(&next) else {
                break;
            };
            let Some(&following) = candidates.iter().find(|c| !visited.contains(&(next, **c)))
            else {
                break;
            };
            current = next;
            next = following;
        }
        rings.push(ring);
    }
    rings
}

/// Drops vertices where the ring continues in the same direction, keeping only
/// direction-change points. `ring` is a closed loop with its start vertex
/// repeated at the end (as produced by [`build_rings`]).
fn simplify_collinear(ring: &[Vertex], to_world: impl Fn(Vertex) -> [f32; 2]) -> Vec<[f32; 2]> {
    let n = ring.len().saturating_sub(1); // exclude the repeated closing vertex
    if n < 3 {
        return ring.iter().map(|&v| to_world(v)).collect();
    }
    let mut simplified = Vec::new();
    for i in 0..n {
        let prev = ring[(i + n - 1) % n];
        let cur = ring[i];
        let next = ring[(i + 1) % n];
        let d1 = (cur.0 - prev.0, cur.1 - prev.1);
        let d2 = (next.0 - cur.0, next.1 - cur.1);
        if d1 != d2 {
            simplified.push(to_world(cur));
        }
    }
    if let Some(&first) = simplified.first() {
        simplified.push(first); // re-close the simplified ring
    }
    simplified
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traces_single_square_sector() {
        // 3x3 grid, sector 1 occupies the middle cell only.
        #[rustfmt::skip]
        let grid = [
            0, 0, 0,
            0, 1, 0,
            0, 0, 0,
        ];
        let rings = trace_rings(&grid, 3, 3);
        let sector1 = &rings[&1];
        assert_eq!(sector1.len(), 1, "single cell should trace one ring");
        // A single cell is already a minimal square: 4 corners, ring closed (5 points).
        assert_eq!(sector1[0].len(), 5);
    }

    #[test]
    fn merges_collinear_edges_along_a_straight_run() {
        // 1x3 grid, all one sector: a straight strip should simplify to a
        // 4-corner rectangle regardless of length.
        let grid = [1, 1, 1];
        let rings = trace_rings(&grid, 3, 1);
        let sector1 = &rings[&1];
        assert_eq!(sector1.len(), 1);
        assert_eq!(
            sector1[0].len(),
            5,
            "strip should simplify to 4 corners + close"
        );
    }

    #[test]
    fn two_adjacent_sectors_share_exact_boundary_coordinates() {
        // 4x2 grid split into left sector 1 / right sector 2.
        #[rustfmt::skip]
        let grid = [
            1, 1, 2, 2,
            1, 1, 2, 2,
        ];
        let rings = trace_rings(&grid, 4, 2);
        let s1_points: HashSet<[i64; 2]> = rings[&1][0]
            .iter()
            .map(|p| [p[0] as i64, p[1] as i64])
            .collect();
        let s2_points: HashSet<[i64; 2]> = rings[&2][0]
            .iter()
            .map(|p| [p[0] as i64, p[1] as i64])
            .collect();
        // The shared vertical boundary's two corners must appear in both rings
        // at identical coordinates.
        let shared: Vec<_> = s1_points.intersection(&s2_points).collect();
        assert_eq!(
            shared.len(),
            2,
            "adjacent sectors should share exactly the boundary corners"
        );
    }

    #[test]
    fn no_sectors_present_yields_empty_map() {
        let grid = [0, 0, 0, 0];
        let rings = trace_rings(&grid, 2, 2);
        assert!(rings.is_empty());
    }
}
