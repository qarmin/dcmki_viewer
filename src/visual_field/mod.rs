mod data;
mod layout;
mod panels;
mod patterns;
mod render;
mod svg_helpers;

use dicom::object::InMemDicomObject;
use image::DynamicImage;

use data::extract_data;
use layout::{DIAG_SZ, GAP, HEADER_H, STATS_W, SVG_W, SVG_W_SUPRA, TITLE_H, svg_h};
use panels::{draw_grayscale, draw_header, draw_pd, draw_pd_prob, draw_stats, draw_td, draw_td_prob, draw_threshold, panel_h};
use render::svg_to_image;
use svg_helpers::rect;

fn build_svg(data: &data::DicomVfData) -> String {
    let supra = data.test_strategy.is_suprathreshold();
    let svg_w = if supra { SVG_W_SUPRA } else { SVG_W };
    let w = svg_w as f32;
    let rows = if supra { 1u32 } else { 3u32 };
    let h = svg_h(rows) as f32;

    let mut defs = "<defs>".to_string();
    let mut svg = String::new();

    // Background
    svg.push_str(&rect(0.0, 0.0, w, h, panels::BG, 0.0));

    // Header
    svg.push_str(&draw_header(data, w));

    // Column X positions
    let col_left  = GAP;
    let col_right = GAP + DIAG_SZ + GAP;
    let col_stats = if supra { GAP + DIAG_SZ + GAP } else { GAP + DIAG_SZ + GAP + DIAG_SZ + GAP };

    // Row Y positions (below header)
    let row0 = HEADER_H + GAP;

    // Clip paths for row 0
    defs.push_str(&svg_helpers::clip_path("cl0l", col_left,  row0 + TITLE_H, DIAG_SZ, DIAG_SZ));

    // Row 0: Threshold (left) | Grayscale (right, threshold only)
    svg.push_str(&draw_threshold(data, col_left,  row0, "cl0l"));
    if !supra {
        defs.push_str(&svg_helpers::clip_path("cl0r", col_right, row0 + TITLE_H, DIAG_SZ, DIAG_SZ));
        svg.push_str(&draw_grayscale(data, col_right, row0, "cl0r"));
    }

    if !data.test_strategy.is_suprathreshold() {
        let row1 = row0 + panel_h() + GAP;
        let row2 = row1 + panel_h() + GAP;

        defs.push_str(&svg_helpers::clip_path("cl1l", col_left,  row1 + TITLE_H, DIAG_SZ, DIAG_SZ));
        defs.push_str(&svg_helpers::clip_path("cl1r", col_right, row1 + TITLE_H, DIAG_SZ, DIAG_SZ));
        defs.push_str(&svg_helpers::clip_path("cl2l", col_left,  row2 + TITLE_H, DIAG_SZ, DIAG_SZ));
        defs.push_str(&svg_helpers::clip_path("cl2r", col_right, row2 + TITLE_H, DIAG_SZ, DIAG_SZ));

        // Row 1: TD values (left) | PD values (right)
        svg.push_str(&draw_td(data, col_left,  row1, "cl1l"));
        svg.push_str(&draw_pd(data, col_right, row1, "cl1r"));

        // Row 2: TD Probability (left) | PD Probability (right)
        svg.push_str(&draw_td_prob(data, col_left,  row2, "cl2l"));
        svg.push_str(&draw_pd_prob(data, col_right, row2, "cl2r"));
    }

    defs.push_str("</defs>");

    // Stats column: spans all diagram rows
    let stats_h = rows as f32 * panel_h() + (rows - 1) as f32 * GAP;
    svg.push_str(&draw_stats(data, col_stats, row0, STATS_W, stats_h));

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{svg_w}" height="{h}">{defs}{svg}</svg>"#
    )
}

/// Render an ophthalmic VF DICOM object as a rich diagram image.
pub fn render(obj: &InMemDicomObject) -> Vec<DynamicImage> {
    let data = extract_data(obj);
    if data.points.is_empty() {
        return vec![];
    }
    svg_to_image(&build_svg(&data))
        .map(|img| vec![img])
        .unwrap_or_default()
}
