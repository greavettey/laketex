//! # Viewer Panel & Mode Switcher
//!
//! Provides the right-side preview pane ([`ViewerPanel`]), toggling between:
//! - Native Vector View: Pure-Rust sub-millisecond vector rendering
//! - PDF Raster View: High-DPI PDFium rendering with color inversion

use crate::engine::layout::TypesetPage;
use crate::engine::native_renderer::NativePageRenderer;
use crate::t;
use crate::viewer::canvas::PdfCanvas;
use crate::viewer::PdfRenderer;
use egui::{Color32, RichText, ScrollArea, Ui};
use serde::{Deserialize, Serialize};

/// Document display rendering mode in the preview panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    /// Pure-Rust vector layout and immediate vector drawing
    NativeVector,
    /// High-DPI rasterized PDFium rendering
    PdfRaster,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self::NativeVector
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

        ui.vertical(|ui| {
            // Viewer Header
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Preview")
                        .strong()
                        .color(Color32::from_rgb(97, 175, 239)),
                );

                // View mode toggle: Native Vector (Pure Rust) vs PDF Engine
                ui.selectable_value(view_mode, ViewMode::NativeVector, t!("viewer.native_view"));
                ui.selectable_value(view_mode, ViewMode::PdfRaster, t!("viewer.pdf_view"));

                if has_syntax_errors {
                    ui.colored_label(
                        Color32::from_rgb(242, 201, 76),
                        "● Draft with syntax error (Showing live preview)",
                    );
                } else {
                    ui.colored_label(Color32::from_rgb(152, 195, 121), "● Live Sync Active");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let total = match view_mode {
                        ViewMode::NativeVector => typeset_pages.len().max(1),
                        ViewMode::PdfRaster => canvas.total_pages.max(1),
                    };

                    if ui.button("Next ▶").clicked() && canvas.current_page + 1 < total {
                        canvas.current_page += 1;
                    }

                    ui.label(format!("{}/{}", canvas.current_page + 1, total));

                    if ui.button("◀ Prev").clicked() && canvas.current_page > 0 {
                        canvas.current_page -= 1;
                    }
                });
            });

            ui.separator();

            match view_mode {
                ViewMode::NativeVector => {
                    // 100% Pure Rust Native Live Vector Preview
                    let page_idx = canvas.current_page.min(typeset_pages.len().saturating_sub(1));
                    if let Some(page) = typeset_pages.get(page_idx) {
                        ScrollArea::both()
                            .id_salt("native_page_scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(8.0);
                                    if let Some(line) = NativePageRenderer::render_page_ui_with_mode(
                                        ui,
                                        page,
                                        canvas.color_mode,
                                        canvas.zoom,
                                    ) {
                                        action = ViewerAction::JumpToLine(line);
                                    }
                                    ui.add_space(24.0);
                                });
                            });
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label("Document layout is being computed...");
                        });
                    }
                }
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
            }
        });

        action
    }
}
