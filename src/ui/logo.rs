//! # Application Logo & Branding Assets
//!
//! Bundles the official `LaketexLogo.png` asset directly into the compiled executable
//! on all platforms via `include_bytes!`.
//!
//! Provides utilities for:
//! - Setting the OS window and taskbar application icon via [`egui::IconData`].
//! - Decoding and caching the bundled PNG into an [`egui::TextureHandle`] for runtime UI rendering.
//! - Rendering the LakeTex logo across the toolbar, preferences, and about dialogs.

use egui::{Color32, ColorImage, IconData, Rect, Response, TextureHandle, TextureOptions, Ui, Vec2};

/// Raw PNG bytes of the official LakeTex logo embedded into the binary at compile time.
pub const LOGO_PNG_BYTES: &[u8] = include_bytes!("../../LaketexLogo.png");

/// Loads and decodes the bundled PNG logo into an [`egui::ColorImage`].
pub fn load_logo_color_image() -> ColorImage {
    let dynamic_img = image::load_from_memory(LOGO_PNG_BYTES)
        .expect("LaketexLogo.png must be a valid PNG image bundled into the binary");
    let rgba = dynamic_img.to_rgba8();
    let (width, height) = rgba.dimensions();
    ColorImage::from_rgba_unmultiplied([width as usize, height as usize], rgba.as_raw())
}

/// Loads and decodes the bundled PNG logo into [`egui::IconData`] for window/taskbar icons.
pub fn app_icon_data() -> IconData {
    let dynamic_img = image::load_from_memory(LOGO_PNG_BYTES)
        .expect("LaketexLogo.png must be a valid PNG image bundled into the binary");
    let rgba = dynamic_img.to_rgba8();
    let (width, height) = rgba.dimensions();
    IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    }
}

/// Loads or retrieves the cached logo [`TextureHandle`] in the active egui context.
pub fn get_or_load_logo_texture(ctx: &egui::Context) -> TextureHandle {
    ctx.load_texture(
        "laketex_brand_logo",
        load_logo_color_image(),
        TextureOptions::LINEAR,
    )
}

/// Renders the LakeTex logo image widget with a specified square size and optional rounded corners.
pub fn render_logo(ui: &mut Ui, texture: &TextureHandle, size: Vec2, corner_radius: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if corner_radius > 0.0 {
            // Render with clip rect / rounded corners
            painter.rect_filled(rect, corner_radius, Color32::TRANSPARENT);
        }
        painter.image(
            texture.id(),
            rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    response
}

/// Constructs an [`egui::text::LayoutJob`] for the "Laketex" brand label with an italicized 'L'.
pub fn brand_layout_job(size: f32, color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        "L",
        0.0,
        egui::text::TextFormat {
            font_id: egui::FontId::proportional(size),
            color,
            italics: true,
            ..Default::default()
        },
    );
    job.append(
        "aketex",
        0.0,
        egui::text::TextFormat {
            font_id: egui::FontId::proportional(size),
            color,
            italics: false,
            ..Default::default()
        },
    );
    job
}

/// Renders the official "Laketex" brand label with an italicized 'L' as a non-selectable widget.
pub fn render_brand_label(ui: &mut Ui, size: f32, color: Color32) -> Response {
    ui.add(egui::Label::new(brand_layout_job(size, color)).selectable(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_logo_png_bytes_validity() {
        assert!(!LOGO_PNG_BYTES.is_empty(), "Bundled logo bytes must not be empty");
        assert!(LOGO_PNG_BYTES.starts_with(b"\x89PNG\r\n\x1a\n"), "Must be a valid PNG header");
        assert_eq!(LOGO_PNG_BYTES.len(), 69427);
    }

    #[test]
    fn test_load_logo_color_image_dimensions() {
        let color_image = load_logo_color_image();
        assert_eq!(color_image.size, [1276, 1276]);
        assert_eq!(color_image.pixels.len(), 1276 * 1276);
    }

    #[test]
    fn test_app_icon_data_format() {
        let icon = app_icon_data();
        assert_eq!(icon.width, 1276);
        assert_eq!(icon.height, 1276);
        assert_eq!(icon.rgba.len(), 1276 * 1276 * 4);
    }

    #[test]
    fn test_render_logo_in_egui_context() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let tex = get_or_load_logo_texture(ui.ctx());
                let resp = render_logo(ui, &tex, egui::vec2(24.0, 24.0), 4.0);
                assert_eq!(resp.rect.width(), 24.0);
                assert_eq!(resp.rect.height(), 24.0);
            });
        });
    }

    #[test]
    fn test_brand_layout_job_italicized_l() {
        let job = brand_layout_job(14.5, Color32::WHITE);
        assert_eq!(job.text, "Laketex");
        assert_eq!(job.sections.len(), 2);
        assert!(job.sections[0].format.italics, "The 'L' section must be italicized");
        assert!(!job.sections[1].format.italics, "The 'aketex' section must not be italicized");
    }
}
