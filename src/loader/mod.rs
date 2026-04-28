mod dicom;
mod pdf;
mod types;

use std::path::{Path, PathBuf};

use ::dicom::object::open_file;
pub use dicom::get_storage_sop_class_name;
use dicom_dictionary_std::{
    tags::{ENCAPSULATED_DOCUMENT, PIXEL_DATA, SOP_CLASS_UID},
    uids::OPHTHALMIC_VISUAL_FIELD_STATIC_PERIMETRY_MEASUREMENTS_STORAGE,
};
use image::DynamicImage;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
pub use types::{ContentKind, DirFileEntry, FileData, TagEntry};

use crate::error::AppError;

fn truncate(s: String, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        let t: String = s.chars().take(max_chars).collect();
        format!("{t}…")
    } else {
        s
    }
}

fn format_bytes(n: u64) -> String {
    if n >= 1_048_576 {
        format!("{:.1} MB", n as f64 / 1_048_576.0)
    } else {
        format!("{:.0} KB", n as f64 / 1024.0)
    }
}

fn is_supported_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("dcm")
    )
}

/// Recursively collect all .dcm paths under `dir`, sorted.
pub fn collect_file_paths(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(read_dir) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() && is_supported_file(&path) {
                result.push(path);
            }
        }
    }
    result.sort();
    result
}

/// Read metadata (SOP class + content kind) for a single file — no pixel decoding.
pub fn scan_single_file(path: &Path) -> DirFileEntry {
    let path_str = path.to_string_lossy().into_owned();

    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
        == Some("pdf")
    {
        return DirFileEntry {
            path: path_str,
            sop_class: "PDF Document".to_string(),
            content_kind: ContentKind::EncapsulatedPdf,
        };
    }

    let Ok(obj) = open_file(path) else {
        return DirFileEntry {
            path: path_str,
            sop_class: "DICOM".to_string(),
            content_kind: ContentKind::Other,
        };
    };

    let sop_uid: String = obj
        .get(SOP_CLASS_UID)
        .and_then(|el| el.value().to_str().ok().map(|s| s.into_owned()))
        .unwrap_or_default();

    let sop = if sop_uid.is_empty() {
        "DICOM".to_string()
    } else {
        get_storage_sop_class_name(&sop_uid)
    };

    let encapsulated_pdf = obj
        .get(ENCAPSULATED_DOCUMENT)
        .and_then(|el| el.value().to_bytes().ok())
        .is_some_and(|b| b.starts_with(b"%PDF"));

    const OPHTHALMIC_VF_UID: &str = OPHTHALMIC_VISUAL_FIELD_STATIC_PERIMETRY_MEASUREMENTS_STORAGE;

    let kind = if encapsulated_pdf {
        ContentKind::EncapsulatedPdf
    } else if obj.get(PIXEL_DATA).is_some() || sop_uid == OPHTHALMIC_VF_UID {
        ContentKind::PixelData
    } else {
        ContentKind::Other
    };

    DirFileEntry {
        path: path_str,
        sop_class: sop,
        content_kind: kind,
    }
}

pub fn dynamic_image_to_slint(img: &DynamicImage) -> Image {
    let rgba = img.to_rgba8();
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(rgba.as_raw(), rgba.width(), rgba.height());
    Image::from_rgba8(buffer)
}

pub fn load(path: &Path) -> Result<FileData, AppError> {
    let file_size = std::fs::metadata(path).map_or(0, |m| m.len());
    let size_str = format_bytes(file_size);
    dicom::load_dicom(path, &size_str)
}
