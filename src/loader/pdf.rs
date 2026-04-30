use hayro::{
    RenderCache, RenderSettings, hayro_interpret::InterpreterSettings, hayro_syntax::Pdf, render,
    vello_cpu::color::palette::css::WHITE,
};
use image::{DynamicImage, RgbaImage};
use log::warn;

pub(super) fn render_pages(pdf: &Pdf) -> Vec<DynamicImage> {
    let interp = InterpreterSettings::default();
    let render_settings = RenderSettings {
        x_scale: 2.0,
        y_scale: 2.0,
        bg_color: WHITE,
        ..Default::default()
    };
    let cache = RenderCache::new();

    pdf.pages()
        .iter()
        .enumerate()
        .filter_map(|(i, page)| {
            let pixmap = render(page, &cache, &interp, &render_settings);
            let width = pixmap.width() as u32;
            let height = pixmap.height() as u32;
            let bytes: Vec<u8> = pixmap.take_unpremultiplied().into_iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();
            RgbaImage::from_raw(width, height, bytes)
                .map(DynamicImage::ImageRgba8)
                .or_else(|| { warn!("page {i}: buffer size mismatch"); None })
        })
        .collect()
}
