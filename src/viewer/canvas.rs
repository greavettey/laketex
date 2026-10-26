//! Interactive scrollable preview canvas for rasterized PDF pages.

use crate::viewer::{PdfColorMode, PdfRenderer};
use egui::{vec2, Color32, Pos2, Rect, ScrollArea, Stroke, TextureHandle, TextureOptions, Ui};

pub struct PdfCanvas {
    pub current_page: usize,
    pub total_pages: usize,
    pub zoom: f32,
    pub color_mode: PdfColorMode,
    pub invert_colors: bool,
    pub base_render_width: u16,
    pub fit_to_width: bool,

    pub texture: Option<TextureHandle>,
    pub last_error: Option<String>,
    last_rendered_page: Option<usize>,
    last_rendered_zoom: Option<f32>,
    last_rendered_invert: Option<bool>,
    last_rendered_mode: Option<PdfColorMode>,
    last_rendered_base_width: Option<u16>,
    last_pdf_hash: Option<u64>,
}

impl Default for PdfCanvas {
    fn default() -> Self {
        Self {
            current_page: 0,
            total_pages: 1,
            zoom: 1.0,
            color_mode: PdfColorMode::Normal,
            invert_colors: false, // Default to clean light view
            base_render_width: 1200,
            fit_to_width: true,
            texture: None,
            last_error: None,
            last_rendered_page: None,
            last_rendered_zoom: None,
            last_rendered_invert: None,
            last_rendered_mode: None,
            last_rendered_base_width: None,
            last_pdf_hash: None,
        }
    }
}

impl PdfCanvas {
    /// Creates a new canvas with default zoom and dark mode enabled
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets all cached textures, hashes, and error states.
    pub fn reset_cache(&mut self) {
        self.texture = None;
        self.last_pdf_hash = None;
        self.last_rendered_page = None;
        self.last_rendered_zoom = None;
        self.last_rendered_invert = None;
        self.last_rendered_mode = None;
        self.last_rendered_base_width = None;
        self.last_error = None;
        self.current_page = 0;
        self.total_pages = 1;
    }

    /// Invalidates cached PDF texture and hash to force re-rasterization on the next frame.
    pub fn invalidate(&mut self) {
        self.last_pdf_hash = None;
        self.texture = None;
    }

    /// Updates color mode and keeps invert_colors in sync.
    pub fn set_color_mode(&mut self, mode: PdfColorMode) {
        if self.color_mode != mode {
            self.color_mode = mode;
            self.invert_colors = mode == PdfColorMode::AcademicDark;
        }
    }

    /// Updates base render resolution width.
    pub fn set_base_render_width(&mut self, width: u16) {
        let w = width.clamp(600, 4000);
        if self.base_render_width != w {
            self.base_render_width = w;
        }
    }

    /// Computes a lightweight 64-bit hash of PDF bytes to detect changes
    pub fn hash_pdf(bytes: &[u8]) -> u64 {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        hasher.write(bytes);
        hasher.finish()
    }

    /// Updates texture if document changed, page changed, or zoom/invert changed.
    /// Retains existing texture on compilation failure so the preview never blanks out.
    pub fn update_texture(
        &mut self,
        ui: &mut Ui,
        renderer: &PdfRenderer,
        pdf_bytes: Option<&[u8]>,
    ) {
        let Some(bytes) = pdf_bytes else {
            return;
        };

        let current_hash = Self::hash_pdf(bytes);
        let base_w = if self.base_render_width >= 600 {
            self.base_render_width
        } else {
            1200
        };
        let target_width = (base_w as f32 * self.zoom.clamp(0.5, 3.0)) as u16;

        // Keep legacy invert_colors synced to the active color_mode
        self.invert_colors = self.color_mode == PdfColorMode::AcademicDark;

        let hash_changed = self.last_pdf_hash != Some(current_hash);
        let page_changed = self.last_rendered_page != Some(self.current_page);
        let mode_changed = self.last_rendered_mode != Some(self.color_mode);
        let width_changed = self.last_rendered_base_width != Some(self.base_render_width);
        let zoom_changed = self
            .last_rendered_zoom
            .map(|z| (z - self.zoom).abs() > 0.25)
            .unwrap_or(true);

        let needs_rerender = (self.texture.is_none() && self.last_error.is_none())
            || hash_changed
            || page_changed
            || mode_changed
            || width_changed
            || zoom_changed;

        if needs_rerender {
            match renderer.render_page_to_color_image_with_mode(
                bytes,
                self.current_page,
                target_width,
                self.color_mode,
            ) {
                Ok((color_image, count)) => {
                    self.last_error = None;
                    self.total_pages = count.max(1);
                    if self.current_page >= self.total_pages {
                        self.current_page = self.total_pages - 1;
                    }
                    self.texture = Some(ui.ctx().load_texture(
                        "pdf_page_preview",
                        color_image,
                        TextureOptions::LINEAR,
                    ));
                    self.last_pdf_hash = Some(current_hash);
                    self.last_rendered_page = Some(self.current_page);
                    self.last_rendered_zoom = Some(self.zoom);
                    self.last_rendered_invert = Some(self.invert_colors);
                    self.last_rendered_mode = Some(self.color_mode);
                    self.last_rendered_base_width = Some(self.base_render_width);
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if self.last_error.as_ref() != Some(&err_str) {
                        eprintln!("[Laketex] Render page error: {err_str}");
                    }
                    self.last_error = Some(err_str);
                    self.last_pdf_hash = Some(current_hash);
                    self.last_rendered_page = Some(self.current_page);
                    self.last_rendered_zoom = Some(self.zoom);
                    self.last_rendered_invert = Some(self.invert_colors);
                    self.last_rendered_mode = Some(self.color_mode);
                    self.last_rendered_base_width = Some(self.base_render_width);
                }
            }
        }
    }

