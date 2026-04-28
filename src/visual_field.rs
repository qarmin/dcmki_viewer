use dicom::{
    core::{Tag, value::Value},
    object::InMemDicomObject,
};
use dicom_dictionary_std::tags::{
    SENSITIVITY_VALUE, STIMULUS_RESULTS, VISUAL_FIELD_TEST_POINT_SEQUENCE, VISUAL_FIELD_TEST_POINT_X_COORDINATE,
    VISUAL_FIELD_TEST_POINT_Y_COORDINATE,
};
use image::{DynamicImage, Rgb, RgbImage};

// Canvas
const W: u32 = 720;
const H: u32 = 720;
const CX: i32 = 360;
const CY: i32 = 360;
const PPD: f32 = 9.0; // pixels per degree

// Colors
const BG: Rgb<u8> = Rgb([26, 27, 38]);
const AXIS: Rgb<u8> = Rgb([122, 162, 247]);
const TICK: Rgb<u8> = Rgb([65, 72, 104]);
const LABEL: Rgb<u8> = Rgb([169, 177, 214]);
const VAL_SEEN: Rgb<u8> = Rgb([158, 206, 106]);
const VAL_UNSEEN: Rgb<u8> = Rgb([255, 85, 85]);

// 5×7 bitmap font: indices 0–9 = digits, 10 = '-', 11 = ' '
// Each row is a u8; bit 4 = leftmost column, bit 0 = rightmost.
const FONT: [[u8; 7]; 12] = [
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110], // 0
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // 1
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111], // 2
    [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110], // 3
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010], // 4
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110], // 5
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110], // 6
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000], // 7
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110], // 8
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100], // 9
    [0b00000, 0b00000, 0b00000, 0b01110, 0b00000, 0b00000, 0b00000], // -
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000], // (space)
];
const FONT_W: i32 = 5;
const FONT_H: i32 = 7;
const FONT_SCALE: i32 = 2;

//  drawing helpers

fn put_pixel(img: &mut RgbImage, x: i32, y: i32, c: Rgb<u8>) {
    if x >= 0 && y >= 0 && (x as u32) < W && (y as u32) < H {
        img.put_pixel(x as u32, y as u32, c);
    }
}

fn draw_hline(img: &mut RgbImage, x0: i32, x1: i32, y: i32, c: Rgb<u8>) {
    let (a, b) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    for x in a..=b {
        put_pixel(img, x, y, c);
    }
}

fn draw_vline(img: &mut RgbImage, x: i32, y0: i32, y1: i32, c: Rgb<u8>) {
    let (a, b) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
    for y in a..=b {
        put_pixel(img, x, y, c);
    }
}

fn draw_char(img: &mut RgbImage, ox: i32, oy: i32, c: char, color: Rgb<u8>) {
    let idx = match c {
        '0'..='9' => (c as u8 - b'0') as usize,
        '-' => 10,
        _ => 11,
    };
    let bitmap = &FONT[idx];
    #[expect(clippy::needless_range_loop)]
    for row in 0..FONT_H as usize {
        for col in 0..FONT_W as usize {
            let bit = (bitmap[row] >> (FONT_W - 1 - col as i32)) & 1;
            if bit == 1 {
                for sy in 0..FONT_SCALE {
                    for sx in 0..FONT_SCALE {
                        put_pixel(
                            img,
                            ox + col as i32 * FONT_SCALE + sx,
                            oy + row as i32 * FONT_SCALE + sy,
                            color,
                        );
                    }
                }
            }
        }
    }
}

fn text_pixel_width(s: &str) -> i32 {
    s.chars().count() as i32 * (FONT_W + 1) * FONT_SCALE
}

/// Draw text centred on (cx, cy).
fn draw_text_centered(img: &mut RgbImage, cx: i32, cy: i32, s: &str, color: Rgb<u8>) {
    let w = text_pixel_width(s);
    let h = FONT_H * FONT_SCALE;
    let mut xpos = cx - w / 2;
    let y = cy - h / 2;
    for c in s.chars() {
        draw_char(img, xpos, y, c, color);
        xpos += (FONT_W + 1) * FONT_SCALE;
    }
}

