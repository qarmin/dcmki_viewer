// Layout constants and coordinate mapping.

// Each diagram panel: square DIAG_SZ x DIAG_SZ (the drawable field area).
// Plus a TITLE_H header strip, so total panel height = DIAG_SZ + TITLE_H.
pub const DIAG_SZ: f32 = 510.0;
pub const TITLE_H: f32 = 20.0;
pub const MAR: f32 = 20.0; // inner margin inside the diagram area

pub const GAP: f32 = 10.0;
pub const HEADER_H: f32 = 64.0;
pub const STATS_W: f32 = 320.0;

// Overall SVG width
pub const SVG_W: u32 = (GAP + DIAG_SZ + GAP + DIAG_SZ + GAP + STATS_W + GAP) as u32;
/// Narrower width for suprathreshold (no grayscale column).
pub const SVG_W_SUPRA: u32 = (GAP + DIAG_SZ + GAP + STATS_W + GAP) as u32;

/// Compute SVG height for a given number of diagram rows.
pub const fn svg_h(rows: u32) -> u32 {
    let panel_h = DIAG_SZ + TITLE_H;
    (HEADER_H + GAP + rows as f32 * panel_h + (rows - 1) as f32 * GAP + GAP) as u32
}

/// Map visual field coordinates (degrees) to SVG pixel coordinates inside
/// a square diagram area at (panel_x, inner_py) with side length `sz`.
///
/// VF convention: +x = right, +y = up.
/// SVG convention: +x = right, +y = down.
pub fn vf_to_px(vx: f32, vy: f32, panel_x: f32, inner_py: f32, sz: f32, extent: f32) -> (f32, f32) {
    let usable = sz - 2.0 * MAR;
    let px = panel_x + MAR + (vx + extent) / (2.0 * extent) * usable;
    let py = inner_py + MAR + (extent - vy) / (2.0 * extent) * usable;
    (px, py)
}

/// Half-width of one cell (point) in SVG pixels.
/// Based on the degree spacing between adjacent test points.
pub fn cell_radius(extent: f32) -> f32 {
    let deg_step = if extent <= 12.0 { 2.0 } else { 6.0 };
    let usable = DIAG_SZ - 2.0 * MAR;
    let px_per_deg = usable / (2.0 * extent);
    // 0.5 so adjacent cells just touch without overlapping
    (deg_step * px_per_deg * 0.50).clamp(6.0, 22.0)
}

