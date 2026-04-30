use dicom::object::DefaultDicomObject;
use dicom_pixeldata::PixelDecoder;
use image::DynamicImage;
use log::error;

#[derive(Clone)]
pub struct TagEntry {
    pub tag: String,
    pub name: String,
    pub vr: String,
    pub value: String,
    pub value_full: String,
    pub is_private: bool,
    pub depth: u32,
    pub is_item_header: bool,
    pub is_sequence: bool,
}

/// On-demand pixel decoder.  Keeps the raw (undecoded) DICOM object in memory
/// and decodes exactly one frame at a time.  Only one decoded `DynamicImage` is
/// alive at a time; callers are responsible for dropping the old one.
pub struct LazyPixelDecoder {
    pub obj: DefaultDicomObject,
    pub frame_count: u32,
}

impl LazyPixelDecoder {
    /// Decode a single frame.  Returns `None` on any error.
    pub fn decode_frame(&self, idx: u32) -> Option<DynamicImage> {
        let pd = self.obj.decode_pixel_data_frame(idx)
            .map_err(|e| error!("decode frame {idx}: {e}"))
            .ok()?;
        pd.to_dynamic_image(0)
            .map_err(|e| error!("to_dynamic_image frame {idx}: {e}"))
            .ok()
    }
}

pub struct FileData {
    /// Pre-decoded frames (VF, PDF, small DICOM files).
    pub frames: Vec<DynamicImage>,
    /// Lazy per-frame decoder for large DICOM files.  Mutually exclusive with
    /// non-empty `frames`.
    pub lazy_decoder: Option<LazyPixelDecoder>,
    pub tags: Vec<crate::loader::types::TagEntry>,
    pub sop_class: String,
    pub image_info: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    Other,
    PixelData,
    EncapsulatedPdf,
}

#[derive(Clone)]
pub struct DirFileEntry {
    pub path: String,
    pub sop_class: String,
    pub content_kind: ContentKind,
}
