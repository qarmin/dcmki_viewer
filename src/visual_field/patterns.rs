#![allow(dead_code)]
// Bitmap patterns + render helper kept for future probability diagram enhancement.

/// HVF probability bitmap patterns, identical to diagram_images.rs in ee3.
///
/// Each array is [[u8; 6]; 8] where 1=white, 0=black.
/// They are rendered inline per cell (not as tiled SVG patterns) to preserve
/// the exact same visual as the PDF reports.
// Matches IM1_ARRAY - normal (almost fully white, one dot)
pub const PAT_NORMAL: [[u8; 6]; 8] = [
    [1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1],
    [1, 0, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1],
];

// Matches IM3_ARRAY - p < 5%
pub const PAT_P5: [[u8; 6]; 8] = [
    [1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1],
    [1, 0, 1, 1, 0, 1],
    [1, 1, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1],
    [1, 0, 1, 1, 0, 1],
    [1, 1, 1, 1, 1, 1],
];

// Matches IM5_ARRAY - p < 2%
pub const PAT_P2: [[u8; 6]; 8] = [
    [1, 1, 1, 1, 0, 1],
    [1, 1, 1, 0, 1, 0],
    [0, 1, 0, 1, 0, 1],
    [1, 0, 1, 1, 1, 0],
    [0, 1, 1, 1, 0, 1],
    [1, 0, 1, 0, 1, 1],
    [0, 1, 0, 1, 1, 1],
    [1, 0, 1, 0, 1, 0],
];

// Matches IM7_ARRAY - p < 1%
pub const PAT_P1: [[u8; 6]; 8] = [
    [1, 0, 1, 1, 0, 1],
    [0, 0, 0, 1, 0, 1],
    [1, 0, 0, 0, 0, 0],
    [0, 0, 1, 0, 0, 1],
    [1, 0, 0, 1, 0, 0],
    [1, 0, 0, 0, 0, 0],
    [0, 0, 1, 0, 1, 0],
    [0, 1, 0, 0, 0, 1],
];

// Matches IM9_ARRAY - p < 0.5% (fully black)
pub const PAT_P05: [[u8; 6]; 8] = [
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0],
];

/// Choose the bitmap array for a given p-value (same logic as get_img_p in ee3).
pub fn pattern_for_p(p: Option<f32>) -> &'static [[u8; 6]; 8] {
    match p {
        Some(p) if p <= 0.005 => &PAT_P05,
        Some(p) if p <= 0.01  => &PAT_P1,
        Some(p) if p <= 0.02  => &PAT_P2,
        Some(p) if p <= 0.05  => &PAT_P5,
        _                      => &PAT_NORMAL,
    }
}

/// Render one 6x8 bitmap pattern into SVG as individual black rectangles.
///
/// The pattern fills the rectangle `(cell_x, cell_y, cell_w, cell_h)`.
/// The background (white pixels) is filled first, then black pixels are drawn on top.
pub fn render_bitmap_cell(
    svg: &mut String,
    arr: &[[u8; 6]; 8],
    cell_x: f32,
    cell_y: f32,
    cell_w: f32,
    cell_h: f32,
) {
    // White background
    svg.push_str(&format!(
        r#"<rect x="{cell_x:.2}" y="{cell_y:.2}" width="{cell_w:.2}" height="{cell_h:.2}" fill="white"/>"#
    ));

    let pw = cell_w / 6.0;
    let ph = cell_h / 8.0;

    for (row, cols) in arr.iter().enumerate() {
        for (col, &val) in cols.iter().enumerate() {
            if val == 0 {
                let rx = cell_x + col as f32 * pw;
                let ry = cell_y + row as f32 * ph;
                svg.push_str(&format!(
                    r#"<rect x="{rx:.2}" y="{ry:.2}" width="{pw:.2}" height="{ph:.2}" fill="black"/>"#
                ));
            }
        }
    }
}

/// HVF grayscale: 0 dB => black, 35 dB => white.
pub fn hvf_gray(sensitivity: Option<f32>, seen: bool) -> String {
    if !seen {
        return "#000000".to_string();
    }
    match sensitivity {
        Some(s) if s >= 0.0 => {
            let v = ((s / 35.0).clamp(0.0, 1.0) * 255.0) as u8;
            format!("#{v:02x}{v:02x}{v:02x}")
        }
        _ => "#000000".to_string(),
    }
}
