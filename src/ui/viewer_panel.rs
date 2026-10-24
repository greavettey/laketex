//! # Viewer Panel & Mode Switcher
//!
//! Provides the right-side preview pane ([`ViewerPanel`]), toggling between:
//! - PDF Raster View: High-DPI PDFium rendering with color inversion
//! - Native Vector View: Pure-Rust sub-millisecond vector rendering

use crate::engine::layout::TypesetPage;
use crate::engine::native_renderer::NativePageRenderer;
use crate::t;
use crate::viewer::canvas::PdfCanvas;
use crate::viewer::{PdfColorMode, PdfRenderer};
use egui::{Color32, RichText, ScrollArea, Ui};
use serde::{Deserialize, Serialize};

/// Document display rendering mode in the preview panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    /// High-DPI rasterized PDFium rendering (favored / default)
    PdfRaster,
    /// Pure-Rust vector layout and immediate vector drawing
    NativeVector,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self::PdfRaster
    }
}

/// Action emitted by user interaction on the viewer panel.
pub enum ViewerAction {
    /// No action
    None,
    /// Jump editor caret to source line number (backward SyncTeX)
    JumpToLine(usize),
}

/// Right-hand document preview panel.
pub struct ViewerPanel;

impl ViewerPanel {
    /// Renders the right-hand document preview pane with zoom controls,
    /// page jumping, color inversion, and interactive canvas rasterization.
    pub fn show(
        ui: &mut Ui,
        canvas: &mut PdfCanvas,
        renderer: &PdfRenderer,
        typeset_pages: &[TypesetPage],
        pdf_bytes: Option<&[u8]>,
        view_mode: &mut ViewMode,
        _is_dark: bool,
        has_syntax_errors: bool,
        total_document_lines: usize,
    ) -> ViewerAction {
        let mut action = ViewerAction::None;

        // Ctrl+Mouse Wheel zoom for preview canvas
        let ctrl_zoom = ui.input(|i| {
            if (i.modifiers.command || i.modifiers.ctrl) && i.raw_scroll_delta.y != 0.0 {
                let dy = i.raw_scroll_delta.y;
                if dy > 0.0 {
                    Some(0.1)
                } else {
                    Some(-0.1)
                }
            } else {
                None
            }
        });
        if let Some(delta) = ctrl_zoom {
            canvas.zoom = (canvas.zoom + delta).clamp(0.25, 4.0);
            canvas.fit_to_width = false;
        }

        // Viewer Header
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), crate::ui::PANEL_HEADER_HEIGHT),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_height(crate::ui::PANEL_HEADER_HEIGHT);
                ui.spacing_mut().item_spacing = egui::vec2(5.0, 0.0);

                ui.add(
                    egui::Label::new(
                        RichText::new("Preview")
                            .strong()
                            .color(Color32::from_rgb(97, 175, 239)),
                    )
                    .selectable(false),
                );

                // View mode toggle: PDF Engine (favored / default) vs Native Vector (Pure Rust)
                ui.selectable_value(view_mode, ViewMode::PdfRaster, t!("viewer.pdf_view"));
                ui.selectable_value(view_mode, ViewMode::NativeVector, t!("viewer.native_view"));

                if has_syntax_errors {
                    ui.add(
                        egui::Label::new(
                            RichText::new("● Draft with syntax error (Showing live preview)")
                                .color(Color32::from_rgb(242, 201, 76)),
                        )
                        .selectable(false),
                    );
                } else {
                    ui.add(
                        egui::Label::new(
                            RichText::new("● Live Sync Active")
                                .color(Color32::from_rgb(152, 195, 121)),
                        )
                        .selectable(false),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let total = match view_mode {
                        ViewMode::PdfRaster => canvas.total_pages.max(1),
                        ViewMode::NativeVector => typeset_pages.len().max(1),
                    };

                    if ui.button("Next ▶").clicked() && canvas.current_page + 1 < total {
                        canvas.current_page += 1;
                    }

                    ui.add(
                        egui::Label::new(format!("{}/{}", canvas.current_page + 1, total))
                            .selectable(false),
                    );

                    if ui.button("◀ Prev").clicked() && canvas.current_page > 0 {
                        canvas.current_page -= 1;
                    }
                });
            },
        );

        ui.separator();

            match view_mode {
                ViewMode::PdfRaster => {
                    // PDF Rasterizer preview
                    canvas.update_texture(ui, renderer, pdf_bytes);
                    if let Some(rel_y) = canvas.show(ui) {
                        let page_idx = canvas.current_page;
                        if let Some(page) = typeset_pages.get(page_idx) {
                            let y_unzoomed = rel_y * page.height - page.margin;
                            if let Some(el) = page.find_element_at_y(y_unzoomed) {
                                action = ViewerAction::JumpToLine(el.source_line);
                            }
                        } else if total_document_lines > 0 {
                            let line = ((rel_y * total_document_lines as f32).round() as usize).max(1);
                            action = ViewerAction::JumpToLine(line);
                        }
                    }
                }
                ViewMode::NativeVector => {
                    // 100% Pure Rust Native Live Vector Preview
                    let page_idx = canvas.current_page.min(typeset_pages.len().saturating_sub(1));
                    if let Some(page) = typeset_pages.get(page_idx) {
                        let bg_color = match canvas.color_mode {
                            PdfColorMode::AcademicDark => Color32::from_rgb(18, 20, 24),
                            PdfColorMode::Sepia => Color32::from_rgb(243, 235, 218),
                            PdfColorMode::Normal => Color32::from_rgb(238, 240, 245),
                        };

                        egui::Frame::NONE
                            .fill(bg_color)
                            .inner_margin(egui::Margin::same(12))
                            .show(ui, |ui| {
                                ScrollArea::both()
                                    .id_salt("native_page_scroll")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.vertical_centered(|ui| {
                                            ui.add_space(8.0);
                                            let effective_zoom = if canvas.fit_to_width {
                                                ((ui.available_width() - 48.0) / page.width).clamp(0.4, 3.0)
                                            } else {
                                                canvas.zoom
                                            };
                                            if let Some(line) = NativePageRenderer::render_page_ui_with_mode(
                                                ui,
                                                page,
                                                canvas.color_mode,
                                                effective_zoom,
                                            ) {
                                                action = ViewerAction::JumpToLine(line);
                                            }
                                            ui.add_space(24.0);
                                        });
                                    });
                            });
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label("Document layout is being computed...");
                        });
                    }
                }
            }

        action
    }
}
