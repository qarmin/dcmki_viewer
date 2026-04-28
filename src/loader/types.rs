use image::DynamicImage;

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

pub struct FileData {
    pub frames: Vec<DynamicImage>,
    pub tags: Vec<TagEntry>,
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
