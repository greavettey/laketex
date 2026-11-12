//! Text layout extraction, selection, and exact LaTeX source character correlation.

use egui::{Pos2, Rect};
use std::collections::HashSet;

/// Normalized character bounding box and metadata on a rendered page.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfCharLayout {
    /// Unicode character
    pub c: char,
    /// Normalized coordinates in [0..1, 0..1] relative to page (origin at top-left)
    pub rect: Rect,
    /// 0-based character index within the page's text sequence
    pub index: usize,
}

/// Normalized text line span on a rendered page.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfLineLayout {
    /// Character start index (inclusive)
    pub char_start: usize,
    /// Character end index (inclusive)
    pub char_end: usize,
    /// Normalized bounding box of the entire line
    pub rect: Rect,
    /// Extracted line string
    pub text: String,
}

/// Context around a double-click caret position in PDF text.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfClickContext {
    /// Normalized text immediately before the clicked insertion boundary.
    pub prefix: String,
    /// Normalized text immediately after the clicked insertion boundary.
    pub suffix: String,
    /// True if the pointer landed on/after the right half of the preceding glyph.
    pub caret_after_prev: bool,
    /// Normalized coordinate on page for SyncTeX tie-breaking.
    pub anchor_pos: Pos2,
}

/// Text layout metadata for a single page, containing characters, lines, and page dimensions.
#[derive(Debug, Clone, PartialEq)]
pub struct PageTextLayout {
    /// 0-based page index
    pub page: usize,
    /// Individual characters with normalized bounding boxes
    pub chars: Vec<PdfCharLayout>,
    /// Line spans with bounding boxes
    pub lines: Vec<PdfLineLayout>,
    /// Full text of the page
    pub full_text: String,
    /// True PDF page width in points (default US Letter 612pt)
    pub page_width_pt: f32,
    /// True PDF page height in points (default US Letter 792pt)
    pub page_height_pt: f32,
}

impl Default for PageTextLayout {
    fn default() -> Self {
        Self {
            page: 0,
            chars: Vec::new(),
            lines: Vec::new(),
            full_text: String::new(),
            page_width_pt: 612.0,
            page_height_pt: 792.0,
        }
    }
}

fn is_valid_glyph_rect(rect: Rect) -> bool {
    rect.width() > 0.0001 && rect.height() > 0.0001
}

impl PageTextLayout {
    /// Returns true if the page has no extracted text characters.
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// Finds the visible glyph closest to normalized position `pos` (returning char index and vertical distance).
    fn find_nearest_glyph(&self, pos: Pos2) -> Option<(&PdfCharLayout, f32)> {
        if self.chars.is_empty() {
            return None;
        }

        // 1. Direct hit inside a visible character bounding box (prefer closest center horizontally)
        let mut best_direct: Option<&PdfCharLayout> = None;
        let mut best_direct_dx = f32::MAX;
        for ch in &self.chars {
            if is_valid_glyph_rect(ch.rect) && ch.rect.contains(pos) {
                let dx = (pos.x - ch.rect.center().x).abs();
                if dx < best_direct_dx {
                    best_direct_dx = dx;
                    best_direct = Some(ch);
                }
            }
        }
        if let Some(ch) = best_direct {
            return Some((ch, 0.0));
        }

        // 2. Nearest character weighted heavily toward vertical line alignment
        let mut best_ch: Option<&PdfCharLayout> = None;
        let mut best_dist_sq = f32::MAX;
        let mut best_dy = f32::MAX;

        for ch in &self.chars {
            if !is_valid_glyph_rect(ch.rect) {
                continue;
            }
            let center = ch.rect.center();
            let dx = pos.x - center.x;
            let dy = if pos.y < ch.rect.top() {
                ch.rect.top() - pos.y
            } else if pos.y > ch.rect.bottom() {
                pos.y - ch.rect.bottom()
            } else {
                0.0
            };
            let dist_sq = dx * dx + dy * dy * 25.0;
            if dist_sq < best_dist_sq {
                best_dist_sq = dist_sq;
                best_dy = dy;
                best_ch = Some(ch);
            }
        }

        best_ch.map(|ch| (ch, best_dy))
    }

    /// Finds the character index closest to a normalized pointer position (0..1, 0..1).
    pub fn find_char_at(&self, pos: Pos2) -> Option<usize> {
        self.find_nearest_glyph(pos).map(|(ch, _)| ch.index)
    }

    /// Finds the insertion boundary index in `0..=self.chars.len()` relative to left/right glyph halves.
    pub fn find_caret_boundary_at(&self, pos: Pos2) -> Option<usize> {
        let (ch, _) = self.find_nearest_glyph(pos)?;
        let idx = if pos.x >= ch.rect.center().x {
            (ch.index + 1).min(self.chars.len())
        } else {
            ch.index
        };
        Some(idx)
    }

    /// Extracts local text context before and after a double-click spot for exact source matching.
    /// Returns `None` if no glyph is near `pos` vertically (e.g. clicking inside a figure/image).
    pub fn extract_click_context(&self, pos: Pos2) -> Option<PdfClickContext> {
        let (ch, dy) = self.find_nearest_glyph(pos)?;
        let line_tol = (ch.rect.height() * 3.5).max(0.025);
        if dy > line_tol {
            return None;
        }

        let caret_after_prev = pos.x >= ch.rect.center().x;
        let split_idx = if caret_after_prev {
            (ch.index + 1).min(self.chars.len())
        } else {
            ch.index
        };

        let left_start = split_idx.saturating_sub(140);
        let right_end = (split_idx + 140).min(self.chars.len());

        let prefix = normalize_pdf_char_slice(&self.chars[left_start..split_idx], true, false);
        let suffix = normalize_pdf_char_slice(&self.chars[split_idx..right_end], false, true);

        if prefix.trim().is_empty() && suffix.trim().is_empty() {
            return None;
        }

        Some(PdfClickContext {
            prefix,
            suffix,
            caret_after_prev,
            anchor_pos: ch.rect.center(),
        })
    }

    /// Returns the text extracted between two character indices (inclusive).
    pub fn get_text_range(&self, from_idx: usize, to_idx: usize) -> String {
        if self.chars.is_empty() {
            return String::new();
        }
        let min_i = from_idx.min(to_idx).min(self.chars.len().saturating_sub(1));
        let max_i = from_idx.max(to_idx).min(self.chars.len().saturating_sub(1));
        self.chars[min_i..=max_i].iter().map(|c| c.c).collect()
    }

