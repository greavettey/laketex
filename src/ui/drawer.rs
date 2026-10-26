//! # Console Drawer & Diagnostic Inspector
//!
//! Provides the bottom slide-up drawer for inspecting compiler diagnostics, stdout/stderr
//! logs, contextual error summaries, headless API status, and rendering audits.
//!
//! Includes buttons for copying markdown error reports, exporting full diagnostic logs,
//! and running deep render audits on math and matrix commands.

use crate::compiler::error_report::ErrorReport;
use crate::compiler::{Diagnostic, DiagnosticSeverity, EngineKind};
use crate::document::packages::{PackageDependency, PackageManager};
use crate::engine::render_debugger::{RenderDebugReport, RenderDebugger};
use egui::{Color32, RichText, ScrollArea, Ui};
use std::collections::VecDeque;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum number of audit log entries kept in memory during a session.
pub const MAX_AUDIT_LOG_ENTRIES: usize = 2000;

/// Origin category of an audit log event, determining badge label and row color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditActor {
    /// Significant change or action performed by the human user in the GUI.
    User,
    /// Stateless or read-only API / headless request (`GET /api/document`, `POST /api/check`, etc.).
    ApiRead,
    /// Stateful API mutation or navigation that modifies the live GUI (`PATCH /api/document`, `POST /api/navigate`, etc.).
    ApiMutation,
}

impl AuditActor {
    /// Short badge label displayed in the Audit Log column.
    pub fn badge_label(self) -> &'static str {
        match self {
            Self::User => "👤 USER",
            Self::ApiMutation => "⚡ API EDIT",
            Self::ApiRead => "📡 API READ",
        }
    }

    /// Badge color for dark or light mode.
    pub fn badge_color(self, is_dark: bool) -> Color32 {
        match self {
            Self::User => {
                if is_dark {
                    Color32::from_rgb(97, 175, 239) // Cornflower blue
                } else {
                    Color32::from_rgb(20, 100, 190)
                }
            }
            Self::ApiMutation => {
                if is_dark {
                    Color32::from_rgb(255, 184, 108) // Vibrant warm amber/orange
                } else {
                    Color32::from_rgb(195, 85, 0)
                }
            }
            Self::ApiRead => {
                if is_dark {
                    Color32::from_rgb(86, 182, 194) // Bright cyan/teal
                } else {
                    Color32::from_rgb(0, 125, 145)
                }
            }
        }
    }

    /// Primary text color for the event description to ensure instant visual distinction between User and API events.
    pub fn text_color(self, is_dark: bool) -> Color32 {
        match self {
            Self::User => {
                if is_dark {
                    Color32::from_rgb(220, 223, 228) // Crisp neutral off-white
                } else {
                    Color32::from_rgb(30, 35, 45)
                }
            }
            Self::ApiMutation => {
                if is_dark {
                    Color32::from_rgb(241, 205, 255) // High-contrast lavender/violet tint for API edits
                } else {
                    Color32::from_rgb(125, 20, 150)
                }
            }
            Self::ApiRead => {
                if is_dark {
                    Color32::from_rgb(165, 225, 235) // Soft cyan tint for API reads
                } else {
                    Color32::from_rgb(10, 95, 115)
                }
            }
        }
    }

    /// Subtle background row fill color so API actions stand out from user actions at a glance.
    pub fn row_bg_color(self, is_dark: bool) -> Color32 {
        match self {
            Self::User => Color32::TRANSPARENT,
            Self::ApiMutation => {
                if is_dark {
                    Color32::from_rgba_unmultiplied(198, 120, 221, 26)
                } else {
                    Color32::from_rgba_unmultiplied(198, 120, 221, 22)
                }
            }
            Self::ApiRead => {
                if is_dark {
                    Color32::from_rgba_unmultiplied(86, 182, 194, 18)
                } else {
                    Color32::from_rgba_unmultiplied(86, 182, 194, 16)
                }
            }
        }
    }
}

/// Filter mode for the Console Drawer Audit Log tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditFilter {
    /// Show all events (User + API Mutations + API Reads)
    All,
    /// Show only significant user changes
    UserOnly,
    /// Show only API / headless mutations
    ApiMutationsOnly,
    /// Show only API / headless read & lint calls
    ApiReadsOnly,
}

