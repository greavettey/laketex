//! # Compiler Diagnostics & Error Log Parsing
//!
//! Parses raw standard output, standard error, and `.log` streams from TeX compilers into
//! structured [`Diagnostic`] items with line locations and severity classifications.

use serde::{Deserialize, Serialize};

/// Severity classification of a compiler diagnostic message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    /// Fatal or breaking compilation error
    Error,
    /// Non-fatal formatting or reference warning (e.g. undefined reference, overfull box)
    Warning,
    /// Informational compiler status message
    Info,
}

impl DiagnosticSeverity {
    /// Returns the UI badge color as an `(r, g, b)` tuple
    pub fn badge_color(&self) -> (u8, u8, u8) {
        match self {
            Self::Error => (235, 87, 87),     // Soft red
            Self::Warning => (242, 201, 76),  // Amber yellow
            Self::Info => (86, 182, 194),     // Cyan blue
        }
    }

    /// Returns the short display label (e.g., "ERROR", "WARN")
    pub fn label(&self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warning => "WARN",
            Self::Info => "INFO",
        }
    }
}

/// A structured diagnostic message generated during LaTeX compilation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Severity level of the diagnostic
    pub severity: DiagnosticSeverity,
    /// Primary error or warning message
    pub message: String,
    /// 1-based source code line number where the issue occurred, if determinable
    pub line: Option<usize>,
    /// Source file path where the issue occurred, if determinable
    pub file: Option<String>,
    /// Multi-line contextual details or TeX snippet extracted from the log
    pub details: Option<String>,
}

/// Parser for extracting structured diagnostics from TeX compiler output.
pub struct DiagnosticParser;

impl DiagnosticParser {
    /// Parses raw stdout / stderr / log lines into structured diagnostics.
    ///
    /// Recognizes:
    /// - Classical TeX errors starting with `!` followed by `l.<num>` line indicators
    /// - Tectonic formatted errors (`error: ... line <num>`)
    /// - LaTeX package warnings (`LaTeX Warning: ...`)
    /// - Typesetting box warnings (`Overfull \hbox`, `Underfull \hbox`)
    pub fn parse(output: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let lines: Vec<&str> = output.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // TeX Error starting with '!'
            if line.starts_with('!') {
                let error_msg = line.trim_start_matches('!').trim().to_string();
                let mut error_line = None;
                let mut details = Vec::new();

                // Look ahead for "l.<number>"
                let mut j = i + 1;
                while j < lines.len() && j < i + 6 {
                    let next_line = lines[j].trim();
                    if next_line.starts_with('!')
                        || next_line.contains("LaTeX Warning:")
                        || next_line.starts_with("Overfull")
                        || next_line.starts_with("Underfull")
                    {
                        break;
                    }
                    if next_line.starts_with("l.") {
                        if let Some(space_idx) = next_line[2..].find(' ') {
                            if let Ok(num) = next_line[2..2 + space_idx].parse::<usize>() {
                                error_line = Some(num);
                            }
                        } else if let Ok(num) = next_line[2..].parse::<usize>() {
                            error_line = Some(num);
                        }
                        details.push(next_line.to_string());
                    } else if !next_line.is_empty() {
                        details.push(next_line.to_string());
                    } else if error_line.is_some() {
                        // Empty line after l.<num> terminates error message block
                        break;
                    }
                    j += 1;
                }

                diagnostics.push(Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    message: error_msg,
                    line: error_line,
                    file: None,
                    details: if details.is_empty() {
                        None
                    } else {
                        Some(details.join("\n"))
                    },
                });

                i = j;
                continue;
            }

            // Tectonic error: "error: ..."
            if let Some(msg) = line.strip_prefix("error: ") {
                let mut err_line = None;
                let mut err_file = None;
                let mut clean_msg = msg.to_string();

                // Check for pattern "filename.tex:10: message"
                if let Some(colon1) = msg.find(':') {
                    let file_part = &msg[..colon1];
                    let after_colon1 = &msg[colon1 + 1..];
                    if let Some(colon2) = after_colon1.find(':') {
                        let line_part = after_colon1[..colon2].trim();
                        if let Ok(l) = line_part.parse::<usize>() {
                            err_line = Some(l);
                            err_file = Some(file_part.trim().to_string());
                            clean_msg = after_colon1[colon2 + 1..].trim().to_string();
                        }
                    }
                }

                if err_line.is_none() {
                    if let Some(pos) = msg.find("line ") {
                        let rest = &msg[pos + 5..];
                        if let Some(end) = rest.find(|c: char| !c.is_ascii_digit()) {
                            if let Ok(l) = rest[..end].parse::<usize>() {
                                err_line = Some(l);
                            }
                        }
                    }
                }

                diagnostics.push(Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    message: clean_msg,
                    line: err_line,
                    file: err_file,
                    details: None,
                });
            }
            // LaTeX Warning
            else if line.contains("LaTeX Warning:") {
                let msg = if let Some(pos) = line.find("LaTeX Warning:") {
                    &line[pos + 14..]
                } else {
                    line
                };
                let mut warn_line = None;
                if let Some(pos) = line.find("line ") {
                    let rest = &line[pos + 5..];
                    if let Some(end) = rest.find(|c: char| !c.is_ascii_digit()) {
                        if let Ok(l) = rest[..end].parse::<usize>() {
                            warn_line = Some(l);
                        }
                    } else if let Ok(l) = rest.trim().parse::<usize>() {
                        warn_line = Some(l);
                    }
                }

                diagnostics.push(Diagnostic {
                    severity: DiagnosticSeverity::Warning,
                    message: msg.trim().to_string(),
                    line: warn_line,
                    file: None,
                    details: None,
                });
            }
            // Overfull / Underfull boxes
            else if line.starts_with("Overfull \\hbox") || line.starts_with("Overfull \\vbox") {
                let mut box_line = None;
                if let Some(pos) = line.find("at lines ") {
                    let rest = &line[pos + 9..];
                    if let Some(end) = rest.find(|c: char| !c.is_ascii_digit()) {
                        if let Ok(l) = rest[..end].parse::<usize>() {
                            box_line = Some(l);
                        }
                    }
                }
                diagnostics.push(Diagnostic {
                    severity: DiagnosticSeverity::Warning,
                    message: line.to_string(),
                    line: box_line,
                    file: None,
                    details: None,
                });
            }

            i += 1;
        }

        diagnostics
    }
}
