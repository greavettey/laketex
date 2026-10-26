//! # LaTeX Lexer & Syntax Highlighter
//!
//! Fast tokenization of LaTeX source text into lexical tokens ([`Token`]) and conversion
//! into styled `egui::text::LayoutJob` objects for real-time syntax highlighting.

#![allow(dead_code)]

use egui::{Color32, FontId, TextFormat};

/// Lexical token types for LaTeX syntax highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    /// LaTeX comments (`% ...`)
    Comment,
    /// Backslash commands (e.g. `\section`, `\textbf`, `\begin`)
    Command,
    /// Inline math environments (`$ ... $`)
    MathInline,
    /// Display math blocks (`$$ ... $$`)
    MathBlock,
    /// Structural curly braces (`{` and `}`)
    Brace,
    /// Optional argument square brackets (`[` and `]`)
    Bracket,
    /// Special LaTeX control characters (`&`, `_`, `^`, `~`, `#`)
    SpecialChar,
    /// Macro and environment arguments (e.g. `itemize` in `\begin{itemize}`, `article` in `\documentclass{article}`)
    Argument,
    /// Package names imported in the document (e.g. `amsmath`, `graphicx` in `\usepackage{amsmath, graphicx}`)
    PackageName,
    /// Tag properties and argument keys (e.g. `leftmargin`, `label` in `\begin{itemize}[leftmargin=0.15in, label={}]`)
    TagProperty,
    /// Integer, floating point, or dimensional values of properties within tags (e.g. `0.15in`, `12pt`, `0.8\textwidth`, `10`)
    PropertyValueNumber,
    /// Boolean values of properties within tags (`true`, `false`)
    PropertyValueBoolean,
    /// General values assigned to properties within tags (e.g. string literals, paths, or expressions)
    PropertyValue,
    /// Regular prose text
    PlainText,
}

/// A slice of tokenized LaTeX source code with byte offsets.
#[derive(Debug, Clone)]
pub struct Token<'a> {
    /// Token category
    pub token_type: TokenType,
    /// Sliced string reference
    pub text: &'a str,
    /// Starting byte offset in source
    pub start: usize,
    /// Ending byte offset in source
    pub end: usize,
}

/// Delimiter family in the LaTeX bracket tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BracketKind {
    /// Curly braces `{` and `}`
    Curly,
    /// Square brackets `[` and `]`
    Square,
    /// Parentheses `(` and `)`
    Paren,
}

impl BracketKind {
    /// Returns the opening and closing characters for this delimiter family.
    pub fn delimiters(self) -> (char, char) {
        match self {
            Self::Curly => ('{', '}'),
            Self::Square => ('[', ']'),
            Self::Paren => ('(', ')'),
        }
    }
}

/// A single bracket delimiter node within the parsed [`BracketTree`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BracketNode {
    /// 0-based Unicode scalar (`char`) index in the document.
    pub char_idx: usize,
    /// 0-based UTF-8 byte offset in the document.
    pub byte_idx: usize,
    /// 1-based line number where this bracket appears.
    pub line: usize,
    /// Delimiter family (`Curly`, `Square`, `Paren`).
    pub kind: BracketKind,
    /// `true` if opening (`{`, `[`, `(`), `false` if closing (`}`, `]`, `)`).
    pub is_open: bool,
    /// 0-based nesting depth in the bracket tree.
    pub depth: usize,
    /// Index into [`BracketTree::nodes`] of the matching partner bracket, if balanced.
    pub paired_node_idx: Option<usize>,
}

/// Active bracket match resolved from the current editor caret position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveBracketMatch {
    /// The bracket node located at or immediately adjacent to the caret.
    pub clicked: BracketNode,
    /// The paired opening/closing bracket node in the tree, if matched.
    pub partner: Option<BracketNode>,
}

impl ActiveBracketMatch {
    /// Returns `(open_node, close_node)` ordered from opening to closing bracket when paired.
    pub fn ordered_pair(&self) -> Option<(BracketNode, BracketNode)> {
        let partner = self.partner?;
        if self.clicked.is_open {
            Some((self.clicked, partner))
        } else {
            Some((partner, self.clicked))
        }
    }
}

/// Syntax-aware tree of structural brackets (`{...}`, `[...]`, `(...)`) in a LaTeX document.
///
/// Ignores brackets inside `%` comments and escaped literal delimiters (`\{`, `\}`, `\[`, `\]`, `\(`, `\)`).
/// Resilient to unmatched inner brackets (such as half-open math intervals `[0, 1)`) so outer `{...}`
/// blocks remain accurately paired.
#[derive(Debug, Clone, Default)]
pub struct BracketTree {
    /// All bracket nodes ordered strictly by ascending `byte_idx` and `char_idx`.
    pub nodes: Vec<BracketNode>,
}

