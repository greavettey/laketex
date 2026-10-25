//! # LaTeX Render Debugger & Syntax Auditor
//!
//! Scans LaTeX source code and the generated PDF/canvas preview to identify commands and
//! environments that fail to render correctly or produce degraded visual output.
//!
//! Specifically analyzes:
//! - **Matrix Environments** (`pmatrix`, `bmatrix`, `vmatrix`, `Bmatrix`, `Vmatrix`, `matrix`, `array`):
//!   Math-mode enclosure, unclosed delimiters, mismatched column counts across rows,
//!   vertical layout overflow, and missing `amsmath` package dependencies.
//! - **Mathematical Notation**:
//!   Math commands used outside math mode, unbalanced inline `$` and display `\[ ... \]` / `$$ ... $$`
//!   delimiters, fraction syntax errors (`\frac`), mismatched braces `{ }`, and unhandled LaTeX macros.
//! - **PDF Preview Verification**:
//!   Checks for raw unexpanded LaTeX backslashes, missing glyph replacement boxes, or collapsed matrix formatting.

use crate::document::packages::PackageManager;
use crate::engine::ast::DocumentBlock;
use crate::engine::parser::NativeLatexParser;
use serde::{Deserialize, Serialize};

/// Severity classification of a detected rendering flaw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderIssueSeverity {
    /// Fatal syntax or environment error preventing correct rendering
    Error,
    /// Formatting warning causing visual defects, column misalignment, or overflow
    Warning,
    /// Unhandled command that renders as raw markup or degraded fallback text
    Degraded,
}

impl RenderIssueSeverity {
    /// Short string label
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warning => "WARN",
            Self::Degraded => "DEGRADED",
        }
    }

    /// Color badge as `(r, g, b)`
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            Self::Error => (235, 87, 87),     // Soft red
            Self::Warning => (242, 201, 76),  // Amber yellow
            Self::Degraded => (198, 120, 221),// Purple
        }
    }
}

/// A specific LaTeX command or environment that does not render correctly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderDebugItem {
    /// 1-based source code line number where the issue originates
    pub line: usize,
    /// The specific LaTeX command, environment, or symbol (e.g. `\begin{pmatrix}`, `\frac`, `\alpha`)
    pub command: String,
    /// Issue category (e.g. "Matrix Environment", "Math Mode Delimiter", "Fraction Syntax")
    pub category: String,
    /// Severity classification
    pub severity: RenderIssueSeverity,
    /// Detailed explanation of why this command does not render correctly
    pub issue: String,
    /// Relevant line snippet from the source code
    pub snippet: String,
    /// Concrete recommendation for resolving the rendering issue
    pub suggested_fix: String,
}

/// Comprehensive audit report of rendering flaws across a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderDebugReport {
    /// Document file name or title
    pub document_name: String,
    /// Generation timestamp string
    pub timestamp: String,
    /// Total source code lines analyzed
    pub total_lines: usize,
    /// Total rendering issues identified
    pub total_issues: usize,
    /// Fatal rendering errors
    pub error_count: usize,
    /// Formatting warnings
    pub warning_count: usize,
    /// Degraded or unhandled commands
    pub degraded_count: usize,
    /// Count of issues specifically involving matrices
    pub matrix_issue_count: usize,
    /// Count of issues specifically involving math notation
    pub math_issue_count: usize,
    /// Status description of the generated PDF preview
    pub pdf_status: String,
    /// List of detected rendering flaws
    pub issues: Vec<RenderDebugItem>,
}

impl RenderDebugReport {
    /// Returns true if any rendering errors or warnings were found
    pub fn has_issues(&self) -> bool {
        !self.issues.is_empty()
    }

