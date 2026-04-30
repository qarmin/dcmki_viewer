use crate::visual_field::{
    data::{DicomVfData, TestStrategy, VfPoint},
    layout::{cell_radius, vf_to_px, DIAG_SZ, TITLE_H, MAR},
    patterns::hvf_gray,
    svg_helpers::{group_clip, line, rect, txt},
};

#[derive(Clone, Copy)]
enum SupraStatus {
    NoDefect,       // ○  white  — seen at normal sensitivity
    RelativeDefect, // X  gray   — seen only at max stimulus (sensitivity ≈ 0)
    PossibleDefect, // ■  dark   — not seen, TwoZone (can't rule out absolute)
    AbsoluteDefect, // ■  black  — not seen, ThreeZone / QuantifyDefects
}

fn classify_suprathreshold(pt: &VfPoint, strategy: TestStrategy) -> SupraStatus {
    if !pt.seen {
        if strategy == TestStrategy::TwoZone {
            SupraStatus::PossibleDefect
        } else {
            SupraStatus::AbsoluteDefect
        }
    } else if pt.sensitivity.is_some_and(|s| s.abs() < 0.5) {
        // sensitivity == 0 dB: seen at maximum stimulus → relative defect
        SupraStatus::RelativeDefect
    } else {
        SupraStatus::NoDefect
    }
}


/// Draw a suprathreshold point in the threshold panel.
/// Returns SVG for the symbol: circle/X as text, solid rect for black squares.
fn supra_threshold_cell(ppx: f32, ppy: f32, cr: f32, fs: f32, status: SupraStatus, is_blind_spot: bool) -> String {
    if is_blind_spot {
        return txt(ppx, ppy, "△", TEXT_DIM, fs, "middle", "normal");
    }
    match status {
        SupraStatus::NoDefect       => txt(ppx, ppy, "○", TEXT_PRIMARY, fs, "middle", "normal"),
        SupraStatus::RelativeDefect => txt(ppx, ppy, "X", ORANGE, fs, "middle", "bold"),
        SupraStatus::PossibleDefect => rect(ppx - cr * 0.7, ppy - cr * 0.7, cr * 1.4, cr * 1.4, "#555555", 0.0),
        SupraStatus::AbsoluteDefect => rect(ppx - cr * 0.7, ppy - cr * 0.7, cr * 1.4, cr * 1.4, "#000000", 0.0),
    }
}

// Color palette
pub const BG: &str = "#1a1b26";
pub const PANEL_BG: &str = "#24283b";
pub const PANEL_TITLE_BG: &str = "#1e2235";
pub const HEADER_BG: &str = "#16213e";
pub const BORDER: &str = "#7aa2f7";
pub const TEXT_PRIMARY: &str = "#c0caf5";
pub const TEXT_DIM: &str = "#565f89";
pub const TEXT_LABEL: &str = "#7aa2f7";
pub const GREEN: &str = "#9ece6a";
pub const RED: &str = "#f7768e";
pub const ORANGE: &str = "#e0af68";
pub const CYAN: &str = "#7dcfff";