impl BracketTree {
    /// Parses a LaTeX source string into a bracket tree in a single `O(N)` pass.
    pub fn parse(input: &str) -> Self {
        let mut nodes: Vec<BracketNode> = Vec::new();
        let mut stack: Vec<usize> = Vec::new();

        let mut iter = input.char_indices().enumerate().peekable();
        let mut line = 1usize;

        while let Some((char_idx, (byte_idx, ch))) = iter.next() {
            if ch == '\n' {
                line += 1;
                continue;
            }

            // Skip LaTeX line comments (% ... \n)
            if ch == '%' {
                for (_, (_, c)) in iter.by_ref() {
                    if c == '\n' {
                        line += 1;
                        break;
                    }
                }
                continue;
            }

            // Skip escaped characters (e.g. \{, \}, \%, \\, \[)
            if ch == '\\' {
                if let Some((_, (_, next_ch))) = iter.next() {
                    if next_ch == '\n' {
                        line += 1;
                    }
                }
                continue;
            }

            let bracket_info = match ch {
                '{' => Some((BracketKind::Curly, true)),
                '}' => Some((BracketKind::Curly, false)),
                '[' => Some((BracketKind::Square, true)),
                ']' => Some((BracketKind::Square, false)),
                '(' => Some((BracketKind::Paren, true)),
                ')' => Some((BracketKind::Paren, false)),
                _ => None,
            };

            if let Some((kind, is_open)) = bracket_info {
                if is_open {
                    let depth = stack.len();
                    let node_idx = nodes.len();
                    nodes.push(BracketNode {
                        char_idx,
                        byte_idx,
                        line,
                        kind,
                        is_open: true,
                        depth,
                        paired_node_idx: None,
                    });
                    stack.push(node_idx);
                } else {
                    // Search stack from top to bottom for the nearest open bracket of the same kind.
                    // For Curly braces `{ ... }`, we unwind any unclosed inner `[` or `(` (common in math intervals like `$[0, 1)$`).
                    // For Square/Paren, we do not unwind across an open Curly brace `{` so a stray `]` inside `{ ... }` cannot match outside the block.
                    let match_stack_pos = stack.iter().rposition(|&open_idx| {
                        let open_kind = nodes[open_idx].kind;
                        open_kind == kind
                    }).filter(|&pos| {
                        if kind == BracketKind::Curly {
                            true
                        } else {
                            // Ensure no open Curly brace sits between `pos` and the top of the stack
                            !stack[pos + 1..].iter().any(|&idx| nodes[idx].kind == BracketKind::Curly)
                        }
                    });

                    if let Some(pos) = match_stack_pos {
                        // Pop any unclosed inner brackets above `pos`
                        stack.truncate(pos + 1);
                        let open_node_idx = stack.pop().unwrap();
                        let depth = nodes[open_node_idx].depth;
                        let close_node_idx = nodes.len();
                        nodes[open_node_idx].paired_node_idx = Some(close_node_idx);
                        nodes.push(BracketNode {
                            char_idx,
                            byte_idx,
                            line,
                            kind,
                            is_open: false,
                            depth,
                            paired_node_idx: Some(open_node_idx),
                        });
                    } else {
                        // Unmatched closing bracket
                        nodes.push(BracketNode {
                            char_idx,
                            byte_idx,
                            line,
                            kind,
                            is_open: false,
                            depth: stack.len(),
                            paired_node_idx: None,
                        });
                    }
                }
            }
        }

        Self { nodes }
    }

    /// Looks up a bracket node by its exact UTF-8 byte offset in `O(log N)` time.
    pub fn node_at_byte(&self, byte_idx: usize) -> Option<&BracketNode> {
        self.nodes
            .binary_search_by_key(&byte_idx, |n| n.byte_idx)
            .ok()
            .map(|idx| &self.nodes[idx])
    }

    /// Looks up a bracket node by its exact character index in `O(log N)` time.
    pub fn node_at_char(&self, char_idx: usize) -> Option<&BracketNode> {
        self.nodes
            .binary_search_by_key(&char_idx, |n| n.char_idx)
            .ok()
            .map(|idx| &self.nodes[idx])
    }

    /// Resolves the active bracket (and its paired partner, if any) for a given caret character index.
    ///
    /// Checks the character immediately at `cursor_char_idx` first (e.g. clicking on `{` places the
    /// caret before or after it), and falls back to `cursor_char_idx - 1` so clicking either side
    /// of a bracket reliably highlights the pair.
    pub fn active_match_at_cursor(&self, cursor_char_idx: usize) -> Option<ActiveBracketMatch> {
        let node = self
            .node_at_char(cursor_char_idx)
            .or_else(|| cursor_char_idx.checked_sub(1).and_then(|prev| self.node_at_char(prev)))?;

        let partner = node.paired_node_idx.and_then(|idx| self.nodes.get(idx).copied());
        Some(ActiveBracketMatch {
            clicked: *node,
            partner,
        })
    }
}

/// Helper: Checks if `input` at position 0 begins with a valid LaTeX number or dimension.
/// Returns the byte length of the matched token if found.
pub(crate) fn parse_number_or_dimension(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return None;
    }

    let mut i = 0;
    // Optional leading sign (+ or -)
    if bytes[i] == b'+' || bytes[i] == b'-' {
        i += 1;
        if i >= len {
            return None;
        }
    }

    let mut has_digits = false;
    // Digits before decimal point
    while i < len && bytes[i].is_ascii_digit() {
        has_digits = true;
        i += 1;
    }

    // Optional decimal point
    if i < len && bytes[i] == b'.' {
        i += 1;
        // Digits after decimal point
        while i < len && bytes[i].is_ascii_digit() {
            has_digits = true;
            i += 1;
        }
    }

    if !has_digits {
        return None;
    }

    // Check for optional TeX dimension unit or macro length
    let rem = &input[i..];
    // Check for backslash length like \textwidth, \linewidth, \columnwidth, etc.
    if rem.starts_with('\\') {
        let macro_rem = &rem[1..];
        let mut macro_len = 0;
        while macro_len < macro_rem.len() && macro_rem.as_bytes()[macro_len].is_ascii_alphabetic() {
            macro_len += 1;
        }
        let macro_name = &macro_rem[..macro_len];
        match macro_name {
            "textwidth" | "linewidth" | "columnwidth" | "paperwidth" | "paperheight"
            | "textheight" | "unitlength" | "fill" | "stretch" => {
                return Some(i + 1 + macro_len);
            }
            _ => {}
        }
    }

    // Check for units like pt, mm, cm, in, ex, em, bp, dd, pc, sp, px, %
    let units = [
        "pt", "mm", "cm", "in", "ex", "em", "bp", "dd", "pc", "sp", "px", "%",
    ];
    for u in units {
        if rem.starts_with(u) {
            let after_unit = &rem[u.len()..];
            if after_unit.is_empty() || !after_unit.as_bytes()[0].is_ascii_alphabetic() {
                return Some(i + u.len());
            }
        }
    }

    // Pure number (integer or float)
    if i < len && bytes[i].is_ascii_alphabetic() {
        return None;
    }

    Some(i)
}