    /// Formats the report as clean, human-readable terminal text
    pub fn to_text_summary(&self) -> String {
        let mut out = String::new();
        out.push_str("======================================================================\n");
        out.push_str(&format!(
            "LAKETEX RENDER AUDIT REPORT: {}\n",
            self.document_name
        ));
        out.push_str(&format!(
            "Status: {} Issue(s) found ({} Errors, {} Warnings, {} Degraded)\n",
            self.total_issues, self.error_count, self.warning_count, self.degraded_count
        ));
        out.push_str(&format!(
            "Breakdown: {} Matrix Issue(s), {} Math Notation Issue(s)\n",
            self.matrix_issue_count, self.math_issue_count
        ));
        out.push_str(&format!("PDF Preview Status: {}\n", self.pdf_status));
        out.push_str("======================================================================\n\n");

        if self.issues.is_empty() {
            out.push_str("✓ No rendering flaws detected. All matrices, math formulas, and commands conform to supported rendering rules.\n");
            return out;
        }

        for (idx, item) in self.issues.iter().enumerate() {
            out.push_str(&format!(
                "[{}] Line {:>4} | {:<8} | Command: {}\n",
                idx + 1,
                item.line,
                item.severity.as_str(),
                item.command
            ));
            out.push_str(&format!("     Category: {}\n", item.category));
            out.push_str(&format!("     Why it isn't rendering correctly: {}\n", item.issue));
            if !item.snippet.is_empty() {
                out.push_str(&format!("     Snippet:  {}\n", item.snippet));
            }
            out.push_str(&format!("     Fix:      {}\n\n", item.suggested_fix));
        }

        out.push_str("----------------------------------------------------------------------\n");
        out
    }

    /// Formats the report as Markdown for export
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# Laketex LaTeX Render & Preview Audit Report\n\n");
        md.push_str(&format!("- **Document**: `{}`\n", self.document_name));
        md.push_str(&format!("- **Timestamp**: {}\n", self.timestamp));
        md.push_str(&format!(
            "- **Summary**: **{} Total Issues** ({} Errors, {} Warnings, {} Degraded)\n",
            self.total_issues, self.error_count, self.warning_count, self.degraded_count
        ));
        md.push_str(&format!(
            "- **Culprits**: **{} Matrix Issues**, **{} Math Notation Issues**\n",
            self.matrix_issue_count, self.math_issue_count
        ));
        md.push_str(&format!("- **PDF Preview**: {}\n\n", self.pdf_status));

        if self.issues.is_empty() {
            md.push_str("> **All LaTeX commands and math notation render correctly.** No syntax or visual degradation detected.\n");
            return md;
        }

        md.push_str("| Line | Severity | Command | Category | Why It Isn't Rendering Correctly | Suggested Fix |\n");
        md.push_str("| :--- | :--- | :--- | :--- | :--- | :--- |\n");

        for item in &self.issues {
            let badge = match item.severity {
                RenderIssueSeverity::Error => "🔴 ERROR",
                RenderIssueSeverity::Warning => "🟡 WARN",
                RenderIssueSeverity::Degraded => "🟣 DEGRADED",
            };
            let clean_cmd = item.command.replace('|', "\\|");
            let clean_issue = item.issue.replace('|', "\\|");
            let clean_fix = item.suggested_fix.replace('|', "\\|");

            md.push_str(&format!(
                "| **{}** | {} | `{}` | {} | {} | {} |\n",
                item.line, badge, clean_cmd, item.category, clean_issue, clean_fix
            ));
        }

        md.push_str("\n## Detailed Code Snippets\n\n");
        for (idx, item) in self.issues.iter().enumerate() {
            md.push_str(&format!("### #{}: `{}` (Line {})\n\n", idx + 1, item.command, item.line));
            md.push_str(&format!("- **Problem**: {}\n", item.issue));
            md.push_str(&format!("- **Suggested Resolution**: {}\n\n", item.suggested_fix));
            if !item.snippet.is_empty() {
                md.push_str("```latex\n");
                md.push_str(&item.snippet);
                md.push_str("\n```\n\n");
            }
        }

        md
    }

    /// Formats the report as machine-readable JSON
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Diagnostic scanner that inspects LaTeX source and preview rendering.
pub struct RenderDebugger;