pub fn draw_header(data: &DicomVfData, svg_w: f32) -> String {
    let mut s = String::new();
    s.push_str(&rect(0.0, 0.0, svg_w, 64.0, HEADER_BG, 0.0));
    s.push_str(&txt(14.0, 26.0, "Visual Field Analysis", CYAN, 16.0, "start", "bold"));

    // Second line: device info + series description
    let mut sub_parts: Vec<&str> = Vec::new();
    let device_label = match (&data.manufacturer, &data.model_name) {
        (Some(m), Some(model)) => { let l = format!("{m} {model}"); sub_parts.push(""); l }
        (Some(m), None) => { sub_parts.push(""); m.clone() }
        (None, Some(model)) => { sub_parts.push(""); model.clone() }
        _ => String::new(),
    };
    // Fix up: replace placeholder with actual ref
    if !device_label.is_empty() {
        sub_parts.clear();
        sub_parts.push(&device_label);
    }
    if let Some(desc) = &data.series_description {
        sub_parts.push(desc);
    }
    let sub_line = sub_parts.join(" | ");
    if !sub_line.is_empty() {
        s.push_str(&txt(14.0, 48.0, &sub_line, TEXT_LABEL, 10.0, "start", "normal"));
    } else {
        s.push_str(&txt(14.0, 48.0, "Ophthalmic Perimetry", TEXT_LABEL, 10.0, "start", "normal"));
    }

    // Center: patient name + demographics
    let mut name_line = data.patient_name.clone();
    let mut demo_parts: Vec<String> = Vec::new();
    if let Some(age) = &data.patient_age {
        demo_parts.push(format!("Age: {age}"));
    }
    if let Some(sex) = &data.patient_sex {
        demo_parts.push(sex.clone());
    }
    if !demo_parts.is_empty() {
        name_line = format!("{} ({})", name_line, demo_parts.join(", "));
    }
    s.push_str(&txt(svg_w / 2.0, 26.0, &name_line, TEXT_PRIMARY, 16.0, "middle", "bold"));

    // Date + DOB
    let mut date_parts: Vec<String> = Vec::new();
    if !data.study_date.is_empty() {
        date_parts.push(format!("Study: {}", data.study_date));
    }
    if let Some(dob) = &data.patient_birth_date {
        date_parts.push(format!("DOB: {dob}"));
    }
    if !date_parts.is_empty() {
        s.push_str(&txt(
            svg_w / 2.0, 48.0,
            &date_parts.join("  |  "),
            TEXT_DIM, 10.0, "middle", "normal",
        ));
    }

    let lat = data.laterality.to_uppercase();
    let eye_label = match lat.as_str() {
        "R" | "RIGHT" | "OD" => "OD  Right Eye",
        "L" | "LEFT" | "OS" => "OS  Left Eye",
        other if !other.is_empty() => other,
        _ => "",
    };
    if !eye_label.is_empty() {
        let bx = svg_w - 140.0;
        let bc = if eye_label.contains("Right") { "#1a3a8a" } else { "#8a2a1a" };
        s.push_str(&rect(bx, 14.0, 132.0, 36.0, bc, 6.0));
        s.push_str(&txt(bx + 66.0, 32.0, eye_label, "#ffffff", 11.0, "middle", "bold"));
    }

    s.push_str(&line(0.0, 64.0, svg_w, 64.0, BORDER, 0.8));
    s
}

// Shared: draw grid (crosshair + ticks) inside the diagram area
fn draw_grid(extent: f32, px: f32, inner_py: f32) -> String {
    let sz = DIAG_SZ;
    let (cx, cy) = vf_to_px(0.0, 0.0, px, inner_py, sz, extent);
    let mut s = String::new();

    // Center crosshair
    s.push_str(&line(px + MAR, cy, px + sz - MAR, cy, TEXT_DIM, 0.7));
    s.push_str(&line(cx, inner_py + MAR, cx, inner_py + sz - MAR, TEXT_DIM, 0.7));

    // Tick marks every 10 degrees
    let ticks: &[f32] = if extent <= 12.0 {
        &[-10.0, -5.0, 5.0, 10.0]
    } else {
        &[-30.0, -20.0, -10.0, 10.0, 20.0, 30.0]
    };
    for &deg in ticks {
        let (tx, _) = vf_to_px(deg, 0.0, px, inner_py, sz, extent);
        s.push_str(&line(tx, cy - 4.0, tx, cy + 4.0, TEXT_DIM, 0.5));
        let (_, ty) = vf_to_px(0.0, deg, px, inner_py, sz, extent);
        s.push_str(&line(cx - 4.0, ty, cx + 4.0, ty, TEXT_DIM, 0.5));
    }
    s
}

fn panel_title(px: f32, py: f32, label: &str) -> String {
    let mut s = rect(px, py, DIAG_SZ, TITLE_H, PANEL_TITLE_BG, 4.0);
    s.push_str(&txt(px + DIAG_SZ / 2.0, py + TITLE_H / 2.0, label, TEXT_LABEL, 10.5, "middle", "bold"));
    s
}