/// Helper: Checks if `input` starts with a boolean keyword `true` or `false` (case-insensitive).
pub(crate) fn parse_boolean(input: &str) -> Option<usize> {
    let check = |word: &str| -> bool {
        if input.len() < word.len() {
            return false;
        }
        if !input[..word.len()].eq_ignore_ascii_case(word) {
            return false;
        }
        if input.len() > word.len() {
            let next_byte = input.as_bytes()[word.len()];
            if next_byte.is_ascii_alphanumeric() || next_byte == b'_' {
                return false;
            }
        }
        true
    };

    if check("true") {
        Some(4)
    } else if check("false") {
        Some(5)
    } else {
        None
    }
}

/// Helper: Checks if `input` starts with an identifier suitable for a tag property name.
pub(crate) fn parse_property_name(input: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return None;
    }
    if !bytes[0].is_ascii_alphabetic() && bytes[0] != b'@' && bytes[0] != b'_' {
        return None;
    }
    let mut i = 1;
    while i < len {
        let b = bytes[i];
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b':' {
            i += 1;
        } else {
            break;
        }
    }
    Some(i)
}

/// Tokenizer for LaTeX syntax highlighting.
pub struct LatexLexer;

impl LatexLexer {
    /// Tokenizes LaTeX source code into tokens for syntax highlighting.
    pub fn tokenize(input: &str) -> Vec<Token<'_>> {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum ScopeKind {
            /// Inside optional argument brackets `[...]` or key-value braces `{...}` of a tag / environment
            TagProperties { in_value: bool },
            /// Inside `{...}` of `\usepackage{...}` or `\RequirePackage{...}`
            PackageList,
            /// Inside `{...}` of `\begin{...}`, `\end{...}`, `\documentclass{...}`, `\cite{...}`, etc.
            CommandArgument { is_begin: bool },
            /// Inside a general brace block `{...}`
            GeneralGroup,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum PendingCommand {
            PackageImport,
            Begin,
            AfterBegin,
            End,
            DocumentClass,
            CommandArg,
            KeyValueConfig,
            GenericCommand,
        }

        let mut tokens = Vec::new();
        let bytes = input.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        let mut scope_stack: Vec<ScopeKind> = Vec::new();
        let mut pending: Option<PendingCommand> = None;

