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
    /// Backslash commands (e.g. `\section`, `\textbf`)
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
    /// Macro argument content
    Argument,
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

/// Tokenizer for LaTeX syntax highlighting.
pub struct LatexLexer;

impl LatexLexer {
    /// Tokenizes LaTeX source code into tokens for syntax highlighting.
    pub fn tokenize(input: &str) -> Vec<Token<'_>> {
        let mut tokens = Vec::new();
        let bytes = input.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            // Check for comment
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

            // Check for command: starts with \ and followed by letters or a single non-letter
            if bytes[i] == b'\\' {
                let start = i;
                i += 1;
                if i < len {
                    if bytes[i].is_ascii_alphabetic() {
                        while i < len && bytes[i].is_ascii_alphabetic() {
                            i += 1;
                        }
                    } else {
                        // Single symbol command like \$, \&, \\, etc.
                        i += 1;
                    }
                }
                tokens.push(Token {
                    token_type: TokenType::Command,
                    text: &input[start..i],
                    start,
                    end: i,
                });
                continue;
            }

            // Display math $$ ... $$
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
                continue;
            }

            // Inline math $ ... $
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
                continue;
            }

            // Braces and Brackets
            if bytes[i] == b'{' || bytes[i] == b'}' {
                let start = i;
                i += 1;
                tokens.push(Token {
                    token_type: TokenType::Brace,
                    text: &input[start..i],
                    start,
                    end: i,
                });
                continue;
            }

            if bytes[i] == b'[' || bytes[i] == b']' {
                let start = i;
                i += 1;
                tokens.push(Token {
                    token_type: TokenType::Bracket,
                    text: &input[start..i],
                    start,
                    end: i,
                });
                continue;
            }

            // Special characters like &, _, ^, ~, #
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

            // Plain text sequence
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
            {
                i += 1;
            }
            tokens.push(Token {
                token_type: TokenType::PlainText,
                text: &input[start..i],
                start,
                end: i,
            });
        }

        tokens
    }

    /// Color for token in dark mode
    pub fn token_color_dark(token_type: TokenType) -> Color32 {
        match token_type {
            TokenType::Comment => Color32::from_rgb(108, 118, 133), // Slate gray
            TokenType::Command => Color32::from_rgb(97, 175, 239),  // Light cyan / blue
            TokenType::MathInline => Color32::from_rgb(229, 192, 123), // Soft gold
            TokenType::MathBlock => Color32::from_rgb(209, 154, 102),  // Warm amber
            TokenType::Brace => Color32::from_rgb(198, 120, 221),   // Purple
            TokenType::Bracket => Color32::from_rgb(86, 182, 194), // Teal
            TokenType::SpecialChar => Color32::from_rgb(224, 108, 117), // Coral red
            TokenType::Argument => Color32::from_rgb(152, 195, 121), // Muted green
            TokenType::PlainText => Color32::from_rgb(220, 223, 228), // Off-white
        }
    }

    /// Color for token in light mode
    pub fn token_color_light(token_type: TokenType) -> Color32 {
        match token_type {
            TokenType::Comment => Color32::from_rgb(120, 130, 140),
            TokenType::Command => Color32::from_rgb(16, 92, 176),
            TokenType::MathInline => Color32::from_rgb(160, 90, 10),
            TokenType::MathBlock => Color32::from_rgb(170, 70, 0),
            TokenType::Brace => Color32::from_rgb(130, 30, 160),
            TokenType::Bracket => Color32::from_rgb(0, 120, 130),
            TokenType::SpecialChar => Color32::from_rgb(180, 20, 30),
            TokenType::Argument => Color32::from_rgb(30, 120, 50),
            TokenType::PlainText => Color32::from_rgb(30, 35, 45),
        }
    }

    /// Generates an egui LayoutJob with syntax highlighting
    pub fn highlight(input: &str, is_dark: bool, font_id: FontId) -> egui::text::LayoutJob {
        let mut job = egui::text::LayoutJob::default();
        let tokens = Self::tokenize(input);

        for token in tokens {
            let color = if is_dark {
                Self::token_color_dark(token.token_type)
            } else {
                Self::token_color_light(token.token_type)
            };

            job.append(
                token.text,
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color,
                    ..Default::default()
                },
            );
        }

        job
    }
}