    /// Computes cohesive, horizontal highlight rectangles for a character range [from_idx, to_idx].
    pub fn get_selection_rects(&self, from_idx: usize, to_idx: usize) -> Vec<Rect> {
        if self.chars.is_empty() {
            return Vec::new();
        }

        let min_i = from_idx.min(to_idx).min(self.chars.len().saturating_sub(1));
        let max_i = from_idx.max(to_idx).min(self.chars.len().saturating_sub(1));

        let mut line_rects: Vec<Rect> = Vec::new();
        let mut current_line: Option<Rect> = None;

        for i in min_i..=max_i {
            let ch = &self.chars[i];
            if !is_valid_glyph_rect(ch.rect) {
                continue;
            }

            match current_line {
                None => current_line = Some(ch.rect),
                Some(line) => {
                    let y_diff = (ch.rect.center().y - line.center().y).abs();
                    let same_line = y_diff < line.height().max(ch.rect.height()) * 0.65;

                    if same_line {
                        current_line = Some(line.union(ch.rect));
                    } else {
                        line_rects.push(line);
                        current_line = Some(ch.rect);
                    }
                }
            }
        }

        if let Some(line) = current_line {
            line_rects.push(line);
        }

        line_rects
    }

    /// Searches for a query string in the page's text and returns line-by-line highlight rectangles.
    pub fn find_text_rects(&self, query: &str) -> Vec<Rect> {
        let norm_query = normalize_visible_string(query, false, false).to_lowercase();
        if norm_query.is_empty() || self.chars.is_empty() {
            return Vec::new();
        }

        let (norm_full, norm_to_pdf_idx) = normalize_pdf_chars_with_map(&self.chars);
        let full_lower = norm_full.to_lowercase();
        let norm_chars: Vec<char> = full_lower.chars().collect();
        let byte_offsets = char_byte_offsets(&full_lower);

        let mut rects = Vec::new();
        let mut search_start = 0;
        while let Some(byte_idx) = full_lower[search_start..].find(&norm_query) {
            let abs_byte = search_start + byte_idx;
            let end_byte = abs_byte + norm_query.len();
            if let (Some(c_start), Some(c_end_excl)) = (
                byte_to_char_idx(&byte_offsets, abs_byte),
                byte_to_char_idx(&byte_offsets, end_byte),
            ) {
                if c_start < c_end_excl && c_end_excl <= norm_chars.len() {
                    let pdf_start = norm_to_pdf_idx[c_start];
                    let pdf_end = norm_to_pdf_idx[c_end_excl - 1];
                    rects.extend(self.get_selection_rects(pdf_start, pdf_end));
                }
            }
            search_start = abs_byte + norm_query.len().max(1);
        }

        rects
    }
}

/// Maps one normalized visible character back to its exact span in the LaTeX source.
#[derive(Debug, Clone)]
struct MappedChar {
    ch: char,
    src_start: usize,
    src_end: usize,
    line: usize,
}

fn push_normalized_char(c: char, out: &mut String) {
    match c {
        '\u{FB00}' => out.push_str("ff"),
        '\u{FB01}' => out.push_str("fi"),
        '\u{FB02}' => out.push_str("fl"),
        '\u{FB03}' => out.push_str("ffi"),
        '\u{FB04}' => out.push_str("ffl"),
        '\u{FB05}' | '\u{FB06}' => out.push_str("st"),
        '“' | '”' | '„' | '‟' => out.push('"'),
        '‘' | '’' | '‚' | '‛' | '`' => out.push('\''),
        '—' | '–' | '\u{2212}' | '‐' => out.push('-'),
        '\u{00A0}' | '\u{2009}' | '\u{202F}' | '\r' | '\n' | '\t' => out.push(' '),
        '\u{00AD}' => {}
        other => out.push(other),
    }
}

fn normalize_visible_string(input: &str, keep_leading_ws: bool, keep_trailing_ws: bool) -> String {
    let mut raw = String::with_capacity(input.len());
    for c in input.chars() {
        push_normalized_char(c, &mut raw);
    }
    collapse_spaces(&raw, keep_leading_ws, keep_trailing_ws)
}

fn normalize_pdf_char_slice(chars: &[PdfCharLayout], keep_trailing_ws: bool, keep_leading_ws: bool) -> String {
    let mut raw = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i].c;
        // Join line-break hyphenation (`foo-\nbar` -> `foobar`) when alphabetic on left and right
        if c == '-'
            && i > 0
            && chars[i - 1].c.is_alphabetic()
            && i + 1 < chars.len()
            && matches!(chars[i + 1].c, '\r' | '\n')
        {
            let mut j = i + 1;
            while j < chars.len() && matches!(chars[j].c, '\r' | '\n' | ' ') {
                j += 1;
            }
            if j < chars.len() && chars[j].c.is_alphabetic() {
                i = j;
                continue;
            }
        }
        push_normalized_char(c, &mut raw);
        i += 1;
    }
    collapse_spaces(&raw, keep_leading_ws, keep_trailing_ws)
}

fn normalize_pdf_chars_with_map(chars: &[PdfCharLayout]) -> (String, Vec<usize>) {
    let mut out = String::with_capacity(chars.len());
    let mut map = Vec::with_capacity(chars.len());
    let mut prev_space = true;

    for ch in chars {
        let mut expanded = String::with_capacity(3);
        push_normalized_char(ch.c, &mut expanded);
        for nc in expanded.chars() {
            if nc == ' ' {
                if !prev_space {
                    out.push(' ');
                    map.push(ch.index);
                    prev_space = true;
                }
            } else {
                out.push(nc);
                map.push(ch.index);
                prev_space = false;
            }
        }
    }
    while out.ends_with(' ') {
        out.pop();
        map.pop();
    }
    (out, map)
}

