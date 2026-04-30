// SVG primitive helpers - keep short names since they are used many times.

pub fn rect(x: f32, y: f32, w: f32, h: f32, fill: &str, rx: f32) -> String {
    format!(
        r#"<rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" fill="{fill}" rx="{rx:.1}"/>"#
    )
}


pub fn txt(x: f32, y: f32, text: &str, fill: &str, size: f32, anchor: &str, weight: &str) -> String {
    let esc = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;");
    format!(
        r#"<text x="{x:.1}" y="{y:.1}" fill="{fill}" font-size="{size}" font-family="DejaVu Sans,Liberation Sans,Arial,sans-serif" text-anchor="{anchor}" font-weight="{weight}" dominant-baseline="middle">{esc}</text>"#
    )
}

pub fn line(x1: f32, y1: f32, x2: f32, y2: f32, stroke: &str, sw: f32) -> String {
    format!(
        r#"<line x1="{x1:.1}" y1="{y1:.1}" x2="{x2:.1}" y2="{y2:.1}" stroke="{stroke}" stroke-width="{sw}"/>"#
    )
}

pub fn clip_path(id: &str, x: f32, y: f32, w: f32, h: f32) -> String {
    format!(
        r#"<clipPath id="{id}"><rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}"/></clipPath>"#
    )
}

pub fn group_clip(id: &str, inner: &str) -> String {
    format!(r#"<g clip-path="url(#{id})">{inner}</g>"#)
}

