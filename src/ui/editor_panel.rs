//! # Code Editor Panel
//!
//! Provides the primary LaTeX source code editor ([`EditorPanel`]) with:
//! - Syntax highlighting via [`LatexLexer`]
//! - Line numbers and error marker gutters
//! - Integrated Search and Replace (`Ctrl+F` / `Ctrl+H`)
//! - Auto-scrolling and caret focusing on SyncTeX jumps from the outline or PDF preview
//! - Zoomable font size controls

use crate::compiler::Diagnostic;
use crate::document::lexer::LatexLexer;
use crate::t;
use egui::{vec2, Color32, FontId, Key, RichText, ScrollArea, Stroke, TextEdit, Ui};

/// Search and replace state for the editor.
#[derive(Debug, Clone, Default)]
pub struct FindReplaceState {
    /// Whether the search bar is currently visible
    pub is_open: bool,
    /// Whether replace controls are visible
    pub show_replace: bool,
    /// Query string to search for
    pub search_query: String,
    /// Replacement string
    pub replace_text: String,
    /// Case sensitive search toggle
    pub case_sensitive: bool,
    /// Match whole words only toggle
    pub whole_word: bool,
    /// Index of currently selected match (0-based)
    pub current_match_idx: usize,
    /// Cached matches: list of (char_start, char_end) ranges
    pub matches: Vec<(usize, usize)>,
    /// Match count or status string (e.g. "2 of 5", "No matches", "Replaced 3 match(es)")
    pub status: String,
    /// Flag requesting keyboard focus on the search input box
    pub focus_input: bool,
}

