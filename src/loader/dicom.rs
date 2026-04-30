use std::path::Path;

use dicom::{
    core::{
        VR,
        dictionary::{DataDictionary, UidDictionary},
        value::{PrimitiveValue, Value},
    },
    object::{InMemDicomObject, open_file},
};
#[expect(deprecated)]
use dicom_dictionary_std::uids::{
    DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN, EXPLICIT_VR_BIG_ENDIAN, EXPLICIT_VR_LITTLE_ENDIAN, IMPLICIT_VR_LITTLE_ENDIAN,
    JPEG_BASELINE8_BIT, JPEG_EXTENDED12_BIT, JPEG_LOSSLESS, JPEG_LOSSLESS_SV1, JPEG2000, JPEG2000_LOSSLESS, JPEG2000MC,
    JPEG2000MC_LOSSLESS, JPEGLS_LOSSLESS, JPEGLS_NEAR_LOSSLESS, MPEG2MPHL, MPEG2MPML, MPEG4HP41, MPEG4HP41BD,
    OPHTHALMIC_VISUAL_FIELD_STATIC_PERIMETRY_MEASUREMENTS_STORAGE, RLE_LOSSLESS,
};
use dicom_dictionary_std::{
    StandardDataDictionary, StandardSopClassDictionary,
    tags::{BITS_ALLOCATED, COLUMNS, ENCAPSULATED_DOCUMENT, NUMBER_OF_FRAMES, ROWS, SAMPLES_PER_PIXEL, SOP_CLASS_UID},
};
use dicom_pixeldata::PixelDecoder;
use hayro::hayro_syntax::Pdf;
use image::DynamicImage;
use log::error;

use super::{
    pdf::render_pages,
    types::{FileData, LazyPixelDecoder, TagEntry},
};
use crate::{error::AppError, visual_field};

/// Files larger than this are decoded lazily (one frame at a time).
const LAZY_THRESHOLD_BYTES: u64 = 50 * 1024 * 1024; // 50 MB

fn resolve_uid(uid: &str) -> String {
    let uid = uid.trim();
    if uid.is_empty() {
        return uid.to_string();
    }
    if let Some(entry) = StandardSopClassDictionary.by_uid(uid) {
        return format!("{uid} ({})", entry.name);
    }
    #[expect(deprecated)]
    let ts: Option<&str> = match uid {
        IMPLICIT_VR_LITTLE_ENDIAN => Some("Implicit VR Little Endian"),
        EXPLICIT_VR_LITTLE_ENDIAN => Some("Explicit VR Little Endian"),
        DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN => Some("Deflated Explicit VR Little Endian"),
        EXPLICIT_VR_BIG_ENDIAN => Some("Explicit VR Big Endian"),
        JPEG_BASELINE8_BIT => Some("JPEG Baseline (Process 1)"),
        JPEG_EXTENDED12_BIT => Some("JPEG Extended (Process 2 & 4)"),
        JPEG_LOSSLESS => Some("JPEG Lossless (Process 14)"),
        JPEG_LOSSLESS_SV1 => Some("JPEG Lossless (Process 14, SV1)"),
        JPEGLS_LOSSLESS => Some("JPEG-LS Lossless"),
        JPEGLS_NEAR_LOSSLESS => Some("JPEG-LS Near-Lossless"),
        JPEG2000_LOSSLESS => Some("JPEG 2000 Lossless"),
        JPEG2000 => Some("JPEG 2000"),
        JPEG2000MC_LOSSLESS => Some("JPEG 2000 Part 2 Lossless"),
        JPEG2000MC => Some("JPEG 2000 Part 2"),
        RLE_LOSSLESS => Some("RLE Lossless"),
        MPEG2MPML => Some("MPEG2 Main Profile / Main Level"),
        MPEG2MPHL => Some("MPEG2 Main Profile / High Level"),
        MPEG4HP41 => Some("MPEG-4 AVC/H.264 High Profile"),
        MPEG4HP41BD => Some("MPEG-4 AVC/H.264 BD-compatible"),
        _ => None,
    };
    match ts {
        Some(name) => format!("{uid} ({name})"),
        None => uid.to_string(),
    }
}

fn image_color_str(img: &DynamicImage) -> &'static str {
    use image::ColorType::{L8, L16, La8, La16, Rgb8, Rgb16, Rgb32F, Rgba8, Rgba16, Rgba32F};
    match img.color() {
        L8 | L16 => "Grayscale",
        La8 | La16 => "Grayscale+A",
        Rgb8 | Rgb16 | Rgb32F => "RGB",
        Rgba8 | Rgba16 | Rgba32F => "RGBA",
        _ => "Color",
    }
}