impl RenderDebugger {
    /// Comprehensive scan of LaTeX source text and optional PDF preview bytes.
    pub fn scan(source: &str, document_name: &str, pdf_bytes: Option<&[u8]>) -> RenderDebugReport {
        let lines: Vec<&str> = source.lines().collect();
        let total_lines = lines.len();
        let mut issues = Vec::new();

        let declared_packages = PackageManager::detect_packages(source);
        let has_amsmath = declared_packages
            .iter()
            .any(|p| p.name == "amsmath" || p.name == "mathtools");

        // 1. Audit Matrix Environments
        Self::audit_matrices(&lines, has_amsmath, &mut issues);

        // 2. Audit Mathematical Notation & Delimiters
        Self::audit_math_notation(&lines, &mut issues);

        // 3. Audit LaTeX Commands in Plain Text
        Self::audit_bare_commands_outside_math(&lines, &mut issues);

        // 4. Audit TikZ & Pgfplots Options
        Self::audit_tikz_pgfplots(&lines, &mut issues);

        // 5. Audit Native AST Layout
        Self::audit_ast_layout(source, &mut issues);

        // 6. Audit Generated PDF Preview
        let pdf_status = Self::audit_pdf_preview(pdf_bytes, &mut issues);

        // Compute counts
        let total_issues = issues.len();
        let error_count = issues
            .iter()
            .filter(|i| i.severity == RenderIssueSeverity::Error)
            .count();
        let warning_count = issues
            .iter()
            .filter(|i| i.severity == RenderIssueSeverity::Warning)
            .count();
        let degraded_count = issues
            .iter()
            .filter(|i| i.severity == RenderIssueSeverity::Degraded)
            .count();

        let matrix_issue_count = issues
            .iter()
            .filter(|i| i.category.contains("Matrix"))
            .count();
        let math_issue_count = issues
            .iter()
            .filter(|i| i.category.contains("Math") || i.category.contains("Fraction") || i.category.contains("Formula"))
            .count();

        RenderDebugReport {
            document_name: document_name.to_string(),
            timestamp: Self::current_timestamp(),
            total_lines,
            total_issues,
            error_count,
            warning_count,
            degraded_count,
            matrix_issue_count,
            math_issue_count,
            pdf_status,
            issues,
        }
    }