        while i < len {
            // 1. Check for comment
            if bytes[i] == b'%' {
                let start = i;
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                tokens.push(Token {
                    token_type: TokenType::Comment,
                    text: &input[start..i],
                    start,
                    end: i,
                });
                continue;
            }

            // 2. Check for backslash command
            if bytes[i] == b'\\' {
                let start = i;
                i += 1;
                if i < len {
                    if bytes[i].is_ascii_alphabetic() {
                        while i < len && bytes[i].is_ascii_alphabetic() {
                            i += 1;
                        }
                    } else if let Some(ch) = input[i..].chars().next() {
                        // Single symbol or UTF-8 character command like \$, \&, \\, \é, etc.
                        i += ch.len_utf8();
                    }
                }
                let cmd_text = &input[start..i];
                tokens.push(Token {
                    token_type: TokenType::Command,
                    text: cmd_text,
                    start,
                    end: i,
                });

                let cmd_name = if cmd_text.len() > 1 && cmd_text.starts_with('\\') {
                    &cmd_text[1..]
                } else {
                    ""
                };

                pending = match cmd_name {
                    "usepackage" | "RequirePackage" | "RequirePackageWithOptions" | "PassOptionsToPackage" => {
                        Some(PendingCommand::PackageImport)
                    }
                    "begin" => Some(PendingCommand::Begin),
                    "end" => Some(PendingCommand::End),
                    "documentclass" | "documentstyle" => Some(PendingCommand::DocumentClass),
                    "cite" | "nocite" | "ref" | "eqref" | "pageref" | "autoref" | "cref" | "Cref"
                    | "label" | "input" | "include" | "subfile" | "bibliography" | "bibliographystyle" => {
                        Some(PendingCommand::CommandArg)
                    }
                    "lstset" | "hypersetup" | "tikzset" | "captionsetup" | "geometry" | "presetkeys" => {
                        Some(PendingCommand::KeyValueConfig)
                    }
                    s if !s.is_empty() => Some(PendingCommand::GenericCommand),
                    _ => None,
                };
                continue;
            }

            // 3. Display math $$ ... $$
            if i + 1 < len && bytes[i] == b'$' && bytes[i + 1] == b'$' {
                let start = i;
                i += 2;
                while i + 1 < len && !(bytes[i] == b'$' && bytes[i + 1] == b'$') {
                    i += 1;
                }
                if i + 1 < len {
                    i += 2;
                }
                tokens.push(Token {
                    token_type: TokenType::MathBlock,
                    text: &input[start..i],
                    start,
                    end: i,
                });
                pending = None;
                continue;
            }

            // 4. Inline math $ ... $
            if bytes[i] == b'$' {
                let start = i;
                i += 1;
                while i < len && bytes[i] != b'$' && bytes[i] != b'\n' {
                    if bytes[i] == b'\\' && i + 1 < len {
                        i += 2; // skip escaped char
                    } else {
                        i += 1;
                    }
                }
                if i < len && bytes[i] == b'$' {
                    i += 1;
                }
                tokens.push(Token {
                    token_type: TokenType::MathInline,
                    text: &input[start..i],
                    start,
                    end: i,
                });
                pending = None;
                continue;
            }

            // 5. Braces { and }
            if bytes[i] == b'{' {
                let start = i;
                i += 1;
                tokens.push(Token {
                    token_type: TokenType::Brace,
                    text: &input[start..i],
                    start,
                    end: i,
                });

                match pending.take() {
                    Some(PendingCommand::PackageImport) => {
                        scope_stack.push(ScopeKind::PackageList);
                    }
                    Some(PendingCommand::Begin) => {
                        scope_stack.push(ScopeKind::CommandArgument { is_begin: true });
                    }
                    Some(PendingCommand::End) => {
                        scope_stack.push(ScopeKind::CommandArgument { is_begin: false });
                    }
                    Some(PendingCommand::DocumentClass)
                    | Some(PendingCommand::CommandArg)
                    | Some(PendingCommand::GenericCommand) => {
                        scope_stack.push(ScopeKind::CommandArgument { is_begin: false });
                    }
                    Some(PendingCommand::KeyValueConfig) => {
                        scope_stack.push(ScopeKind::TagProperties { in_value: false });
                    }
                    _ => {
                        scope_stack.push(ScopeKind::GeneralGroup);
                    }
                }
                continue;
            }

            if bytes[i] == b'}' {
                let start = i;
                i += 1;
                tokens.push(Token {
                    token_type: TokenType::Brace,
                    text: &input[start..i],
                    start,
                    end: i,
                });

                if let Some(top) = scope_stack.pop() {
                    if let ScopeKind::CommandArgument { is_begin: true } = top {
                        pending = Some(PendingCommand::AfterBegin);
                    }
                }
                if let Some(ScopeKind::TagProperties { in_value }) = scope_stack.last_mut() {
                    *in_value = false;
                }
                continue;
            }

            // 6. Brackets [ and ]
            if bytes[i] == b'[' {
                let start = i;
                i += 1;
                tokens.push(Token {
                    token_type: TokenType::Bracket,
                    text: &input[start..i],
                    start,
                    end: i,
                });

                if pending == Some(PendingCommand::AfterBegin) {
                    pending = None;
                }
                scope_stack.push(ScopeKind::TagProperties { in_value: false });
                continue;
            }

            if bytes[i] == b']' {
                let start = i;
                i += 1;
                tokens.push(Token {
                    token_type: TokenType::Bracket,
                    text: &input[start..i],
                    start,
                    end: i,
                });

                if let Some(pos) = scope_stack.iter().rposition(|s| matches!(s, ScopeKind::TagProperties { .. })) {
                    scope_stack.truncate(pos);
                }
                continue;
            }

            // 7. Special characters &, _, ^, ~, #
            if matches!(bytes[i], b'&' | b'_' | b'^' | b'~' | b'#') {
                let start = i;
                i += 1;
                tokens.push(Token {
                    token_type: TokenType::SpecialChar,
                    text: &input[start..i],
                    start,
                    end: i,
                });
                continue;
            }

            // 8. Whitespace
            if bytes[i].is_ascii_whitespace() {
                let start = i;
                while i < len && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                tokens.push(Token {
                    token_type: TokenType::PlainText,
                    text: &input[start..i],
                    start,
                    end: i,
                });
                continue;
            }

            // 9. Context-sensitive tokenization based on scope
            match scope_stack.last().copied() {
                Some(ScopeKind::PackageList) => {
                    if bytes[i] == b',' {
                        let start = i;
                        i += 1;
                        tokens.push(Token {
                            token_type: TokenType::PlainText,
                            text: &input[start..i],
                            start,
                            end: i,
                        });
                        continue;
                    } else if bytes[i].is_ascii_alphabetic() || bytes[i] == b'@' || bytes[i] == b'_' {
                        let start = i;
                        while i < len
                            && (bytes[i].is_ascii_alphanumeric()
                                || bytes[i] == b'-'
                                || bytes[i] == b'_'
                                || bytes[i] == b'@')
                        {
                            i += 1;
                        }
                        tokens.push(Token {
                            token_type: TokenType::PackageName,
                            text: &input[start..i],
                            start,
                            end: i,
                        });
                        continue;
                    }
                }
                Some(ScopeKind::CommandArgument { .. }) => {
                    if bytes[i] == b',' {
                        let start = i;
                        i += 1;
                        tokens.push(Token {
                            token_type: TokenType::PlainText,
                            text: &input[start..i],
                            start,
                            end: i,
                        });
                        continue;
                    } else if bytes[i] != b'}' && bytes[i] != b'%' && bytes[i] != b'\\' {
                        let start = i;
                        while i < len
                            && bytes[i] != b','
                            && bytes[i] != b'}'
                            && bytes[i] != b'%'
                            && bytes[i] != b'\\'
                            && !bytes[i].is_ascii_whitespace()
                        {
                            i += 1;
                        }
                        if i > start {
                            tokens.push(Token {
                                token_type: TokenType::Argument,
                                text: &input[start..i],
                                start,
                                end: i,
                            });
                            continue;
                        }
                    }
                }
                Some(ScopeKind::TagProperties { in_value }) => {
                    if in_value {
                        if bytes[i] == b',' {
                            let start = i;
                            i += 1;
                            tokens.push(Token {
                                token_type: TokenType::PlainText,
                                text: &input[start..i],
                                start,
                                end: i,
                            });
                            if let Some(ScopeKind::TagProperties { in_value: v }) = scope_stack.last_mut() {
                                *v = false;
                            }
                            continue;
                        }

                        // Check for boolean value (true, false)
                        if let Some(bool_len) = parse_boolean(&input[i..]) {
                            let start = i;
                            i += bool_len;
                            tokens.push(Token {
                                token_type: TokenType::PropertyValueBoolean,
                                text: &input[start..i],
                                start,
                                end: i,
                            });
                            if let Some(ScopeKind::TagProperties { in_value: v }) = scope_stack.last_mut() {
                                *v = false;
                            }
                            continue;
                        }

                        // Check for number or dimension (0.15in, 12pt, 0.8\textwidth, 10, -5mm)
                        if let Some(num_len) = parse_number_or_dimension(&input[i..]) {
                            let start = i;
                            i += num_len;
                            tokens.push(Token {
                                token_type: TokenType::PropertyValueNumber,
                                text: &input[start..i],
                                start,
                                end: i,
                            });
                            if let Some(ScopeKind::TagProperties { in_value: v }) = scope_stack.last_mut() {
                                *v = false;
                            }
                            continue;
                        }

                        // Otherwise consume general property value text up to delimiter
                        let start = i;
                        while i < len
                            && bytes[i] != b','
                            && bytes[i] != b']'
                            && bytes[i] != b'}'
                            && bytes[i] != b'%'
                            && bytes[i] != b'\\'
                            && bytes[i] != b'\n'
                            && !bytes[i].is_ascii_whitespace()
                        {
                            i += 1;
                        }
                        if i > start {
                            tokens.push(Token {
                                token_type: TokenType::PropertyValue,
                                text: &input[start..i],
                                start,
                                end: i,
                            });
                            if let Some(ScopeKind::TagProperties { in_value: v }) = scope_stack.last_mut() {
                                *v = false;
                            }
                            continue;
                        }
                    } else {
                        // Not in value: expecting property name, '=', or standalone option
                        if bytes[i] == b'=' {
                            let start = i;
                            i += 1;
                            tokens.push(Token {
                                token_type: TokenType::PlainText,
                                text: &input[start..i],
                                start,
                                end: i,
                            });
                            if let Some(ScopeKind::TagProperties { in_value: v }) = scope_stack.last_mut() {
                                *v = true;
                            }
                            continue;
                        }

                        if bytes[i] == b',' {
                            let start = i;
                            i += 1;
                            tokens.push(Token {
                                token_type: TokenType::PlainText,
                                text: &input[start..i],
                                start,
                                end: i,
                            });
                            continue;
                        }

                        // Check if this standalone option is a number/dimension (e.g. 11pt, 0.15in)
                        if let Some(num_len) = parse_number_or_dimension(&input[i..]) {
                            let start = i;
                            i += num_len;
                            tokens.push(Token {
                                token_type: TokenType::PropertyValueNumber,
                                text: &input[start..i],
                                start,
                                end: i,
                            });
                            continue;
                        }

                        // Check for property key identifier (e.g. leftmargin, label, width, draft)
                        if let Some(prop_len) = parse_property_name(&input[i..]) {
                            let start = i;
                            i += prop_len;
                            tokens.push(Token {
                                token_type: TokenType::TagProperty,
                                text: &input[start..i],
                                start,
                                end: i,
                            });
                            continue;
                        }
                    }
                }
                Some(ScopeKind::GeneralGroup) => {
                    // Check if parent scope is TagProperties; if so, content inside { ... } is a property value
                    let parent_is_tag_props = scope_stack.len() >= 2
                        && matches!(scope_stack[scope_stack.len() - 2], ScopeKind::TagProperties { .. });
                    if parent_is_tag_props {
                        if let Some(bool_len) = parse_boolean(&input[i..]) {
                            let start = i;
                            i += bool_len;
                            tokens.push(Token {
                                token_type: TokenType::PropertyValueBoolean,
                                text: &input[start..i],
                                start,
                                end: i,
                            });
                            continue;
                        }
                        if let Some(num_len) = parse_number_or_dimension(&input[i..]) {
                            let start = i;
                            i += num_len;
                            tokens.push(Token {
                                token_type: TokenType::PropertyValueNumber,
                                text: &input[start..i],
                                start,
                                end: i,
                            });
                            continue;
                        }
                        let start = i;
                        while i < len
                            && bytes[i] != b'}'
                            && bytes[i] != b'%'
                            && bytes[i] != b'\\'
                            && bytes[i] != b'\n'
                        {
                            i += 1;
                        }
                        if i > start {
                            tokens.push(Token {
                                token_type: TokenType::PropertyValue,
                                text: &input[start..i],
                                start,
                                end: i,
                            });
                            continue;
                        }
                    }
                }
                None => {}
            }

            // 10. Fallback: Plain text sequence
            pending = None;
            let start = i;
            while i < len
                && bytes[i] != b'%'
                && bytes[i] != b'\\'
                && bytes[i] != b'$'
                && bytes[i] != b'{'
                && bytes[i] != b'}'
                && bytes[i] != b'['
                && bytes[i] != b']'
                && !matches!(bytes[i], b'&' | b'_' | b'^' | b'~' | b'#')
                && !bytes[i].is_ascii_whitespace()
            {
                i += 1;
            }
            if i > start {
                tokens.push(Token {
                    token_type: TokenType::PlainText,
                    text: &input[start..i],
                    start,
                    end: i,
                });
            }
        }

