//! Dilates a heightfield by the camera's maximum horizontal reach and
//! block-downsamples it to a coarse lookup grid, per the research script
//! (`analysis/zoomhack/terrain/05_coarse_grid.py`) this is a faithful port of.
//!
//! Both steps only ever raise the stored value, so `tmax` stays a rigorous
//! upper bound on terrain height within the camera's reach of any cell in its
//! block — the certificate's `D >= (eye_y - tmax) / sin(pitch)` bound depends
//! on that never being violated.

use crate::heightfield::{Heightfield, WorldExtent};

/// Downsample block size. Fixed by the research script, not derived from the
/// camera constants — unlike the dilation radius, which *is* the emitted
/// `distance_max` (see [`dilate_and_downsample`]).
pub const BLOCK: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridCell {
    pub bi: u32,
    pub bj: u32,
    pub tmax: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct MapMeta {
    pub w: i32,
    pub h: i32,
    pub cx: f64,
    pub cz: f64,
    pub xhalf: f64,
    pub zhalf: f64,
    pub block_x: f64,
    pub block_z: f64,
    pub hmax: f32,
}

/// World-space cell size, half-extents and the raw height ceiling — computed
/// before dilation, from the undilated heightfield, matching the research
/// script's `meta` row exactly.
pub fn compute_meta(hf: &Heightfield, extent: WorldExtent, block: usize) -> MapMeta {
    let cx = extent.x / (hf.w as f64 - 1.0);
    let cz = extent.z / (hf.h as f64 - 1.0);
    let xhalf = (hf.w as f64 - 1.0) * cx / 2.0;
    let zhalf = (hf.h as f64 - 1.0) * cz / 2.0;
    let hmax = hf.height.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    MapMeta {
        w: hf.w,
        h: hf.h,
        cx,
        cz,
        xhalf,
        zhalf,
        block_x: cx * block as f64,
        block_z: cz * block as f64,
        hmax,
    }
}

/// Dilates `hf`'s plane-1 heights by `reach` world units (a separable sliding
/// max along x then z, matching the window radius the research script derives
/// from `2*reach/cell_size`) and block-downsamples the result by `block`,
/// taking the block maximum.
///
/// The sliding max wraps circularly at the map edges (`np.roll` semantics in
/// the reference script) rather than clamping — a faithful port keeps that,
/// even though it's a minor over-conservatism at map boundaries, since the
/// bound only needs to never be violated, and cell wrap only ever adds more
/// terrain to the max, never removes any.
pub fn dilate_and_downsample(
    hf: &Heightfield,
    cx: f64,
    cz: f64,
    reach: f32,
    block: usize,
) -> Vec<GridCell> {
    let w = hf.w as usize;
    let h = hf.h as usize;
    // Matches the research script's `k = int(2*REACH/cell) + 1; k // 2`
    // exactly — window size k is always computed before halving, and integer
    // truncation makes `(k+1)//2` differ from `k//2` when the raw quotient's
    // integer part is odd, so the `+ 1` must happen first.
    let kx = (2.0 * reach as f64 / cx) as usize + 1;
    let kz = (2.0 * reach as f64 / cz) as usize + 1;
    let rx = kx / 2;
    let rz = kz / 2;

    // Pass 1: circular sliding max along x, within each row.
    let mut m1 = vec![0f32; w * h];
    for j in 0..h {
        let row = &hf.height[j * w..j * w + w];
        let out = circular_window_max(row, rx);
        m1[j * w..j * w + w].copy_from_slice(&out);
    }

    // Pass 2: circular sliding max along z, within each column, over pass 1's output.
    let mut m2 = vec![0f32; w * h];
    let mut col = vec![0f32; h];
    for i in 0..w {
        for (j, slot) in col.iter_mut().enumerate() {
            *slot = m1[j * w + i];
        }
        let out = circular_window_max(&col, rz);
        for (j, v) in out.into_iter().enumerate() {
            m2[j * w + i] = v;
        }
    }

    let nj = h.div_ceil(block);
    let ni = w.div_ceil(block);
    let mut cells = Vec::with_capacity(ni * nj);
    for bj in 0..nj {
        for bi in 0..ni {
            let mut tmax = f32::NEG_INFINITY;
            for jj in 0..block {
                let j = bj * block + jj;
                if j >= h {
                    break;
                }
                for ii in 0..block {
                    let i = bi * block + ii;
                    if i >= w {
                        break;
                    }
                    tmax = tmax.max(m2[j * w + i]);
                }
            }
            cells.push(GridCell {
                bi: bi as u32,
                bj: bj as u32,
                tmax,
            });
        }
    }
    cells
}

/// Circular sliding max with radius `radius` (window size `2*radius+1`),
/// matching `np.roll`-based dilation: out-of-range shifts wrap around rather
/// than clamp at the edges.
fn circular_window_max(values: &[f32], radius: usize) -> Vec<f32> {
    let n = values.len();
    let mut out = values.to_vec();
    if n == 0 {
        return out;
    }
    for s in 1..=radius {
        let s = (s % n) as i64;
        for (idx, slot) in out.iter_mut().enumerate() {
            let plus = values[((idx as i64 + s).rem_euclid(n as i64)) as usize];
            let minus = values[((idx as i64 - s).rem_euclid(n as i64)) as usize];
            *slot = slot.max(plus).max(minus);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heightfield::Heightfield;

    #[test]
    fn circular_window_max_wraps_at_edges() {
        // radius 1 over [1,2,3,4,5]: each cell sees itself and its circular neighbors.
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let out = circular_window_max(&values, 1);
        // idx0 sees values[4],values[0],values[1] = 5,1,2 -> max 5
        // idx4 sees values[3],values[4],values[0] = 4,5,1 -> max 5
        assert_eq!(out, vec![5.0, 3.0, 4.0, 5.0, 5.0]);
    }

    #[test]
    fn circular_window_max_radius_zero_is_identity() {
        let values = vec![1.0, 9.0, 2.0];
        assert_eq!(circular_window_max(&values, 0), values);
    }

    #[test]
    fn dilate_and_downsample_small_grid_matches_hand_computed() {
        // 8x8 heightfield, cell size 1.0 in both axes, reach 1.0 -> kx=kz=3, radius 1.
        // Block size 4 -> a 2x2 grid of 4x4 blocks. Peaks placed far enough apart
        // (and from the circular wrap) that dilation doesn't blend them, cross-
        // checked against the Python reference implementation directly.
        let mut height = vec![0.0f32; 64];
        height[1 * 8 + 1] = 9.0;
        height[6 * 8 + 6] = 5.0;
        let hf = Heightfield { w: 8, h: 8, height };
        let cells = dilate_and_downsample(&hf, 1.0, 1.0, 1.0, 4);

        let at = |bi, bj| {
            cells
                .iter()
                .find(|c| c.bi == bi && c.bj == bj)
                .unwrap()
                .tmax
        };
        assert_eq!(at(0, 0), 9.0);
        assert_eq!(at(1, 0), 0.0);
        assert_eq!(at(0, 1), 0.0);
        assert_eq!(at(1, 1), 5.0);
    }

    #[test]
    fn compute_meta_matches_hand_computed_cliff_crossing_2p_values() {
        // Real depot values: cliff_crossing_2p is 673x705 cells, world 352x384.
        let hf = Heightfield {
            w: 673,
            h: 705,
            height: vec![0.0; 673 * 705],
        };
        let extent = crate::heightfield::WorldExtent { x: 352.0, z: 384.0 };
        let meta = compute_meta(&hf, extent, BLOCK);
        assert!((meta.cx - 352.0 / 672.0).abs() < 1e-9);
        assert!((meta.cz - 384.0 / 704.0).abs() < 1e-9);
        assert!((meta.block_x - meta.cx * 32.0).abs() < 1e-9);
    }
}