// Helper: total panel height = DIAG_SZ (diagram) + TITLE_H (header strip)
pub fn panel_h() -> f32 {
    DIAG_SZ + TITLE_H
}

// Panel 1: Threshold (dB) - plain numbers for threshold exams, symbols for suprathreshold.
pub fn draw_threshold(data: &DicomVfData, px: f32, py: f32, clip_id: &str) -> String {
    let extent = data.horizontal_extent;
    let inner_py = py + TITLE_H;
    let cr = cell_radius(extent);
    let fs = (cr * 1.15).clamp(8.0, 14.0);

    let mut body = String::new();
    body.push_str(&rect(px, inner_py, DIAG_SZ, DIAG_SZ, PANEL_BG, 0.0));
    body.push_str(&draw_grid(extent, px, inner_py));

    if data.test_strategy.is_suprathreshold() {
        for pt in &data.points {
            let (ppx, ppy) = vf_to_px(pt.x, pt.y, px, inner_py, DIAG_SZ, extent);
            let status = classify_suprathreshold(pt, data.test_strategy);
            body.push_str(&supra_threshold_cell(ppx, ppy, cr, fs, status, pt.is_blind_spot));
        }
    } else {
        for pt in &data.points {
            let (ppx, ppy) = vf_to_px(pt.x, pt.y, px, inner_py, DIAG_SZ, extent);
            if pt.is_blind_spot {
                body.push_str(&txt(ppx, ppy, "△", TEXT_DIM, fs, "middle", "normal"));
            } else if !pt.seen {
                body.push_str(&txt(ppx, ppy, "<0", RED, fs, "middle", "bold"));
            } else if let Some(s) = pt.sensitivity {
                body.push_str(&txt(ppx, ppy, &format!("{s:.0}"), TEXT_PRIMARY, fs, "middle", "normal"));
            }
        }
    }

    let mut s = rect(px, py, DIAG_SZ, panel_h(), PANEL_BG, 4.0);
    s.push_str(&panel_title(px, py, "Threshold (dB)"));
    s.push_str(&group_clip(clip_id, &body));
    s
}

// Panel 2: HVF Grayscale - filled squares with continuous shading (threshold only).
pub fn draw_grayscale(data: &DicomVfData, px: f32, py: f32, clip_id: &str) -> String {
    let extent = data.horizontal_extent;
    let inner_py = py + TITLE_H;
    let cr = cell_radius(extent);

    let mut body = String::new();
    body.push_str(&rect(px, inner_py, DIAG_SZ, DIAG_SZ, PANEL_BG, 0.0));

    for pt in &data.points {
        let (ppx, ppy) = vf_to_px(pt.x, pt.y, px, inner_py, DIAG_SZ, extent);
        if pt.is_blind_spot {
            body.push_str(&txt(ppx, ppy, "△", TEXT_DIM, (cr * 1.4).clamp(7.0, 13.0), "middle", "normal"));
            continue;
        }
        let fill = hvf_gray(pt.sensitivity, pt.seen);
        body.push_str(&rect(ppx - cr, ppy - cr, cr * 2.0, cr * 2.0, &fill, 0.0));
    }
    body.push_str(&draw_grid(extent, px, inner_py));

    let mut s = rect(px, py, DIAG_SZ, panel_h(), PANEL_BG, 4.0);
    s.push_str(&panel_title(px, py, "Grayscale"));
    s.push_str(&group_clip(clip_id, &body));
    s
}

