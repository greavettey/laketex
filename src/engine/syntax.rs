//! # Native Syntax Highlighting Engine
//!
//! Provides syntax highlighting for code blocks (`minted`, `lstlisting`, `verbatim`)
//! in both HTML PDF export and interactive vector preview.
//!
//! Workflow:
//! 1. If `pygmentize` (Python Pygments) is available on the user's system, it delegates
//!    to `pygmentize -f html -O nowrap=True` for 100% parity with LaTeX `minted`.
//! 2. If `pygmentize` is not installed or offline, it falls back to an internal, zero-dependency,
//!    pure-Rust lexer supporting Python, Rust, C/C++, Java, JavaScript, TypeScript, Go, HTML,
//!    LaTeX, and Shell with Pygments-compatible token classes (`.k`, `.nf`, `.s`, `.c`, `.m`, etc.).

use egui::Color32;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Write;
use std::process::Stdio;
use std::sync::{LazyLock, Mutex};

static PYGMENTIZE_CACHE: LazyLock<Mutex<HashMap<(u64, String), Option<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Syntax token classification matching Pygments standards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Language keywords (e.g., `fn`, `let`, `def`, `class`, `import`).
    Keyword,
    /// Function and method declarations or calls.
    Function,
    /// Types, traits, structs, and class names.
    Type,
    /// String and character literals.
    StringLiteral,
    /// Single-line and multi-line comments.
    Comment,
    /// Numeric constants (integers, floats, hex).
    Number,
    /// Mathematical, logical, and assignment operators.
    Operator,
    /// Built-in primitive types, macros, and standard constants.
    Builtin,
    /// Standard unhighlighted identifiers and plain text.
    Plain,
}

impl TokenKind {
    /// Pygments HTML span class name.
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Keyword => "k",
            Self::Function => "nf",
            Self::Type => "nc",
            Self::StringLiteral => "s",
            Self::Comment => "c1",
            Self::Number => "mi",
            Self::Operator => "o",
            Self::Builtin => "nb",
            Self::Plain => "n",
        }
    }

    /// UI canvas Color32 depending on light or dark background mode.
    pub fn color(&self, is_dark: bool) -> Color32 {
        if is_dark {
            match self {
                Self::Keyword => Color32::from_rgb(255, 121, 198),     // Magenta / Pink
                Self::Function => Color32::from_rgb(80, 250, 123),     // Green
                Self::Type => Color32::from_rgb(139, 233, 253),         // Cyan
                Self::StringLiteral => Color32::from_rgb(241, 250, 140),// Yellow
                Self::Comment => Color32::from_rgb(98, 114, 164),       // Muted Purple
                Self::Number => Color32::from_rgb(189, 147, 249),       // Purple
                Self::Operator => Color32::from_rgb(255, 184, 108),     // Orange
                Self::Builtin => Color32::from_rgb(139, 233, 253),      // Cyan
                Self::Plain => Color32::from_rgb(248, 248, 242),        // Bright foreground
            }
        } else {
            match self {
                Self::Keyword => Color32::from_rgb(0, 128, 0),          // Green bold
                Self::Function => Color32::from_rgb(0, 0, 255),         // Blue
                Self::Type => Color32::from_rgb(43, 145, 175),          // Teal
                Self::StringLiteral => Color32::from_rgb(186, 33, 33),   // Reddish brown
                Self::Comment => Color32::from_rgb(64, 128, 128),       // Gray teal italic
                Self::Number => Color32::from_rgb(0, 136, 0),           // Dark green
                Self::Operator => Color32::from_rgb(102, 102, 102),     // Gray
                Self::Builtin => Color32::from_rgb(0, 134, 179),        // Dark cyan
                Self::Plain => Color32::from_rgb(36, 41, 47),           // Dark slate
            }
        }
    }
}

/// Syntax highlighter for minted and listings code blocks.
pub struct SyntaxHighlighter;

impl SyntaxHighlighter {
    /// Highlights code into HTML markup with embedded `<span class="...">` tags.
    pub fn highlight_html(code: &str, language: &str) -> String {
        // Attempt external pygmentize first if available
        if let Some(pyg_html) = Self::try_pygmentize_html(code, language) {
            return pyg_html;
        }

        // Native pure-Rust fallback highlighter
        Self::native_highlight_html(code, language)
    }

    /// Invokes external `pygmentize` process via pipe if installed.
    ///
    /// # Arguments
    /// * `code` - Raw source code snippet to highlight.
    /// * `language` - Target programming language or lexer identifier.
    ///
    /// # Security
    /// Strictly sanitizes `language` to ASCII alphanumeric characters, `+`, `#`, `-`, and `_`.
    /// Rejects flags starting with `-` to avoid argument injection, and rejects file paths or
    /// class separators (such as `path/to/lexer.py:CustomLexer`) that could trigger arbitrary
    /// Python code execution. Returns `None` if validation fails, safely falling back to native highlighting.
    fn try_pygmentize_html(code: &str, language: &str) -> Option<String> {
        let trimmed = language.trim();
        let lang = if trimmed.is_empty() || trimmed == "code" {
            "text"
        } else {
            trimmed
        };

        // Strictly validate language identifier to prevent command injection,
        // argument injection (e.g. flags starting with '-'), or Pygments custom lexer execution (e.g. 'lexer.py:Custom')
        if lang.starts_with('-') || !lang.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '#' || c == '-' || c == '_') {
            return None;
        }