/// Draw text with its left edge at (x, cy) vertically centred.
fn draw_text_left(img: &mut RgbImage, x: i32, cy: i32, s: &str, color: Rgb<u8>) {
    let h = FONT_H * FONT_SCALE;
    let y = cy - h / 2;
    let mut xpos = x;
    for c in s.chars() {
        draw_char(img, xpos, y, c, color);
        xpos += (FONT_W + 1) * FONT_SCALE;
    }
}

//  DICOM data extraction

struct VfPoint {
    x: f32,
    y: f32,
    sensitivity: Option<f32>,
    seen: bool,
}

fn read_fl(item: &InMemDicomObject, tag: Tag) -> Option<f32> {
    item.get(tag)
        .and_then(|e| e.value().to_str().ok().map(|s| s.into_owned()))
        .and_then(|s| s.trim().parse::<f32>().ok())
}

fn extract_vf_points(obj: &InMemDicomObject) -> Vec<VfPoint> {
    let Some(seq_elem) = obj.get(VISUAL_FIELD_TEST_POINT_SEQUENCE) else {
        return vec![];
    };
    let Value::Sequence(seq) = seq_elem.value() else {
        return vec![];
    };

    seq.items()
        .iter()
        .filter_map(|item| {
            let x = read_fl(item, VISUAL_FIELD_TEST_POINT_X_COORDINATE)?;
            let y = read_fl(item, VISUAL_FIELD_TEST_POINT_Y_COORDINATE)?;
            let sensitivity = read_fl(item, SENSITIVITY_VALUE);
            let seen = item
                .get(STIMULUS_RESULTS)
                .and_then(|e| e.value().to_str().ok().map(|s| s.into_owned()))
                .is_none_or(|s| s.trim() != "NOT SEEN");
            Some(VfPoint {
                x,
                y,
                sensitivity,
                seen,
            })
        })
        .collect()
}

//  chart rendering

fn draw_vf_chart(points: &[VfPoint]) -> RgbImage {
    let mut img = RgbImage::from_pixel(W, H, BG);

    // Full-width horizontal axis and full-height vertical axis
    draw_hline(&mut img, 0, W as i32 - 1, CY, AXIS);
    draw_vline(&mut img, CX, 0, H as i32 - 1, AXIS);

    let tick = 8i32;
    let gap = 5i32; // pixels between tick end and label

    // Ticks and labels at ±10, ±20, ±30 degrees on both axes
    for &deg in &[10i32, 20, 30] {
        let label_pos = format!("{deg}");
        let label_neg = format!("-{deg}");

        //  Horizontal axis
        for &sign in &[1i32, -1] {
            let px = CX + sign * (deg as f32 * PPD) as i32;
            draw_vline(&mut img, px, CY - tick, CY + tick, TICK);
            let label = if sign > 0 { &label_pos } else { &label_neg };
            draw_text_centered(&mut img, px, CY + tick + gap + FONT_H * FONT_SCALE / 2, label, LABEL);
        }

        //  Vertical axis
        for &sign in &[1i32, -1] {
            // positive Y = up in visual field = up in image (smaller y)
            let py = CY - sign * (deg as f32 * PPD) as i32;
            draw_hline(&mut img, CX - tick, CX + tick, py, TICK);
            let label = if sign > 0 { &label_pos } else { &label_neg };
            draw_text_left(&mut img, CX + tick + gap, py, label, LABEL);
        }
    }

    // Small fixation cross at centre
    let fix = 5i32;
    for d in -fix..=fix {
        if d != 0 {
            put_pixel(&mut img, CX + d, CY, LABEL);
            put_pixel(&mut img, CX, CY + d, LABEL);
        }
    }

    // Test points
    for pt in points {
        let px = CX + (pt.x * PPD) as i32;
        let py = CY - (pt.y * PPD) as i32; // invert Y (superior = up)

        if !pt.seen {
            // Solid red square for unseen / not-seen points
            for dy in -4..=4i32 {
                for dx in -4..=4i32 {
                    put_pixel(&mut img, px + dx, py + dy, VAL_UNSEEN);
                }
            }
        } else if let Some(sens) = pt.sensitivity {
            let text = format!("{sens:.0}");
            draw_text_centered(&mut img, px, py, &text, VAL_SEEN);
        }
    }

    img
}

//  public API

pub fn render(obj: &InMemDicomObject) -> Vec<DynamicImage> {
    let points = extract_vf_points(obj);
    if points.is_empty() {
        return vec![];
    }
    vec![DynamicImage::ImageRgb8(draw_vf_chart(&points))]
}
