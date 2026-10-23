//! # Native Vector Page Renderer
//!
//! Renders paginated typeset documents directly onto the `egui` UI canvas with crisp
//! vector typography, proper drop shadows, margin framing, and double-click forward/backward
//! SyncTeX navigation to the code editor.

use crate::engine::ast::*;
use crate::engine::layout::{ElementKind, TypesetPage};
use crate::engine::math::MathRenderer;
use crate::viewer::PdfColorMode;
use egui::{vec2, Color32, FontId, Rect, RichText, Stroke, Ui};

/// Direct vector canvas renderer for native typeset pages.
pub struct NativePageRenderer;

impl NativePageRenderer {
    /// Renders a typeset page onto the UI canvas with boolean dark flag.
    pub fn render_page_ui(
        ui: &mut Ui,
        page: &TypesetPage,
        is_dark: bool,
        zoom: f32,
    ) -> Option<usize> {
        let mode = if is_dark {
            PdfColorMode::AcademicDark
        } else {
            PdfColorMode::Normal
        };
        Self::render_page_ui_with_mode(ui, page, mode, zoom)
    }

    /// Renders a typeset page onto the UI canvas supporting Normal, Dark, and Sepia modes.
    ///
    /// Returns `Some(source_line)` if the user double-clicked an element on this page,
    /// triggering backward SyncTeX navigation.
    pub fn render_page_ui_with_mode(
        ui: &mut Ui,
        page: &TypesetPage,
        color_mode: PdfColorMode,
        zoom: f32,
    ) -> Option<usize> {
        let display_width = page.width * zoom;
        let display_height = page.height * zoom;

        let (paper_bg, text_color, line_color) = match color_mode {
            PdfColorMode::AcademicDark => (
                Color32::from_rgb(26, 29, 36),
                Color32::from_rgb(225, 230, 240),
                Color32::from_rgb(60, 68, 85),
            ),
            PdfColorMode::Sepia => (
                Color32::from_rgb(248, 241, 227),
                Color32::from_rgb(60, 48, 38),
                Color32::from_rgb(210, 195, 175),
            ),
            PdfColorMode::Normal => (
                Color32::from_rgb(255, 255, 255),
                Color32::from_rgb(20, 24, 32),
                Color32::from_rgb(215, 220, 230),
            ),
        };
        let is_dark = color_mode.is_dark();

        let mut double_clicked_line = None;

        let margin_i8 = ((page.margin * zoom) as i32).clamp(0, 120) as i8;

        // Outer page container with drop shadow
        let frame_response = egui::Frame::new()
            .fill(paper_bg)
            .stroke(Stroke::new(1.0_f32, line_color))
            .inner_margin(egui::Margin::symmetric(margin_i8, margin_i8))
            .show(ui, |ui| {
                ui.set_width(display_width - (page.margin * 2.0 * zoom));
                ui.set_min_height(display_height - (page.margin * 2.0 * zoom));

                for element in &page.elements {
                    match &element.kind {
                        ElementKind::Title { text, font_size } => {
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(text)
                                        .font(FontId::proportional(*font_size * zoom))
                                        .strong()
                                        .color(text_color),
                                );
                            });
                        }
                        ElementKind::Author { text, font_size } => {
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(text)
                                        .font(FontId::proportional(*font_size * zoom))
                                        .color(text_color),
                                );
                            });
                        }
                        ElementKind::Date { text, font_size } => {
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(text)
                                        .font(FontId::proportional(*font_size * zoom))
                                        .color(Color32::from_gray(140)),
                                );
                            });
                        }
                        ElementKind::Divider => {
                            ui.add_space(4.0 * zoom);
                            ui.separator();
                            ui.add_space(4.0 * zoom);
                        }
                        ElementKind::Abstract { title, spans } => {
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(title)
                                        .font(FontId::proportional(11.0 * zoom))
                                        .strong()
                                        .color(text_color),
                                );
                            });
                            ui.indent("abstract_indent", |ui| {
                                Self::render_inline_spans(ui, spans, is_dark, zoom, 10.5);
                            });
                        }
                        ElementKind::Heading {
                            level,
                            number,
                            title,
                            font_size,
                        } => {
                            ui.add_space(8.0 * zoom);
                            let heading_str = if number.is_empty() {
                                title.clone()
                            } else {
                                format!("{number} {title}")
                            };
                            let mut text = RichText::new(heading_str)
                                .font(FontId::proportional(*font_size * zoom))
                                .color(text_color);
                            if *level <= 2 {
                                text = text.strong();
                            }
                            ui.label(text);
                        }
                        ElementKind::Paragraph(spans) => {
                            Self::render_inline_spans(ui, spans, is_dark, zoom, 11.0);
                        }
                        ElementKind::Equation { formula, number } => {
                            MathRenderer::render_equation_block(ui, formula, *number, is_dark);
                        }
                        ElementKind::List { ordered, items } => {
                            for (idx, item_spans) in items.iter().enumerate() {
                                ui.horizontal_top(|ui| {
                                    let bullet = if *ordered {
                                        format!("{}. ", idx + 1)
                                    } else {
                                        "• ".to_string()
                                    };
                                    ui.label(
                                        RichText::new(bullet)
                                            .font(FontId::proportional(11.0 * zoom))
                                            .strong()
                                            .color(text_color),
                                    );
                                    Self::render_inline_spans(ui, item_spans, is_dark, zoom, 11.0);
                                });
                            }
                        }
                        ElementKind::Table {
                            caption,
                            headers,
                            rows,
                            has_booktabs,
                        } => {
                            ui.vertical_centered(|ui| {
                                if *has_booktabs {
                                    ui.add_space(3.0 * zoom);
                                    ui.separator();
                                }
                                egui::Grid::new(format!("table_grid_{:p}", element))
                                    .striped(!*has_booktabs)
                                    .spacing(vec2(16.0 * zoom, 6.0 * zoom))
                                    .show(ui, |ui| {
                                        if !headers.is_empty() {
                                            for h in headers {
                                                ui.label(
                                                    RichText::new(h)
                                                        .font(FontId::proportional(11.5 * zoom))
                                                        .strong()
                                                        .color(text_color),
                                                );
                                            }
                                            ui.end_row();
                                        }

                                        for row in rows {
                                            for cell in &row.cells {
                                                Self::render_inline_spans(
                                                    ui, cell, is_dark, zoom, 10.5,
                                                );
                                            }
                                            ui.end_row();
                                        }
                                    });

                                if *has_booktabs {
                                    ui.separator();
                                }

                                if let Some(cap) = caption {
                                    ui.add_space(4.0 * zoom);
                                    ui.label(
                                        RichText::new(cap)
                                            .font(FontId::proportional(10.0 * zoom))
                                            .italics()
                                            .color(Color32::from_gray(140)),
                                    );
                                }
                            });
                        }
                        ElementKind::Figure {
                            caption,
                            image_path,
                            number,
                            ..
                        } => {
                            ui.vertical_centered(|ui| {
                                egui::Frame::new()
                                    .fill(if is_dark {
                                        Color32::from_rgb(32, 36, 44)
                                    } else {
                                        Color32::from_rgb(248, 250, 252)
                                    })
                                    .stroke(Stroke::new(1.0_f32, line_color))
                                    .corner_radius(4.0)
                                    .inner_margin(egui::Margin::symmetric(20, 14))
                                    .show(ui, |ui| {
                                        let img_label = image_path.as_deref().unwrap_or("Graphic Placeholder");
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("🖼").size(13.0 * zoom));
                                            ui.label(
                                                RichText::new(format!("[ {img_label} ]"))
                                                    .font(FontId::monospace(10.5 * zoom))
                                                    .color(if is_dark {
                                                        Color32::from_rgb(180, 200, 225)
                                                    } else {
                                                        Color32::from_rgb(70, 85, 105)
                                                    }),
                                            );
                                        });
                                    });

                                if let Some(cap_text) = caption.as_deref() {
                                    if !cap_text.is_empty() {
                                        ui.add_space(4.0 * zoom);
                                        ui.label(
                                            RichText::new(format!("Figure {number}: {cap_text}"))
                                                .font(FontId::proportional(10.5 * zoom))
                                                .italics()
                                                .color(text_color),
                                        );
                                    }
                                }
                            });
                        }
                        ElementKind::CodeBlock { language, code } => {
                            let line_tokens_list = crate::engine::syntax::SyntaxHighlighter::tokenize_for_ui(code, language, is_dark);
                            egui::Frame::new()
                                .fill(if is_dark {
                                    Color32::from_rgb(30, 33, 42)
                                } else {
                                    Color32::from_rgb(246, 248, 250)
                                })
                                .stroke(Stroke::new(1.0_f32, line_color))
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::symmetric(12, 10))
                                .show(ui, |ui| {
                                    if !language.is_empty() {
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                            ui.label(
                                                RichText::new(language)
                                                    .font(FontId::monospace(9.0 * zoom))
                                                    .color(Color32::from_gray(140)),
                                            );
                                        });
                                    }
                                    ui.vertical(|ui| {
                                        for line_tokens in line_tokens_list {
                                            ui.horizontal(|ui| {
                                                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                                                if line_tokens.is_empty() {
                                                    ui.label(RichText::new(" ").font(FontId::monospace(10.0 * zoom)));
                                                } else {
                                                    for (token_text, token_color) in line_tokens {
                                                        ui.label(
                                                            RichText::new(token_text)
                                                                .font(FontId::monospace(10.0 * zoom))
                                                                .color(token_color),
                                                        );
                                                    }
                                                }
                                            });
                                        }
                                    });
                                });
                        }
                        ElementKind::Space(amt) => {
                            ui.add_space(*amt * zoom);
                        }
                    }
                }

                // Page footer: Page X of Y
                ui.add_space(16.0 * zoom);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(format!("— {} —", page.page_number))
                            .font(FontId::proportional(10.0 * zoom))
                            .color(Color32::from_gray(130)),
                    );
                });
            });

        // Double click detection on Native page
        let page_rect: Rect = frame_response.response.rect;
        if ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary)) {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                if page_rect.contains(pos) {
                    let rel_y_unzoomed = ((pos.y - page_rect.top()) / zoom) - page.margin;
                    if let Some(el) = page.find_element_at_y(rel_y_unzoomed) {
                        double_clicked_line = Some(el.source_line);
                    }
                }
            }
        }

        double_clicked_line
    }

    fn render_inline_spans(
        ui: &mut Ui,
        spans: &[InlineSpan],
        is_dark: bool,
        zoom: f32,
        base_size: f32,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 2.0);

            for span in spans {
                match span {
                    InlineSpan::Text(t) => {
                        ui.label(
                            RichText::new(t)
                                .font(FontId::proportional(base_size * zoom))
                                .color(if is_dark {
                                    Color32::from_rgb(220, 225, 235)
                                } else {
                                    Color32::from_rgb(25, 30, 40)
                                }),
                        );
                    }
                    InlineSpan::Bold(t) => {
                        ui.label(
                            RichText::new(t)
                                .font(FontId::proportional(base_size * zoom))
                                .strong()
                                .color(if is_dark {
                                    Color32::from_rgb(240, 245, 255)
                                } else {
                                    Color32::from_rgb(10, 15, 20)
                                }),
                        );
                    }
                    InlineSpan::Italic(t) => {
                        ui.label(
                            RichText::new(t)
                                .font(FontId::proportional(base_size * zoom))
                                .italics()
                                .color(if is_dark {
                                    Color32::from_rgb(215, 225, 240)
                                } else {
                                    Color32::from_rgb(40, 45, 55)
                                }),
                        );
                    }
                    InlineSpan::Code(t) => {
                        ui.label(
                            RichText::new(t)
                                .font(FontId::monospace((base_size - 1.0) * zoom))
                                .background_color(if is_dark {
                                    Color32::from_rgb(40, 45, 55)
                                } else {
                                    Color32::from_rgb(240, 242, 246)
                                })
                                .color(if is_dark {
                                    Color32::from_rgb(220, 230, 245)
                                } else {
                                    Color32::from_rgb(30, 35, 45)
                                }),
                        );
                    }
                    InlineSpan::Math(m) => {
                        let clean = MathRenderer::translate_math_symbols(m);
                        ui.label(
                            RichText::new(clean)
                                .font(FontId::proportional(base_size * zoom))
                                .italics()
                                .color(if is_dark {
                                    Color32::from_rgb(242, 163, 101)
                                } else {
                                    Color32::from_rgb(180, 80, 0)
                                }),
                        );
                    }
                    InlineSpan::Colored(c, t) => {
                        ui.label(
                            RichText::new(t)
                                .font(FontId::proportional(base_size * zoom))
                                .color(*c),
                        );
                    }
                    InlineSpan::Url(u) => {
                        ui.hyperlink(u);
                    }
                    InlineSpan::Ref(r) | InlineSpan::Cite(r) => {
                        ui.label(
                            RichText::new(format!("[{r}]"))
                                .font(FontId::proportional(base_size * zoom))
                                .color(Color32::from_rgb(97, 175, 239)),
                        );
                    }
                }
            }
        });
    }
}
