use hayro::{
    RenderCache, RenderSettings, hayro_interpret::InterpreterSettings, hayro_syntax::Pdf, render,
    vello_cpu::color::palette::css::WHITE,
};
use image::DynamicImage;
use log::error;

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
            pixmap.into_png().ok().and_then(|png| {
                image::load_from_memory(&png)
                    .map_err(|e| error!("Warning: page {i} decode: {e}"))
                    .ok()
            })
        })
        .collect()
}