// Panel 3: Total Deviation (dB) - signed numbers, color coded
pub fn draw_td(data: &DicomVfData, px: f32, py: f32, clip_id: &str) -> String {
    let extent = data.horizontal_extent;
    let inner_py = py + TITLE_H;
    let cr = cell_radius(extent);
    let fs = (cr * 1.05).clamp(7.5, 13.0);

    let mut body = String::new();
    body.push_str(&rect(px, inner_py, DIAG_SZ, DIAG_SZ, PANEL_BG, 0.0));

    let has_td = data.points.iter().any(|p| p.td.is_some());
    if has_td {
        body.push_str(&draw_grid(extent, px, inner_py));
        for pt in &data.points {
            if let Some(td) = pt.td {
                let (ppx, ppy) = vf_to_px(pt.x, pt.y, px, inner_py, DIAG_SZ, extent);
                let col = if td < -4.0 { RED } else if td < -2.0 { ORANGE } else { GREEN };
                body.push_str(&txt(ppx, ppy, &format!("{td:+.0}"), col, fs, "middle", "bold"));
            }
        }
    } else {
        body.push_str(&txt(px + DIAG_SZ / 2.0, inner_py + DIAG_SZ / 2.0, "No TD data", TEXT_DIM, 11.0, "middle", "normal"));
    }

    let mut s = rect(px, py, DIAG_SZ, panel_h(), PANEL_BG, 4.0);
    s.push_str(&panel_title(px, py, "Total Deviation (dB)"));
    s.push_str(&group_clip(clip_id, &body));
    s
}

// Panel 4: Pattern Deviation (dB) - signed numbers, color coded
pub fn draw_pd(data: &DicomVfData, px: f32, py: f32, clip_id: &str) -> String {
    let extent = data.horizontal_extent;
    let inner_py = py + TITLE_H;
    let cr = cell_radius(extent);
    let fs = (cr * 1.05).clamp(7.5, 13.0);

    let mut body = String::new();
    body.push_str(&rect(px, inner_py, DIAG_SZ, DIAG_SZ, PANEL_BG, 0.0));

    let has_pd = data.points.iter().any(|p| p.pd.is_some());
    if has_pd {
        body.push_str(&draw_grid(extent, px, inner_py));
        for pt in &data.points {
            if let Some(pd) = pt.pd {
                let (ppx, ppy) = vf_to_px(pt.x, pt.y, px, inner_py, DIAG_SZ, extent);
                let col = if pd < -4.0 { RED } else if pd < -2.0 { ORANGE } else { GREEN };
                body.push_str(&txt(ppx, ppy, &format!("{pd:+.0}"), col, fs, "middle", "bold"));
            }
        }
    } else {
        body.push_str(&txt(px + DIAG_SZ / 2.0, inner_py + DIAG_SZ / 2.0, "No PD data", TEXT_DIM, 11.0, "middle", "normal"));
    }

    let mut s = rect(px, py, DIAG_SZ, panel_h(), PANEL_BG, 4.0);
    s.push_str(&panel_title(px, py, "Pattern Deviation (dB)"));
    s.push_str(&group_clip(clip_id, &body));
    s
}

// Map a probability p-value to a grayscale fill color.
// DICOM stores probability as percentage (0-100 range).
// Normal (p >= 5%): white; lower p = darker shades; p < 0.5%: black.
fn prob_gray(p: Option<f32>) -> &'static str {
    match p {
        Some(p) if p <= 0.5 => "#000000",
        Some(p) if p <= 1.0 => "#333333",
        Some(p) if p <= 2.0 => "#666666",
        Some(p) if p <= 5.0 => "#aaaaaa",
        _                    => "#ffffff",
    }
}

// Shared internals for probability diagram panels
fn draw_prob_body(
    points: &[VfPoint],
    use_pd_p: bool,
    extent: f32,
    px: f32,
    inner_py: f32,
) -> String {
    let cr = cell_radius(extent);

    let mut body = String::new();
    body.push_str(&rect(px, inner_py, DIAG_SZ, DIAG_SZ, PANEL_BG, 0.0));

    for pt in points {
        let p_val = if use_pd_p { pt.pd_p } else { pt.td_p };
        let fill = prob_gray(p_val);
        let (ppx, ppy) = vf_to_px(pt.x, pt.y, px, inner_py, DIAG_SZ, extent);
        body.push_str(&rect(ppx - cr, ppy - cr, cr * 2.0, cr * 2.0, fill, 0.0));
    }

    body.push_str(&draw_grid(extent, px, inner_py));
    body
}