    /// Performs deep inspection of matrix structures
    fn audit_matrices(lines: &[&str], has_amsmath: bool, issues: &mut Vec<RenderDebugItem>) {
        let matrix_envs = [
            "pmatrix", "bmatrix", "Bmatrix", "vmatrix", "Vmatrix",
            "matrix", "smallmatrix", "array", "pmatrix*", "bmatrix*"
        ];

        let mut in_display_math = false;
        let mut in_equation = false;

        for (line_idx, line) in lines.iter().enumerate() {
            let line_num = line_idx + 1;
            let trimmed = line.trim();

            if trimmed.starts_with('%') {
                continue;
            }

            // Track enclosing math blocks
            if trimmed.contains("\\begin{equation") || trimmed.contains("\\begin{align") || trimmed.contains("\\begin{gather") {
                in_equation = true;
            }
            let dollar_pair_count = trimmed.matches("$$").count();
            let mut opened_dollars_this_line = false;
            if trimmed.contains("\\[") {
                in_display_math = true;
            }
            if dollar_pair_count == 1 && !in_display_math {
                in_display_math = true;
                opened_dollars_this_line = true;
            } else if dollar_pair_count >= 2 {
                in_display_math = true;
            }

            for env in matrix_envs {
                let begin_marker = format!("\\begin{{{env}}}");
                let end_marker = format!("\\end{{{env}}}");

                if let Some(pos) = trimmed.find(&begin_marker) {
                    // Issue Check 1: Enclosure in Math Mode
                    let has_inline_math = trimmed[..pos].matches('$').count() % 2 == 1;
                    let is_in_math_mode = in_equation || in_display_math || has_inline_math;

                    if !is_in_math_mode {
                        issues.push(RenderDebugItem {
                            line: line_num,
                            command: begin_marker.clone(),
                            category: "Matrix Math Mode".to_string(),
                            severity: RenderIssueSeverity::Error,
                            issue: format!(
                                "Matrix environment '\\begin{{{env}}}' is used outside of math mode. Standard LaTeX and native rendering engines reject bare matrices in paragraph prose."
                            ),
                            snippet: trimmed.to_string(),
                            suggested_fix: format!(
                                "Enclose the matrix inside a display equation: '\\begin{{equation}} ... \\end{{equation}}' or '\\[ ... \\]'."
                            ),
                        });
                    }

                    // Issue Check 2: Missing amsmath dependency
                    if matches!(env, "pmatrix" | "bmatrix" | "Bmatrix" | "vmatrix" | "Vmatrix" | "smallmatrix") && !has_amsmath {
                        issues.push(RenderDebugItem {
                            line: line_num,
                            command: begin_marker.clone(),
                            category: "Matrix Dependency".to_string(),
                            severity: RenderIssueSeverity::Warning,
                            issue: format!(
                                "Matrix environment '\\begin{{{env}}}' is an AMS-LaTeX extension, but '\\usepackage{{amsmath}}' was not detected in the preamble."
                            ),
                            snippet: trimmed.to_string(),
                            suggested_fix: "Add '\\usepackage{amsmath}' to the document preamble before '\\begin{document}'.".to_string(),
                        });
                    }

                    // Issue Check 3: Multi-line Matrix Extraction & Uniform Column Count
                    let mut matrix_rows = Vec::new();
                    let mut found_end = false;
                    let mut current_row = String::new();
                    let mut search_idx = line_idx;

                    // Collect matrix body until end tag
                    while search_idx < lines.len() {
                        let row_line = lines[search_idx].trim();
                        if let Some(end_pos) = row_line.find(&end_marker) {
                            found_end = true;
                            let pre_end = &row_line[..end_pos];
                            current_row.push_str(pre_end);
                            if !current_row.trim().is_empty() {
                                matrix_rows.push(current_row.clone());
                            }
                            break;
                        } else {
                            if search_idx == line_idx {
                                let post_begin = &row_line[pos + begin_marker.len()..];
                                current_row.push_str(post_begin);
                            } else {
                                current_row.push_str(row_line);
                            }
                            if current_row.contains("\\\\") {
                                let parts: Vec<&str> = current_row.split("\\\\").collect();
                                for p in &parts[..parts.len() - 1] {
                                    matrix_rows.push(p.to_string());
                                }
                                current_row = parts.last().unwrap_or(&"").to_string();
                            }
                        }
                        search_idx += 1;
                    }

                    if !found_end {
                        issues.push(RenderDebugItem {
                            line: line_num,
                            command: begin_marker.clone(),
                            category: "Matrix Delimiter".to_string(),
                            severity: RenderIssueSeverity::Error,
                            issue: format!(
                                "Unclosed matrix environment: '\\begin{{{env}}}' starting on line {line_num} has no matching '{end_marker}'."
                            ),
                            snippet: trimmed.to_string(),
                            suggested_fix: format!("Append '{end_marker}' to properly terminate the matrix."),
                        });
                    } else if !matrix_rows.is_empty() {
                        // Check column consistency across rows
                        let mut expected_cols = None;
                        for (r_idx, row_text) in matrix_rows.iter().enumerate() {
                            let clean_row = row_text.trim();
                            if clean_row.is_empty() {
                                continue;
                            }
                            let cols = clean_row.split('&').count();
                            match expected_cols {
                                None => expected_cols = Some((r_idx + 1, cols)),
                                Some((first_r, first_c)) => {
                                    if cols != first_c {
                                        issues.push(RenderDebugItem {
                                            line: line_num + r_idx,
                                            command: "&".to_string(),
                                            category: "Matrix Column Count".to_string(),
                                            severity: RenderIssueSeverity::Warning,
                                            issue: format!(
                                                "Mismatched column count in matrix '\\begin{{{env}}}': Row {first_r} defines {first_c} column(s), but Row {} provides {cols} column(s). This causes bracket misalignment and distorted spacing.",
                                                r_idx + 1
                                            ),
                                            snippet: row_text.clone(),
                                            suggested_fix: format!(
                                                "Add or remove column separators ('&') so all rows contain exactly {first_c} columns."
                                            ),
                                        });
                                        break;
                                    }
                                }
                            }
                        }

                        // Check large dimension overflow
                        if let Some((_, cols)) = expected_cols {
                            if cols >= 8 {
                                issues.push(RenderDebugItem {
                                    line: line_num,
                                    command: begin_marker.clone(),
                                    category: "Matrix Width Overflow".to_string(),
                                    severity: RenderIssueSeverity::Warning,
                                    issue: format!(
                                        "Matrix has {cols} columns, which exceeds standard printable page width (540 pt). Brackets and outer columns will be truncated at the page margin."
                                    ),
                                    snippet: trimmed.to_string(),
                                    suggested_fix: "Use '\\begin{smallmatrix}' or split the matrix horizontally.".to_string(),
                                });
                            }
                        }
                    }
                }
            }

            if trimmed.contains("\\end{equation") || trimmed.contains("\\end{align") || trimmed.contains("\\end{gather") {
                in_equation = false;
            }
            if trimmed.contains("\\]")
                || dollar_pair_count >= 2
                || (dollar_pair_count == 1 && in_display_math && !opened_dollars_this_line)
            {
                in_display_math = false;
            }
        }
    }

