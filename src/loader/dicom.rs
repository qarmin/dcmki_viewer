use std::path::Path;

use dicom::{
    core::{
        VR,
        dictionary::{DataDictionary, UidDictionary},
        value::Value,
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
    tags::{ENCAPSULATED_DOCUMENT, SOP_CLASS_UID},
};
use dicom_pixeldata::PixelDecoder;
use hayro::hayro_syntax::Pdf;
use image::DynamicImage;
use log::error;

use super::{
    pdf::render_pages,
    types::{FileData, TagEntry},
};
use crate::{error::AppError, visual_field};

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
            Value::PixelSequence(_) => {
                out.push(TagEntry {
                    tag: tag_str,
                    name,
                    vr: vr_str,
                    value: "[Pixel Data]".to_string(),
                    value_full: "[Pixel Data]".to_string(),
                    is_private,
                    depth,
                    is_item_header: false,
                    is_sequence: false,
                });
            }
            Value::Primitive(_v) => {
                // avoid clippy::map_unwrap_or by matching on the Result explicitly
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

pub(super) fn load_dicom(path: &Path, size_str: &str) -> Result<FileData, AppError> {
    let obj = open_file(path).map_err(|e| AppError::Dicom(e.to_string()))?;

    let dict = StandardDataDictionary;
    let mut tags: Vec<TagEntry> = Vec::new();
    flatten_obj(&obj, dict, 0, &mut tags);

    let sop_class = obj
        .get(SOP_CLASS_UID)
        .and_then(|el| el.value().to_str().ok().map(|s| s.into_owned()))
        .map_or_else(|| "DICOM".to_string(), |uid| get_storage_sop_class_name(uid.trim()));

    let enc_pdf = obj
        .get(ENCAPSULATED_DOCUMENT)
        .and_then(|el| el.value().to_bytes().ok())
        .map(|b| b.into_owned())
        .filter(|b| b.starts_with(b"%PDF"))
        .and_then(|bytes| {
            Pdf::new(bytes)
                .map_err(|e| error!("Warning: encapsulated PDF parse: {e:?}"))
                .ok()
        });

    const VF_SOP: &str = OPHTHALMIC_VISUAL_FIELD_STATIC_PERIMETRY_MEASUREMENTS_STORAGE;
    let sop_uid = obj
        .get(SOP_CLASS_UID)
        .and_then(|el| el.value().to_str().ok().map(|s| s.into_owned()))
        .unwrap_or_default();

    let frames = if let Some(pdf) = enc_pdf {
        render_pages(&pdf)
    } else if sop_uid.trim() == VF_SOP {
        visual_field::render(&obj)
    } else {
        match obj.decode_pixel_data() {
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
        }
    };

    let image_info = if sop_uid.trim() == VF_SOP {
        String::new()
    } else {
        frames
            .first()
            .map(|img| {
                format!(
                    "{}×{}  {}  {}",
                    img.width(),
                    img.height(),
                    image_color_str(img),
                    size_str
                )
            })
            .unwrap_or_default()
    };

    Ok(FileData {
        frames,
        tags,
        sop_class,
        image_info,
    })
}
