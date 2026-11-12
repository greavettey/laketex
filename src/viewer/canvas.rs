//! Interactive scrollable preview canvas for rasterized PDF pages.

use crate::viewer::text_selection::PageTextLayout;
use crate::viewer::{PdfColorMode, PdfRenderer};
use egui::{vec2, Color32, Pos2, Rect, ScrollArea, Stroke, TextureHandle, TextureOptions, Ui};
use serde::{Deserialize, Serialize};

/// Page layout display mode in the document preview canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageViewMode {
    /// Scroll continuously through all rendered pages (Overleaf style)
    Continuous,
    /// View one page at a time with Prev / Next page buttons
    SinglePage,
}

impl Default for PageViewMode {
    fn default() -> Self {
        Self::Continuous
    }
}

/// Active text selection on a page.
#[derive(Debug, Clone, PartialEq)]
pub struct TextSelection {
    pub page: usize,
    pub start_char: usize,
    pub end_char: usize,
    pub text: String,
}

/// Interactive event emitted by user interaction on the PDF canvas.
#[derive(Debug, Clone, PartialEq)]
pub enum CanvasEvent {
    /// Double-click to jump to code: (page_index, rel_x, rel_y)
    DoubleClick(usize, f32, f32),
    /// User selected/highlighted text on a page
    TextSelected {
        page: usize,
        text: String,
        start_pos: Pos2,
        end_pos: Pos2,
    },
    /// Selection was cleared (e.g. single click without dragging)
    ClearSelection,
}

pub struct PdfCanvas {
    pub current_page: usize,
    pub total_pages: usize,
    pub zoom: f32,
    pub color_mode: PdfColorMode,
    pub invert_colors: bool,
    pub base_render_width: u16,
    pub fit_to_width: bool,
    pub page_view_mode: PageViewMode,

    /// Active text selection on a page
    pub text_selection: Option<TextSelection>,
    /// Legacy normalized selection box (retained for backward compatibility)
    pub selection: Option<(usize, Pos2, Pos2)>,
    /// Cached text layout for each rendered page
    pub page_text_layouts: Vec<Option<PageTextLayout>>,
    /// Additional normalized highlight rectangles to render on specific pages (e.g. from editor selection)
    pub highlight_rects: Vec<(usize, Rect)>,

