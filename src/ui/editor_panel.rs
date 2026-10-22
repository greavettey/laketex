//! # Code Editor Panel
//!
//! Provides the primary LaTeX source code editor ([`EditorPanel`]) with:
//! - Syntax highlighting via [`LatexLexer`]
//! - Line numbers and error marker gutters
//! - Auto-scrolling and caret focusing on SyncTeX jumps from the outline or PDF preview
//! - Zoomable font size controls

use crate::compiler::Diagnostic;
use crate::document::lexer::LatexLexer;
use egui::{vec2, Color32, FontId, RichText, ScrollArea, Stroke, TextEdit, Ui};

/// Primary source code editor widget.
pub struct EditorPanel {
    /// Pending 1-based line target for programmatic scrolling and caret positioning
    pub target_scroll_line: Option<usize>,
    /// Editor monospace font size in points
    pub font_size: f32,
}

impl Default for EditorPanel {
    fn default() -> Self {
        Self {
            target_scroll_line: None,
            font_size: 13.5,
        }
    }
}

impl EditorPanel {
    /// Creates a new editor panel
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests that the editor smoothly scroll and place the caret at `line`.
    pub fn jump_to_line(&mut self, line: usize) {
        self.target_scroll_line = Some(line);
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        content: &mut String,
        diagnostics: &[Diagnostic],
        is_dark: bool,
    ) -> bool {
        let mut text_changed = false;
        let line_count = content.lines().count().max(1);
        let error_lines: Vec<usize> = diagnostics
            .iter()
            .filter(|d| d.severity == crate::compiler::DiagnosticSeverity::Error)
            .filter_map(|d| d.line)
            .collect();

        let gutter_width = format!("{line_count}").len() as f32 * 9.0 + 20.0;

        ui.horizontal(|ui| {
            // Editor header / info line
            ui.label(
                RichText::new("LaTeX Source")
                    .strong()
                    .color(Color32::from_rgb(97, 175, 239)),
            );

            if !error_lines.is_empty() {
                ui.colored_label(
                    Color32::from_rgb(235, 87, 87),
                    format!("• {} line(s) with errors", error_lines.len()),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("A+").clicked() {
                    self.font_size = (self.font_size + 1.0).min(24.0);
                }
                if ui.button("A-").clicked() {
                    self.font_size = (self.font_size - 1.0).max(10.0);
                }
                ui.label(format!("{:.0}pt", self.font_size));
                ui.label(format!("Lines: {line_count}"));
            });
        });

        ui.separator();

        let font_id = FontId::monospace(self.font_size);

        let editor_id = egui::Id::new("latex_source_code_editor");

        let mut scroll = ScrollArea::vertical()
            .id_salt("editor_scroll_area")
            .auto_shrink([false, false]);

        if let Some(target_line) = self.target_scroll_line.take() {
            let mut char_idx = 0;
            for (idx, line) in content.lines().enumerate() {
                if idx + 1 == target_line {
                    break;
                }
                char_idx += line.chars().count() + 1;
            }
            char_idx = char_idx.min(content.chars().count());

            let mut state = egui::widgets::text_edit::TextEditState::load(ui.ctx(), editor_id).unwrap_or_default();
            state.cursor.set_char_range(Some(egui::text_selection::CCursorRange::one(egui::text::CCursor::new(char_idx))));
            state.store(ui.ctx(), editor_id);

            ui.ctx().memory_mut(|mem| mem.request_focus(editor_id));

            let row_height = ui.fonts(|f| f.row_height(&font_id));
            let target_y = (target_line.saturating_sub(4) as f32 * row_height).max(0.0);
            scroll = scroll.vertical_scroll_offset(target_y);
        }

        scroll.show(ui, |ui| {
            ui.horizontal_top(|ui| {
                // Line numbers gutter
                ui.allocate_ui_with_layout(
                    vec2(gutter_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::RIGHT),
                    |ui| {
                        let gutter_color = if is_dark {
                            Color32::from_rgb(90, 95, 110)
                        } else {
                            Color32::from_rgb(150, 155, 165)
                        };

                        for i in 1..=line_count {
                            let has_error = error_lines.contains(&i);
                            if has_error {
                                ui.colored_label(
                                    Color32::from_rgb(235, 87, 87),
                                    format!("{i:>4} ●"),
                                );
                            } else {
                                ui.colored_label(gutter_color, format!("{i:>4} "));
                            }
                        }
                    },
                );

                // Separator line between gutter and text edit
                let stroke_color = if is_dark {
                    Color32::from_rgb(45, 50, 60)
                } else {
                    Color32::from_rgb(215, 220, 230)
                };
                let (sep_rect, _) = ui.allocate_exact_size(vec2(1.0, ui.available_height()), egui::Sense::hover());
                ui.painter().vline(sep_rect.left(), sep_rect.y_range(), Stroke::new(1.0_f32, stroke_color));

                ui.add_space(4.0);

                // Main syntax-highlighted code editor
                let mut layouter = |ui: &Ui, text: &str, wrap_width: f32| {
                    let mut job = LatexLexer::highlight(text, is_dark, font_id.clone());
                    job.wrap.max_width = wrap_width;
                    ui.fonts(|f| f.layout_job(job))
                };

                let response = ui.add_sized(
                    ui.available_size(),
                    TextEdit::multiline(content)
                        .id(editor_id)
                        .font(font_id.clone())
                        .code_editor()
                        .layouter(&mut layouter)
                        .lock_focus(true)
                        .desired_width(f32::INFINITY),
                );

                    if response.changed() {
                        text_changed = true;
                    }
                });
            });

        text_changed
    }
}