impl FindReplaceState {
    /// Scans the content text and caches all matching character ranges
    pub fn update_matches(&mut self, content: &str) {
        self.matches.clear();
        if self.search_query.is_empty() {
            self.status.clear();
            self.current_match_idx = 0;
            return;
        }

        let haystack: Vec<char> = content.chars().collect();
        let q_chars: Vec<char> = self.search_query.chars().collect();
        let q_len = q_chars.len();

        if q_len == 0 || haystack.len() < q_len {
            self.status = "No matches".to_string();
            self.current_match_idx = 0;
            return;
        }

        let mut i = 0;
        while i + q_len <= haystack.len() {
            let slice = &haystack[i..i + q_len];
            let is_match = if self.case_sensitive {
                slice == q_chars.as_slice()
            } else {
                slice
                    .iter()
                    .zip(q_chars.iter())
                    .all(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
            };

            if is_match {
                let is_word_valid = if self.whole_word {
                    let before_ok = if i == 0 {
                        true
                    } else {
                        !haystack[i - 1].is_alphanumeric() && haystack[i - 1] != '_'
                    };
                    let after_ok = if i + q_len >= haystack.len() {
                        true
                    } else {
                        !haystack[i + q_len].is_alphanumeric() && haystack[i + q_len] != '_'
                    };
                    before_ok && after_ok
                } else {
                    true
                };

                if is_word_valid {
                    self.matches.push((i, i + q_len));
                    i += q_len;
                    continue;
                }
            }
            i += 1;
        }

        if self.matches.is_empty() {
            self.status = t!("editor.no_matches");
            self.current_match_idx = 0;
        } else {
            if self.current_match_idx >= self.matches.len() {
                self.current_match_idx = 0;
            }
            self.status = t!("editor.match_count", "current" => self.current_match_idx + 1, "total" => self.matches.len());
        }
    }

    /// Advances to the next match index
    pub fn next_match(&mut self) -> Option<(usize, usize)> {
        if self.matches.is_empty() {
            return None;
        }
        self.current_match_idx = (self.current_match_idx + 1) % self.matches.len();
        self.status = t!("editor.match_count", "current" => self.current_match_idx + 1, "total" => self.matches.len());
        Some(self.matches[self.current_match_idx])
    }

    /// Moves to the previous match index
    pub fn prev_match(&mut self) -> Option<(usize, usize)> {
        if self.matches.is_empty() {
            return None;
        }
        self.current_match_idx = if self.current_match_idx == 0 {
            self.matches.len() - 1
        } else {
            self.current_match_idx - 1
        };
        self.status = t!("editor.match_count", "current" => self.current_match_idx + 1, "total" => self.matches.len());
        Some(self.matches[self.current_match_idx])
    }

    /// Replaces the currently focused match with `replace_text`
    pub fn replace_current(&mut self, content: &mut String) -> bool {
        if self.matches.is_empty() {
            return false;
        }
        let idx = self.current_match_idx.min(self.matches.len() - 1);
        let (start, end) = self.matches[idx];

        let mut chars: Vec<char> = content.chars().collect();
        let rep_chars: Vec<char> = self.replace_text.chars().collect();
        chars.splice(start..end, rep_chars);
        *content = chars.into_iter().collect();

        self.update_matches(content);
        if !self.matches.is_empty() && self.current_match_idx >= self.matches.len() {
            self.current_match_idx = 0;
        }
        true
    }

    /// Replaces all matches in the document with `replace_text`
    pub fn replace_all(&mut self, content: &mut String) -> bool {
        if self.matches.is_empty() {
            return false;
        }
        let count = self.matches.len();
        let mut chars: Vec<char> = content.chars().collect();
        let rep_chars: Vec<char> = self.replace_text.chars().collect();

        for &(start, end) in self.matches.iter().rev() {
            chars.splice(start..end, rep_chars.iter().copied());
        }
        *content = chars.into_iter().collect();

        self.update_matches(content);
        self.status = format!("Replaced {count} match(es)");
        true
    }
}

/// Primary source code editor widget.
pub struct EditorPanel {
    /// Pending 1-based line target for programmatic scrolling and caret positioning
    pub target_scroll_line: Option<usize>,
    /// Pending character range to highlight / select
    pub target_selection: Option<(usize, usize)>,
    /// Editor monospace font size in points
    pub font_size: f32,
    /// Find and replace panel state
    pub find_replace: FindReplaceState,
}

impl Default for EditorPanel {
    fn default() -> Self {
        Self {
            target_scroll_line: None,
            target_selection: None,
            font_size: 13.5,
            find_replace: FindReplaceState::default(),
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

    /// Opens the Find bar and focuses the input query field
    pub fn open_find(&mut self) {
        self.find_replace.is_open = true;
        self.find_replace.focus_input = true;
    }

    /// Opens the Find and Replace bar and focuses the input query field
    pub fn open_replace(&mut self) {
        self.find_replace.is_open = true;
        self.find_replace.show_replace = true;
        self.find_replace.focus_input = true;
    }

    /// Closes the Find & Replace bar and clears match state
    pub fn close_find_replace(&mut self) {
        self.find_replace.is_open = false;
        self.find_replace.matches.clear();
        self.find_replace.status.clear();
    }

    /// Positions caret and scrolls editor to the currently focused match
    pub fn select_and_scroll_to_match(&mut self, content: &str) {
        if self.find_replace.matches.is_empty() {
            return;
        }
        let idx = self.find_replace.current_match_idx.min(self.find_replace.matches.len() - 1);
        let (start_char, end_char) = self.find_replace.matches[idx];

        self.target_selection = Some((start_char, end_char));

        let line_num = content
            .chars()
            .take(start_char)
            .filter(|&c| c == '\n')
            .count()
            + 1;
        self.target_scroll_line = Some(line_num);
    }

    /// Renders the central code editor pane with line numbers, error margin badges,
    /// find/replace search bar, and live syntax highlighting. Returns `true` if text changed.
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
        let editor_id = egui::Id::new("latex_source_code_editor");

        // Handle Escape to close find/replace
        if self.find_replace.is_open && ui.input(|i| i.key_pressed(Key::Escape)) {
            self.close_find_replace();
            ui.ctx().memory_mut(|mem| mem.request_focus(editor_id));
        }

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

                let find_label = if self.find_replace.is_open {
                    "🔍 Find ✕"
                } else {
                    "🔍 Find (Ctrl+F)"
                };
                if ui
                    .button(find_label)
                    .on_hover_text("Search & Replace in document (Ctrl+F / Ctrl+H)")
                    .clicked()
                {
                    if self.find_replace.is_open {
                        self.close_find_replace();
                        ui.ctx().memory_mut(|mem| mem.request_focus(editor_id));
                    } else {
                        self.open_find();
                    }
                }
            });
        });

        // Search & Replace Bar
        if self.find_replace.is_open {
            ui.add_space(2.0);
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Find:").small().strong());
                    let find_input_id = egui::Id::new("editor_find_query_input");
                    let search_resp = ui.add(
                        egui::TextEdit::singleline(&mut self.find_replace.search_query)
                            .id(find_input_id)
                            .hint_text("Find in document...")
                            .desired_width(170.0),
                    );

                    if self.find_replace.focus_input {
                        search_resp.request_focus();
                        self.find_replace.focus_input = false;
                    }

                    if search_resp.changed() {
                        self.find_replace.update_matches(content);
                        if !self.find_replace.matches.is_empty() {
                            self.select_and_scroll_to_match(content);
                        }
                    }

                    // Enter and Shift+Enter navigation within the search input
                    if search_resp.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        if ui.input(|i| i.modifiers.shift) {
                            if self.find_replace.prev_match().is_some() {
                                self.select_and_scroll_to_match(content);
                            }
                        } else {
                            if self.find_replace.next_match().is_some() {
                                self.select_and_scroll_to_match(content);
                            }
                        }
                    }

                    // Match count badge
                    if !self.find_replace.search_query.is_empty() {
                        let badge_color = if self.find_replace.matches.is_empty() {
                            Color32::from_rgb(220, 80, 80)
                        } else if is_dark {
                            Color32::from_rgb(140, 190, 240)
                        } else {
                            Color32::from_rgb(40, 110, 200)
                        };
                        ui.label(RichText::new(&self.find_replace.status).small().color(badge_color));
                    }

                    // Toggles: Case Sensitivity & Whole Word
                    let case_btn = ui
                        .selectable_label(self.find_replace.case_sensitive, "Aa")
                        .on_hover_text("Match Case (Case Sensitive)");
                    if case_btn.clicked() {
                        self.find_replace.case_sensitive = !self.find_replace.case_sensitive;
                        self.find_replace.update_matches(content);
                        if !self.find_replace.matches.is_empty() {
                            self.select_and_scroll_to_match(content);
                        }
                    }

                    let word_btn = ui
                        .selectable_label(self.find_replace.whole_word, "\\b")
                        .on_hover_text("Match Whole Word");
                    if word_btn.clicked() {
                        self.find_replace.whole_word = !self.find_replace.whole_word;
                        self.find_replace.update_matches(content);
                        if !self.find_replace.matches.is_empty() {
                            self.select_and_scroll_to_match(content);
                        }
                    }

                    // Previous / Next buttons
                    if ui.button("▲").on_hover_text("Previous match (Shift+Enter)").clicked() {
                        if self.find_replace.prev_match().is_some() {
                            self.select_and_scroll_to_match(content);
                        }
                    }
                    if ui.button("▼").on_hover_text("Next match (Enter)").clicked() {
                        if self.find_replace.next_match().is_some() {
                            self.select_and_scroll_to_match(content);
                        }
                    }

                    let rep_toggle = ui
                        .button(if self.find_replace.show_replace { "▾ Replace" } else { "▸ Replace" })
                        .on_hover_text("Toggle Replace controls (Ctrl+H)");
                    if rep_toggle.clicked() {
                        self.find_replace.show_replace = !self.find_replace.show_replace;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("✕").on_hover_text("Close Search (Esc)").clicked() {
                            self.close_find_replace();
                            ui.ctx().memory_mut(|mem| mem.request_focus(editor_id));
                        }
                    });
                });

                if self.find_replace.show_replace {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Repl:").small().strong());
                        ui.add(
                            egui::TextEdit::singleline(&mut self.find_replace.replace_text)
                                .hint_text("Replace with...")
                                .desired_width(170.0),
                        );

                        if ui
                            .button("Replace")
                            .on_hover_text("Replace currently selected match and move to next")
                            .clicked()
                        {
                            if self.find_replace.replace_current(content) {
                                text_changed = true;
                                if !self.find_replace.matches.is_empty() {
                                    self.select_and_scroll_to_match(content);
                                }
                            }
                        }

                        if ui
                            .button("Replace All")
                            .on_hover_text("Replace all occurrences throughout document")
                            .clicked()
                        {
                            if self.find_replace.replace_all(content) {
                                text_changed = true;
                            }
                        }
                    });
                }
            });
            ui.add_space(2.0);
        }

        ui.separator();

        let font_id = FontId::monospace(self.font_size);

        let mut scroll = ScrollArea::vertical()
            .id_salt("editor_scroll_area")
            .auto_shrink([false, false]);

        if let Some((start_c, end_c)) = self.target_selection.take() {
            let mut state = egui::widgets::text_edit::TextEditState::load(ui.ctx(), editor_id).unwrap_or_default();
            state.cursor.set_char_range(Some(egui::text_selection::CCursorRange::two(
                egui::text::CCursor::new(start_c),
                egui::text::CCursor::new(end_c),
            )));
            state.store(ui.ctx(), editor_id);
            ui.ctx().memory_mut(|mem| mem.request_focus(editor_id));
        } else if let Some(target_line) = self.target_scroll_line {
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
        }

        if let Some(target_line) = self.target_scroll_line.take() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_matches_case_insensitive() {
        let mut state = FindReplaceState::default();
        state.search_query = "matrix".to_string();
        let content = "A matrix and another Matrix in a pmatrix block.";
        state.update_matches(content);

        assert_eq!(state.matches.len(), 3);
        assert_eq!(state.status, "1 of 3");

        // Next match navigation
        assert_eq!(state.next_match(), Some(state.matches[1]));
        assert_eq!(state.status, "2 of 3");
        assert_eq!(state.next_match(), Some(state.matches[2]));
        assert_eq!(state.status, "3 of 3");
        // Wraps around to 0
        assert_eq!(state.next_match(), Some(state.matches[0]));
        assert_eq!(state.status, "1 of 3");
    }

    #[test]
    fn test_find_matches_case_sensitive() {
        let mut state = FindReplaceState::default();
        state.search_query = "Matrix".to_string();
        state.case_sensitive = true;
        let content = "A matrix and another Matrix in a pmatrix block.";
        state.update_matches(content);

        assert_eq!(state.matches.len(), 1);
        assert_eq!(state.status, "1 of 1");
    }

    #[test]
    fn test_find_matches_whole_word() {
        let mut state = FindReplaceState::default();
        state.search_query = "matrix".to_string();
        state.whole_word = true;
        let content = "A matrix and another Matrix in a pmatrix block.";
        state.update_matches(content);

        // "pmatrix" should NOT match because 'p' precedes it
        assert_eq!(state.matches.len(), 2);
    }

    #[test]
    fn test_replace_current_and_replace_all() {
        let mut state = FindReplaceState::default();
        state.search_query = "foo".to_string();
        state.replace_text = "bar".to_string();
        let mut content = "foo test foo again foo".to_string();

        state.update_matches(&content);
        assert_eq!(state.matches.len(), 3);

        // Replace first occurrence
        assert!(state.replace_current(&mut content));
        assert_eq!(content, "bar test foo again foo");
        assert_eq!(state.matches.len(), 2);

        // Replace all remaining
        assert!(state.replace_all(&mut content));
        assert_eq!(content, "bar test bar again bar");
        assert_eq!(state.matches.len(), 0);
    }

    #[test]
    fn test_unicode_character_indices() {
        let mut state = FindReplaceState::default();
        state.search_query = "theta".to_string();
        let content = "Angle θ with theta parameter and θ_0.";
        state.update_matches(content);

        assert_eq!(state.matches.len(), 1);
        let (start, end) = state.matches[0];
        let sub: String = content.chars().skip(start).take(end - start).collect();
        assert_eq!(sub, "theta");
    }
}