fn collapse_spaces(input: &str, keep_leading_ws: bool, keep_trailing_ws: bool) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_space = !keep_leading_ws;
    for c in input.chars() {
        if c == ' ' {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    if !keep_trailing_ws && out.ends_with(' ') {
        out.pop();
    }
    out
}

fn push_mapped_str(mapped: &mut Vec<MappedChar>, text: &str, src_start: usize, src_end: usize, line: usize) {
    for c in text.chars() {
        if c == ' ' {
            push_mapped_space(mapped, src_start, src_end, line);
        } else {
            mapped.push(MappedChar { ch: c, src_start, src_end, line });
        }
    }
}

fn push_mapped_space(mapped: &mut Vec<MappedChar>, src_start: usize, src_end: usize, line: usize) {
    if mapped.last().map(|m| m.ch == ' ').unwrap_or(true) {
        return;
    }
    mapped.push(MappedChar {
        ch: ' ',
        src_start,
        src_end,
        line,
    });
}

fn skip_ws_inline(src: &[char], i: &mut usize) {
    while *i < src.len() && matches!(src[*i], ' ' | '\t') {
        *i += 1;
    }
}

fn skip_ws_and_newlines(src: &[char], i: &mut usize, line: &mut usize) {
    while *i < src.len() {
        match src[*i] {
            ' ' | '\t' | '\r' => *i += 1,
            '\n' => {
                *line += 1;
                *i += 1;
            }
            _ => break,
        }
    }
}

fn skip_comment_eol(src: &[char], i: &mut usize, line: &mut usize) {
    while *i < src.len() && src[*i] != '\n' {
        *i += 1;
    }
    if *i < src.len() && src[*i] == '\n' {
        *line += 1;
        *i += 1;
    }
    skip_ws_inline(src, i);
}

fn skip_delimited_group(src: &[char], i: &mut usize, line: &mut usize, open: char, close: char) -> bool {
    let mut probe = *i;
    let mut probe_line = *line;
    skip_ws_and_newlines(src, &mut probe, &mut probe_line);
    if src.get(probe) != Some(&open) {
        return false;
    }
    probe += 1;
    let mut depth = 1usize;
    while probe < src.len() && depth > 0 {
        let c = src[probe];
        if c == '\\' && probe + 1 < src.len() {
            probe += 2;
            continue;
        }
        if c == '\n' {
            probe_line += 1;
        } else if c == '%' {
            while probe < src.len() && src[probe] != '\n' {
                probe += 1;
            }
            continue;
        } else if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
        }
        probe += 1;
    }
    *i = probe;
    *line = probe_line;
    true
}

fn read_brace_text(src: &[char], i: &mut usize, line: &mut usize) -> Option<String> {
    let mut probe = *i;
    let mut probe_line = *line;
    skip_ws_and_newlines(src, &mut probe, &mut probe_line);
    if src.get(probe) != Some(&'{') {
        return None;
    }
    probe += 1;
    let start = probe;
    let mut depth = 1usize;
    while probe < src.len() && depth > 0 {
        match src[probe] {
            '\\' if probe + 1 < src.len() => probe += 2,
            '\n' => {
                probe_line += 1;
                probe += 1;
            }
            '{' => {
                depth += 1;
                probe += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let s: String = src[start..probe].iter().collect();
                    *i = probe + 1;
                    *line = probe_line;
                    return Some(s);
                }
                probe += 1;
            }
            _ => probe += 1,
        }
    }
    None
}

fn matches_slice_at(src: &[char], pos: usize, pattern: &str) -> bool {
    let pat_chars: Vec<char> = pattern.chars().collect();
    if pos + pat_chars.len() > src.len() {
        return false;
    }
    src[pos..pos + pat_chars.len()] == pat_chars[..]
}