    pub texture: Option<TextureHandle>,
    pub page_textures: Vec<Option<TextureHandle>>,
    pub last_error: Option<String>,
    drag_anchor_boundary: Option<usize>,
    last_rendered_page: Option<usize>,
    last_rendered_zoom: Option<f32>,
    last_rendered_invert: Option<bool>,
    last_rendered_mode: Option<PdfColorMode>,
    last_rendered_base_width: Option<u16>,
    last_rendered_view_mode: Option<PageViewMode>,
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
            page_view_mode: PageViewMode::Continuous,
            text_selection: None,
            selection: None,
            page_text_layouts: Vec::new(),
            highlight_rects: Vec::new(),
            texture: None,
            page_textures: Vec::new(),
            last_error: None,
            drag_anchor_boundary: None,
            last_rendered_page: None,
            last_rendered_zoom: None,
            last_rendered_invert: None,
            last_rendered_mode: None,
            last_rendered_base_width: None,
            last_rendered_view_mode: None,
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
        self.page_textures.clear();
        self.page_text_layouts.clear();
        self.last_pdf_hash = None;
        self.last_rendered_page = None;
        self.last_rendered_zoom = None;
        self.last_rendered_invert = None;
        self.last_rendered_mode = None;
        self.last_rendered_base_width = None;
        self.last_rendered_view_mode = None;
        self.last_error = None;
        self.text_selection = None;
        self.selection = None;
        self.drag_anchor_boundary = None;
        self.highlight_rects.clear();
        self.current_page = 0;
        self.total_pages = 1;
    }

    /// Invalidates cached PDF texture and hash to force re-rasterization on the next frame.
    pub fn invalidate(&mut self) {
        self.last_pdf_hash = None;
        self.texture = None;
        self.page_textures.clear();
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
        let view_mode_changed = self.last_rendered_view_mode != Some(self.page_view_mode);

        match self.page_view_mode {
            PageViewMode::Continuous => {
                let needs_rerender = hash_changed
                    || mode_changed
                    || width_changed
                    || zoom_changed
                    || view_mode_changed
                    || self.page_textures.is_empty()
                    || (self.page_textures.iter().all(|t| t.is_none()) && self.last_error.is_none());

                if needs_rerender {
                    match renderer.render_page_to_color_image_with_mode(
                        bytes,
                        0,
                        target_width,
                        self.color_mode,
                    ) {
                        Ok((color_image_0, count)) => {
                            self.last_error = None;
                            self.total_pages = count.max(1);
                            self.page_textures.clear();
                            self.page_textures.reserve(self.total_pages);

                            let tex_0 = ui.ctx().load_texture(
                                "pdf_page_0",
                                color_image_0,
                                TextureOptions::LINEAR,
                            );
                            self.page_textures.push(Some(tex_0.clone()));
                            self.texture = Some(tex_0);

                            for p in 1..self.total_pages {
                                if let Ok((img, _)) = renderer.render_page_to_color_image_with_mode(
                                    bytes,
                                    p,
                                    target_width,
                                    self.color_mode,
                                ) {
                                    let tex = ui.ctx().load_texture(
                                        format!("pdf_page_{p}"),
                                        img,
                                        TextureOptions::LINEAR,
                                    );
                                    self.page_textures.push(Some(tex));
                                } else {
                                    self.page_textures.push(None);
                                }
                            }

                            // Extract text layouts for all pages
                            if hash_changed || self.page_text_layouts.is_empty() {
                                self.page_text_layouts.clear();
                                self.page_text_layouts.reserve(self.total_pages);
                                for p in 0..self.total_pages {
                                    let layout = renderer.extract_page_text_layout(bytes, p).ok();
                                    self.page_text_layouts.push(layout);
                                }
                            }

                            self.last_pdf_hash = Some(current_hash);
                            self.last_rendered_page = Some(self.current_page);
                            self.last_rendered_zoom = Some(self.zoom);
                            self.last_rendered_invert = Some(self.invert_colors);
                            self.last_rendered_mode = Some(self.color_mode);
                            self.last_rendered_base_width = Some(self.base_render_width);
                            self.last_rendered_view_mode = Some(self.page_view_mode);
                        }
                        Err(e) => {
                            let err_str = e.to_string();
                            if self.last_error.as_ref() != Some(&err_str) {
                                eprintln!("[Laketex] Render page error: {err_str}");
                            }
                            self.last_error = Some(err_str);
                            self.last_pdf_hash = Some(current_hash);
                            self.last_rendered_view_mode = Some(self.page_view_mode);
                        }
                    }
                }
            }
            PageViewMode::SinglePage => {
                let needs_rerender = (self.texture.is_none() && self.last_error.is_none())
                    || hash_changed
                    || page_changed
                    || mode_changed
                    || width_changed
                    || zoom_changed
                    || view_mode_changed;

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
                            let tex = ui.ctx().load_texture(
                                format!("pdf_page_{}", self.current_page),
                                color_image,
                                TextureOptions::LINEAR,
                            );
                            self.texture = Some(tex);

                            // Extract text layout for current page
                            if hash_changed || self.page_text_layouts.len() <= self.current_page || self.page_text_layouts[self.current_page].is_none() {
                                if self.page_text_layouts.len() < self.total_pages {
                                    self.page_text_layouts.resize(self.total_pages, None);
                                }
                                self.page_text_layouts[self.current_page] = renderer.extract_page_text_layout(bytes, self.current_page).ok();
                            }

                            self.last_pdf_hash = Some(current_hash);
                            self.last_rendered_page = Some(self.current_page);
                            self.last_rendered_zoom = Some(self.zoom);
                            self.last_rendered_invert = Some(self.invert_colors);
                            self.last_rendered_mode = Some(self.color_mode);
                            self.last_rendered_base_width = Some(self.base_render_width);
                            self.last_rendered_view_mode = Some(self.page_view_mode);
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
                            self.last_rendered_view_mode = Some(self.page_view_mode);
                        }
                    }
                }
            }
        }
    }

    /// Renders the page inside a scrollable canvas.
    ///
    /// Emits `CanvasEvent` for double-clicking (SyncTeX line jump), dragging (highlighting / text range selection),
    /// or single clicking (clearing highlights).
    pub fn show(&mut self, ui: &mut Ui) -> Option<CanvasEvent> {
        let available_size = ui.available_size();
        let mut canvas_event = None;

        let bg_color = match self.color_mode {
            PdfColorMode::AcademicDark => Color32::from_rgb(18, 20, 24),
            PdfColorMode::Sepia => Color32::from_rgb(243, 235, 218),
            PdfColorMode::Normal => Color32::from_rgb(238, 240, 245),
        };

        egui::Frame::NONE
            .fill(bg_color)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                if self.page_view_mode == PageViewMode::Continuous && !self.page_textures.is_empty() {
                    ScrollArea::both()
                        .id_salt("pdf_canvas_scroll_continuous")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                for page_idx in 0..self.page_textures.len() {
                                    if let Some((tex_id, tex_size)) = self.page_textures[page_idx]
                                        .as_ref()
                                        .map(|t| (t.id(), t.size_vec2()))
                                    {
                                        ui.add_space(8.0);
                                        let aspect = tex_size.y / tex_size.x.max(1.0);
                                        let display_width = if self.fit_to_width {
                                            (available_size.x - 48.0).clamp(240.0, 1400.0)
                                        } else {
                                            (available_size.x - 48.0).clamp(240.0, 1400.0) * self.zoom
                                        };
                                        let display_size = vec2(display_width, display_width * aspect);

                                        if let Some(ev) = self.render_page_with_interactions(
                                            ui,
                                            page_idx,
                                            tex_id,
                                            display_size,
                                            true,
                                        ) {
                                            canvas_event = Some(ev);
                                        }

                                        if self.total_pages > 1 {
                                            ui.add_space(4.0);
                                            ui.label(
                                                egui::RichText::new(format!("— Page {} of {} —", page_idx + 1, self.total_pages))
                                                    .font(egui::FontId::proportional(11.0))
                                                    .color(if self.invert_colors {
                                                        Color32::from_gray(110)
                                                    } else {
                                                        Color32::from_gray(140)
                                                    }),
                                            );
                                        }
                                        ui.add_space(16.0);
                                    }
                                }
                            });
                        });
                } else if let Some((tex_id, tex_size)) = self.texture.as_ref().map(|t| (t.id(), t.size_vec2())) {
                    let aspect = tex_size.y / tex_size.x.max(1.0);

                    let display_width = if self.fit_to_width {
                        (available_size.x - 48.0).clamp(240.0, 1400.0)
                    } else {
                        (available_size.x - 48.0).clamp(240.0, 1400.0) * self.zoom
                    };
                    let display_size = vec2(display_width, display_width * aspect);
                    let page_idx = self.current_page;

                    ScrollArea::both()
                        .id_salt("pdf_canvas_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.add_space(8.0);
                                if let Some(ev) = self.render_page_with_interactions(
                                    ui,
                                    page_idx,
                                    tex_id,
                                    display_size,
                                    false,
                                ) {
                                    canvas_event = Some(ev);
                                }
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

        canvas_event
    }

    /// Renders a single PDF page image, handles double-click and boundary-accurate drag selection,
    /// and draws active selection and editor-sync highlight overlays.
    fn render_page_with_interactions(
        &mut self,
        ui: &mut Ui,
        page_idx: usize,
        tex_id: egui::TextureId,
        display_size: egui::Vec2,
        update_current_page_on_click: bool,
    ) -> Option<CanvasEvent> {
        let mut canvas_event = None;
        let (rect, response) = ui.allocate_exact_size(display_size, egui::Sense::click_and_drag());
        response.clone().on_hover_cursor(egui::CursorIcon::Text);

        let norm_pos = |p: Pos2| {
            Pos2::new(
                ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
                ((p.y - rect.top()) / rect.height()).clamp(0.0, 1.0),
            )
        };

        if response.double_clicked() {
            self.drag_anchor_boundary = None;
            if let Some(pos) = response.interact_pointer_pos() {
                let rel = norm_pos(pos);
                canvas_event = Some(CanvasEvent::DoubleClick(page_idx, rel.x, rel.y));
            }
        } else if response.drag_started() {
            if let Some(start_pos) = ui.input(|i| i.pointer.press_origin()) {
                let rel_start = norm_pos(start_pos);
                let anchor_b = self
                    .page_text_layouts
                    .get(page_idx)
                    .and_then(|opt| opt.as_ref())
                    .and_then(|layout| layout.find_caret_boundary_at(rel_start))
                    .unwrap_or(0);
                self.drag_anchor_boundary = Some(anchor_b);
                self.text_selection = None;
                self.selection = Some((page_idx, rel_start, rel_start));
            }
        } else if response.dragged() {
            if let Some(current_pos) = response.interact_pointer_pos() {
                let start_pos = ui.input(|i| i.pointer.press_origin()).unwrap_or(current_pos);
                let rel_start = norm_pos(start_pos);
                let rel_curr = norm_pos(current_pos);

                if let Some(layout) = self.page_text_layouts.get(page_idx).and_then(|opt| opt.as_ref()) {
                    let anchor_b = self
                        .drag_anchor_boundary
                        .unwrap_or_else(|| layout.find_caret_boundary_at(rel_start).unwrap_or(0));
                    let curr_b = layout.find_caret_boundary_at(rel_curr).unwrap_or(anchor_b);
                    let min_b = anchor_b.min(curr_b);
                    let max_b = anchor_b.max(curr_b);

                    if min_b < max_b {
                        let start_char = min_b;
                        let end_char = max_b - 1;
                        let selected_text = layout.get_text_range(start_char, end_char);
                        self.text_selection = Some(TextSelection {
                            page: page_idx,
                            start_char,
                            end_char,
                            text: selected_text.clone(),
                        });
                        self.selection = Some((page_idx, rel_start, rel_curr));

                        if !selected_text.trim().is_empty() {
                            let s_pos = layout
                                .chars
                                .get(start_char)
                                .filter(|c| c.rect.width() > 0.0001)
                                .map(|c| c.rect.center())
                                .unwrap_or(rel_start);
                            let e_pos = layout
                                .chars
                                .get(end_char)
                                .filter(|c| c.rect.width() > 0.0001)
                                .map(|c| c.rect.center())
                                .unwrap_or(rel_curr);
                            canvas_event = Some(CanvasEvent::TextSelected {
                                page: page_idx,
                                text: selected_text,
                                start_pos: s_pos,
                                end_pos: e_pos,
                            });
                        }
                    } else {
                        self.text_selection = None;
                        self.selection = Some((page_idx, rel_start, rel_curr));
                    }
                }
            }
        } else if response.clicked() {
            if update_current_page_on_click {
                self.current_page = page_idx;
            }
            self.drag_anchor_boundary = None;
            self.text_selection = None;
            self.selection = None;
            canvas_event = Some(CanvasEvent::ClearSelection);
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

        painter.image(
            tex_id,
            rect,
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );

        let paint_norm_rect = |norm_r: Rect, color: Color32| {
            let screen_rect = Rect::from_min_max(
                Pos2::new(rect.left() + norm_r.left() * rect.width(), rect.top() + norm_r.top() * rect.height()),
                Pos2::new(rect.left() + norm_r.right() * rect.width(), rect.top() + norm_r.bottom() * rect.height()),
            );
            painter.rect_filled(screen_rect, 1.5, color);
        };

        if let Some(sel) = &self.text_selection {
            if sel.page == page_idx {
                if let Some(layout) = self.page_text_layouts.get(page_idx).and_then(|l| l.as_ref()) {
                    for l_rect in layout.get_selection_rects(sel.start_char, sel.end_char) {
                        paint_norm_rect(l_rect, Color32::from_rgba_unmultiplied(66, 133, 244, 95));
                    }
                }
            }
        }

        for (p_idx, h_rect) in &self.highlight_rects {
            if *p_idx == page_idx {
                paint_norm_rect(*h_rect, Color32::from_rgba_unmultiplied(250, 204, 21, 95));
            }
        }

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

        canvas_event
    }
}
