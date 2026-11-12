//! # Viewer Panel & Mode Switcher
//!
//! Provides the right-side preview pane ([`ViewerPanel`]), toggling between:
//! - PDF Raster View: High-DPI PDFium rendering with color inversion
//! - Native Vector View: Pure-Rust sub-millisecond vector rendering

use crate::document::SyncTexData;
use crate::engine::layout::TypesetPage;
use crate::engine::native_renderer::NativePageRenderer;
use crate::t;
use crate::viewer::canvas::{CanvasEvent, PageViewMode, PdfCanvas};
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
    /// Jump editor caret to micro-precise line and column (backward SyncTeX)
    JumpToLocation { line: usize, column: usize },
    /// Jump editor caret directly to exact 0-based source character index
    JumpToChar(usize),
    /// Select / highlight a chunk of LaTeX source code (from dragging on preview)
    SelectRange { start_char: usize, end_char: usize },
    /// Clear selection in editor
    ClearSelection,
}

fn resolve_page_points(canvas: &PdfCanvas, typeset_pages: &[TypesetPage], page_idx: usize) -> (f32, f32) {
    if let Some(layout) = canvas.page_text_layouts.get(page_idx).and_then(|l| l.as_ref()) {
        if layout.page_width_pt > 1.0 && layout.page_height_pt > 1.0 {
            return (layout.page_width_pt, layout.page_height_pt);
        }
    }
    typeset_pages
        .get(page_idx)
        .map(|p| (p.width, p.height))
        .unwrap_or((612.0, 792.0))
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
        synctex_data: Option<&SyncTexData>,
        source_text: Option<&str>,
        view_mode: &mut ViewMode,
        _is_dark: bool,
        has_syntax_errors: bool,
        total_document_lines: usize,
    ) -> ViewerAction {
        let mut action = ViewerAction::None;

        // Ctrl+Mouse Wheel zoom for preview canvas (scoped to viewer panel hover)
        let ctrl_zoom = if ui.ui_contains_pointer() {
            ui.input(|i| {
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
            })
        } else {
            None
        };
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

                // Page layout mode toggle: Continuous Scroll (Overleaf style) vs Single Page
                ui.separator();
                let is_continuous = canvas.page_view_mode == PageViewMode::Continuous;
                if ui
                    .selectable_label(is_continuous, "📜 Continuous")
                    .on_hover_text("Scroll continuously through all pages (Overleaf style)")
                    .clicked()
                {
                    canvas.page_view_mode = PageViewMode::Continuous;
                }
                if ui
                    .selectable_label(!is_continuous, "📄 Paged")
                    .on_hover_text("Single page view with Prev/Next buttons")
                    .clicked()
                {
                    canvas.page_view_mode = PageViewMode::SinglePage;
                }

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
                    if canvas.current_page >= total {
                        canvas.current_page = total.saturating_sub(1);
                    }

                    if canvas.page_view_mode == PageViewMode::SinglePage {
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
                    } else {
                        ui.add(
                            egui::Label::new(format!("{total} Pages"))
                                .selectable(false),
                        );
                    }
                });
            },
        );

        ui.separator();

            match view_mode {
                ViewMode::PdfRaster => {
                    // PDF Rasterizer preview
                    canvas.update_texture(ui, renderer, pdf_bytes);
                    if let Some(event) = canvas.show(ui) {
                        match event {
                            CanvasEvent::DoubleClick(page_idx, rel_x, rel_y) => {
                                canvas.current_page = page_idx;
                                let (page_w, page_h) = resolve_page_points(canvas, typeset_pages, page_idx);
                                let click_ctx = canvas
                                    .page_text_layouts
                                    .get(page_idx)
                                    .and_then(|l| l.as_ref())
                                    .and_then(|layout| layout.extract_click_context(egui::Pos2::new(rel_x, rel_y)));
                                let anchor = click_ctx
                                    .as_ref()
                                    .map(|c| c.anchor_pos)
                                    .unwrap_or(egui::Pos2::new(rel_x, rel_y));

                                let sync_loc = synctex_data.and_then(|synctex| {
                                    synctex.find_location_at(page_idx + 1, anchor.x, anchor.y, page_w, page_h, source_text)
                                });

                                // 1. Primary: Exact character-level alignment against rendered PDF text layout
                                if let (Some(src), Some(ctx)) = (source_text, click_ctx.as_ref()) {
                                    let line_hint = sync_loc.as_ref().map(|l| l.line);
                                    if let Some(char_idx) = crate::viewer::text_selection::find_caret_char_index_in_latex_source(
                                        src,
                                        ctx,
                                        line_hint,
                                    ) {
                                        action = ViewerAction::JumpToChar(char_idx);
                                    }
                                }

                                // 2. Fallback for non-text clicks (images, figures, blank rules): SyncTeX -> Native -> Proportional
                                if matches!(action, ViewerAction::None) {
                                    let mut resolved_location: Option<(usize, usize)> =
                                        sync_loc.map(|loc| (loc.line, loc.column));

                                    if resolved_location.is_none() {
                                        if let Some(page) = typeset_pages.get(page_idx) {
                                            let y_unzoomed = rel_y * page.height - page.margin;
                                            if let Some((el, offset_in_el)) = page.find_element_at_y(y_unzoomed) {
                                                let line = el.line_at_offset(offset_in_el);
                                                let col = el.column_at_offset(rel_x, offset_in_el, page.width, source_text);
                                                resolved_location = Some((line, col));
                                            }
                                        }
                                    }

                                    if resolved_location.is_none() && total_document_lines > 0 {
                                        let total_p = canvas.total_pages.max(1);
                                        let overall_fraction = (page_idx as f32 + rel_y) / total_p as f32;
                                        let line = ((overall_fraction * total_document_lines as f32).round() as usize).max(1);
                                        resolved_location = Some((line, 0));
                                    }

                                    if let Some((line, column)) = resolved_location {
                                        action = ViewerAction::JumpToLocation { line, column };
                                    }
                                }
                            }
                            CanvasEvent::TextSelected { page, text, start_pos, end_pos: _ } => {
                                canvas.current_page = page;
                                if let Some(src) = source_text {
                                    let (page_w, page_h) = resolve_page_points(canvas, typeset_pages, page);
                                    // Query SyncTeX at the actual start of the dragged selection to break repeated-phrase ties
                                    let line_hint = synctex_data.and_then(|synctex| {
                                        synctex
                                            .find_location_at(page + 1, start_pos.x, start_pos.y, page_w, page_h, Some(src))
                                            .map(|l| l.line)
                                    });

                                    if let Some((s_char, e_char)) = crate::viewer::text_selection::find_chunk_in_latex_source(src, &text, line_hint) {
                                        action = ViewerAction::SelectRange {
                                            start_char: s_char,
                                            end_char: e_char,
                                        };
                                    }
                                }
                            }
                            CanvasEvent::ClearSelection => {
                                action = ViewerAction::ClearSelection;
                            }
                        }
                    }
                }
                ViewMode::NativeVector => {
                    // 100% Pure Rust Native Live Vector Preview
                    let bg_color = match canvas.color_mode {
                        PdfColorMode::AcademicDark => Color32::from_rgb(18, 20, 24),
                        PdfColorMode::Sepia => Color32::from_rgb(243, 235, 218),
                        PdfColorMode::Normal => Color32::from_rgb(238, 240, 245),
                    };

                    if typeset_pages.is_empty() {
                        ui.centered_and_justified(|ui| {
                            ui.label("Document layout is being computed...");
                        });
                    } else {
                        egui::Frame::NONE
                            .fill(bg_color)
                            .inner_margin(egui::Margin::same(12))
                            .show(ui, |ui| {
                                ScrollArea::both()
                                    .id_salt("native_page_scroll")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.vertical_centered(|ui| {
                                            let pages_to_render: Vec<(usize, &TypesetPage)> = match canvas.page_view_mode {
                                                PageViewMode::Continuous => typeset_pages.iter().enumerate().collect(),
                                                PageViewMode::SinglePage => {
                                                    let idx = canvas.current_page.min(typeset_pages.len().saturating_sub(1));
                                                    vec![(idx, &typeset_pages[idx])]
                                                }
                                            };

                                            for (page_idx, page) in pages_to_render {
                                                ui.add_space(8.0);
                                                let effective_zoom = if canvas.fit_to_width {
                                                    ((ui.available_width() - 48.0) / page.width).clamp(0.4, 3.0)
                                                } else {
                                                    canvas.zoom
                                                };
                                                if let Some((line, column)) = NativePageRenderer::render_page_ui_with_mode(
                                                    ui,
                                                    page,
                                                    canvas.color_mode,
                                                    effective_zoom,
                                                ) {
                                                    action = ViewerAction::JumpToLocation { line, column };
                                                }
                                                if canvas.page_view_mode == PageViewMode::Continuous && typeset_pages.len() > 1 {
                                                    ui.add_space(4.0);
                                                    ui.label(
                                                        egui::RichText::new(format!("— Page {} of {} —", page_idx + 1, typeset_pages.len()))
                                                            .font(egui::FontId::proportional(11.0))
                                                            .color(Color32::from_gray(140)),
                                                    );
                                                }
                                                ui.add_space(16.0);
                                            }
                                        });
                                    });
                            });
                    }
                }
            }

        action
    }
}