pub fn get_storage_sop_class_name(sop_class_uid: &str) -> String {
    StandardSopClassDictionary
        .by_uid(sop_class_uid)
        .map(|e| e.name.to_string())
        .unwrap_or(format!("<unknown name {sop_class_uid}>"))
}

fn read_u32_tag(obj: &InMemDicomObject, tag: dicom::core::Tag) -> Option<u32> {
    obj.get(tag)
        .and_then(|e| e.value().to_str().ok().map(|s| s.trim().to_string()))
        .and_then(|s| s.parse::<u32>().ok())
}

/// Build a human-readable summary for a Pixel Data element.
fn pixel_data_summary(obj: &InMemDicomObject, value: &Value<InMemDicomObject>) -> String {
    let rows = read_u32_tag(obj, ROWS).unwrap_or(0);
    let cols = read_u32_tag(obj, COLUMNS).unwrap_or(0);
    let frames = read_u32_tag(obj, NUMBER_OF_FRAMES).unwrap_or(1);
    let bits = read_u32_tag(obj, BITS_ALLOCATED).unwrap_or(8);
    let samples = read_u32_tag(obj, SAMPLES_PER_PIXEL).unwrap_or(1);

    let dim_str = if rows > 0 && cols > 0 {
        format!("{cols}×{rows}")
    } else {
        String::new()
    };

    match value {
        Value::PixelSequence(ps) => {
            let n = ps.fragments().len();
            let total: usize = ps.fragments().iter().map(|f| f.len()).sum();
            let size_str = super::format_bytes(total as u64);
            let mut parts = vec![];
            if !dim_str.is_empty() { parts.push(dim_str); }
            if frames > 1 { parts.push(format!("{frames} frames")); }
            parts.push(format!("{n} fragments"));
            parts.push(format!("{size_str} compressed"));
            format!("[Pixel Data: {}]", parts.join(", "))
        }
        Value::Primitive(_) => {
            let uncompressed = (rows as u64) * (cols as u64) * (frames as u64)
                * (bits as u64 / 8).max(1)
                * (samples as u64);
            let size_str = super::format_bytes(uncompressed);
            let mut parts = vec![];
            if !dim_str.is_empty() { parts.push(dim_str); }
            if frames > 1 { parts.push(format!("{frames} frames")); }
            parts.push(size_str);
            format!("[Pixel Data: {}]", parts.join(", "))
        }
        _ => "[Pixel Data]".to_string(),
    }
}

pub(super) fn flatten_obj(obj: &InMemDicomObject, dict: StandardDataDictionary, depth: u32, out: &mut Vec<TagEntry>) {
    for element in obj.iter() {
        let tag = element.header().tag;
        let vr = element.header().vr;
        let is_private = tag.group() % 2 == 1;
        let tag_str = format!("({:04X},{:04X})", tag.group(), tag.element());
        let name = dict
            .by_tag(tag)
            .map_or_else(|| "Unknown".to_string(), |e| e.alias.to_string());

        let vr_str = format!("{vr}");
        match element.value() {
            Value::Sequence(seq) => {
                let n = seq.items().len();
                let seq_val = format!("[Sequence: {n} items]");
                out.push(TagEntry {
                    tag: tag_str,
                    name,
                    vr: vr_str,
                    value: seq_val.clone(),
                    value_full: seq_val,
                    is_private,
                    depth,
                    is_item_header: false,
                    is_sequence: true,
                });
                for (idx, item) in seq.items().iter().enumerate() {
                    out.push(TagEntry {
                        tag: String::new(),
                        name: format!("Item {}", idx + 1),
                        vr: String::new(),
                        value: String::new(),
                        value_full: String::new(),
                        is_private: false,
                        depth: depth + 1,
                        is_item_header: true,
                        is_sequence: false,
                    });
                    flatten_obj(item, dict, depth + 2, out);
                }
            }
            v @ Value::PixelSequence(_) => {
                let summary = pixel_data_summary(obj, v);
                out.push(TagEntry {
                    tag: tag_str,
                    name,
                    vr: vr_str,
                    value: summary.clone(),
                    value_full: summary,
                    is_private,
                    depth,
                    is_item_header: false,
                    is_sequence: false,
                });
            }
            Value::Primitive(_v) => {
                // Check if this is unencapsulated pixel data (OB/OW with large size)
                let is_pixel_data = (vr == VR::OB || vr == VR::OW || vr == VR::UN)
                    && tag == dicom_dictionary_std::tags::PIXEL_DATA;

                if is_pixel_data {
                    let summary = pixel_data_summary(obj, element.value());
                    out.push(TagEntry {
                        tag: tag_str,
                        name,
                        vr: vr_str,
                        value: summary.clone(),
                        value_full: summary,
                        is_private,
                        depth,
                        is_item_header: false,
                        is_sequence: false,
                    });
                } else {
                    let raw = match element.value().to_str() {
                        Ok(s) => s.into_owned(),
                        Err(_) => match element.value().to_bytes() {
                            Ok(b) => format!("[binary: {} B]", b.len()),
                            Err(_) => "[binary]".to_string(),
                        },
                    };
                    let raw = raw.replace(['\r', '\n'], " ");
                    let (value, value_full) = if vr == VR::UI {
                        let resolved = resolve_uid(raw.trim());
                        (resolved.clone(), resolved)
                    } else {
                        (super::truncate(raw.clone(), 100), raw)
                    };
                    out.push(TagEntry {
                        tag: tag_str,
                        name,
                        vr: vr_str,
                        value,
                        value_full,
                        is_private,
                        depth,
                        is_item_header: false,
                        is_sequence: false,
                    });
                }
            }
        }
    }
}

