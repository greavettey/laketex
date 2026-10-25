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
use egui::{vec2, Align, Align2, Color32, FontId, Key, Pos2, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui};

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
    /// Last known character range of the cursor/caret in the editor
    pub last_cursor_range: Option<(usize, usize)>,
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
            last_cursor_range: None,
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

    /// Inserts a snippet into content at the current caret or selection position,
    /// or before `\end{document}` if no caret is recorded. Returns the new character index.
    pub fn insert_snippet_at_caret(
        &mut self,
        ctx: &egui::Context,
        content: &mut String,
        snippet: &str,
    ) -> usize {
        let editor_id = egui::Id::new("latex_source_code_editor");
        let active_char_range = self.last_cursor_range.or_else(|| {
            egui::widgets::text_edit::TextEditState::load(ctx, editor_id)
                .and_then(|state| state.cursor.char_range().map(|r| {
                    (
                        r.primary.index.min(r.secondary.index),
                        r.primary.index.max(r.secondary.index),
                    )
                }))
        });

        let new_pos = if let Some((start_char, end_char)) = active_char_range {
            let total_chars = content.chars().count();
            let start = start_char.min(total_chars);
            let end = end_char.min(total_chars);

            let byte_start = content
                .char_indices()
                .nth(start)
                .map(|(b, _)| b)
                .unwrap_or(content.len());
            let byte_end = content
                .char_indices()
                .nth(end)
                .map(|(b, _)| b)
                .unwrap_or(content.len());

            content.replace_range(byte_start..byte_end, snippet);
            start + snippet.chars().count()
        } else if let Some(byte_pos) = content.rfind("\\end{document}") {
            let char_pos = content[..byte_pos].chars().count();
            content.insert_str(byte_pos, snippet);
            char_pos + snippet.chars().count()
        } else {
            let char_pos = content.chars().count();
            content.push_str(snippet);
            char_pos + snippet.chars().count()
        };

        self.last_cursor_range = Some((new_pos, new_pos));
        self.target_selection = Some((new_pos, new_pos));

        let mut state = egui::widgets::text_edit::TextEditState::load(ctx, editor_id).unwrap_or_default();
        state.cursor.set_char_range(Some(egui::text_selection::CCursorRange::one(
            egui::text::CCursor::new(new_pos),
        )));
        state.store(ctx, editor_id);
        ctx.memory_mut(|mem| mem.request_focus(editor_id));

        new_pos
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

        let editor_id = egui::Id::new("latex_source_code_editor");

        // Handle Escape to close find/replace
        if self.find_replace.is_open && ui.input(|i| i.key_pressed(Key::Escape)) {
            self.close_find_replace();
            ui.ctx().memory_mut(|mem| mem.request_focus(editor_id));
        }

        // 1. Editor header / info line
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), crate::ui::PANEL_HEADER_HEIGHT),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_height(crate::ui::PANEL_HEADER_HEIGHT);
                ui.spacing_mut().item_spacing = egui::vec2(5.0, 0.0);

                ui.add(
                    egui::Label::new(
                        RichText::new("LaTeX Source")
                            .strong()
                            .color(Color32::from_rgb(97, 175, 239)),
                    )
                    .selectable(false),
                );

                if !error_lines.is_empty() {
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("• {} line(s) with errors", error_lines.len()))
                                .color(Color32::from_rgb(235, 87, 87)),
                        )
                        .selectable(false),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("A+").clicked() {
                        self.font_size = (self.font_size + 1.0).min(24.0);
                    }
                    if ui.button("A-").clicked() {
                        self.font_size = (self.font_size - 1.0).max(10.0);
                    }
                    ui.add(egui::Label::new(format!("{:.0}pt", self.font_size)).selectable(false));
                    ui.add(egui::Label::new(format!("Lines: {line_count}")).selectable(false));

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
            },
        );

        ui.separator();

        // Search & Replace Bar
        if self.find_replace.is_open {
            let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter));
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
                        // If editor had a single-line selection, pre-populate search query
                        if let Some(state) = egui::widgets::text_edit::TextEditState::load(ui.ctx(), editor_id) {
                            if let Some(range) = state.cursor.char_range() {
                                let (s, e) = (
                                    range.primary.index.min(range.secondary.index),
                                    range.primary.index.max(range.secondary.index),
                                );
                                if s < e {
                                    let selected_text: String = content.chars().skip(s).take(e - s).collect();
                                    if !selected_text.contains('\n') && !selected_text.trim().is_empty() {
                                        self.find_replace.search_query = selected_text;
                                        self.find_replace.update_matches(content);
                                    }
                                }
                            }
                        }

                        search_resp.request_focus();
                        let mut state = egui::widgets::text_edit::TextEditState::load(ui.ctx(), find_input_id).unwrap_or_default();
                        state.cursor.set_char_range(Some(egui::text_selection::CCursorRange::two(
                            egui::text::CCursor::new(0),
                            egui::text::CCursor::new(self.find_replace.search_query.chars().count()),
                        )));
                        state.store(ui.ctx(), find_input_id);
                        self.find_replace.focus_input = false;
                    }

                    if search_resp.changed() {
                        self.find_replace.update_matches(content);
                        if !self.find_replace.matches.is_empty() {
                            self.select_and_scroll_to_match(content);
                        }
                    }

                    // Enter and Shift+Enter navigation within the search input (also supports F3 / Ctrl+G)
                    let f3_pressed = ui.input(|i| i.key_pressed(Key::F3));
                    let ctrl_g_pressed = ui.input(|i| (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(Key::G));
                    let search_input_active = search_resp.has_focus() || (search_resp.lost_focus() && enter_pressed);

                    if (search_input_active && enter_pressed) || f3_pressed || ctrl_g_pressed {
                        let is_shift = ui.input(|i| i.modifiers.shift)
                            || ui.input(|i| {
                                i.events.iter().any(|e| {
                                    matches!(
                                        e,
                                        egui::Event::Key {
                                            key: Key::Enter | Key::F3 | Key::G,
                                            modifiers,
                                            ..
                                        } if modifiers.shift
                                    )
                                })
                            });

                        if is_shift {
                            if self.find_replace.prev_match().is_some() {
                                self.select_and_scroll_to_match(content);
                            }
                        } else {
                            if self.find_replace.next_match().is_some() {
                                self.select_and_scroll_to_match(content);
                            }
                        }
                        search_resp.request_focus();
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
                        search_resp.request_focus();
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
                        search_resp.request_focus();
                    }

                    // Previous / Next buttons
                    if ui.button("▲").on_hover_text("Previous match (Shift+Enter)").clicked() {
                        if self.find_replace.prev_match().is_some() {
                            self.select_and_scroll_to_match(content);
                        }
                        search_resp.request_focus();
                    }
                    if ui.button("▼").on_hover_text("Next match (Enter)").clicked() {
                        if self.find_replace.next_match().is_some() {
                            self.select_and_scroll_to_match(content);
                        }
                        search_resp.request_focus();
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
                        let rep_resp = ui.add(
                            egui::TextEdit::singleline(&mut self.find_replace.replace_text)
                                .hint_text("Replace with...")
                                .desired_width(170.0),
                        );

                        let rep_input_active = rep_resp.has_focus() || (rep_resp.lost_focus() && enter_pressed);
                        if rep_input_active && enter_pressed {
                            if self.find_replace.replace_current(content) {
                                text_changed = true;
                                if !self.find_replace.matches.is_empty() {
                                    self.select_and_scroll_to_match(content);
                                }
                            }
                            rep_resp.request_focus();
                        }

                        if ui
                            .button("Replace")
                            .on_hover_text("Replace currently selected match and move to next (Enter)")
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
            ui.separator();
        }

        // Ctrl+Mouse Wheel zoom for editor font size (scoped to editor panel hover)
        let ctrl_font_zoom = if ui.ui_contains_pointer() {
            ui.input(|i| {
                if (i.modifiers.command || i.modifiers.ctrl) && i.raw_scroll_delta.y != 0.0 {
                    if i.raw_scroll_delta.y > 0.0 {
                        Some(0.5)
                    } else {
                        Some(-0.5)
                    }
                } else {
                    None
                }
            })
        } else {
            None
        };
        if let Some(delta) = ctrl_font_zoom {
            self.font_size = (self.font_size + delta).clamp(10.0, 24.0);
        }

        let font_id = FontId::monospace(self.font_size);
        let char_width = ui.fonts(|f| f.glyph_width(&font_id, '0'));
        let gutter_digits = format!("{line_count}").len().max(4);
        let gutter_width = (gutter_digits as f32 + 2.5) * char_width;

        let scroll = ScrollArea::vertical()
            .id_salt("editor_scroll_area")
            .auto_shrink([false, false]);

        let mut scroll_target_range: Option<(usize, usize)> = None;

        if let Some((start_c, end_c)) = self.target_selection.take() {
            let mut state = egui::widgets::text_edit::TextEditState::load(ui.ctx(), editor_id).unwrap_or_default();
            state.cursor.set_char_range(Some(egui::text_selection::CCursorRange::two(
                egui::text::CCursor::new(start_c),
                egui::text::CCursor::new(end_c),
            )));
            state.store(ui.ctx(), editor_id);
            scroll_target_range = Some((start_c, end_c));
            self.target_scroll_line = None;
            // Crucial: Do NOT request focus on editor_id here!
            // When navigating or typing search terms, focus MUST remain in the search input box.
        } else if let Some(target_line) = self.target_scroll_line.take() {
            let mut char_idx = 0;
            if target_line > 1 {
                let mut current_line = 1;
                for (idx, ch) in content.chars().enumerate() {
                    if ch == '\n' {
                        current_line += 1;
                        if current_line == target_line {
                            char_idx = idx + 1;
                            break;
                        }
                    }
                }
            }
            char_idx = char_idx.min(content.chars().count());

            let mut state = egui::widgets::text_edit::TextEditState::load(ui.ctx(), editor_id).unwrap_or_default();
            state.cursor.set_char_range(Some(egui::text_selection::CCursorRange::one(egui::text::CCursor::new(char_idx))));
            state.store(ui.ctx(), editor_id);
            scroll_target_range = Some((char_idx, char_idx));

            // Only request focus on the editor if find/replace is closed (e.g. outline click, diagnostic jump)
            if !self.find_replace.is_open {
                ui.ctx().memory_mut(|mem| mem.request_focus(editor_id));
            }
        }

        let mut clicked_gutter_line: Option<usize> = None;

        scroll.show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

                // Reserve gutter width
                let (gutter_rect, _) = ui.allocate_exact_size(vec2(gutter_width, 0.0), Sense::hover());

                // Separator line rect
                let (sep_rect, _) = ui.allocate_exact_size(vec2(1.0, 0.0), Sense::hover());

                // Padding between separator and text
                let (_space_rect, _) = ui.allocate_exact_size(vec2(4.0, 0.0), Sense::hover());

                // Main syntax-highlighted code editor
                let mut layouter = |ui: &Ui, text: &str, wrap_width: f32| {
                    let mut job = LatexLexer::highlight(text, is_dark, font_id.clone());
                    job.wrap.max_width = wrap_width;
                    ui.fonts(|f| f.layout_job(job))
                };

                let output = TextEdit::multiline(content)
                    .id(editor_id)
                    .font(font_id.clone())
                    .code_editor()
                    .layouter(&mut layouter)
                    .lock_focus(true)
                    .desired_width(f32::INFINITY)
                    .show(ui);

                if output.response.changed() {
                    text_changed = true;
                }

                // Update last known cursor range from output or TextEditState
                if let Some(r) = output.cursor_range {
                    let s = r.primary.ccursor.index.min(r.secondary.ccursor.index);
                    let e = r.primary.ccursor.index.max(r.secondary.ccursor.index);
                    self.last_cursor_range = Some((s, e));
                } else if let Some(state) = egui::widgets::text_edit::TextEditState::load(ui.ctx(), editor_id) {
                    if let Some(r) = state.cursor.char_range() {
                        let s = r.primary.index.min(r.secondary.index);
                        let e = r.primary.index.max(r.secondary.index);
                        self.last_cursor_range = Some((s, e));
                    }
                }

                let clip_rect = ui.clip_rect();

                // Accurately scroll to target words or lines using actual galley layout geometry
                if let Some((s, e)) = scroll_target_range {
                    let c1 = egui::text::CCursor::new(s);
                    let c2 = egui::text::CCursor::new(e.max(s));
                    let cur1 = output.galley.from_ccursor(c1);
                    let cur2 = output.galley.from_ccursor(c2);
                    let range = egui::text_selection::CursorRange::two(cur1, cur2);
                    let [min, max] = range.sorted_cursors();
                    if min.rcursor.row < output.galley.rows.len() {
                        let row = &output.galley.rows[min.rcursor.row];
                        let left = row.x_offset(min.rcursor.column);
                        let right = if min.rcursor.row == max.rcursor.row && s != e {
                            row.x_offset(max.rcursor.column)
                        } else {
                            left + 4.0
                        };
                        let target_rect = Rect::from_min_max(
                            Pos2::new(left, row.min_y()),
                            Pos2::new(right, row.max_y()),
                        ).translate(output.galley_pos.to_vec2());
                        ui.scroll_to_rect(target_rect, Some(Align::Center));
                    }
                }

                // Highlight occurrences in Find & Replace modal
                if self.find_replace.is_open && !self.find_replace.matches.is_empty() {
                    let current_idx = self.find_replace.current_match_idx.min(self.find_replace.matches.len().saturating_sub(1));

                    for (idx, &(start_char, end_char)) in self.find_replace.matches.iter().enumerate() {
                        let is_current = idx == current_idx;
                        let c1 = egui::text::CCursor::new(start_char);
                        let c2 = egui::text::CCursor::new(end_char);
                        let cur1 = output.galley.from_ccursor(c1);
                        let cur2 = output.galley.from_ccursor(c2);
                        let range = egui::text_selection::CursorRange::two(cur1, cur2);
                        let [min, max] = range.sorted_cursors();

                        for ri in min.rcursor.row..=max.rcursor.row {
                            if ri >= output.galley.rows.len() {
                                break;
                            }
                            let row = &output.galley.rows[ri];
                            let left = if ri == min.rcursor.row {
                                row.x_offset(min.rcursor.column)
                            } else {
                                row.rect.left()
                            };
                            let right = if ri == max.rcursor.row {
                                row.x_offset(max.rcursor.column)
                            } else {
                                row.rect.right()
                            };

                            let match_rect = Rect::from_min_max(
                                Pos2::new(left, row.min_y()),
                                Pos2::new(right, row.max_y()),
                            ).translate(output.galley_pos.to_vec2());

                            if match_rect.intersects(clip_rect) {
                                if is_current {
                                    // Current active match: vibrant amber highlight box with crisp golden border
                                    let fill_color = if is_dark {
                                        Color32::from_rgba_unmultiplied(255, 170, 0, 110)
                                    } else {
                                        Color32::from_rgba_unmultiplied(255, 160, 0, 130)
                                    };
                                    let stroke_color = if is_dark {
                                        Color32::from_rgb(255, 215, 0)
                                    } else {
                                        Color32::from_rgb(220, 120, 0)
                                    };
                                    ui.painter().rect(
                                        match_rect.expand(1.0),
                                        2.0,
                                        fill_color,
                                        Stroke::new(1.5_f32, stroke_color),
                                        StrokeKind::Outside,
                                    );
                                } else {
                                    // Other matches: subtle translucent highlight
                                    let fill_color = if is_dark {
                                        Color32::from_rgba_unmultiplied(255, 220, 50, 45)
                                    } else {
                                        Color32::from_rgba_unmultiplied(255, 200, 0, 60)
                                    };
                                    ui.painter().rect_filled(match_rect.expand(0.5), 1.5, fill_color);
                                }
                            }
                        }
                    }
                }

                // Draw separator line full height
                let stroke_color = if is_dark {
                    Color32::from_rgb(45, 50, 60)
                } else {
                    Color32::from_rgb(215, 220, 230)
                };
                let full_height = output.response.rect.height().max(ui.available_height());
                let sep_top = output.galley_pos.y;
                let sep_bottom = (sep_top + full_height).max(output.response.rect.bottom());
                ui.painter().vline(sep_rect.left(), sep_top..=sep_bottom, Stroke::new(1.0_f32, stroke_color));

                // Contextual line highlighting: determine active line being edited from cursor
                let active_line = output.cursor_range.map(|r| {
                    content.chars().take(r.primary.ccursor.index).filter(|&c| c == '\n').count() + 1
                }).or_else(|| {
                    egui::widgets::text_edit::TextEditState::load(ui.ctx(), editor_id).and_then(|state| {
                        state.cursor.char_range().map(|r| {
                            content.chars().take(r.primary.index).filter(|&c| c == '\n').count() + 1
                        })
                    })
                });

                // Paint gutter line numbers and diagnostics aligned with the galley's visual rows
                let gutter_color = if is_dark {
                    Color32::from_rgb(90, 95, 110)
                } else {
                    Color32::from_rgb(150, 155, 165)
                };
                let error_color = Color32::from_rgb(235, 87, 87);

                let mut current_logical_line = 1;
                let mut is_first_row_of_logical_line = true;

                for row in &output.galley.rows {
                    let row_top = output.galley_pos.y + row.rect.top();
                    let row_bottom = row_top + row.rect.height();

                    if is_first_row_of_logical_line {
                        if row_bottom >= clip_rect.top() && row_top <= clip_rect.bottom() {
                            let has_error = error_lines.contains(&current_logical_line);
                            let is_active_line = active_line == Some(current_logical_line);

                            let label_text = if has_error {
                                format!("{current_logical_line:>gutter_digits$} ●")
                            } else {
                                format!("{current_logical_line:>gutter_digits$} ")
                            };
                            let text_color = if has_error {
                                error_color
                            } else if is_active_line {
                                if is_dark {
                                    Color32::WHITE
                                } else {
                                    Color32::from_rgb(20, 25, 35)
                                }
                            } else {
                                gutter_color
                            };

                            ui.painter().text(
                                Pos2::new(gutter_rect.right() - 2.0, row_top),
                                Align2::RIGHT_TOP,
                                label_text,
                                font_id.clone(),
                                text_color,
                            );

                            let row_rect = Rect::from_min_size(
                                Pos2::new(gutter_rect.left(), row_top),
                                vec2(gutter_width, row.rect.height()),
                            );

                            if has_error {
                                let line_errors: Vec<&str> = diagnostics
                                    .iter()
                                    .filter(|d| d.line == Some(current_logical_line) && d.severity == crate::compiler::DiagnosticSeverity::Error)
                                    .map(|d| d.message.as_str())
                                    .collect();
                                if !line_errors.is_empty() {
                                    ui.interact(row_rect, ui.id().with(("gutter_diag", current_logical_line)), Sense::hover())
                                        .on_hover_text(line_errors.join("\n"));
                                }
                            }

                            let row_resp = ui.interact(
                                row_rect,
                                ui.id().with(("gutter_click", current_logical_line)),
                                Sense::click(),
                            );
                            if row_resp.clicked() {
                                clicked_gutter_line = Some(current_logical_line);
                            }
                        }
                    }

                    if row.ends_with_newline {
                        current_logical_line += 1;
                        is_first_row_of_logical_line = true;
                    } else {
                        is_first_row_of_logical_line = false;
                    }
                }
            });
        });

        if let Some(line) = clicked_gutter_line {
            self.jump_to_line(line);
        }

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

    #[test]
    fn test_select_and_scroll_to_match() {
        let mut panel = EditorPanel::new();
        panel.find_replace.search_query = "second".to_string();
        let content = "first line\nsecond line\nthird line";
        panel.find_replace.update_matches(content);
        assert_eq!(panel.find_replace.matches.len(), 1);

        panel.select_and_scroll_to_match(content);
        assert_eq!(panel.target_selection, Some((11, 17)));
        assert_eq!(panel.target_scroll_line, Some(2));
    }

    #[test]
    fn test_active_editing_line_detection() {
        let content = "first line\nsecond line is very long\nthird line\n";
        let compute_line = |char_idx: usize| -> usize {
            content.chars().take(char_idx).filter(|&c| c == '\n').count() + 1
        };

        assert_eq!(compute_line(0), 1);
        assert_eq!(compute_line(5), 1);
        assert_eq!(compute_line(10), 1); // at first '\n'
        assert_eq!(compute_line(11), 2); // 's' in second
        assert_eq!(compute_line(25), 2);
        assert_eq!(compute_line(35), 2); // at second '\n'
        assert_eq!(compute_line(36), 3); // 't' in third
        assert_eq!(compute_line(47), 4); // after trailing '\n'
    }

    #[test]
    fn test_insert_snippet_at_caret() {
        let ctx = egui::Context::default();
        let mut panel = EditorPanel::new();
        let mut content = "Let $x = $ and finish.".to_string();

        // Caret is right after "$x = " (char index 9)
        panel.last_cursor_range = Some((9, 9));
        let new_pos = panel.insert_snippet_at_caret(&ctx, &mut content, "\\alpha ");

        assert_eq!(content, "Let $x = \\alpha $ and finish.");
        assert_eq!(new_pos, 16);
        assert_eq!(panel.last_cursor_range, Some((16, 16)));
        assert_eq!(panel.target_selection, Some((16, 16)));
    }

    #[test]
    fn test_insert_snippet_replaces_selection() {
        let ctx = egui::Context::default();
        let mut panel = EditorPanel::new();
        let mut content = "Replace [SELECTED] text.".to_string();

        // Selection covers "[SELECTED]" (chars 8 to 18)
        panel.last_cursor_range = Some((8, 18));
        let new_pos = panel.insert_snippet_at_caret(&ctx, &mut content, "\\beta");

        assert_eq!(content, "Replace \\beta text.");
        assert_eq!(new_pos, 13);
        assert_eq!(panel.last_cursor_range, Some((13, 13)));
    }

    #[test]
    fn test_insert_snippet_fallback_before_end_document() {
        let ctx = egui::Context::default();
        let mut panel = EditorPanel::new();
        let mut content = "\\begin{document}\nHello\n\\end{document}\n".to_string();

        // No caret recorded
        panel.last_cursor_range = None;
        let new_pos = panel.insert_snippet_at_caret(&ctx, &mut content, "\\begin{equation}\nx=1\n\\end{equation}\n");

        assert!(content.contains("\\begin{equation}\nx=1\n\\end{equation}\n\\end{document}"));
        assert!(content.ends_with("\\end{document}\n"));
        assert_eq!(panel.last_cursor_range, Some((new_pos, new_pos)));
    }

    #[test]
    fn test_insert_snippet_unicode_character_indices() {
        let ctx = egui::Context::default();
        let mut panel = EditorPanel::new();
        // String contains multi-byte Unicode characters: θ (2 bytes), λ (2 bytes)
        let mut content = "Angle θ and λ = .".to_string();

        // Caret is at the '.' (char index 16)
        let caret_char = content.chars().position(|c| c == '.').unwrap();
        panel.last_cursor_range = Some((caret_char, caret_char));
        panel.insert_snippet_at_caret(&ctx, &mut content, "\\gamma");

        assert_eq!(content, "Angle θ and λ = \\gamma.");
    }
}