// Panel 5: TD Probability - grayscale squares (white=normal, black=p<0.5%)
pub fn draw_td_prob(data: &DicomVfData, px: f32, py: f32, clip_id: &str) -> String {
    let inner_py = py + TITLE_H;
    let body = draw_prob_body(&data.points, false, data.horizontal_extent, px, inner_py);

    let mut s = rect(px, py, DIAG_SZ, panel_h(), PANEL_BG, 4.0);
    s.push_str(&panel_title(px, py, "TD Probability"));
    s.push_str(&group_clip(clip_id, &body));
    s
}

// Panel 6: PD Probability - grayscale squares (white=normal, black=p<0.5%)
pub fn draw_pd_prob(data: &DicomVfData, px: f32, py: f32, clip_id: &str) -> String {
    let inner_py = py + TITLE_H;
    let body = draw_prob_body(&data.points, true, data.horizontal_extent, px, inner_py);

    let mut s = rect(px, py, DIAG_SZ, panel_h(), PANEL_BG, 4.0);
    s.push_str(&panel_title(px, py, "PD Probability"));
    s.push_str(&group_clip(clip_id, &body));
    s
}

// Statistics panel (right column, spans all 3 rows)
#[expect(clippy::allow_attributes, reason = "row! macro increments ry; last invocation unused")]
#[allow(unused_assignments)]
pub fn draw_stats(data: &DicomVfData, px: f32, py: f32, stats_w: f32, stats_h: f32) -> String {
    let mut s = rect(px, py, stats_w, stats_h, PANEL_BG, 4.0);
    s.push_str(&rect(px, py, stats_w, TITLE_H, PANEL_TITLE_BG, 4.0));
    s.push_str(&txt(px + stats_w / 2.0, py + TITLE_H / 2.0, "Statistics", TEXT_LABEL, 11.0, "middle", "bold"));

    let mut ry = py + TITLE_H + 18.0;
    let lh = 22.0;

    macro_rules! row {
        ($label:expr, $val:expr, $col:expr) => {{
            s.push_str(&line(px + 6.0, ry - 11.0, px + stats_w - 6.0, ry - 11.0, TEXT_DIM, 0.3));
            s.push_str(&txt(px + 10.0, ry, $label, TEXT_DIM, 10.5, "start", "normal"));
            s.push_str(&txt(px + stats_w - 10.0, ry, $val, $col, 10.5, "end", "bold"));
            ry += lh;
        }};
    }

    let md_s = data.md.map_or_else(|| "N/A".to_string(), |v| format!("{v:+.2} dB"));
    let md_c = match data.md {
        Some(v) if v < -3.0 => RED,
        Some(v) if v < -1.0 => ORANGE,
        Some(_) => GREEN,
        None => TEXT_DIM,
    };
    row!("MD", &md_s, md_c);

    let psd_s = data.psd.map_or_else(|| "N/A".to_string(), |v| format!("{v:.2} dB"));
    let psd_c = match data.psd {
        Some(v) if v > 4.0 => RED,
        Some(v) if v > 2.0 => ORANGE,
        Some(_) => GREEN,
        None => TEXT_DIM,
    };
    row!("PSD", &psd_s, psd_c);

    if let Some(ms) = data.mean_sensitivity {
        row!("MS", &format!("{ms:.1} dB"), TEXT_PRIMARY);
    }

    if let Some(f) = data.foveal_sensitivity {
        row!("Fovea", &format!("{f:.0} dB"), TEXT_PRIMARY);
    }

    if let Some(dur) = data.test_duration_s {
        let dur_s = format!("{}:{:02}", dur as u32 / 60, dur as u32 % 60);
        row!("Duration", &dur_s, TEXT_PRIMARY);
    }

    // Test parameters section
    let has_test_params = data.stimulus_size.is_some()
        || data.background_luminance.is_some()
        || data.stimuli_count.is_some();
    if has_test_params {
        ry += 6.0;
        s.push_str(&txt(px + stats_w / 2.0, ry, "Test Parameters", TEXT_LABEL, 10.0, "middle", "bold"));
        ry += lh;

        if let Some(size) = &data.stimulus_size {
            row!("Stimulus", &format!("Goldmann {size}"), TEXT_PRIMARY);
        }
        if let Some(bg) = data.background_luminance {
            row!("Background", &format!("{bg:.0} cd/m\u{b2}"), TEXT_PRIMARY);
        }
        if let Some(count) = data.stimuli_count {
            let retested = data.stimuli_retested.map_or(String::new(), |r| format!(" ({r} retested)"));
            row!("Stimuli", &format!("{count}{retested}"), TEXT_PRIMARY);
        }
    }

    // Reliability section
    ry += 6.0;
    s.push_str(&txt(px + stats_w / 2.0, ry, "Reliability", TEXT_LABEL, 10.0, "middle", "bold"));
    ry += lh;

    // Fixation loss with fraction
    let fix_s = match (data.fixation_lost, data.fixation_checked) {
        (Some(lost), Some(checked)) => {
            let flag = match data.fixation_loss {
                Some(true) => " EXCESSIVE",
                _ => "",
            };
            format!("{lost}/{checked}{flag}")
        }
        _ => match data.fixation_loss {
            Some(true) => "EXCESSIVE".into(),
            Some(false) => "OK".into(),
            None => "N/A".into(),
        },
    };
    let fix_c = match data.fixation_loss {
        Some(true) => RED,
        Some(false) => GREEN,
        None => TEXT_DIM,
    };
    row!("Fixation Loss", &fix_s, fix_c);

    // False Positives with fraction
    let fp_s = match (data.fp_quantity, data.fp_catch_trials) {
        (Some(q), Some(t)) => {
            let pct = data.false_pos.map(|v| format!(" ({:.0}%)", v * 100.0)).unwrap_or_default();
            format!("{q}/{t}{pct}")
        }
        _ => data
            .false_pos
            .map(|v| format!("{:.0}%", v * 100.0))
            .unwrap_or_else(|| match data.fp_flag {
                Some(true) => "EXCESSIVE".into(),
                Some(false) => "OK".into(),
                None => "N/A".into(),
            }),
    };
    let fp_c = match (data.false_pos, data.fp_flag) {
        (Some(v), _) if v > 0.25 => RED,
        (Some(v), _) if v > 0.15 => ORANGE,
        (Some(_), _) => GREEN,
        (_, Some(true)) => RED,
        (_, Some(false)) => GREEN,
        _ => TEXT_DIM,
    };
    row!("False Pos", &fp_s, fp_c);

    // False Negatives with fraction
    let fn_s = match (data.fn_quantity, data.fn_catch_trials) {
        (Some(q), Some(t)) => {
            let pct = data.false_neg.map(|v| format!(" ({:.0}%)", v * 100.0)).unwrap_or_default();
            format!("{q}/{t}{pct}")
        }
        _ => data
            .false_neg
            .map(|v| format!("{:.0}%", v * 100.0))
            .unwrap_or_else(|| match data.fn_flag {
                Some(true) => "EXCESSIVE".into(),
                Some(false) => "OK".into(),
                None => "N/A".into(),
            }),
    };
    let fn_c = match (data.false_neg, data.fn_flag) {
        (Some(v), _) if v > 0.33 => RED,
        (Some(v), _) if v > 0.20 => ORANGE,
        (Some(_), _) => GREEN,
        (_, Some(true)) => RED,
        (_, Some(false)) => GREEN,
        _ => TEXT_DIM,
    };
    row!("False Neg", &fn_s, fn_c);

    if let Some(fix_method) = &data.fixation_method {
        row!("Fixation", fix_method.as_str(), TEXT_PRIMARY);
    }

    s
}



