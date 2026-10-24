//! # Contextual Error Report Generation & Export
//!
//! Generates deeply contextual diagnostic reports when LaTeX compilation fails.
//!
//! Rather than presenting bare terminal errors, [`ErrorReport`] correlates each diagnostic
//! with:
//! - Exact surrounding LaTeX source code lines with targeted line markers (`>>>`)
//! - Inferred environment intent (e.g. "Typesetting '\begin{equation}' inside section 'Methodology'")
//! - Specific categorization (e.g., Undefined Macro, Missing Package, Math Mode Syntax)
//! - Actionable human-readable suggested resolutions
//! - Dual export to GitHub-Flavored Markdown (`.md`) and structured JSON (`.json`)

use crate::compiler::diagnostic::{Diagnostic, DiagnosticSeverity};
use crate::compiler::engine::EngineKind;
use crate::document::packages::PackageManager;
use serde::{Deserialize, Serialize};

/// A single source line included in surrounding error context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextLine {
    /// 1-based source line number
    pub line_number: usize,
    /// Line text content
    pub content: String,
    /// Whether this is the exact line that triggered the compiler error
    pub is_target: bool,
}

/// A diagnostic entry augmented with source context, inferred intent, and suggested fixes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextualDiagnostic {
    /// Diagnostic severity
    pub severity: DiagnosticSeverity,
    /// Compiler error message
    pub message: String,
    /// 1-based line number if known
    pub line: Option<usize>,
    /// High-level error classification category
    pub category: String,
    /// Surrounding source lines showing the problem in context
    pub surrounding_context: Vec<ContextLine>,
    /// Inferred intention of what the LaTeX document was doing at this position
    pub environment_intent: String,
    /// Concrete recommendation for how the user can resolve the error
    pub suggested_fix: String,
    /// Raw output snippet from the compiler log
    pub details: Option<String>,
}

/// Comprehensive contextual error report containing execution environment, source metrics,
/// diagnostics, and raw compiler streams.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorReport {
    /// LakeTex version
    pub app_version: String,
    /// Operating system platform and architecture
    pub os_platform: String,
    /// Timestamp of generation
    pub timestamp: String,
    /// Name or path of the compiled document
    pub document_name: String,
    /// Compilation engine backend used
    pub engine_name: String,
    /// Total lines in the document
    pub total_lines: usize,
    /// Total words in the document
    pub word_count: usize,
    /// CTAN packages detected in the document preamble
    pub detected_packages: Vec<String>,
    /// Number of fatal errors
    pub error_count: usize,
    /// Number of warnings
    pub warning_count: usize,
    /// Context-rich diagnostics list
    pub diagnostics: Vec<ContextualDiagnostic>,
    /// Raw stdout captured from the compiler process
    pub raw_stdout: String,
    /// Raw stderr captured from the compiler process
    pub raw_stderr: String,
}

