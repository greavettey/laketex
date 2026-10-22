//! # Console Drawer & Diagnostic Inspector
//!
//! Provides the bottom slide-up drawer for inspecting compiler diagnostics, stdout/stderr
//! logs, contextual error summaries, headless API status, and rendering audits.
//!
//! Includes buttons for copying markdown error reports, exporting full diagnostic logs,
//! and running deep render audits on math and matrix commands.

use crate::compiler::error_report::ErrorReport;
use crate::compiler::{Diagnostic, DiagnosticSeverity, EngineKind};
use crate::engine::render_debugger::RenderDebugger;
use egui::{Color32, RichText, ScrollArea, Ui};

/// Active view tab in the console drawer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawerTab {
    /// Structured warnings and errors with line jump targets
    Diagnostics,
    /// Rendering audit identifying defective matrices, math commands, and layout flaws
    RenderAudit,
    /// Raw standard output and error compiler streams
    Logs,
    /// Headless REST API endpoint documentation and server status
    Api,
}

/// Bottom console drawer panel widget.
pub struct Drawer {
    /// Active tab in the drawer
    pub current_tab: DrawerTab,
    /// Ephemeral notification banner message and display timestamp
    pub status_feedback: Option<(String, std::time::Instant)>,
}

impl Default for Drawer {
    fn default() -> Self {
        Self {
            current_tab: DrawerTab::Diagnostics,
            status_feedback: None,
        }
    }
}

/// Action triggered from inside the console drawer.
pub enum DrawerAction {
    /// No action
    None,
    /// Jump editor caret to the given line
    JumpToLine(usize),
    /// Open file dialog to export error report
    ExportErrorReport,
    /// Open file dialog to export render audit report
    ExportAuditReport,
}