        let mut hasher = DefaultHasher::new();
        code.hash(&mut hasher);
        let cache_key = (hasher.finish(), lang.to_string());
        if let Ok(cache) = PYGMENTIZE_CACHE.lock() {
            if let Some(cached) = cache.get(&cache_key) {
                return cached.clone();
            }
        }

        let pygmentize_cmd = crate::document::packages::PackageManager::find_binary_path("pygmentize")
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "pygmentize".to_string());
        let mut cmd = crate::create_hidden_command(&pygmentize_cmd);
        cmd.args(["-l", lang, "-f", "html", "-O", "nowrap=True"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = cmd.spawn().ok()?;
        let stdin = child.stdin.take();
        let output = std::thread::scope(|s| {
            if let Some(mut stdin_pipe) = stdin {
                let bytes = code.as_bytes();
                s.spawn(move || {
                    let _ = stdin_pipe.write_all(bytes);
                });
            }
            child.wait_with_output().ok()
        })?;

        let result = if output.status.success() {
            let res = String::from_utf8_lossy(&output.stdout).trim_end().to_string();
            if !res.is_empty() {
                Some(res)
            } else {
                None
            }
        } else {
            None
        };

        if let Ok(mut cache) = PYGMENTIZE_CACHE.lock() {
            if cache.len() > 256 {
                cache.clear();
            }
            cache.insert(cache_key, result.clone());
        }
        result
    }

    /// Pure-Rust fallback syntax tokenizer emitting Pygments-compatible HTML tags.
    pub fn native_highlight_html(code: &str, language: &str) -> String {
        let mut html = String::new();
        let lines: Vec<&str> = code.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let tokens = Self::tokenize_line(line, language);
            for (text, kind) in tokens {
                let escaped = crate::engine::pdf_exporter::NativePdfExporter::escape_xml(&text);
                if kind == TokenKind::Plain {
                    html.push_str(&escaped);
                } else {
                    html.push_str(&format!(
                        "<span class=\"{}\">{}</span>",
                        kind.css_class(),
                        escaped
                    ));
                }
            }
            if idx + 1 < lines.len() {
                html.push('\n');
            }
        }

        html
    }

    /// Tokenizes code lines into (text, color) tuples for egui vector canvas rendering.
    pub fn tokenize_for_ui(code: &str, language: &str, is_dark: bool) -> Vec<Vec<(String, Color32)>> {
        let mut result = Vec::new();
        for line in code.lines() {
            let mut line_tokens = Vec::new();
            let tokens = Self::tokenize_line(line, language);
            for (text, kind) in tokens {
                line_tokens.push((text, kind.color(is_dark)));
            }
            result.push(line_tokens);
        }
        result
    }

    /// Tokenizes a single line of source code into syntax tokens.
    fn tokenize_line(line: &str, language: &str) -> Vec<(String, TokenKind)> {
        let lang_lower = language.to_ascii_lowercase();
        let comment_prefix = match lang_lower.as_str() {
            "python" | "py" | "bash" | "sh" | "yaml" | "yml" | "perl" => Some("#"),
            "latex" | "tex" => Some("%"),
            _ => Some("//"), // C, C++, Rust, Java, JS, TS, Go
        };

        let mut tokens = Vec::new();
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            // Check line comments
            if let Some(cp) = comment_prefix {
                let remaining: String = chars[i..].iter().collect();
                if remaining.starts_with(cp) {
                    tokens.push((remaining, TokenKind::Comment));
                    break;
                }
            }

            let c = chars[i];

            // String literals
            if c == '"' || c == '\'' || c == '`' {
                let quote = c;
                let mut s = String::new();
                s.push(quote);
                i += 1;
                while i < chars.len() {
                    let sc = chars[i];
                    s.push(sc);
                    if sc == '\\' && i + 1 < chars.len() {
                        i += 1;
                        s.push(chars[i]);
                    } else if sc == quote {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                tokens.push((s, TokenKind::StringLiteral));
                continue;
            }

            // Numeric literals
            if c.is_ascii_digit() && (i == 0 || !chars[i - 1].is_alphanumeric() && chars[i - 1] != '_') {
                let mut num = String::new();
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '.' || chars[i] == '_') {
                    num.push(chars[i]);
                    i += 1;
                }
                tokens.push((num, TokenKind::Number));
                continue;
            }

            // Word tokens (keywords, functions, types, identifiers)
            if c.is_alphabetic() || c == '_' {
                let mut word = String::new();
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    word.push(chars[i]);
                    i += 1;
                }

                // Lookahead for function call `foo(`
                let is_func = {
                    let mut look = i;
                    while look < chars.len() && chars[look].is_whitespace() {
                        look += 1;
                    }
                    look < chars.len() && chars[look] == '('
                };

                let kind = if Self::is_keyword(&word, &lang_lower) {
                    TokenKind::Keyword
                } else if is_func {
                    TokenKind::Function
                } else if word.chars().next().map_or(false, |ch| ch.is_uppercase()) && word.len() > 1 {
                    TokenKind::Type
                } else if Self::is_builtin(&word, &lang_lower) {
                    TokenKind::Builtin
                } else {
                    TokenKind::Plain
                };

                tokens.push((word, kind));
                continue;
            }

            // Operators & Punctuation
            if matches!(c, '=' | '+' | '-' | '*' | '/' | '%' | '&' | '|' | '^' | '!' | '<' | '>' | '?' | ':' | '~') {
                tokens.push((c.to_string(), TokenKind::Operator));
                i += 1;
                continue;
            }

            // Whitespace or plain character
            tokens.push((c.to_string(), TokenKind::Plain));
            i += 1;
        }

        tokens
    }

    /// Matches common language keywords.
    fn is_keyword(word: &str, lang: &str) -> bool {
        match lang {
            "python" | "py" => matches!(
                word,
                "def"
                    | "class"
                    | "return"
                    | "if"
                    | "elif"
                    | "else"
                    | "while"
                    | "for"
                    | "in"
                    | "import"
                    | "from"
                    | "as"
                    | "try"
                    | "except"
                    | "finally"
                    | "with"
                    | "lambda"
                    | "pass"
                    | "break"
                    | "continue"
                    | "yield"
                    | "async"
                    | "await"
                    | "None"
                    | "True"
                    | "False"
                    | "is"
                    | "not"
                    | "and"
                    | "or"
            ),
            "rust" | "rs" => matches!(
                word,
                "fn"
                    | "let"
                    | "mut"
                    | "struct"
                    | "enum"
                    | "trait"
                    | "impl"
                    | "pub"
                    | "mod"
                    | "use"
                    | "crate"
                    | "return"
                    | "if"
                    | "else"
                    | "match"
                    | "while"
                    | "loop"
                    | "for"
                    | "in"
                    | "where"
                    | "type"
                    | "as"
                    | "ref"
                    | "self"
                    | "Self"
                    | "super"
                    | "const"
                    | "static"
                    | "async"
                    | "await"
                    | "move"
                    | "unsafe"
            ),
            "c" | "cpp" | "c++" => matches!(
                word,
                "int"
                    | "float"
                    | "double"
                    | "char"
                    | "void"
                    | "auto"
                    | "const"
                    | "static"
                    | "struct"
                    | "class"
                    | "public"
                    | "private"
                    | "protected"
                    | "return"
                    | "if"
                    | "else"
                    | "switch"
                    | "case"
                    | "break"
                    | "continue"
                    | "for"
                    | "while"
                    | "template"
                    | "typename"
                    | "namespace"
                    | "using"
                    | "new"
                    | "delete"
            ),
            "javascript" | "js" | "typescript" | "ts" => matches!(
                word,
                "function"
                    | "const"
                    | "let"
                    | "var"
                    | "return"
                    | "if"
                    | "else"
                    | "switch"
                    | "case"
                    | "break"
                    | "for"
                    | "while"
                    | "import"
                    | "export"
                    | "default"
                    | "from"
                    | "class"
                    | "extends"
                    | "new"
                    | "this"
                    | "async"
                    | "await"
                    | "try"
                    | "catch"
            ),
            _ => matches!(
                word,
                "def"
                    | "fn"
                    | "function"
                    | "class"
                    | "return"
                    | "if"
                    | "else"
                    | "while"
                    | "for"
                    | "in"
                    | "import"
                    | "public"
                    | "private"
                    | "static"
                    | "let"
                    | "const"
            ),
        }
    }

    /// Matches common built-in symbols or types.
    fn is_builtin(word: &str, _lang: &str) -> bool {
        matches!(
            word,
            "print"
                | "println"
                | "len"
                | "range"
                | "str"
                | "int"
                | "float"
                | "bool"
                | "list"
                | "dict"
                | "set"
                | "tuple"
                | "Option"
                | "Some"
                | "None"
                | "Result"
                | "Ok"
                | "Err"
                | "Vec"
                | "String"
                | "format"
                | "panic"
                | "console"
                | "log"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_highlighting_html() {
        let code = "def compute(x):\n    # Return square\n    return x * 2";
        let html = SyntaxHighlighter::highlight_html(code, "python");
        assert!(html.contains("class=\"k\"") || html.contains("class=\"nf\""), "HTML must contain keyword or function styling");
        assert!(html.contains("compute"));
    }

    #[test]
    fn test_syntax_tokenization_ui() {
        let code = "fn main() {\n    let val = 42;\n}";
        let tokens = SyntaxHighlighter::tokenize_for_ui(code, "rust", false);
        assert_eq!(tokens.len(), 3);
        assert!(tokens[0].iter().any(|(t, _)| t == "fn"));
        assert!(tokens[1].iter().any(|(t, _)| t == "let"));
        assert!(tokens[1].iter().any(|(t, _)| t == "42"));
    }
}