fn find_uncommented_tag(src: &[char], start: usize, tag: &str) -> Option<usize> {
    let mut i = start;
    let mut in_comment = false;
    while i < src.len() {
        if src[i] == '\n' {
            in_comment = false;
            i += 1;
            continue;
        }
        if in_comment {
            i += 1;
            continue;
        }
        if src[i] == '%' && (i == 0 || src[i - 1] != '\\') {
            in_comment = true;
            i += 1;
            continue;
        }
        if matches_slice_at(src, i, tag) {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn parse_latex_range_into_map(
    src: &[char],
    start: usize,
    end: usize,
    mut line: usize,
    mapped: &mut Vec<MappedChar>,
) -> usize {
    let mut i = start;
    while i < end {
        let c = src[i];
        match c {
            '\r' => {
                i += 1;
            }
            '\n' => {
                let s = i;
                i += 1;
                line += 1;
                push_mapped_space(mapped, s, i, line - 1);
            }
            ' ' | '\t' => {
                let s = i;
                skip_ws_inline(src, &mut i);
                push_mapped_space(mapped, s, i, line);
            }
            '%' => {
                skip_comment_eol(src, &mut i, &mut line);
            }
            '~' => {
                push_mapped_space(mapped, i, i + 1, line);
                i += 1;
            }
            '{' | '}' => {
                i += 1;
            }
            '$' => {
                if src.get(i + 1) == Some(&'$') {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            '&' => {
                push_mapped_space(mapped, i, i + 1, line);
                i += 1;
            }
            '`' => {
                if src.get(i + 1) == Some(&'`') {
                    mapped.push(MappedChar { ch: '"', src_start: i, src_end: i + 2, line });
                    i += 2;
                } else {
                    mapped.push(MappedChar { ch: '\'', src_start: i, src_end: i + 1, line });
                    i += 1;
                }
            }
            '\'' => {
                if src.get(i + 1) == Some(&'\'') {
                    mapped.push(MappedChar { ch: '"', src_start: i, src_end: i + 2, line });
                    i += 2;
                } else {
                    mapped.push(MappedChar { ch: '\'', src_start: i, src_end: i + 1, line });
                    i += 1;
                }
            }
            '-' => {
                if src.get(i + 1) == Some(&'-') && src.get(i + 2) == Some(&'-') {
                    mapped.push(MappedChar { ch: '-', src_start: i, src_end: i + 3, line });
                    i += 3;
                } else if src.get(i + 1) == Some(&'-') {
                    mapped.push(MappedChar { ch: '-', src_start: i, src_end: i + 2, line });
                    i += 2;
                } else {
                    mapped.push(MappedChar { ch: '-', src_start: i, src_end: i + 1, line });
                    i += 1;
                }
            }
            '\\' => {
                i = handle_latex_command(src, i, end, &mut line, mapped);
            }
            other => {
                let mut exp = String::with_capacity(2);
                push_normalized_char(other, &mut exp);
                push_mapped_str(mapped, &exp, i, i + 1, line);
                i += 1;
            }
        }
    }
    line
}

fn handle_latex_command(
    src: &[char],
    cmd_start: usize,
    limit: usize,
    line: &mut usize,
    mapped: &mut Vec<MappedChar>,
) -> usize {
    let mut i = cmd_start + 1;
    if i >= limit {
        return limit;
    }

    let next_ch = src[i];
    if !next_ch.is_ascii_alphabetic() {
        match next_ch {
            '_' | '%' | '$' | '&' | '#' | '{' | '}' => {
                mapped.push(MappedChar { ch: next_ch, src_start: cmd_start, src_end: i + 1, line: *line });
                return i + 1;
            }
            '\\' => {
                let mut j = i + 1;
                if src.get(j) == Some(&'*') {
                    j += 1;
                }
                skip_delimited_group(src, &mut j, line, '[', ']');
                push_mapped_space(mapped, cmd_start, j, *line);
                return j;
            }
            ' ' | '~' => {
                push_mapped_space(mapped, cmd_start, i + 1, *line);
                return i + 1;
            }
            _ => {
                // Math delimiters \( \) \[ \], thin spaces \,, accents \' \` \" \^ etc.
                return i + 1;
            }
        }
    }

    while i < limit && src[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i < limit && src[i] == '*' {
        i += 1;
    }
    let cmd: String = src[cmd_start + 1..i].iter().collect();
    let cmd_base = cmd.trim_end_matches('*');

    // Inline symbol macros
    let symbol: Option<&str> = match cmd_base {
        "ldots" | "dots" => Some("..."),
        "textbackslash" => Some("\\"),
        "textasciitilde" => Some("~"),
        "textasciicircum" => Some("^"),
        "textendash" | "textemdash" => Some("-"),
        "textquotedblleft" | "textquotedblright" => Some("\""),
        "textquoteleft" | "textquoteright" => Some("'"),
        "times" => Some("×"),
        "cdot" => Some("·"),
        "pm" => Some("±"),
        "leq" | "le" => Some("≤"),
        "geq" | "ge" => Some("≥"),
        "neq" | "ne" => Some("≠"),
        "approx" => Some("≈"),
        "infty" => Some("∞"),
        "rightarrow" | "to" => Some("→"),
        "leftarrow" => Some("←"),
        _ => None,
    };
    if let Some(sym) = symbol {
        let mut s = cmd_start;
        let sym_end = i;
        skip_ws_inline(src, &mut i);
        let _ = &mut s;
        push_mapped_str(mapped, sym, cmd_start, sym_end, *line);
        return i;
    }

    if cmd_base == "begin" || cmd_base == "end" {
        let is_begin = cmd_base == "begin";
        let env = read_brace_text(src, &mut i, line).unwrap_or_default();
        let env_base = env.trim().trim_end_matches('*');
        if is_begin {
            if matches!(env_base, "verbatim" | "lstlisting" | "minted") {
                if env_base == "minted" {
                    skip_delimited_group(src, &mut i, line, '[', ']');
                    skip_delimited_group(src, &mut i, line, '{', '}');
                } else {
                    skip_delimited_group(src, &mut i, line, '[', ']');
                }
                let end_tag = format!("\\end{{{}}}", env.trim());
                if let Some(end_pos) = find_uncommented_tag(src, i, &end_tag) {
                    if env_base == "verbatim" || env_base == "lstlisting" {
                        // Verbatim content renders literally line by line
                        let mut k = i;
                        while k < end_pos {
                            match src[k] {
                                '\r' => k += 1,
                                '\n' => {
                                    *line += 1;
                                    push_mapped_space(mapped, k, k + 1, *line - 1);
                                    k += 1;
                                }
                                ' ' | '\t' => {
                                    push_mapped_space(mapped, k, k + 1, *line);
                                    k += 1;
                                }
                                other => {
                                    mapped.push(MappedChar { ch: other, src_start: k, src_end: k + 1, line: *line });
                                    k += 1;
                                }
                            }
                        }
                    } else {
                        // Strip minted body highlight or keep literal chars
                        let mut k = i;
                        while k < end_pos {
                            match src[k] {
                                '\r' => k += 1,
                                '\n' => {
                                    *line += 1;
                                    push_mapped_space(mapped, k, k + 1, *line - 1);
                                    k += 1;
                                }
                                ' ' | '\t' => {
                                    push_mapped_space(mapped, k, k + 1, *line);
                                    k += 1;
                                }
                                other => {
                                    mapped.push(MappedChar { ch: other, src_start: k, src_end: k + 1, line: *line });
                                    k += 1;
                                }
                            }
                        }
                    }
                    return end_pos + end_tag.chars().count();
                }
            }
            if matches!(env_base, "figure" | "table" | "wrapfigure" | "itemize" | "enumerate" | "minipage") {
                skip_delimited_group(src, &mut i, line, '[', ']');
                if env_base == "minipage" {
                    skip_delimited_group(src, &mut i, line, '{', '}');
                }
            } else if matches!(env_base, "tabular" | "tabularx" | "longtable" | "array") {
                if env.trim().ends_with('*') || env_base == "tabularx" {
                    skip_delimited_group(src, &mut i, line, '{', '}');
                }
                skip_delimited_group(src, &mut i, line, '[', ']');
                skip_delimited_group(src, &mut i, line, '{', '}');
            } else if env_base == "thebibliography" {
                skip_delimited_group(src, &mut i, line, '{', '}');
            }
        }
        push_mapped_space(mapped, cmd_start, i, *line);
        skip_ws_inline(src, &mut i);
        return i;
    }

    // Two-arg unit formatting commands (\SI{val}{unit}, \qty{val}{unit})
    if matches!(cmd_base, "SI" | "qty") {
        skip_delimited_group(src, &mut i, line, '[', ']');
        let mut probe = i;
        let mut pl = *line;
        if let Some(_) = read_brace_text(src, &mut probe, &mut pl) {
            // Find exact char boundaries of arg1 and arg2
            skip_ws_and_newlines(src, &mut i, line);
            if src.get(i) == Some(&'{') {
                let g1_start = i + 1;
                if skip_delimited_group(src, &mut i, line, '{', '}') {
                    let g1_end = i - 1;
                    *line = parse_latex_range_into_map(src, g1_start, g1_end, *line, mapped);
                    push_mapped_space(mapped, g1_end, i, *line);
                    skip_ws_and_newlines(src, &mut i, line);
                    if src.get(i) == Some(&'{') {
                        let g2_start = i + 1;
                        if skip_delimited_group(src, &mut i, line, '{', '}') {
                            let g2_end = i - 1;
                            *line = parse_latex_range_into_map(src, g2_start, g2_end, *line, mapped);
                        }
                    }
                }
            }
            return i;
        }
    }

    // Commands with silent arguments
    let silent_args: usize = match cmd_base {
        "label" | "ref" | "pageref" | "eqref" | "vref" | "autoref" | "nameref"
        | "cite" | "citep" | "citet" | "nocite" | "usepackage" | "RequirePackage"
        | "documentclass" | "includegraphics" | "input" | "include" | "bibliography"
        | "bibliographystyle" | "geometry" | "hypersetup" | "vspace" | "hspace"
        | "phantom" | "hphantom" | "vphantom" | "lstset" | "pagestyle" | "thispagestyle"
        | "urlstyle" | "numberwithin" | "cline" | "usetikzlibrary" | "pgfplotsset" => 1,
        "setlength" | "addtolength" | "setcounter" | "addtocounter" | "inputminted" | "rule" => 2,
        "cmidrule" => {
            skip_delimited_group(src, &mut i, line, '[', ']');
            skip_delimited_group(src, &mut i, line, '(', ')');
            skip_delimited_group(src, &mut i, line, '{', '}');
            skip_ws_inline(src, &mut i);
            return i;
        }
        _ => 0,
    };
    if silent_args > 0 {
        skip_delimited_group(src, &mut i, line, '[', ']');
        for _ in 0..silent_args {
            skip_delimited_group(src, &mut i, line, '{', '}');
        }
        skip_ws_inline(src, &mut i);
        return i;
    }

    // Commands with N hidden leading brace args followed by 1 visible brace arg
    let hidden_leading = match cmd_base {
        "textcolor" | "colorbox" | "href" | "hyperref" => 1,
        "fcolorbox" | "multicolumn" | "multirow" => 2,
        _ => 0,
    };
    if hidden_leading > 0 {
        skip_delimited_group(src, &mut i, line, '[', ']');
        for _ in 0..hidden_leading {
            skip_delimited_group(src, &mut i, line, '{', '}');
        }
        skip_delimited_group(src, &mut i, line, '[', ']');
        skip_ws_inline(src, &mut i);
        return i;
    }

    // Structural / formatting commands that may take an optional [short] arg before {body}
    if matches!(
        cmd_base,
        "section" | "subsection" | "subsubsection" | "chapter" | "part" | "paragraph"
            | "subparagraph" | "caption" | "footnote" | "sqrt" | "parbox" | "makebox"
    ) {
        if cmd_base != "sqrt" {
            push_mapped_space(mapped, cmd_start, i, *line);
        }
        skip_delimited_group(src, &mut i, line, '[', ']');
        skip_ws_inline(src, &mut i);
        return i;
    }

    // Layout-only 0-arg control words
    if matches!(
        cmd_base,
        "noindent" | "noident" | "centering" | "raggedright" | "raggedleft" | "par"
            | "medskip" | "bigskip" | "smallskip" | "newline" | "clearpage"
            | "cleardoublepage" | "pagebreak" | "newpage" | "maketitle"
            | "tableofcontents" | "listoffigures" | "listoftables" | "protect" | "relax"
            | "hline" | "toprule" | "midrule" | "bottomrule" | "item"
    ) {
        if cmd_base != "noindent" && cmd_base != "noident" {
            push_mapped_space(mapped, cmd_start, i, *line);
        }
        skip_ws_inline(src, &mut i);
        return i;
    }

    // Inline wrappers (\textbf, \emph, \texttt, \text, etc.) or unknown macros: skip command + inline space
    skip_ws_inline(src, &mut i);
    i
}

fn build_source_visible_map(source: &str) -> Vec<MappedChar> {
    let src: Vec<char> = source.chars().collect();
    let mut mapped = Vec::with_capacity(src.len());

    let begin_doc = find_uncommented_tag(&src, 0, "\\begin{document}");
    let body_end = find_uncommented_tag(&src, 0, "\\end{document}").unwrap_or(src.len());
    let has_maketitle = find_uncommented_tag(&src, 0, "\\maketitle").is_some();

    let mut start_line = 1usize;
    let body_start = if let Some(bpos) = begin_doc {
        let mut p = 0usize;
        let mut pline = 1usize;
        while p < bpos {
            if src[p] == '\n' {
                pline += 1;
                p += 1;
                continue;
            }
            if src[p] == '%' && (p == 0 || src[p - 1] != '\\') {
                skip_comment_eol(&src, &mut p, &mut pline);
                continue;
            }
            if has_maketitle
                && (matches_slice_at(&src, p, "\\title{")
                    || matches_slice_at(&src, p, "\\author{")
                    || matches_slice_at(&src, p, "\\date{"))
            {
                let brace_pos = p + src[p..bpos].iter().position(|&c| c == '{').unwrap();
                let mut end_probe = brace_pos;
                let mut end_line = pline;
                if skip_delimited_group(&src, &mut end_probe, &mut end_line, '{', '}') {
                    pline = parse_latex_range_into_map(&src, brace_pos + 1, end_probe - 1, pline, &mut mapped);
                    push_mapped_space(&mut mapped, brace_pos, end_probe, pline);
                    p = end_probe;
                    continue;
                }
            }
            p += 1;
        }
        start_line = pline;
        bpos + "\\begin{document}".chars().count()
    } else {
        0
    };

    for &c in &src[begin_doc.unwrap_or(0)..body_start] {
        if c == '\n' {
            start_line += 1;
        }
    }

    parse_latex_range_into_map(&src, body_start, body_end.max(body_start), start_line, &mut mapped);

    while mapped.first().map(|m| m.ch == ' ').unwrap_or(false) {
        mapped.remove(0);
    }
    while mapped.last().map(|m| m.ch == ' ').unwrap_or(false) {
        mapped.pop();
    }

    mapped
}

fn char_byte_offsets(s: &str) -> Vec<usize> {
    let mut offsets: Vec<usize> = s.char_indices().map(|(b, _)| b).collect();
    offsets.push(s.len());
    offsets
}

fn byte_to_char_idx(offsets: &[usize], byte_idx: usize) -> Option<usize> {
    offsets.binary_search(&byte_idx).ok()
}

fn bounded_edit_similarity(a: &[char], b: &[char]) -> isize {
    // Score matching prefix-to-prefix overlap using banded alignment on small windows (up to 64 chars)
    let m = a.len().min(64);
    let n = b.len().min(64);
    if m == 0 || n == 0 {
        return 0;
    }
    let band = 14usize;
    let mut prev = vec![0isize; n + 1];
    let mut curr = vec![0isize; n + 1];
    let mut best = 0isize;

    for i in 1..=m {
        curr.fill(0);
        let j_min = i.saturating_sub(band).max(1);
        let j_max = (i + band).min(n);
        let ai = a[i - 1];
        for j in j_min..=j_max {
            let bj = b[j - 1];
            let eq_bonus = if ai == bj {
                3
            } else if ai.eq_ignore_ascii_case(&bj) {
                2
            } else {
                -2
            };
            let diag = prev[j - 1] + eq_bonus;
            let up = prev[j] - 2;
            let left = curr[j - 1] - 2;
            let val = diag.max(up).max(left).max(0);
            curr[j] = val;
            if val > best {
                best = val;
            }
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    best
}

fn collect_caret_candidates(
    vis_str: &str,
    vis_lower: &str,
    byte_offsets: &[usize],
    prefix: &str,
    suffix: &str,
) -> HashSet<usize> {
    let mut candidates = HashSet::new();
    let suf_chars: Vec<char> = suffix.chars().collect();
    let pre_chars: Vec<char> = prefix.chars().collect();

    for &win in &[24usize, 14, 8, 5, 3, 2] {
        if suf_chars.len() >= win {
            let pat: String = suf_chars[..win].iter().collect();
            for (b, _) in vis_str.match_indices(&pat) {
                if let Some(c) = byte_to_char_idx(byte_offsets, b) {
                    candidates.insert(c);
                }
            }
            if candidates.is_empty() {
                let pat_l = pat.to_lowercase();
                for (b, _) in vis_lower.match_indices(&pat_l) {
                    if let Some(c) = byte_to_char_idx(byte_offsets, b) {
                        candidates.insert(c);
                    }
                }
            }
        }
        if pre_chars.len() >= win {
            let pat: String = pre_chars[pre_chars.len() - win..].iter().collect();
            for (b, _) in vis_str.match_indices(&pat) {
                if let Some(c) = byte_to_char_idx(byte_offsets, b + pat.len()) {
                    candidates.insert(c);
                }
            }
            if candidates.is_empty() {
                let pat_l = pat.to_lowercase();
                for (b, _) in vis_lower.match_indices(&pat_l) {
                    if let Some(c) = byte_to_char_idx(byte_offsets, b + pat_l.len()) {
                        candidates.insert(c);
                    }
                }
            }
        }
        if candidates.len() >= 1 && win >= 5 {
            break;
        }
    }

    // Fallback: match trimmed first word of suffix or last word of prefix (for clicks near synthetic tokens)
    if candidates.is_empty() {
        if let Some(first_word) = suffix.trim_start().split_whitespace().next() {
            if first_word.chars().count() >= 2 {
                let lead_spaces = suffix.chars().take_while(|&c| c == ' ').count();
                for (b, _) in vis_lower.match_indices(&first_word.to_lowercase()) {
                    if let Some(c) = byte_to_char_idx(byte_offsets, b) {
                        candidates.insert(c.saturating_sub(lead_spaces));
                    }
                }
            }
        }
        if let Some(last_word) = prefix.trim_end().split_whitespace().next_back() {
            if last_word.chars().count() >= 2 {
                let tail_spaces = prefix.chars().rev().take_while(|&c| c == ' ').count();
                let lw_lower = last_word.to_lowercase();
                for (b, _) in vis_lower.match_indices(&lw_lower) {
                    if let Some(c) = byte_to_char_idx(byte_offsets, b + lw_lower.len()) {
                        candidates.insert((c + tail_spaces).min(byte_offsets.len() - 1));
                    }
                }
            }
        }
    }

    candidates
}

/// Resolves a double-click PDF context `(prefix, suffix)` to the **exact** 0-based `char` index in `source`.
pub fn find_caret_char_index_in_latex_source(
    source: &str,
    ctx: &PdfClickContext,
    line_hint: Option<usize>,
) -> Option<usize> {
    if source.is_empty() {
        return None;
    }

    let mapped = build_source_visible_map(source);
    if mapped.is_empty() {
        return None;
    }

    let vis_chars: Vec<char> = mapped.iter().map(|m| m.ch).collect();
    let vis_str: String = vis_chars.iter().collect();
    let vis_lower = vis_str.to_lowercase();
    let byte_offsets = char_byte_offsets(&vis_str);

    let pre_chars: Vec<char> = ctx.prefix.chars().collect();
    let suf_chars: Vec<char> = ctx.suffix.chars().collect();
    let candidates = collect_caret_candidates(&vis_str, &vis_lower, &byte_offsets, &ctx.prefix, &ctx.suffix);
    if candidates.is_empty() {
        return None;
    }

    let mut best_split = None;
    let mut best_score = isize::MIN;
    let mut best_line_dist = usize::MAX;

    for b in candidates {
        // Immediate boundary match (touching `b` directly)
        let mut left_touch = 0isize;
        let mut pi = pre_chars.len();
        let mut vi = b;
        while pi > 0 && vi > 0 {
            let pc = pre_chars[pi - 1];
            let vc = vis_chars[vi - 1];
            if pc == vc {
                left_touch += 12;
            } else if pc.eq_ignore_ascii_case(&vc) {
                left_touch += 9;
            } else {
                break;
            }
            pi -= 1;
            vi -= 1;
        }

        let mut right_touch = 0isize;
        let mut si = 0usize;
        let mut vj = b;
        while si < suf_chars.len() && vj < vis_chars.len() {
            let sc = suf_chars[si];
            let vc = vis_chars[vj];
            if sc == vc {
                right_touch += 12;
            } else if sc.eq_ignore_ascii_case(&vc) {
                right_touch += 9;
            } else {
                break;
            }
            si += 1;
            vj += 1;
        }

        // Broader local context similarity around `b` (handles inline \ref, hyphenation, etc.)
        let pre_rev: Vec<char> = pre_chars.iter().rev().take(64).copied().collect();
        let vis_left_rev: Vec<char> = vis_chars[..b].iter().rev().take(64).copied().collect();
        let left_ctx = bounded_edit_similarity(&pre_rev, &vis_left_rev);

        let suf_slice = &suf_chars[..suf_chars.len().min(64)];
        let vis_right_slice = &vis_chars[b..(b + 64).min(vis_chars.len())];
        let right_ctx = bounded_edit_similarity(suf_slice, vis_right_slice);

        let b_line = mapped
            .get(b.min(mapped.len() - 1))
            .map(|m| m.line)
            .unwrap_or(1);
        let line_dist = line_hint
            .map(|h| (b_line as isize - h as isize).unsigned_abs())
            .unwrap_or(0);

        // Gentle proximity bonus so ambiguous identical phrases prefer the SyncTeX page/paragraph
        let sync_bonus = if line_hint.is_some() {
            15isize.saturating_sub(line_dist as isize)
        } else {
            0
        };
        let total = left_touch.min(360) + right_touch.min(360) + left_ctx + right_ctx + sync_bonus;

        if total > best_score || (total == best_score && line_dist < best_line_dist) {
            best_score = total;
            best_line_dist = line_dist;
            best_split = Some(b);
        }
    }

    let b = best_split?;
    if best_score < 8 {
        return None;
    }

    // Determine exact char offset in raw `source`
    let char_idx = if ctx.caret_after_prev && b > 0 {
        mapped[b - 1].src_end
    } else if b < mapped.len() {
        mapped[b].src_start
    } else {
        mapped.last().map(|m| m.src_end).unwrap_or(0)
    };

    Some(char_idx)
}

fn find_exact_vis_spans(
    haystack: &str,
    needle: &str,
    byte_offsets: &[usize],
) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }
    let mut spans = Vec::new();
    let mut pos = 0;
    while let Some(b_idx) = haystack[pos..].find(needle) {
        let abs_b = pos + b_idx;
        let end_b = abs_b + needle.len();
        if let (Some(c_start), Some(c_end)) = (
            byte_to_char_idx(byte_offsets, abs_b),
            byte_to_char_idx(byte_offsets, end_b),
        ) {
            spans.push((c_start, c_end));
        }
        pos = abs_b + needle.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    spans
}

fn find_spaceless_spans(
    vis_chars: &[char],
    sel_str: &str,
    case_sensitive: bool,
) -> Vec<(usize, usize)> {
    let mut vis_compact = String::with_capacity(vis_chars.len());
    let mut compact_to_vis = Vec::with_capacity(vis_chars.len());
    for (idx, &c) in vis_chars.iter().enumerate() {
        if c != ' ' {
            if case_sensitive {
                vis_compact.push(c);
            } else {
                for lc in c.to_lowercase() {
                    vis_compact.push(lc);
                }
            }
            compact_to_vis.push(idx);
        }
    }

    let sel_compact: String = sel_str
        .chars()
        .filter(|&c| c != ' ')
        .flat_map(|c| {
            if case_sensitive {
                vec![c].into_iter()
            } else {
                c.to_lowercase().collect::<Vec<_>>().into_iter()
            }
        })
        .collect();

    if sel_compact.is_empty() {
        return Vec::new();
    }

    let offsets = char_byte_offsets(&vis_compact);
    let mut spans = Vec::new();
    let mut pos = 0;
    while let Some(b_idx) = vis_compact[pos..].find(&sel_compact) {
        let abs_b = pos + b_idx;
        let end_b = abs_b + sel_compact.len();
        if let (Some(cs), Some(ce)) = (
            byte_to_char_idx(&offsets, abs_b),
            byte_to_char_idx(&offsets, end_b),
        ) {
            if cs < ce && ce <= compact_to_vis.len() {
                let vis_s = compact_to_vis[cs];
                let vis_e = compact_to_vis[ce - 1] + 1;
                spans.push((vis_s, vis_e));
            }
        }
        pos = abs_b + 1;
    }
    spans
}

fn find_anchored_span(
    vis_chars: &[char],
    norm_sel: &str,
    mapped: &[MappedChar],
    line_hint: Option<usize>,
) -> Option<(usize, usize)> {
    let words: Vec<&str> = norm_sel.split_whitespace().collect();
    if words.len() < 2 {
        return None;
    }

    // Build start & end multi-word anchors (skip purely numeric synthesized tokens if possible)
    let mut prefix_anchor = String::new();
    for w in &words {
        if !prefix_anchor.is_empty() {
            prefix_anchor.push(' ');
        }
        prefix_anchor.push_str(w);
        if prefix_anchor.chars().count() >= 16 {
            break;
        }
    }
    let mut suffix_parts = Vec::new();
    let mut suffix_len = 0usize;
    for w in words.iter().rev() {
        suffix_parts.push(*w);
        suffix_len += w.chars().count() + 1;
        if suffix_len >= 16 {
            break;
        }
    }
    suffix_parts.reverse();
    let suffix_anchor = suffix_parts.join(" ");

    let vis_lower: String = vis_chars.iter().flat_map(|c| c.to_lowercase()).collect();
    let offsets = char_byte_offsets(&vis_lower);
    let pre_l = prefix_anchor.to_lowercase();
    let suf_l = suffix_anchor.to_lowercase();

    let starts = find_exact_vis_spans(&vis_lower, &pre_l, &offsets);
    let ends = find_exact_vis_spans(&vis_lower, &suf_l, &offsets);
    if starts.is_empty() || ends.is_empty() {
        return None;
    }

    let target_len = norm_sel.chars().count();
    let mut best: Option<((usize, usize), isize)> = None;

    for &(s_start, _) in &starts {
        for &(_, e_end) in &ends {
            if e_end <= s_start {
                continue;
            }
            let span_len = e_end - s_start;
            if span_len > target_len * 2 + 120 {
                continue;
            }
            let len_diff = (span_len as isize - target_len as isize).abs();
            let line = mapped[s_start].line;
            let line_pen = line_hint
                .map(|h| (line as isize - h as isize).abs())
                .unwrap_or(0);
            let penalty = len_diff * 2 + line_pen;
            if best.map(|(_, p)| penalty < p).unwrap_or(true) {
                best = Some(((s_start, e_end), penalty));
            }
        }
    }

    best.map(|(span, _)| span)
}

fn close_adjacent_opened_brace(src_chars: &[char], start_char: usize, end_char: usize) -> usize {
    let mut depth = 0isize;
    let mut i = start_char;
    while i < end_char && i < src_chars.len() {
        match src_chars[i] {
            '\\' => i += 2,
            '{' => {
                depth += 1;
                i += 1;
            }
            '}' => {
                if depth > 0 {
                    depth -= 1;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    let mut adjusted = end_char;
    while depth > 0 && src_chars.get(adjusted) == Some(&'}') {
        depth -= 1;
        adjusted += 1;
    }
    adjusted
}

/// Strips standard LaTeX macro wrappers from text to reveal the core words.
pub fn strip_latex_macros(input: &str) -> String {
    let mapped = build_source_visible_map(input);
    mapped.into_iter().map(|m| m.ch).collect()
}

/// Finds the exact matching character range `(start_char, end_char)` in the LaTeX source
/// corresponding to `selected_text` from the PDF preview.
pub fn find_chunk_in_latex_source(
    source: &str,
    selected_text: &str,
    line_hint: Option<usize>,
) -> Option<(usize, usize)> {
    let norm_sel = normalize_visible_string(selected_text, false, false);
    if norm_sel.is_empty() || source.is_empty() {
        return None;
    }

    let mapped = build_source_visible_map(source);
    if mapped.is_empty() {
        return None;
    }

    let vis_chars: Vec<char> = mapped.iter().map(|m| m.ch).collect();
    let vis_str: String = vis_chars.iter().collect();
    let byte_offsets = char_byte_offsets(&vis_str);

    // 1. Exact visible match (case-sensitive)
    let mut candidates = find_exact_vis_spans(&vis_str, &norm_sel, &byte_offsets);

    // 2. Exact visible match (case-insensitive, handles small-caps / title casing)
    if candidates.is_empty() {
        let vis_lower = vis_str.to_lowercase();
        let lower_offsets = char_byte_offsets(&vis_lower);
        candidates = find_exact_vis_spans(&vis_lower, &norm_sel.to_lowercase(), &lower_offsets);
    }

    // 3. Space-normalized match (handles PDFium inserting/dropping spaces across wraps or table columns)
    if candidates.is_empty() {
        candidates = find_spaceless_spans(&vis_chars, &norm_sel, true);
    }
    if candidates.is_empty() {
        candidates = find_spaceless_spans(&vis_chars, &norm_sel, false);
    }

    // 4. Anchored match across inline auto-generated tokens (\ref, Figure 1:, etc.)
    let chosen_vis_span = if !candidates.is_empty() {
        let with_lines: Vec<(usize, usize, usize)> = candidates
            .into_iter()
            .filter(|&(s, e)| s < e && e <= mapped.len())
            .map(|(s, e)| (s, e, mapped[s].line))
            .collect();
        pick_best_match(&with_lines, line_hint).map(|(s, e, _)| (s, e))
    } else {
        find_anchored_span(&vis_chars, &norm_sel, &mapped, line_hint)
    }?;

    let (vis_s, vis_e) = chosen_vis_span;
    if vis_s >= vis_e || vis_e > mapped.len() {
        return None;
    }

    let start_char = mapped[vis_s].src_start;
    let raw_end_char = mapped[vis_e - 1].src_end;
    let src_chars: Vec<char> = source.chars().collect();
    let end_char = close_adjacent_opened_brace(&src_chars, start_char, raw_end_char);

    Some((start_char, end_char))
}

/// Chooses the candidate match closest to the SyncTeX line hint, or the first candidate if no hint.
fn pick_best_match(matches: &[(usize, usize, usize)], line_hint: Option<usize>) -> Option<(usize, usize, usize)> {
    if matches.is_empty() {
        return None;
    }
    match line_hint {
        Some(hint) => matches
            .iter()
            .copied()
            .min_by_key(|&(_, _, line)| (line as isize - hint as isize).abs()),
        None => matches.first().copied(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_chunk_highlighting_escaped_macros_and_substrings() {
        let source = "\\documentclass{article}\r\n\\begin{document}\r\n\\section{Architecture}\r\n\\noindent This module has two outputs, `S7\\_now' and `S15\\_now' which pulse once per cycle.\r\n\r\n\\noindent The states were decided to be \\textbf{binary encoded} as it fits into a register.\r\n\\end{document}";

        // Must highlight exact slice with escapes and quotes without shifting characters on CRLF
        let sel1 = "`S7_now' and `S15_now'";
        let (s1, e1) = find_chunk_in_latex_source(source, sel1, Some(4)).expect("match sel1");
        let slice1: String = source.chars().skip(s1).take(e1 - s1).collect();
        assert_eq!(slice1, "`S7\\_now' and `S15\\_now'");

        // Single-char and short stop words like "a" or "it" must match exact words, not substrings like "into"
        let sel2 = "binary encoded as it fits into a register";
        let (s2, e2) = find_chunk_in_latex_source(source, sel2, Some(6)).expect("match sel2");
        let slice2: String = source.chars().skip(s2).take(e2 - s2).collect();
        assert_eq!(slice2, "binary encoded} as it fits into a register");
    }

    #[test]
    fn test_exact_double_click_caret_position_with_latex_markup() {
        let source = "\\documentclass{article}\r\n\\begin{document}\r\n\\noindent Hello \\textbf{counter\\_16} world.\r\n\\end{document}";

        // Build synthetic PageTextLayout for "Hello counter_16 world."
        let rendered = "Hello counter_16 world.";
        let mut layout = PageTextLayout::default();
        for (idx, c) in rendered.chars().enumerate() {
            let x0 = 0.1 + (idx as f32) * 0.02;
            layout.chars.push(PdfCharLayout {
                c,
                rect: Rect::from_min_max(Pos2::new(x0, 0.20), Pos2::new(x0 + 0.018, 0.22)),
                index: idx,
            });
            layout.full_text.push(c);
        }

        // Click left half of '_' (index 13 -> between 'r' and '\_')
        let underscore_left = Pos2::new(0.1 + 13.0 * 0.02 + 0.002, 0.21);
        let ctx_before_under = layout.extract_click_context(underscore_left).expect("ctx");
        let char_idx_before = find_caret_char_index_in_latex_source(source, &ctx_before_under, Some(3)).expect("caret");
        let after_cursor: String = source.chars().skip(char_idx_before).collect();
        assert!(after_cursor.starts_with("\\_16} world."), "got: {after_cursor:?}");

        // Click right half of '_' (index 13 -> immediately after '\_', before '1')
        let underscore_right = Pos2::new(0.1 + 13.0 * 0.02 + 0.015, 0.21);
        let ctx_after_under = layout.extract_click_context(underscore_right).expect("ctx2");
        let char_idx_after = find_caret_char_index_in_latex_source(source, &ctx_after_under, Some(3)).expect("caret2");
        let after_cursor2: String = source.chars().skip(char_idx_after).collect();
        assert!(after_cursor2.starts_with("16} world."), "got: {after_cursor2:?}");

        // Click left half of 'c' in counter_16 (inside \textbf{)
        let c_left = Pos2::new(0.1 + 6.0 * 0.02 + 0.002, 0.21);
        let ctx_c = layout.extract_click_context(c_left).expect("ctx_c");
        let char_idx_c = find_caret_char_index_in_latex_source(source, &ctx_c, Some(3)).expect("caret_c");
        let after_c: String = source.chars().skip(char_idx_c).collect();
        assert!(after_c.starts_with("counter\\_16} world."), "got: {after_c:?}");
    }

    #[test]
    fn test_selection_rects_line_grouping() {
        let mut layout = PageTextLayout::default();
        layout.chars.push(PdfCharLayout { c: 'H', rect: Rect::from_min_max(Pos2::new(0.1, 0.10), Pos2::new(0.15, 0.12)), index: 0 });
        layout.chars.push(PdfCharLayout { c: 'i', rect: Rect::from_min_max(Pos2::new(0.15, 0.10), Pos2::new(0.20, 0.12)), index: 1 });
        layout.chars.push(PdfCharLayout { c: 'B', rect: Rect::from_min_max(Pos2::new(0.1, 0.15), Pos2::new(0.15, 0.17)), index: 2 });
        layout.chars.push(PdfCharLayout { c: 'y', rect: Rect::from_min_max(Pos2::new(0.15, 0.15), Pos2::new(0.20, 0.17)), index: 3 });

        let rects = layout.get_selection_rects(0, 3);
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], Rect::from_min_max(Pos2::new(0.1, 0.10), Pos2::new(0.20, 0.12)));
        assert_eq!(rects[1], Rect::from_min_max(Pos2::new(0.1, 0.15), Pos2::new(0.20, 0.17)));
    }
}