    /// Renders the page inside a scrollable canvas.
    ///
    /// Returns `Some(relative_y)` (0.0 to 1.0) if double-clicked, triggering backward SyncTeX.
    pub fn show(&mut self, ui: &mut Ui) -> Option<f32> {
        let available_size = ui.available_size();
        let mut double_clicked_rel_y = None;

        let bg_color = match self.color_mode {
            PdfColorMode::AcademicDark => Color32::from_rgb(18, 20, 24),
            PdfColorMode::Sepia => Color32::from_rgb(243, 235, 218),
            PdfColorMode::Normal => Color32::from_rgb(238, 240, 245),
        };

        egui::Frame::NONE
            .fill(bg_color)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                if let Some(texture) = &self.texture {
                    let tex_size = texture.size_vec2();
                    let aspect = tex_size.y / tex_size.x.max(1.0);

                    let display_width = if self.fit_to_width {
                        (available_size.x - 48.0).clamp(240.0, 1400.0)
                    } else {
                        (available_size.x - 48.0).clamp(240.0, 1400.0) * self.zoom
                    };
                    let display_height = display_width * aspect;
                    let display_size = vec2(display_width, display_height);

                    ScrollArea::both()
                        .id_salt("pdf_canvas_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.add_space(8.0);

                                // Page Drop Shadow and Border with Click Sense
                                let (rect, response) = ui.allocate_exact_size(display_size, egui::Sense::click());

                                if response.double_clicked() {
                                    if let Some(pos) = response.interact_pointer_pos() {
                                        let rel_y = ((pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
                                        double_clicked_rel_y = Some(rel_y);
                                    }
                                }

                                let painter = ui.painter();
                                let shadow_rect = rect.translate(vec2(2.0, 4.0));
                                painter.rect_filled(
                                    shadow_rect,
                                    4.0,
                                    if self.invert_colors {
                                        Color32::from_black_alpha(90)
                                    } else {
                                        Color32::from_black_alpha(40)
                                    },
                                );

                                // Render PDF Texture Image
                                painter.image(
                                    texture.id(),
                                    rect,
                                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                                    Color32::WHITE,
                                );

                                painter.rect_stroke(
                                    rect,
                                    2.0,
                                    Stroke::new(
                                        1.0_f32,
                                        if self.invert_colors {
                                            Color32::from_rgb(55, 62, 75)
                                        } else {
                                            Color32::from_rgb(205, 210, 220)
                                        },
                                    ),
                                    egui::StrokeKind::Inside,
                                );

                                ui.add_space(24.0);
                            });
                        });
                } else if let Some(err) = &self.last_error {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);
                            ui.label(
                                egui::RichText::new("⚠️ Unable to render PDF preview")
                                    .font(egui::FontId::proportional(16.0))
                                    .color(Color32::from_rgb(224, 108, 117))
                                    .strong(),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new(err)
                                    .font(egui::FontId::monospace(12.0))
                                    .color(Color32::from_gray(180)),
                            );
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new(
                                    "Switch to 'Native Live View' above or press Ctrl+B to recompile.",
                                )
                                .font(egui::FontId::proportional(13.0))
                                .color(Color32::from_gray(140)),
                            );
                        });
                    });
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new("Compiling document preview...")
                                .font(egui::FontId::proportional(14.0))
                                .color(Color32::from_gray(140)),
                        );
                    });
                }
            });

        double_clicked_rel_y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_reset_cache() {
        let mut canvas = PdfCanvas::new();
        canvas.current_page = 5;
        canvas.total_pages = 10;
        canvas.last_pdf_hash = Some(12345);
        canvas.last_rendered_page = Some(5);
        canvas.last_rendered_zoom = Some(1.5);
        canvas.last_rendered_invert = Some(true);
        canvas.last_error = Some("Previous error".to_string());

        canvas.reset_cache();

        assert_eq!(canvas.current_page, 0);
        assert_eq!(canvas.total_pages, 1);
        assert!(canvas.texture.is_none());
        assert!(canvas.last_pdf_hash.is_none());
        assert!(canvas.last_rendered_page.is_none());
        assert!(canvas.last_rendered_zoom.is_none());
        assert!(canvas.last_rendered_invert.is_none());
        assert!(canvas.last_error.is_none());
    }

    #[test]
    fn test_canvas_hash_pdf() {
        let pdf_a = b"%PDF-1.4 sample data a";
        let pdf_b = b"%PDF-1.4 sample data b";

        let hash_a = PdfCanvas::hash_pdf(pdf_a);
        let hash_b = PdfCanvas::hash_pdf(pdf_b);

        assert_ne!(hash_a, hash_b);
        assert_eq!(hash_a, PdfCanvas::hash_pdf(pdf_a));
    }
}