impl Drawer {
    /// Creates a new console drawer
    pub fn new() -> Self {
        Self::default()
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        document_source: &str,
        document_name: &str,
        engine: EngineKind,
        diagnostics: &[Diagnostic],
        raw_stdout: &str,
        raw_stderr: &str,
        api_port: u16,
        pdf_bytes: Option<&[u8]>,
    ) -> DrawerAction {
        let mut action = DrawerAction::None;

        let error_count = diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .count();
        let warn_count = diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .count();

        // Perform fast render audit scan
        let audit_report = RenderDebugger::scan(document_source, document_name, pdf_bytes);

        // Clear expired feedback banner after 4 seconds
        if let Some((_, time)) = &self.status_feedback {
            if time.elapsed().as_secs() >= 4 {
                self.status_feedback = None;
            }
        }

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let diag_label = if error_count > 0 {
                    format!("Diagnostics ({error_count} err, {warn_count} warn)")
                } else {
                    format!("Diagnostics ({warn_count} warn)")
                };

                let audit_label = if audit_report.total_issues > 0 {
                    format!("🔍 Render Audit ({} issues)", audit_report.total_issues)
                } else {
                    "🔍 Render Audit".to_string()
                };

                ui.selectable_value(&mut self.current_tab, DrawerTab::Diagnostics, diag_label);
                ui.selectable_value(&mut self.current_tab, DrawerTab::RenderAudit, audit_label);
                ui.selectable_value(&mut self.current_tab, DrawerTab::Logs, "Build Logs");
                ui.selectable_value(&mut self.current_tab, DrawerTab::Api, "Headless API");

                // Right-aligned quick export actions
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((msg, _)) = &self.status_feedback {
                        ui.colored_label(Color32::from_rgb(152, 195, 121), msg);
                    }

                    if self.current_tab == DrawerTab::RenderAudit {
                        if audit_report.has_issues() {
                            if ui.button("⬇ Export Audit...").clicked() {
                                action = DrawerAction::ExportAuditReport;
                            }
                            if ui.button("📋 Copy Audit Report").clicked() {
                                let md = audit_report.to_markdown();
                                ui.ctx().copy_text(md);
                                self.status_feedback = Some((
                                    "✓ Copied render audit report to clipboard!".to_string(),
                                    std::time::Instant::now(),
                                ));
                            }
                        }
                    } else if error_count > 0 || warn_count > 0 || !raw_stderr.trim().is_empty() {
                        if ui.button("⬇ Export Error Log...").clicked() {
                            action = DrawerAction::ExportErrorReport;
                        }

                        if ui.button("📋 Copy Report").clicked() {
                            let report = ErrorReport::generate(
                                document_source,
                                document_name,
                                engine,
                                diagnostics,
                                raw_stdout,
                                raw_stderr,
                            );
                            let md = report.to_markdown();
                            ui.ctx().copy_text(md);
                            self.status_feedback = Some((
                                "✓ Copied full contextual error report to clipboard!".to_string(),
                                std::time::Instant::now(),
                            ));
                        }
                    }
                });
            });

            ui.separator();

            ScrollArea::vertical()
                .id_salt("drawer_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| match self.current_tab {
                    DrawerTab::Diagnostics => {
                        if diagnostics.is_empty() {
                            ui.colored_label(
                                Color32::from_rgb(152, 195, 121),
                                "✓ No errors or warnings detected. Document compiles cleanly!",
                            );
                        } else {
                            let doc_lines: Vec<&str> = document_source.lines().collect();

                            for diag in diagnostics {
                                ui.horizontal(|ui| {
                                    let (r, g, b) = diag.severity.badge_color();
                                    ui.colored_label(
                                        Color32::from_rgb(r, g, b),
                                        format!("[{}]", diag.severity.label()),
                                    );

                                    if let Some(line) = diag.line {
                                        if ui.button(format!("Line {line}")).clicked() {
                                            action = DrawerAction::JumpToLine(line);
                                        }
                                    } else {
                                        ui.colored_label(Color32::from_gray(120), "Line ?");
                                    }

                                    ui.label(RichText::new(&diag.message).strong());
                                });

                                // Code snippet context around the error line
                                if let Some(target_line) = diag.line {
                                    if target_line > 0 && target_line <= doc_lines.len() {
                                        let target_idx = target_line - 1;
                                        let start = target_idx.saturating_sub(1);
                                        let end = (target_idx + 2).min(doc_lines.len());

                                        ui.indent("code_snippet_indent", |ui| {
                                            for idx in start..end {
                                                let is_err = idx == target_idx;
                                                let num = idx + 1;
                                                let text = doc_lines[idx];
                                                let line_color = if is_err {
                                                    Color32::from_rgb(235, 87, 87)
                                                } else {
                                                    Color32::from_gray(140)
                                                };
                                                let marker = if is_err { ">>" } else { "  " };

                                                ui.colored_label(
                                                    line_color,
                                                    format!("{marker} {num:>4} | {text}"),
                                                );
                                            }
                                        });
                                    }
                                }

                                if let Some(details) = &diag.details {
                                    ui.indent("diag_details", |ui| {
                                        ui.colored_label(Color32::from_gray(130), details);
                                    });
                                }
                                ui.separator();
                            }
                        }
                    }
                    DrawerTab::RenderAudit => {
                        if audit_report.issues.is_empty() {
                            ui.colored_label(
                                Color32::from_rgb(152, 195, 121),
                                "✓ No rendering flaws detected. All matrices, formulas, and commands conform to supported rendering rules!",
                            );
                        } else {
                            ui.horizontal(|ui| {
                                ui.colored_label(
                                    Color32::from_rgb(242, 201, 76),
                                    format!(
                                        "Identified {} issue(s): {} matrix issues, {} math notation issues.",
                                        audit_report.total_issues,
                                        audit_report.matrix_issue_count,
                                        audit_report.math_issue_count
                                    ),
                                );
                            });
                            ui.add_space(4.0);

                            for item in &audit_report.issues {
                                let (r, g, b) = item.severity.color();
                                ui.horizontal(|ui| {
                                    ui.colored_label(Color32::from_rgb(r, g, b), format!("[{}]", item.severity.as_str()));
                                    if ui.button(format!("Line {}", item.line)).clicked() {
                                        action = DrawerAction::JumpToLine(item.line);
                                    }
                                    ui.label(RichText::new(&item.command).monospace().strong());
                                    ui.colored_label(Color32::from_gray(140), format!("• {}", item.category));
                                });

                                ui.indent("audit_issue_indent", |ui| {
                                    ui.colored_label(Color32::from_rgb(220, 225, 235), format!("Why it isn't rendering: {}", item.issue));
                                    if !item.snippet.is_empty() {
                                        ui.colored_label(Color32::from_gray(140), format!("Snippet: {}", item.snippet));
                                    }
                                    ui.colored_label(Color32::from_rgb(97, 175, 239), format!("💡 Fix: {}", item.suggested_fix));
                                });
                                ui.separator();
                            }
                        }
                    }
                    DrawerTab::Logs => {
                        let combined = format!("{raw_stdout}\n{raw_stderr}");
                        if combined.trim().is_empty() {
                            ui.colored_label(Color32::from_gray(120), "No build log output yet.");
                        } else {
                            ui.monospace(&combined);
                        }
                    }
                    DrawerTab::Api => {
                        ui.label(RichText::new("Laketex Headless REST API Server").strong());
                        ui.colored_label(
                            Color32::from_rgb(152, 195, 121),
                            format!("● Active on http://127.0.0.1:{api_port}"),
                        );
                        ui.add_space(6.0);

                        ui.label(RichText::new("Available Endpoints:").strong());
                        ui.label("• GET  /health          - Health and engine status");
                        ui.label("• POST /api/compile     - Compile LaTeX source (returns Base64 PDF & diagnostics)");
                        ui.label("• POST /api/debug       - Audit source code and PDF preview for rendering issues");
                        ui.label("• POST /api/outline     - Parse sections & outline tree as JSON");
                        ui.label("• POST /api/packages    - Analyze required packages and CTAN status");
                        ui.label("• POST /api/check       - Lint and check syntax errors");

                        ui.add_space(8.0);
                        ui.label(RichText::new("Example debug scan invocation:").strong());
                        ui.monospace(format!(
                            "curl -X POST http://127.0.0.1:{api_port}/api/debug \\\n  -H \"Content-Type: application/json\" \\\n  -d '{{\"source\": \"\\\\begin{{pmatrix}} 1 & 2 \\\\\\\\ 3 & 4 \\\\end{{pmatrix}}\"}}'"
                        ));
                    }
                });
        });

        action
    }
}
