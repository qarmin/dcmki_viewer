use std::sync::Arc;

use image::DynamicImage;
use resvg::usvg::{Options, Tree, fontdb};
use resvg::{render as resvg_render, tiny_skia};

/// Cached font database — loaded once, reused for all SVG renders.
fn cached_fontdb() -> Arc<fontdb::Database> {
    use std::sync::OnceLock;
    static DB: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
    .clone()
}

pub fn svg_to_image(svg_str: &str) -> Option<DynamicImage> {
    let opt = Options {
        fontdb: cached_fontdb(),
        ..Default::default()
    };

    let tree = Tree::from_str(svg_str, &opt)
        .map_err(|e| log::warn!("SVG parse error: {e}"))
        .ok()?;

    let w = tree.size().width() as u32;
    let h = tree.size().height() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(w, h)?;
    resvg_render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    image::RgbaImage::from_raw(w, h, pixmap.take()).map(DynamicImage::ImageRgba8)
}