pub(super) fn load_dicom(path: &Path, file_size: u64, size_str: &str) -> Result<FileData, AppError> {
    let mut obj = open_file(path).map_err(|e| AppError::Dicom(e.to_string()))?;

    let dict = StandardDataDictionary;
    let mut tags: Vec<TagEntry> = Vec::new();
    flatten_obj(&obj, dict, 0, &mut tags);

    let sop_uid = obj
        .get(SOP_CLASS_UID)
        .and_then(|el| el.value().to_str().ok().map(|s| s.into_owned()))
        .unwrap_or_default();

    let sop_class = if sop_uid.is_empty() {
        "DICOM".to_string()
    } else {
        get_storage_sop_class_name(sop_uid.trim())
    };

    let enc_pdf = obj.take(ENCAPSULATED_DOCUMENT)
        .and_then(|el| match el.into_value() {
            Value::Primitive(PrimitiveValue::U8(v)) => Some(v.into_vec()),
            _ => None,
        })
        .filter(|b| b.starts_with(b"%PDF"))
        .and_then(|bytes| {
            Pdf::new(bytes)
                .map_err(|e| error!("Warning: encapsulated PDF parse: {e:?}"))
                .ok()
        });

    const VF_SOP: &str = OPHTHALMIC_VISUAL_FIELD_STATIC_PERIMETRY_MEASUREMENTS_STORAGE;

    if let Some(pdf) = enc_pdf {
        // PDF: pre-decode all pages (small)
        let frames = render_pages(&pdf);
        return Ok(FileData {
            frames,
            lazy_decoder: None,
            tags,
            sop_class,
            image_info: String::new(),
        });
    }

    if sop_uid.trim() == VF_SOP {
        // Visual field: pre-rendered SVG diagram (small)
        let frames = visual_field::render(&obj);
        return Ok(FileData {
            frames,
            lazy_decoder: None,
            tags,
            sop_class,
            image_info: String::new(),
        });
    }

    // Regular pixel DICOM
    let frame_count = obj
        .get(NUMBER_OF_FRAMES)
        .and_then(|el| el.value().to_str().ok().map(|s| s.trim().to_string()))
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1);

    if file_size > LAZY_THRESHOLD_BYTES {
        // Large file: lazy decoding — keep raw DICOM in memory, decode on demand.
        let rows = read_u32_tag(&obj, ROWS).unwrap_or(0);
        let cols = read_u32_tag(&obj, COLUMNS).unwrap_or(0);
        let image_info = format!("{cols}×{rows}  {frame_count} frame(s)  {size_str}  [lazy]");
        let lazy_decoder = LazyPixelDecoder { obj, frame_count };
        return Ok(FileData {
            frames: vec![],
            lazy_decoder: Some(lazy_decoder),
            tags,
            sop_class,
            image_info,
        });
    }

    // Small file: decode all frames eagerly (current behaviour)
    let frames = match obj.decode_pixel_data() {
        Ok(pd) => (0..pd.number_of_frames())
            .filter_map(|i| {
                pd.to_dynamic_image(i)
                    .map_err(|e| error!("Warning: frame {i}: {e}"))
                    .ok()
            })
            .collect(),
        Err(e) => {
            let msg = e.to_string();
            if !msg.to_lowercase().contains("missing") {
                error!("Note: no displayable content ({e})");
            }
            vec![]
        }
    };

    let image_info = frames
        .first()
        .map(|img| format!("{}×{}  {}  {}", img.width(), img.height(), image_color_str(img), size_str))
        .unwrap_or_default();

    Ok(FileData {
        frames,
        lazy_decoder: None,
        tags,
        sop_class,
        image_info,
    })
}