impl ErrorReport {
    /// Generates a comprehensive contextual error report from document source and diagnostics.
    pub fn generate(
        document_source: &str,
        document_name: &str,
        engine: EngineKind,
        diagnostics: &[Diagnostic],
        raw_stdout: &str,
        raw_stderr: &str,
    ) -> Self {
        let lines: Vec<&str> = document_source.lines().collect();
        let total_lines = lines.len();
        let word_count = document_source.split_whitespace().count();

        let packages = PackageManager::detect_packages(document_source);
        let detected_packages = packages.into_iter().map(|p| p.name).collect();

        let error_count = diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .count();
        let warning_count = diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .count();

        let mut contextual_diags = Vec::new();

        for diag in diagnostics {
            let line_num = diag.line;
            let is_main_doc = match &diag.file {
                None => true,
                Some(f) => f == document_name || f.starts_with(".laketex_") || f == "document.tex",
            };
            let surrounding = if is_main_doc {
                if let Some(target_line) = line_num {
                    if target_line > 0 && target_line <= lines.len() {
                        let target_idx = target_line - 1;
                        let start_idx = target_idx.saturating_sub(3);
                        let end_idx = (target_idx + 4).min(lines.len());

                        (start_idx..end_idx)
                            .map(|idx| ContextLine {
                                line_number: idx + 1,
                                content: lines[idx].to_string(),
                                is_target: idx == target_idx,
                            })
                            .collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            let intent = Self::infer_environment_intent(&lines, line_num);
            let (category, fix) = Self::categorize_and_suggest(&diag.message);

            contextual_diags.push(ContextualDiagnostic {
                severity: diag.severity,
                message: diag.message.clone(),
                line: diag.line,
                category,
                surrounding_context: surrounding,
                environment_intent: intent,
                suggested_fix: fix,
                details: diag.details.clone(),
            });
        }

        let os_platform = format!("{} ({})", std::env::consts::OS, std::env::consts::ARCH);
        let timestamp = Self::current_timestamp();

        Self {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            os_platform,
            timestamp,
            document_name: document_name.to_string(),
            engine_name: engine.display_name().to_string(),
            total_lines,
            word_count,
            detected_packages,
            error_count,
            warning_count,
            diagnostics: contextual_diags,
            raw_stdout: raw_stdout.to_string(),
            raw_stderr: raw_stderr.to_string(),
        }
    }

    /// Infers what the document was trying to do around the given line
    fn infer_environment_intent(lines: &[&str], line_num: Option<usize>) -> String {
        let Some(target) = line_num else {
            return "General document scope / preamble initialization".to_string();
        };

        if target == 0 || target > lines.len() {
            return "End of document / post-processing".to_string();
        }

        let target_idx = target - 1;

        // Check if inside preamble
        let mut doc_start_idx = None;
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("\\begin{document}") {
                doc_start_idx = Some(idx);
                break;
            }
        }

        if let Some(doc_idx) = doc_start_idx {
            if target_idx <= doc_idx {
                return "Declaring document configuration, metadata, or packages in preamble".to_string();
            }
        }

        // Scan backwards for innermost environment or section
        let mut last_env = None;
        let mut last_section = None;

        for idx in (0..=target_idx).rev() {
            let l = lines[idx].trim();
            if last_env.is_none() && l.starts_with("\\begin{") {
                if let Some(end) = l.find('}') {
                    let env_name = &l[7..end];
                    if env_name != "document" {
                        last_env = Some(env_name.to_string());
                    }
                }
            }

            if last_section.is_none()
                && (l.starts_with("\\section{")
                    || l.starts_with("\\subsection{")
                    || l.starts_with("\\subsubsection{")
                    || l.starts_with("\\chapter{"))
            {
                if let Some(start) = l.find('{') {
                    if let Some(end) = l.find('}') {
                        let sec_title = &l[start + 1..end];
                        last_section = Some(sec_title.to_string());
                    }
                }
            }

            if last_env.is_some() && last_section.is_some() {
                break;
            }
        }

        match (last_env, last_section) {
            (Some(env), Some(sec)) => {
                format!("Typesetting '\\begin{{{env}}}' environment inside section '{sec}'")
            }
            (Some(env), None) => format!("Typesetting '\\begin{{{env}}}' environment"),
            (None, Some(sec)) => format!("Composing content in section '{sec}'"),
            (None, None) => "Composing document body".to_string(),
        }
    }

    /// Categorizes the error message and generates tailored suggested fixes
    fn categorize_and_suggest(message: &str) -> (String, String) {
        let msg = message.to_lowercase();

        if msg.contains("pgfkeys") || msg.contains("pgfplots") || msg.contains("/tikz/") {
            if msg.contains("linewidth") {
                (
                    "TikZ / Pgfplots Key Typo ('linewidth')".to_string(),
                    "In TikZ and pgfplots, line thickness must be specified as 'line width=<dimen>' (with a space), e.g. 'line width=1pt', NOT 'linewidth=...'. Alternatively, use predefined thickness keywords: 'thin', 'semithick', 'thick', 'very thick', or 'ultra thick'.".to_string(),
                )
            } else if msg.contains("i do not know the key") {
                (
                    "Unknown TikZ / Pgfkeys Option".to_string(),
                    "The key passed to TikZ/pgfplots is unrecognized. Check for typos or missing spaces. If using advanced plot features (such as fill patterns, custom marks, or decorations), ensure the required TikZ library is loaded in the preamble using '\\usetikzlibrary{...}'.".to_string(),
                )
            } else {
                (
                    "TikZ / Pgfplots Graphics Error".to_string(),
                    "Review pgfplots / TikZ syntax. Verify coordinate lists, axis options, and domain bounds. Ensure '\\usepackage{pgfplots}' is loaded and '\\pgfplotsset{compat=1.18}' is declared in the preamble.".to_string(),
                )
            }
        } else if (msg.contains("reference") || msg.contains("ref `")) && (msg.contains("undefined") || msg.contains("multiply defined")) {
            (
                "Undefined Cross-Reference".to_string(),
                "The label referenced by '\\ref{...}' or '\\pageref{...}' was not found in the document auxiliary table. Ensure a matching '\\label{...}' is defined. LaTeX cross-references resolve on a second compilation pass once .aux tables are finalized.".to_string(),
            )
        } else if msg.contains("fancyhdr") && msg.contains("headheight") {
            (
                "Header Height Layout Warning (fancyhdr)".to_string(),
                "The page header requires more vertical clearance. Add '\\setlength{\\headheight}{14pt}' to your preamble to eliminate fancyhdr underfull header warnings.".to_string(),
            )
        } else if msg.contains("token not allowed in a pdf string") {
            (
                "PDF Bookmark Warning (hyperref)".to_string(),
                "Math mode symbols or formatting macros were used inside a section heading. Hyperref cannot embed raw math into plain-text PDF bookmarks. Wrap the math with '\\texorpdfstring{$math$}{plain text fallback}' (e.g., '\\section{Gain \\texorpdfstring{$A_v$}{Av}}').".to_string(),
            )
        } else if msg.contains("minted") || msg.contains("shell-escape") || msg.contains("pygmentize") {
            (
                "Shell Escape / Minted Package Dependency".to_string(),
                "The 'minted' package delegates code syntax highlighting to Python's Pygments library via shell escape. Laketex automatically enables '-Z shell-escape' for Tectonic. If Pygments is not installed, install it with 'pip install pygments', or use '\\usepackage[draft]{minted}' to compile in draft mode without external Python, or use Laketex's Native engine which renders syntax-highlighted code blocks natively.".to_string(),
            )
        } else if msg.contains("siunitx") || msg.contains("unit") && (msg.contains("invalid") || msg.contains("unknown")) {
            (
                "SI Unit Syntax (siunitx)".to_string(),
                "Verify 'siunitx' macro syntax. Recommended commands are '\\SI{value}{unit}', '\\qty{value}{unit}', '\\num{number}', or '\\ang{angle}'. Ensure unit macros like '\\meter', '\\second', '\\kilo\\gram' are valid.".to_string(),
            )
        } else if msg.contains("geometry") {
            (
                "Page Geometry Configuration".to_string(),
                "Check '\\geometry{...}' parameters. Supported options include 'letterpaper', 'a4paper', 'margin=1in', 'top=25mm', 'bottom=25mm'.".to_string(),
            )
        } else if msg.contains("booktabs") {
            (
                "Booktabs Structure".to_string(),
                "Ensure '\\toprule', '\\midrule', '\\bottomrule', or '\\cmidrule' are placed inside a 'tabular' environment.".to_string(),
            )
        } else if msg.contains("undefined control sequence") {
            (
                "Undefined Command / Macro".to_string(),
                "The command was not recognized by the TeX engine. Verify the spelling of the command. If this is a math macro, ensure \\usepackage{amsmath} or \\usepackage{amssymb} is included in the preamble. If it is a symbol or graphic command, verify that the corresponding CTAN package is declared.".to_string(),
            )
        } else if msg.contains("file") && (msg.contains("not found") || msg.contains("cannot find")) {
            (
                "Missing Package or External File".to_string(),
                "A referenced file, stylesheet (.sty), or graphic could not be located. Check that local image or bibliography file paths exist, or verify package spelling in \\usepackage{...}.".to_string(),
            )
        } else if msg.contains("ended by") || msg.contains("unclosed") || (msg.contains("begin{") && msg.contains("end{")) {
            (
                "Mismatched / Unclosed Environment".to_string(),
                "An environment was opened with \\begin{...} but was not properly terminated before another block or \\end{document}. Ensure every \\begin{environment} has a matching \\end{environment}.".to_string(),
            )
        } else if msg.contains("missing $") || msg.contains("displaymath") {
            (
                "Math Mode Syntax Error".to_string(),
                "A mathematical symbol, subscript (_), or superscript (^) was used outside of math mode. Wrap the expression in inline math ($...$) or a display equation environment (\\begin{equation}).".to_string(),
            )
        } else if msg.contains("overfull") {
            (
                "Typography Layout Warning (Overfull Box)".to_string(),
                "Text or a formula exceeds the printable margin. Consider manually hyphenating long words with \\-, rephrasing the sentence, or splitting wide formulas across multiple lines.".to_string(),
            )
        } else if msg.contains("underfull") {
            (
                "Typography Layout Warning (Underfull Box)".to_string(),
                "TeX could not stretch the spacing evenly across the line or page. This is usually aesthetic and does not prevent successful PDF creation.".to_string(),
            )
        } else if msg.contains("emergency stop") {
            (
                "Fatal Engine Abort".to_string(),
                "The compiler encountered an unrecoverable syntax or I/O error and aborted execution. Check the lines immediately preceding the failure.".to_string(),
            )
        } else {
            (
                "LaTeX Syntax Error".to_string(),
                "Inspect the surrounding code snippet above. Check for unmatched curly braces '{ }', missing backslashes, or invalid characters.".to_string(),
            )
        }
    }

    /// Formats the error report into clean, professional GitHub-Flavored Markdown.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str("# 🚨 Laketex Compilation Error & Diagnostic Report\n\n");
        md.push_str("> Generated automatically by Laketex Diagnostic Diagnostics Engine\n\n");

        md.push_str("## 📋 Execution Context\n");
        md.push_str(&format!("- **Document File**: `{}`\n", self.document_name));
        md.push_str(&format!("- **Timestamp**: {}\n", self.timestamp));
        md.push_str(&format!("- **Laketex Version**: v{}\n", self.app_version));
        md.push_str(&format!("- **Operating System**: {}\n", self.os_platform));
        md.push_str(&format!("- **Typesetting Engine**: {}\n", self.engine_name));
        md.push_str(&format!("- **Document Metrics**: {} lines, {} words\n", self.total_lines, self.word_count));
        md.push_str(&format!(
            "- **Status Summary**: **{} Error(s)**, **{} Warning(s)**\n\n",
            self.error_count, self.warning_count
        ));

        if !self.detected_packages.is_empty() {
            md.push_str("### 📦 Imported Packages\n");
            md.push_str(&format!("`{}`\n\n", self.detected_packages.join(", ")));
        }

        md.push_str("## 🔍 Detailed Error Analysis & Code Context\n\n");

        if self.diagnostics.is_empty() {
            md.push_str("*(No structured diagnostics reported. Compilation succeeded or produced raw terminal log errors.)*\n\n");
        } else {
            for (idx, diag) in self.diagnostics.iter().enumerate() {
                let icon = match diag.severity {
                    DiagnosticSeverity::Error => "❌",
                    DiagnosticSeverity::Warning => "⚠️",
                    DiagnosticSeverity::Info => "ℹ️",
                };

                let line_str = diag
                    .line
                    .map(|l| format!("Line {l}"))
                    .unwrap_or_else(|| "General Document Scope".to_string());

                md.push_str(&format!(
                    "### {} Diagnostic #{}: {} ({})\n\n",
                    icon,
                    idx + 1,
                    diag.category,
                    line_str
                ));

                md.push_str(&format!("- **What Went Wrong**: {}\n", diag.message));
                md.push_str(&format!("- **What The File Was Doing**: {}\n\n", diag.environment_intent));

                if !diag.surrounding_context.is_empty() {
                    md.push_str("```latex\n");
                    for ctx_line in &diag.surrounding_context {
                        let marker = if ctx_line.is_target { ">>>" } else { "   " };
                        md.push_str(&format!(
                            "{} {:>4} | {}\n",
                            marker, ctx_line.line_number, ctx_line.content
                        ));
                    }
                    md.push_str("```\n\n");
                }

                md.push_str(&format!("💡 **Suggested Resolution**: {}\n\n", diag.suggested_fix));

                if let Some(details) = &diag.details {
                    md.push_str("**Engine Output Details**:\n```text\n");
                    md.push_str(details);
                    md.push_str("\n```\n\n");
                }

                md.push_str("---\n\n");
            }
        }

        md.push_str("## 📜 Raw Compiler Console Output\n\n");

        if !self.raw_stderr.trim().is_empty() {
            md.push_str("<details open><summary><strong>Standard Error (stderr)</strong></summary>\n\n```text\n");
            md.push_str(&self.raw_stderr);
            md.push_str("\n```\n</details>\n\n");
        }

        if !self.raw_stdout.trim().is_empty() {
            md.push_str("<details><summary><strong>Standard Output (stdout)</strong></summary>\n\n```text\n");
            md.push_str(&self.raw_stdout);
            md.push_str("\n```\n</details>\n\n");
        }

        md
    }

    /// Formats the report into machine-readable JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Helper for current timestamp
    fn current_timestamp() -> String {
        let now = std::time::SystemTime::now();
        let datetime: ChronoFreeTimestamp = now.into();
        datetime.format()
    }
}

// Lightweight timestamp formatter without pulling heavy chrono crate
struct ChronoFreeTimestamp {
    secs: u64,
}

impl From<std::time::SystemTime> for ChronoFreeTimestamp {
    fn from(t: std::time::SystemTime) -> Self {
        let secs = t
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self { secs }
    }
}

impl ChronoFreeTimestamp {
    fn format(&self) -> String {
        let days = self.secs / 86400;
        let day_secs = self.secs % 86400;
        let hours = day_secs / 3600;
        let mins = (day_secs % 3600) / 60;
        let secs = day_secs % 60;
        format!("Unix timestamp {} (Approx UTC {:02}:{:02}:{:02}, Day {})", self.secs, hours, mins, secs, days)
    }
}