/// Single in-memory session audit log entry (never written to disk or retained on close).
#[derive(Debug, Clone)]
pub struct AuditLogEntry {
    /// Monotonically increasing 1-based sequence ID for this session.
    pub id: u64,
    /// Wall-clock timestamp formatted as `HH:MM:SS.mmm`.
    pub timestamp: String,
    /// Origin actor category for color-coding.
    pub actor: AuditActor,
    /// Short action or endpoint name (e.g. `"File Saved"`, `"PATCH /api/document"`).
    pub action: String,
    /// Human-readable description of the change or request outcome.
    pub details: String,
    /// Optional 1-based editor line number associated with this event for one-click jump.
    pub target_line: Option<usize>,
}

/// Formats the current system time as `HH:MM:SS.mmm` (UTC wall clock) without external dependencies.
pub fn format_audit_timestamp() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = dur.as_secs();
    let millis = dur.subsec_millis();
    let hours = (total_secs / 3600) % 24;
    let mins = (total_secs / 60) % 60;
    let secs = total_secs % 60;
    format!("{hours:02}:{mins:02}:{secs:02}.{millis:03}")
}

/// Lock-free-guarded cross-thread queue for pushing background API / headless audit events into the GUI drawer.
pub struct SharedAuditQueue {
    has_pending: AtomicBool,
    pending: Mutex<VecDeque<(AuditActor, String, String, Option<usize>, String)>>,
    egui_ctx: Mutex<Option<egui::Context>>,
}

static GLOBAL_AUDIT_QUEUE: LazyLock<SharedAuditQueue> = LazyLock::new(|| SharedAuditQueue {
    has_pending: AtomicBool::new(false),
    pending: Mutex::new(VecDeque::new()),
    egui_ctx: Mutex::new(None),
});

/// Attaches the active `egui::Context` so background API audit events immediately wake the GUI window.
pub fn register_audit_egui_context(ctx: egui::Context) {
    if let Ok(mut guard) = GLOBAL_AUDIT_QUEUE.egui_ctx.lock() {
        *guard = Some(ctx);
    }
}

/// Records an API or headless operation into the in-memory session audit queue and wakes the GUI if attached.
pub fn record_api_audit(
    actor: AuditActor,
    action: impl Into<String>,
    details: impl Into<String>,
    target_line: Option<usize>,
) {
    let ts = format_audit_timestamp();
    if let Ok(mut guard) = GLOBAL_AUDIT_QUEUE.pending.lock() {
        if guard.len() >= MAX_AUDIT_LOG_ENTRIES {
            guard.pop_front();
        }
        guard.push_back((actor, action.into(), details.into(), target_line, ts));
        GLOBAL_AUDIT_QUEUE.has_pending.store(true, Ordering::Release);
    }
    if let Ok(guard) = GLOBAL_AUDIT_QUEUE.egui_ctx.lock() {
        if let Some(ref ctx) = *guard {
            ctx.request_repaint();
        }
    }
}

/// Records a significant user-initiated operation into the in-memory session audit queue.
pub fn record_user_audit(
    action: impl Into<String>,
    details: impl Into<String>,
    target_line: Option<usize>,
) {
    record_api_audit(AuditActor::User, action, details, target_line);
}

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
    /// Ephemeral in-memory session audit log of significant user changes and all API/headless operations
    AuditLog,
}

/// Bottom console drawer panel widget.
pub struct Drawer {
    /// Active tab in the drawer
    pub current_tab: DrawerTab,
    /// Ephemeral notification banner message and display timestamp
    pub status_feedback: Option<(String, std::time::Instant)>,
    /// Cached key `(source_hash, doc_name, pdf_len)` for the render audit report
    cached_audit_key: Option<(u64, String, usize)>,
    /// Cached render audit report to avoid re-scanning on every frame
    cached_audit_report: Option<RenderDebugReport>,
    /// Ephemeral in-memory ring buffer of session audit events (never persisted to disk)
    pub audit_entries: VecDeque<AuditLogEntry>,
    /// Next sequential 1-based ID for audit log entries
    next_audit_id: u64,
    /// Active actor filter in the Audit Log tab
    pub audit_filter: AuditFilter,
    /// Search query filter in the Audit Log tab
    pub audit_search: String,
    /// Whether the Audit Log scroll area automatically sticks to the newest entry
    pub audit_auto_scroll: bool,
}