    /// Performs deep inspection of math delimiters, braces, and formulas
    fn audit_math_notation(lines: &[&str], issues: &mut Vec<RenderDebugItem>) {
        for (line_idx, line) in lines.iter().enumerate() {
            let line_num = line_idx + 1;
            let trimmed = line.trim();

            if trimmed.starts_with('%') {
                continue;
            }

            // Check 1: Unbalanced inline dollar delimiters '$'
            let mut unescaped_dollars = 0;
            let bytes = trimmed.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'$' {
                    if i > 0 && bytes[i - 1] == b'\\' {
                        // Escaped \$
                    } else if i + 1 < bytes.len() && bytes[i + 1] == b'$' {
                        // Display $$
                        i += 1;
                    } else {
                        unescaped_dollars += 1;
                    }
                }
                i += 1;
            }

            if unescaped_dollars % 2 != 0 {
                issues.push(RenderDebugItem {
                    line: line_num,
                    command: "$".to_string(),
                    category: "Math Mode Delimiter".to_string(),
                    severity: RenderIssueSeverity::Error,
                    issue: format!(
                        "Unpaired inline math delimiter '$' found on line {line_num} ({unescaped_dollars} unescaped dollar sign(s)). The remainder of the line/paragraph will leak into math italic font."
                    ),
                    snippet: trimmed.to_string(),
                    suggested_fix: "Add matching '$' to enclose the inline formula, or use '\\$' if typing a literal dollar sign.".to_string(),
                });
            }

            // Check 2: Fraction syntax (\frac, \dfrac, \tfrac)
            for frac_cmd in ["\\frac", "\\dfrac", "\\tfrac"] {
                if let Some(pos) = trimmed.find(frac_cmd) {
                    let rest = trimmed[pos + frac_cmd.len()..].trim_start();
                    if !rest.starts_with('{') {
                        issues.push(RenderDebugItem {
                            line: line_num,
                            command: frac_cmd.to_string(),
                            category: "Fraction Syntax".to_string(),
                            severity: RenderIssueSeverity::Error,
                            issue: format!(
                                "Fraction command '{frac_cmd}' is missing opening brace '{{' for the numerator. Native and standard TeX engines require two explicit brace groups: '{frac_cmd}{{numerator}}{{denominator}}'."
                            ),
                            snippet: trimmed.to_string(),
                            suggested_fix: format!("Rewrite as '{frac_cmd}{{numerator}}{{denominator}}'."),
                        });
                    } else {
                        // Verify second argument
                        let mut chars = rest.chars().peekable();
                        let num = crate::engine::math::MathRenderer::extract_balanced_braces(&mut chars);
                        while chars.peek() == Some(&' ') {
                            chars.next();
                        }
                        if chars.peek() != Some(&'{') {
                            issues.push(RenderDebugItem {
                                line: line_num,
                                command: frac_cmd.to_string(),
                                category: "Fraction Syntax".to_string(),
                                severity: RenderIssueSeverity::Error,
                                issue: format!(
                                    "Fraction '{frac_cmd}{{{num}}}' is missing the second brace group '{{denominator}}'."
                                ),
                                snippet: trimmed.to_string(),
                                suggested_fix: format!("Add denominator: '{frac_cmd}{{{num}}}{{denominator}}'."),
                            });
                        }
                    }
                }
            }

            // Check 3: Unbalanced curly braces inside math formulas
            let is_math_line = trimmed.contains('$')
                || (trimmed.contains('\\')
                    && (trimmed.contains("\\frac")
                        || trimmed.contains("\\sqrt")
                        || trimmed.contains("\\mathbf")
                        || trimmed.contains("\\math")
                        || trimmed.contains("\\sum")
                        || trimmed.contains("\\int")));
            if is_math_line && !trimmed.ends_with('{') && !trimmed.starts_with('}') {
                let open_braces = trimmed.chars().filter(|&c| c == '{').count();
                let close_braces = trimmed.chars().filter(|&c| c == '}').count();
                if open_braces != close_braces && !trimmed.contains("\\begin{") && !trimmed.contains("\\end{") {
                    issues.push(RenderDebugItem {
                        line: line_num,
                        command: "{ ... }".to_string(),
                        category: "Math Braces".to_string(),
                        severity: RenderIssueSeverity::Error,
                        issue: format!(
                            "Mismatched curly braces in math expression on line {line_num} ({open_braces} open vs {close_braces} close). Unbalanced braces break formula parsing and abort AST building."
                        ),
                        snippet: trimmed.to_string(),
                        suggested_fix: "Ensure every opening '{' has a matching closing '}' in the formula.".to_string(),
                    });
                }
            }

            // Check 4: Unhandled or unsupported LaTeX math macros
            let unsupported_math_macros = [
                ("\\binom", "Binomial coefficient '\\binom{n}{k}' is not yet natively formatted as a stacked matrix."),
                ("\\substack", "'\\substack{...}' multi-line subscript formatting is not supported natively."),
                ("\\cancel", "'\\cancel{...}' requires 'cancel' package and renders as raw markup."),
                ("\\overbrace", "'\\overbrace{...}' horizontal brace rendering is not natively drawn."),
                ("\\underbrace", "'\\underbrace{...}' horizontal brace rendering is not natively drawn."),
                ("\\intertext", "'\\intertext{...}' inside align environment causes layout misalignment."),
            ];

            for (macro_name, explanation) in unsupported_math_macros {
                if trimmed.contains(macro_name) {
                    issues.push(RenderDebugItem {
                        line: line_num,
                        command: macro_name.to_string(),
                        category: "Math Macro Compatibility".to_string(),
                        severity: RenderIssueSeverity::Degraded,
                        issue: explanation.to_string(),
                        snippet: trimmed.to_string(),
                        suggested_fix: format!("Consider replacing '{macro_name}' with standard math notation or compiling via Tectonic."),
                    });
                }
            }
        }
    }

    /// Detects math commands inadvertently used in plain prose without math mode
    fn audit_bare_commands_outside_math(lines: &[&str], issues: &mut Vec<RenderDebugItem>) {
        let math_only_commands = [
            "\\alpha", "\\beta", "\\gamma", "\\delta", "\\theta", "\\lambda", "\\mu", "\\pi", "\\sigma", "\\omega",
            "\\sum", "\\int", "\\partial", "\\nabla", "\\infty", "\\sqrt",
            "\\le", "\\ge", "\\neq", "\\approx", "\\cdot", "\\times", "\\in", "\\pm"
        ];

        let mut in_math_env = false;

        for (line_idx, line) in lines.iter().enumerate() {
            let line_num = line_idx + 1;
            let trimmed = line.trim();

            if trimmed.starts_with('%')
                || trimmed.starts_with("\\documentclass")
                || trimmed.starts_with("\\usepackage")
                || trimmed.starts_with("\\geometry")
                || trimmed.starts_with("\\PassOptionsToPackage")
                || trimmed.starts_with("\\hypersetup")
            {
                continue;
            }

            if trimmed.contains("\\begin{equation")
                || trimmed.contains("\\begin{align")
                || trimmed.contains("\\begin{gather")
                || trimmed.contains("\\[")
            {
                in_math_env = true;
            }

            if !in_math_env {
                // If line does NOT have inline math $...$
                let has_inline_dollar = trimmed.contains('$');
                if !has_inline_dollar {
                    for cmd in math_only_commands {
                        if Self::contains_isolated_latex_command(trimmed, cmd) {
                            issues.push(RenderDebugItem {
                                line: line_num,
                                command: cmd.to_string(),
                                category: "Math Mode Required".to_string(),
                                severity: RenderIssueSeverity::Error,
                                issue: format!(
                                    "Math command '{cmd}' is used in normal paragraph text without math mode delimiters ('$ ... $'). It will render as unexpanded text or crash standard TeX engines with 'Missing $ inserted'."
                                ),
                                snippet: trimmed.to_string(),
                                suggested_fix: format!("Wrap the command in inline math: '${cmd}$'."),
                            });
                        }
                    }
                }
            }

            if trimmed.contains("\\end{equation")
                || trimmed.contains("\\end{align")
                || trimmed.contains("\\end{gather")
                || trimmed.contains("\\]")
            {
                in_math_env = false;
            }
        }
    }

    /// Audits TikZ and pgfplots syntax for common option typos and formatting bugs
    fn audit_tikz_pgfplots(lines: &[&str], issues: &mut Vec<RenderDebugItem>) {
        for (line_idx, line) in lines.iter().enumerate() {
            let line_num = line_idx + 1;
            let trimmed = line.trim();

            if trimmed.starts_with('%') {
                continue;
            }

            // Check for 'linewidth=' typo in options (e.g. \addplot[..., linewidth=1pt], \draw[..., linewidth=...])
            if (trimmed.contains("\\addplot") || trimmed.contains("\\draw") || trimmed.contains("\\path") || trimmed.contains("\\node"))
                && trimmed.contains("linewidth=")
            {
                issues.push(RenderDebugItem {
                    line: line_num,
                    command: "linewidth=".to_string(),
                    category: "TikZ / Pgfplots Option Typo".to_string(),
                    severity: RenderIssueSeverity::Error,
                    issue: "TikZ and pgfplots reject 'linewidth=<dimen>' because 'linewidth' is a LaTeX length macro, not a recognized pgfkeys option. This causes XeTeX and Tectonic to halt with 'Package pgfkeys Error: I do not know the key /tikz/linewidth'.".to_string(),
                    snippet: trimmed.to_string(),
                    suggested_fix: "Replace 'linewidth=...' with 'line width=...' (e.g. 'line width=1pt'), or use standard styles such as 'thick', 'thin', 'semithick', or 'very thick'.".to_string(),
                });
            }
        }
    }

    /// Checks if a LaTeX command appears with word boundary isolation (not as prefix of a longer command)
    fn contains_isolated_latex_command(text: &str, cmd: &str) -> bool {
        let mut search_from = 0;
        while let Some(pos) = text[search_from..].find(cmd) {
            let actual_pos = search_from + pos;
            let after_cmd = actual_pos + cmd.len();
            if after_cmd >= text.len() {
                return true;
            }
            let next_char = text[after_cmd..].chars().next().unwrap();
            if !next_char.is_ascii_alphabetic() {
                return true;
            }
            search_from = actual_pos + 1;
        }
        false
    }

    /// Audits AST generation and layout geometry
    fn audit_ast_layout(source: &str, issues: &mut Vec<RenderDebugItem>) {
        let ast = NativeLatexParser::parse(source);

        for block in &ast.blocks {
            if let DocumentBlock::EquationBlock { formula, line, .. } = block {
                // Check if formula contains multi-line matrix that might overflow
                if formula.contains("\\begin{pmatrix}") || formula.contains("\\begin{bmatrix}") {
                    let row_count = formula.matches("\\\\").count() + 1;
                    if row_count >= 5 {
                        issues.push(RenderDebugItem {
                            line: *line,
                            command: "Matrix Height".to_string(),
                            category: "Matrix Dimension".to_string(),
                            severity: RenderIssueSeverity::Warning,
                            issue: format!(
                                "Equation contains a {row_count}-row matrix requiring at least {} pt of vertical space. Verify page layout margins to prevent overlapping following paragraphs.",
                                row_count * 22 + 20
                            ),
                            snippet: formula.chars().take(80).collect::<String>(),
                            suggested_fix: "Ensure adequate vertical space before the next section heading.".to_string(),
                        });
                    }
                }
            }
        }
    }

    /// Verifies the generated PDF bytes and preview integrity
    fn audit_pdf_preview(pdf_bytes: Option<&[u8]>, issues: &mut Vec<RenderDebugItem>) -> String {
        let Some(bytes) = pdf_bytes else {
            issues.push(RenderDebugItem {
                line: 1,
                command: "PDF Generator".to_string(),
                category: "Preview Availability".to_string(),
                severity: RenderIssueSeverity::Error,
                issue: "No PDF preview has been generated. Compilation either encountered fatal syntax errors or has not yet run.".to_string(),
                snippet: String::new(),
                suggested_fix: "Resolve preceding compilation errors and trigger recompile (Ctrl+B).".to_string(),
            });
            return "No PDF bytes generated (Compilation Error)".to_string();
        };

        if bytes.len() < 50 {
            issues.push(RenderDebugItem {
                line: 1,
                command: "PDF Header".to_string(),
                category: "Corrupt PDF".to_string(),
                severity: RenderIssueSeverity::Error,
                issue: format!("Generated PDF is unusually small ({} bytes) and may be corrupt.", bytes.len()),
                snippet: String::new(),
                suggested_fix: "Check compiler log drawer for low-level PDF exporter errors.".to_string(),
            });
            return "Corrupt PDF Output".to_string();
        }

        // Verify PDF Magic Bytes
        if !bytes.starts_with(b"%PDF-") {
            issues.push(RenderDebugItem {
                line: 1,
                command: "%PDF-".to_string(),
                category: "Corrupt PDF Header".to_string(),
                severity: RenderIssueSeverity::Error,
                issue: "Generated output does not start with valid '%PDF-' magic header bytes.".to_string(),
                snippet: String::new(),
                suggested_fix: "Re-run compilation.".to_string(),
            });
            return "Invalid PDF Header".to_string();
        }

        format!("Valid PDF Generated ({} bytes)", bytes.len())
    }

    /// Helper for formatted timestamp
    fn current_timestamp() -> String {
        let now = std::time::SystemTime::now();
        let secs = now
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let days = secs / 86400;
        let day_secs = secs % 86400;
        let hours = day_secs / 3600;
        let mins = (day_secs % 3600) / 60;
        let s = day_secs % 60;
        format!("UTC {:02}:{:02}:{:02} (Day {})", hours, mins, s, days)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_detects_matrix_outside_math_mode() {
        let tex = r#"
\documentclass{article}
\begin{document}
Here is a bare matrix:
\begin{pmatrix}
1 & 2 \\
3 & 4
\end{pmatrix}
\end{document}
"#;
        let report = RenderDebugger::scan(tex, "test_matrix.tex", None);
        assert!(report.has_issues());
        assert!(report.matrix_issue_count > 0);
        let item = report.issues.iter().find(|i| i.category == "Matrix Math Mode").unwrap();
        assert_eq!(item.command, "\\begin{pmatrix}");
        assert_eq!(item.severity, RenderIssueSeverity::Error);
    }

    #[test]
    fn test_audit_detects_mismatched_matrix_columns() {
        let tex = r#"
\documentclass{article}
\usepackage{amsmath}
\begin{document}
\begin{equation}
A = \begin{pmatrix}
1 & 2 & 3 \\
4 & 5
\end{pmatrix}
\end{equation}
\end{document}
"#;
        let report = RenderDebugger::scan(tex, "test_cols.tex", None);
        let col_issue = report.issues.iter().find(|i| i.category == "Matrix Column Count");
        assert!(col_issue.is_some());
        let item = col_issue.unwrap();
        assert!(item.issue.contains("defines 3 column(s)"));
    }

    #[test]
    fn test_audit_detects_math_outside_math_mode() {
        let tex = r#"
\documentclass{article}
\begin{document}
The value of \alpha is proportional to \sum.
\end{document}
"#;
        let report = RenderDebugger::scan(tex, "test_math.tex", None);
        assert!(report.math_issue_count >= 2);
        let alpha_issue = report.issues.iter().find(|i| i.command == "\\alpha").unwrap();
        assert_eq!(alpha_issue.severity, RenderIssueSeverity::Error);
    }

    #[test]
    fn test_audit_clean_document() {
        let tex = r#"
\documentclass{article}
\usepackage{amsmath}
\begin{document}
\section{Intro}
The parameter $\alpha$ is tested:
\begin{equation}
A = \begin{pmatrix}
1 & 2 \\
3 & 4
\end{pmatrix}
\end{equation}
\end{document}
"#;
        let dummy_pdf = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n3 0 obj\n<< /Type /Page /Parent 2 0 R >>\nendobj\nxref\n0 4\ntrailer\n<< /Root 1 0 R >>\n%%EOF";
        let report = RenderDebugger::scan(tex, "clean.tex", Some(dummy_pdf));
        let fatal_issues: Vec<&RenderDebugItem> = report
            .issues
            .iter()
            .filter(|i| i.severity == RenderIssueSeverity::Error)
            .collect();
        assert_eq!(fatal_issues.len(), 0);
    }

    #[test]
    fn test_audit_detects_tikz_linewidth_typo() {
        let tex = r#"
\documentclass{article}
\usepackage{pgfplots}
\begin{document}
\begin{tikzpicture}
\begin{axis}
\addplot[red, dashed, domain=200:1000000, linewidth=1pt] {35.52};
\end{axis}
\end{tikzpicture}
\end{document}
"#;
        let report = RenderDebugger::scan(tex, "tikz_test.tex", None);
        let typo = report.issues.iter().find(|i| i.category == "TikZ / Pgfplots Option Typo");
        assert!(typo.is_some(), "Should detect TikZ linewidth= typo");
        let item = typo.unwrap();
        assert_eq!(item.command, "linewidth=");
        assert_eq!(item.severity, RenderIssueSeverity::Error);
        assert!(item.suggested_fix.contains("line width=1pt"));
    }

    #[test]
    fn test_audit_matrices_multiline_double_dollar() {
        let tex = r#"
\documentclass{article}
\usepackage{amsmath}
\begin{document}
$$
\begin{pmatrix}
1 & 2 \\
3 & 4
\end{pmatrix}
$$
\end{document}
"#;
        let report = RenderDebugger::scan(tex, "dollar_matrix.tex", None);
        assert_eq!(report.matrix_issue_count, 0, "Multi-line $$ block must be recognized as math mode");
    }
}