        tokens
    }

    /// Color for token in dark mode
    pub fn token_color_dark(token_type: TokenType) -> Color32 {
        match token_type {
            TokenType::Comment => Color32::from_rgb(108, 118, 133), // Slate gray
            TokenType::Command => Color32::from_rgb(97, 175, 239),  // Light cyan / blue
            TokenType::Argument => Color32::from_rgb(152, 195, 121), // Muted green
            TokenType::PackageName => Color32::from_rgb(229, 192, 123), // Soft gold / amber
            TokenType::TagProperty => Color32::from_rgb(224, 108, 117), // Soft coral / salmon
            TokenType::PropertyValueNumber => Color32::from_rgb(209, 154, 102), // Warm apricot / orange
            TokenType::PropertyValueBoolean => Color32::from_rgb(198, 120, 221), // Orchid purple
            TokenType::PropertyValue => Color32::from_rgb(86, 182, 194), // Teal cyan
            TokenType::MathInline => Color32::from_rgb(229, 192, 123), // Soft gold
            TokenType::MathBlock => Color32::from_rgb(209, 154, 102),  // Warm amber
            TokenType::Brace => Color32::from_rgb(198, 120, 221),   // Purple
            TokenType::Bracket => Color32::from_rgb(86, 182, 194), // Teal
            TokenType::SpecialChar => Color32::from_rgb(224, 108, 117), // Coral red
            TokenType::PlainText => Color32::from_rgb(220, 223, 228), // Off-white
        }
    }

    /// Color for token in light mode
    pub fn token_color_light(token_type: TokenType) -> Color32 {
        match token_type {
            TokenType::Comment => Color32::from_rgb(120, 130, 140),
            TokenType::Command => Color32::from_rgb(16, 92, 176),
            TokenType::Argument => Color32::from_rgb(30, 120, 50),
            TokenType::PackageName => Color32::from_rgb(180, 106, 0),
            TokenType::TagProperty => Color32::from_rgb(166, 36, 56),
            TokenType::PropertyValueNumber => Color32::from_rgb(196, 85, 0),
            TokenType::PropertyValueBoolean => Color32::from_rgb(117, 37, 166),
            TokenType::PropertyValue => Color32::from_rgb(0, 122, 135),
            TokenType::MathInline => Color32::from_rgb(160, 90, 10),
            TokenType::MathBlock => Color32::from_rgb(170, 70, 0),
            TokenType::Brace => Color32::from_rgb(130, 30, 160),
            TokenType::Bracket => Color32::from_rgb(0, 120, 130),
            TokenType::SpecialChar => Color32::from_rgb(180, 20, 30),
            TokenType::PlainText => Color32::from_rgb(30, 35, 45),
        }
    }

    /// Returns a bracket-tree depth-aware color for structural brackets (`{}`, `[]`, `()`).
    /// Unmatched brackets return a high-visibility crimson error color.
    pub fn bracket_depth_color(depth: usize, is_matched: bool, is_dark: bool) -> Color32 {
        if !is_matched {
            return if is_dark {
                Color32::from_rgb(244, 71, 71) // Unmatched error red (dark)
            } else {
                Color32::from_rgb(210, 30, 30) // Unmatched error red (light)
            };
        }

        if is_dark {
            match depth % 4 {
                0 => Color32::from_rgb(198, 120, 221), // Level 0: Orchid purple
                1 => Color32::from_rgb(86, 182, 194),  // Level 1: Bright teal / cyan
                2 => Color32::from_rgb(229, 192, 123), // Level 2: Soft gold
                _ => Color32::from_rgb(152, 195, 121), // Level 3: Sage green
            }
        } else {
            match depth % 4 {
                0 => Color32::from_rgb(130, 30, 160),  // Level 0: Deep purple
                1 => Color32::from_rgb(0, 120, 140),   // Level 1: Deep teal
                2 => Color32::from_rgb(175, 100, 0),   // Level 2: Rich amber
                _ => Color32::from_rgb(30, 125, 55),   // Level 3: Forest green
            }
        }
    }

    /// Generates an egui LayoutJob with syntax highlighting and bracket-tree depth coloring.
    pub fn highlight(input: &str, is_dark: bool, font_id: FontId) -> egui::text::LayoutJob {
        let tree = BracketTree::parse(input);
        Self::highlight_with_brackets(input, is_dark, font_id, &tree, None)
    }

    /// Generates an egui LayoutJob with syntax highlighting, bracket-tree nesting colors,
    /// and active paired-bracket highlighting when a bracket is clicked or focused at the caret.
    pub fn highlight_with_brackets(
        input: &str,
        is_dark: bool,
        font_id: FontId,
        bracket_tree: &BracketTree,
        active_match: Option<ActiveBracketMatch>,
    ) -> egui::text::LayoutJob {
        Self::highlight_with_brackets_and_theme(input, is_dark, font_id, bracket_tree, active_match, None)
    }

    /// Generates an egui LayoutJob with theme-driven syntax highlighting, bracket-tree nesting colors,
    /// and active paired-bracket highlighting.
    pub fn highlight_with_brackets_and_theme(
        input: &str,
        is_dark: bool,
        font_id: FontId,
        bracket_tree: &BracketTree,
        active_match: Option<ActiveBracketMatch>,
        theme: Option<&crate::ui::theme::ThemeConfig>,
    ) -> egui::text::LayoutJob {
        let mut job = egui::text::LayoutJob::default();
        let tokens = Self::tokenize(input);

        let active_byte_1 = active_match.map(|m| m.clicked.byte_idx);
        let active_byte_2 = active_match.and_then(|m| m.partner.map(|p| p.byte_idx));
        let is_active_matched = active_match.and_then(|m| m.partner).is_some();

        let get_token_color = |token_type: TokenType| -> Color32 {
            if let Some(t) = theme {
                t.token_color(token_type)
            } else if is_dark {
                Self::token_color_dark(token_type)
            } else {
                Self::token_color_light(token_type)
            }
        };

        let get_bracket_color = |depth: usize, is_matched: bool| -> Color32 {
            if let Some(t) = theme {
                t.bracket_depth_color(depth, is_matched)
            } else {
                Self::bracket_depth_color(depth, is_matched, is_dark)
            }
        };

        let format_for_bracket = |node: &BracketNode, is_active_pair: bool| -> TextFormat {
            if is_active_pair {
                if is_active_matched {
                    let (fg, bg) = if is_dark {
                        (
                            Color32::from_rgb(255, 235, 140),
                            Color32::from_rgba_unmultiplied(97, 175, 239, 75),
                        )
                    } else {
                        (
                            Color32::from_rgb(10, 45, 110),
                            Color32::from_rgba_unmultiplied(60, 145, 230, 75),
                        )
                    };
                    TextFormat {
                        font_id: font_id.clone(),
                        color: fg,
                        background: bg,
                        ..Default::default()
                    }
                } else {
                    let (fg, bg) = if is_dark {
                        (
                            Color32::from_rgb(255, 180, 180),
                            Color32::from_rgba_unmultiplied(235, 87, 87, 85),
                        )
                    } else {
                        (
                            Color32::from_rgb(160, 15, 15),
                            Color32::from_rgba_unmultiplied(235, 87, 87, 65),
                        )
                    };
                    TextFormat {
                        font_id: font_id.clone(),
                        color: fg,
                        background: bg,
                        ..Default::default()
                    }
                }
            } else {
                let color = get_bracket_color(node.depth, node.paired_node_idx.is_some());
                TextFormat {
                    font_id: font_id.clone(),
                    color,
                    ..Default::default()
                }
            }
        };

        for token in tokens {
            let base_color = get_token_color(token.token_type);
            let base_format = TextFormat {
                font_id: font_id.clone(),
                color: base_color,
                ..Default::default()
            };

            if matches!(token.token_type, TokenType::Brace | TokenType::Bracket) {
                if let Some(node) = bracket_tree.node_at_byte(token.start) {
                    let is_active = Some(node.byte_idx) == active_byte_1
                        || Some(node.byte_idx) == active_byte_2;
                    job.append(token.text, 0.0, format_for_bracket(node, is_active));
                    continue;
                }
            }

            // For tokens that may contain inner structural brackets (such as `$ ... $`, `$$ ... $$`, or `(` / `)` in PlainText),
            // split at any bracket nodes falling inside `token.start..token.end` so inner braces/parens also get
            // bracket-tree coloring and active pair highlighting.
            if matches!(
                token.token_type,
                TokenType::MathInline | TokenType::MathBlock | TokenType::PlainText | TokenType::PropertyValue | TokenType::TagProperty
            ) {
                let start_idx = bracket_tree
                    .nodes
                    .partition_point(|n| n.byte_idx < token.start);
                let end_idx = bracket_tree
                    .nodes
                    .partition_point(|n| n.byte_idx < token.end);

                if start_idx < end_idx {
                    let mut cursor = token.start;
                    for node in &bracket_tree.nodes[start_idx..end_idx] {
                        if node.byte_idx > cursor {
                            job.append(&input[cursor..node.byte_idx], 0.0, base_format.clone());
                        }
                        let is_active = Some(node.byte_idx) == active_byte_1
                            || Some(node.byte_idx) == active_byte_2;
                        let bracket_end = (node.byte_idx + 1).min(token.end);
                        job.append(
                            &input[node.byte_idx..bracket_end],
                            0.0,
                            format_for_bracket(node, is_active),
                        );
                        cursor = bracket_end;
                    }
                    if cursor < token.end {
                        job.append(&input[cursor..token.end], 0.0, base_format);
                    }
                    continue;
                }
            }

            job.append(token.text, 0.0, base_format);
        }

        job
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_multibyte_utf8_commands() {
        let src = "\\é \\α \\— $\\é + \\beta$ \\section{Résumé}";
        let tokens = LatexLexer::tokenize(src);
        assert!(!tokens.is_empty());
        assert_eq!(tokens[0].token_type, TokenType::Command);
        assert_eq!(tokens[0].text, "\\é");
        assert_eq!(tokens[2].token_type, TokenType::Command);
        assert_eq!(tokens[2].text, "\\α");
        assert_eq!(tokens[4].token_type, TokenType::Command);
        assert_eq!(tokens[4].text, "\\—");
    }

    #[test]
    fn test_bracket_tree_pairing_and_nesting_depths() {
        let src = "\\section{Outer \\textbf{Inner [opt]} end}";
        let tree = BracketTree::parse(src);

        // Expect: `{` (depth 0), `{` (depth 1), `[` (depth 2), `]` (depth 2), `}` (depth 1), `}` (depth 0)
        assert_eq!(tree.nodes.len(), 6);
        assert_eq!(tree.nodes[0].depth, 0);
        assert!(tree.nodes[0].is_open);
        assert_eq!(tree.nodes[0].paired_node_idx, Some(5));

        assert_eq!(tree.nodes[1].depth, 1);
        assert!(tree.nodes[1].is_open);
        assert_eq!(tree.nodes[1].paired_node_idx, Some(4));

        assert_eq!(tree.nodes[2].depth, 2);
        assert_eq!(tree.nodes[2].kind, BracketKind::Square);
        assert_eq!(tree.nodes[2].paired_node_idx, Some(3));

        // Clicking on the outer opening `{` (at char index 8) or right after it (char index 9) matches outer closing `}`
        let open_char = tree.nodes[0].char_idx;
        let close_char = tree.nodes[5].char_idx;

        let match_at_open = tree.active_match_at_cursor(open_char).expect("should match at {");
        assert_eq!(match_at_open.clicked.char_idx, open_char);
        assert_eq!(match_at_open.partner.unwrap().char_idx, close_char);

        let match_after_open = tree
            .active_match_at_cursor(open_char + 1)
            .expect("should match right after {");
        assert_eq!(match_after_open.clicked.char_idx, open_char);
        assert_eq!(match_after_open.partner.unwrap().char_idx, close_char);

        // Clicking on the outer closing `}` matches the outer opening `{`
        let match_at_close = tree.active_match_at_cursor(close_char).expect("should match at }");
        assert_eq!(match_at_close.clicked.char_idx, close_char);
        assert_eq!(match_at_close.partner.unwrap().char_idx, open_char);
    }

    #[test]
    fn test_theme_driven_syntax_highlighting() {
        let theme = crate::ui::theme::ThemeConfig::synthwave_neon();
        let src = "\\section{Hello}";
        let font_id = egui::FontId::monospace(13.0);
        let tree = BracketTree::parse(src);
        let job = LatexLexer::highlight_with_brackets_and_theme(
            src,
            true,
            font_id,
            &tree,
            None,
            Some(&theme),
        );
        assert!(!job.sections.is_empty());
        // \section should have the neon electric cyan color (#00f0ff)
        assert_eq!(job.sections[0].format.color, Color32::from_rgb(0x00, 0xf0, 0xff));
        // { bracket should have the hot pink color (#ff2a85)
        assert_eq!(job.sections[1].format.color, Color32::from_rgb(0xff, 0x2a, 0x85));
    }

    #[test]
    fn test_bracket_tree_ignores_comments_and_escaped_braces() {
        let src = "% Unclosed { in comment\n\\{ literal \\} \\textbf{Real}";
        let tree = BracketTree::parse(src);

        // Only the `{` and `}` around `Real` should be in the bracket tree
        assert_eq!(tree.nodes.len(), 2);
        assert_eq!(tree.nodes[0].paired_node_idx, Some(1));
        assert_eq!(tree.nodes[1].paired_node_idx, Some(0));
    }

    #[test]
    fn test_bracket_tree_resilient_to_half_open_math_intervals() {
        let src = "$\\frac{[0, 1)}{2}$";
        let tree = BracketTree::parse(src);

        // Outer `{` and `}` of `\frac{[0, 1)}` must still pair with each other despite `[0, 1)` inside!
        let first_curly = &tree.nodes[0];
        assert_eq!(first_curly.kind, BracketKind::Curly);
        assert!(first_curly.is_open);
        let partner_idx = first_curly.paired_node_idx.expect("outer curly brace should be paired");
        assert_eq!(tree.nodes[partner_idx].kind, BracketKind::Curly);
        assert!(!tree.nodes[partner_idx].is_open);
    }

    #[test]
    fn test_broadened_syntax_tag_properties_and_values() {
        let src = "\\begin{itemize}[leftmargin=0.15in, label={}]";
        let tokens = LatexLexer::tokenize(src);

        // 1. \begin command
        assert_eq!(tokens[0].token_type, TokenType::Command);
        assert_eq!(tokens[0].text, "\\begin");

        // 2. {
        assert_eq!(tokens[1].token_type, TokenType::Brace);

        // 3. Argument to begin: itemize
        assert_eq!(tokens[2].token_type, TokenType::Argument);
        assert_eq!(tokens[2].text, "itemize");

        // 4. }
        assert_eq!(tokens[3].token_type, TokenType::Brace);

        // 5. [
        assert_eq!(tokens[4].token_type, TokenType::Bracket);

        // 6. Tag property: leftmargin
        assert_eq!(tokens[5].token_type, TokenType::TagProperty);
        assert_eq!(tokens[5].text, "leftmargin");

        // 7. =
        assert_eq!(tokens[6].text, "=");

        // 8. Value passed to leftmargin: 0.15in (PropertyValueNumber)
        assert_eq!(tokens[7].token_type, TokenType::PropertyValueNumber);
        assert_eq!(tokens[7].text, "0.15in");

        // Tag property: label
        let label_token = tokens.iter().find(|t| t.text == "label").expect("label property not found");
        assert_eq!(label_token.token_type, TokenType::TagProperty);

        // Closing bracket ]
        assert_eq!(tokens.last().unwrap().token_type, TokenType::Bracket);
    }

    #[test]
    fn test_package_names_imported_in_document() {
        let src = "\\usepackage{amsmath, graphicx, hyperref}\n\\RequirePackage[dvipsnames]{xcolor}";
        let tokens = LatexLexer::tokenize(src);

        let pkg_names: Vec<&str> = tokens
            .iter()
            .filter(|t| t.token_type == TokenType::PackageName)
            .map(|t| t.text)
            .collect();
        assert_eq!(pkg_names, vec!["amsmath", "graphicx", "hyperref", "xcolor"]);

        let opt_token = tokens.iter().find(|t| t.text == "dvipsnames").expect("dvipsnames not found");
        assert_eq!(opt_token.token_type, TokenType::TagProperty);
    }

    #[test]
    fn test_boolean_and_dimension_values_within_tags() {
        let src = "\\includegraphics[width=0.8\\textwidth, keepaspectratio=true, draft=false]{chart.pdf}";
        let tokens = LatexLexer::tokenize(src);

        let width_val = tokens.iter().find(|t| t.text == "0.8\\textwidth").expect("0.8\\textwidth not found");
        assert_eq!(width_val.token_type, TokenType::PropertyValueNumber);

        let true_val = tokens.iter().find(|t| t.text == "true").expect("true not found");
        assert_eq!(true_val.token_type, TokenType::PropertyValueBoolean);

        let false_val = tokens.iter().find(|t| t.text == "false").expect("false not found");
        assert_eq!(false_val.token_type, TokenType::PropertyValueBoolean);

        let arg_val = tokens.iter().find(|t| t.text == "chart.pdf").expect("chart.pdf not found");
        assert_eq!(arg_val.token_type, TokenType::Argument);
    }
}