impl Default for Drawer {
    fn default() -> Self {
        Self {
            current_tab: DrawerTab::Diagnostics,
            status_feedback: None,
            cached_audit_key: None,
            cached_audit_report: None,
            audit_entries: VecDeque::with_capacity(128),
            next_audit_id: 1,
            audit_filter: AuditFilter::All,
            audit_search: String::new(),
            audit_auto_scroll: true,
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
    /// Prompt user consent to install missing external tool dependency
    PromptInstallDependency(PackageDependency),
}

impl Drawer {
    /// Creates a new console drawer
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a new in-memory audit event to the bounded session ring buffer.
    pub fn log_event(
        &mut self,
        actor: AuditActor,
        action: impl Into<String>,
        details: impl Into<String>,
        target_line: Option<usize>,
    ) {
        self.log_event_with_timestamp(
            actor,
            action.into(),
            details.into(),
            target_line,
            format_audit_timestamp(),
        );
    }

    /// Appends a significant user-initiated event to the in-memory session audit log.
    pub fn log_user_event(
        &mut self,
        action: impl Into<String>,
        details: impl Into<String>,
        target_line: Option<usize>,
    ) {
        self.log_event(AuditActor::User, action, details, target_line);
    }

    fn log_event_with_timestamp(
        &mut self,
        actor: AuditActor,
        action: String,
        details: String,
        target_line: Option<usize>,
        timestamp: String,
    ) {
        if self.audit_entries.len() >= MAX_AUDIT_LOG_ENTRIES {
            self.audit_entries.pop_front();
        }
        let id = self.next_audit_id;
        self.next_audit_id += 1;
        self.audit_entries.push_back(AuditLogEntry {
            id,
            timestamp,
            actor,
            action,
            details,
            target_line,
        });
    }

    /// Lock-free poll (`~1 ns` when idle) that drains any pending API / headless audit events
    /// recorded on background server threads into the GUI drawer's in-memory ring buffer.
    pub fn poll_api_audit_events(&mut self) {
        if !GLOBAL_AUDIT_QUEUE.has_pending.load(Ordering::Relaxed) {
            return;
        }
        let drained: Vec<_> = if let Ok(mut guard) = GLOBAL_AUDIT_QUEUE.pending.lock() {
            GLOBAL_AUDIT_QUEUE.has_pending.store(false, Ordering::Release);
            guard.drain(..).collect()
        } else {
            return;
        };
        for (actor, act, det, line, ts) in drained {
            self.log_event_with_timestamp(actor, act, det, line, ts);
        }
    }

    /// Renders the bottom expandable console drawer displaying compiler diagnostics,
    /// contextual error reports, raw TeX logs, and the intelligent render audit debugger.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        document_source: &str,
        document_name: &str,
        engine: EngineKind,
        diagnostics: &[Diagnostic],
        raw_stdout: &str,
        raw_stderr: &str,
        api_port: Option<u16>,
        api_stateful: bool,
        pdf_bytes: Option<&[u8]>,
    ) -> DrawerAction {
        let mut action = DrawerAction::None;
        self.poll_api_audit_events();

        let error_count = diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .count();
        let warn_count = diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .count();

        // Perform cached render audit scan only when document source or PDF output changes
        let mut hasher = DefaultHasher::new();
        document_source.hash(&mut hasher);
        let audit_key = (
            hasher.finish(),
            document_name.to_string(),
            pdf_bytes.map_or(0, |b| b.len()),
        );
        if self.cached_audit_key.as_ref() != Some(&audit_key) || self.cached_audit_report.is_none() {
            self.cached_audit_report = Some(RenderDebugger::scan(
                document_source,
                document_name,
                pdf_bytes,
            ));
            self.cached_audit_key = Some(audit_key);
        }
        let audit_report = self.cached_audit_report.as_ref().unwrap();

        // Clear expired feedback banner after 4 seconds
        if let Some((_, time)) = &self.status_feedback {
            if time.elapsed().as_secs() >= 4 {
                self.status_feedback = None;
            }
        }

        let api_event_count = self
            .audit_entries
            .iter()
            .filter(|e| matches!(e.actor, AuditActor::ApiMutation | AuditActor::ApiRead))
            .count();
        let total_audit_count = self.audit_entries.len();

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let diag_label = if error_count > 0 {
                    format!("Diagnostics ({error_count} err, {warn_count} warn)")
                } else {
                    format!("Diagnostics ({warn_count} warn)")
                };

                let audit_label = if audit_report.total_issues > 0 {
                    format!("Render Audit ({} issues)", audit_report.total_issues)
                } else {
                    "Render Audit".to_string()
                };

                let audit_log_label = if api_event_count > 0 {
                    format!("Audit Log ({total_audit_count} • {api_event_count} API)")
                } else {
                    format!("Audit Log ({total_audit_count})")
                };

                ui.selectable_value(&mut self.current_tab, DrawerTab::Diagnostics, diag_label);
                ui.selectable_value(&mut self.current_tab, DrawerTab::RenderAudit, audit_label);
                ui.selectable_value(&mut self.current_tab, DrawerTab::Logs, "Build Logs");
                ui.selectable_value(&mut self.current_tab, DrawerTab::Api, "Headless API");
                ui.selectable_value(&mut self.current_tab, DrawerTab::AuditLog, audit_log_label);

                // Right-aligned quick export actions
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((msg, _)) = &self.status_feedback {
                        ui.colored_label(Color32::from_rgb(152, 195, 121), msg);
                    }

                    if self.current_tab == DrawerTab::AuditLog {
                        if !self.audit_entries.is_empty() && ui.button("🗑 Clear").on_hover_text("Clear in-memory audit log").clicked() {
                            self.audit_entries.clear();
                        }
                        ui.checkbox(&mut self.audit_auto_scroll, "Auto-scroll");
                    } else if self.current_tab == DrawerTab::RenderAudit {
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

            if self.current_tab == DrawerTab::AuditLog {
                Self::show_audit_log_tab(
                    &self.audit_entries,
                    &mut self.audit_filter,
                    &mut self.audit_search,
                    self.audit_auto_scroll,
                    ui,
                    &mut action,
                );
                return;
            }

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

                                    if let Some(ref f) = diag.file {
                                        ui.colored_label(Color32::from_gray(160), format!("[{f}]"));
                                    }

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

                                if diag.message.contains("minted") || diag.message.contains("pygmentize") || diag.message.contains("Pygments") {
                                    let known = PackageManager::get_known_dependencies();
                                    if let Some(dep) = known.iter().find(|d| d.latex_package == "minted") {
                                        if !dep.installed {
                                            ui.indent("quick_fix_indent", |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.colored_label(Color32::from_rgb(229, 192, 123), "💡 Quick Fix:");
                                                    if ui.button("📦 Install Pygments via pip (Consent Required)...").clicked() {
                                                        action = DrawerAction::PromptInstallDependency(dep.clone());
                                                    }
                                                });
                                            });
                                        }
                                    }
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
                        let example_port = api_port.unwrap_or(3030);
                        if let Some(port) = api_port {
                            let mode_badge = if api_stateful {
                                "Stateful (Live Editor Sync enabled via --with-state)"
                            } else {
                                "Stateless (launch with --with-api --with-state for live editor sync)"
                            };
                            ui.colored_label(
                                Color32::from_rgb(152, 195, 121),
                                format!("● Active on http://127.0.0.1:{port} — {mode_badge}"),
                            );
                        } else {
                            ui.colored_label(
                                Color32::from_gray(140),
                                "○ Disabled by default (launch with `--with-api` or `--with-api --with-state` to enable)",
                            );
                        }
                        ui.add_space(6.0);

                        ui.label(RichText::new("Available Endpoints:").strong());
                        ui.label("• GET  /api/spec        - Machine-readable AI agent API specification (CLI: `laketex api-spec`)");
                        ui.label("• GET  /api/help        - Quick endpoint directory and CLI command overview");
                        ui.label("• GET  /health          - Health, engine status, and stateful mode flag");
                        ui.label("• POST /api/compile     - Compile LaTeX source (in --with-state mode: updates live editor & preview)");
                        ui.label("• GET  /api/document    - [--with-state] Read active editor source, path, dirty flag & diagnostics");
                        ui.label("• POST /api/document    - [--with-state] Replace or patch active editor buffer (optional save & compile)");
                        ui.label("• POST /api/navigate    - [--with-state] Jump editor to line, switch page/view mode, or toggle panels");
                        ui.label("• POST /api/debug       - Audit source code and PDF preview for rendering issues");
                        ui.label("• POST /api/outline     - Parse sections & outline tree as JSON");
                        ui.label("• POST /api/packages    - Analyze required packages and CTAN status");
                        ui.label("• POST /api/check       - Lint and check syntax errors");

                        ui.add_space(8.0);
                        ui.label(RichText::new("Example debug scan invocation:").strong());
                        ui.monospace(format!(
                            "curl -X POST http://127.0.0.1:{example_port}/api/debug \\\n  -H \"Content-Type: application/json\" \\\n  -d '{{\"source\": \"\\\\begin{{pmatrix}} 1 & 2 \\\\\\\\ 3 & 4 \\\\end{{pmatrix}}\"}}'"
                        ));
                    }
                    DrawerTab::AuditLog => {}
                });
        });

        action
    }

    /// Renders the virtualized, color-coded in-memory Audit Log tab.
    fn show_audit_log_tab(
        audit_entries: &std::collections::VecDeque<AuditLogEntry>,
        audit_filter: &mut AuditFilter,
        audit_search: &mut String,
        audit_auto_scroll: bool,
        ui: &mut Ui,
        action: &mut DrawerAction,
    ) {
        let is_dark = ui.visuals().dark_mode;

        let user_count = audit_entries
            .iter()
            .filter(|e| e.actor == AuditActor::User)
            .count();
        let api_mut_count = audit_entries
            .iter()
            .filter(|e| e.actor == AuditActor::ApiMutation)
            .count();
        let api_read_count = audit_entries
            .iter()
            .filter(|e| e.actor == AuditActor::ApiRead)
            .count();
        let total = audit_entries.len();

        ui.horizontal(|ui| {
            ui.selectable_value(
                audit_filter,
                AuditFilter::All,
                format!("All ({total})"),
            );
            ui.selectable_value(
                audit_filter,
                AuditFilter::UserOnly,
                RichText::new(format!("User ({user_count})"))
                    .color(AuditActor::User.badge_color(is_dark)),
            );
            ui.selectable_value(
                audit_filter,
                AuditFilter::ApiMutationsOnly,
                RichText::new(format!("API Edits ({api_mut_count})"))
                    .color(AuditActor::ApiMutation.badge_color(is_dark)),
            );
            ui.selectable_value(
                audit_filter,
                AuditFilter::ApiReadsOnly,
                RichText::new(format!("API Reads ({api_read_count})"))
                    .color(AuditActor::ApiRead.badge_color(is_dark)),
            );

            ui.separator();
            ui.add(
                egui::TextEdit::singleline(audit_search)
                    .hint_text("Filter audit log...")
                    .desired_width(170.0),
            );
            if !audit_search.is_empty() && ui.small_button("✕").clicked() {
                audit_search.clear();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.colored_label(
                    Color32::from_gray(130),
                    "Log is not saved to disk",
                );
            });
        });

        ui.add_space(2.0);
        ui.separator();

        let query_lower = audit_search.trim().to_ascii_lowercase();
        let filtered_indices: Vec<usize> = audit_entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                let actor_ok = match audit_filter {
                    AuditFilter::All => true,
                    AuditFilter::UserOnly => entry.actor == AuditActor::User,
                    AuditFilter::ApiMutationsOnly => entry.actor == AuditActor::ApiMutation,
                    AuditFilter::ApiReadsOnly => entry.actor == AuditActor::ApiRead,
                };
                if !actor_ok {
                    return false;
                }
                if query_lower.is_empty() {
                    return true;
                }
                entry.action.to_ascii_lowercase().contains(&query_lower)
                    || entry.details.to_ascii_lowercase().contains(&query_lower)
            })
            .map(|(i, _)| i)
            .collect();

        if filtered_indices.is_empty() {
            ui.add_space(6.0);
            ui.colored_label(
                Color32::from_gray(135),
                "No matching audit events recorded in this session yet.",
            );
            return;
        }

        let row_height = 22.0;
        ScrollArea::vertical()
            .id_salt("audit_log_scroll")
            .auto_shrink([false, false])
            .stick_to_bottom(audit_auto_scroll)
            .show_rows(ui, row_height, filtered_indices.len(), |ui, row_range| {
                for filtered_pos in row_range {
                    let entry_idx = filtered_indices[filtered_pos];
                    let Some(entry) = audit_entries.get(entry_idx) else {
                        continue;
                    };

                    let row_bg = entry.actor.row_bg_color(is_dark);
                    let badge_color = entry.actor.badge_color(is_dark);
                    let text_color = entry.actor.text_color(is_dark);

                    egui::Frame::NONE
                        .fill(row_bg)
                        .inner_margin(egui::Margin::symmetric(4, 1))
                        .corner_radius(3.0)
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                ui.colored_label(
                                    Color32::from_gray(125),
                                    RichText::new(format!("#{:03} {}", entry.id, entry.timestamp))
                                        .monospace()
                                        .small(),
                                );
                                ui.label(
                                    RichText::new(format!("[{}]", entry.actor.badge_label()))
                                        .monospace()
                                        .small()
                                        .strong()
                                        .color(badge_color),
                                );
                                ui.label(
                                    RichText::new(&entry.action)
                                        .monospace()
                                        .strong()
                                        .color(badge_color),
                                );
                                if let Some(line) = entry.target_line {
                                    if ui
                                        .small_button(format!("Line {line}"))
                                        .on_hover_text("Click to jump editor caret to this line")
                                        .clicked()
                                    {
                                        *action = DrawerAction::JumpToLine(line);
                                    }
                                }
                                ui.label(RichText::new(&entry.details).color(text_color));
                            });
                        });
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_log_actors_and_bounded_ring_buffer() {
        let mut drawer = Drawer::new();
        drawer.log_user_event("File Saved", "Saved 'paper.tex'", Some(1));
        drawer.log_event(
            AuditActor::ApiMutation,
            "PATCH /api/document",
            "Applied 1 surgical edit op(s)",
            Some(42),
        );
        drawer.log_event(
            AuditActor::ApiRead,
            "GET /api/document",
            "Read live document snapshot",
            None,
        );

        assert_eq!(drawer.audit_entries.len(), 3);
        assert_eq!(drawer.audit_entries[0].actor, AuditActor::User);
        assert_eq!(drawer.audit_entries[1].actor, AuditActor::ApiMutation);
        assert_eq!(drawer.audit_entries[1].target_line, Some(42));
        assert_eq!(drawer.audit_entries[2].actor, AuditActor::ApiRead);

        // Verify distinct colors between User, ApiMutation, and ApiRead in both dark and light mode
        for is_dark in [true, false] {
            assert_ne!(
                AuditActor::User.badge_color(is_dark),
                AuditActor::ApiMutation.badge_color(is_dark)
            );
            assert_ne!(
                AuditActor::User.text_color(is_dark),
                AuditActor::ApiMutation.text_color(is_dark)
            );
            assert_ne!(
                AuditActor::ApiMutation.badge_color(is_dark),
                AuditActor::ApiRead.badge_color(is_dark)
            );
        }

        // Verify ring buffer cap
        for i in 0..(MAX_AUDIT_LOG_ENTRIES + 25) {
            drawer.log_user_event("Edit", format!("Entry {i}"), None);
        }
        assert_eq!(drawer.audit_entries.len(), MAX_AUDIT_LOG_ENTRIES);
    }

    #[test]
    fn test_cross_thread_api_audit_queue_draining() {
        let mut drawer = Drawer::new();
        // Clear any leftover global queue items from parallel tests
        drawer.poll_api_audit_events();
        drawer.audit_entries.clear();

        record_api_audit(
            AuditActor::ApiMutation,
            "POST /api/navigate",
            "Navigated GUI (line=15)",
            Some(15),
        );
        drawer.poll_api_audit_events();

        assert!(
            drawer
                .audit_entries
                .iter()
                .any(|e| e.actor == AuditActor::ApiMutation
                    && e.action == "POST /api/navigate"
                    && e.target_line == Some(15))
        );
    }
}
