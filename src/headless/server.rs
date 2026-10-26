//! # Headless HTTP REST API Daemon
//!
//! Exposes an embedded, lightweight HTTP web server providing programmatic endpoints
//! for remote LaTeX compilation, syntax linting, outline extraction, and health checks.
//!
//! ## Endpoints
//! - `GET  /health` or `/api/status`: Probes engine availability and returns JSON status
//! - `POST /api/compile`: Compiles LaTeX code, returning base64 PDF bytes and error reports (updates live GUI editor in `--with-state` mode)
//! - `POST /api/check`: Fast syntax linting returning structured diagnostics
//! - `POST /api/outline`: Extracts document section hierarchy
//! - `POST /api/packages`: Detects imported packages
//! - `GET  /api/document`: Reads the active GUI editor buffer, file metadata, and diagnostics (`--with-state` mode)
//! - `POST /api/document`: Updates or patches the active GUI editor buffer, optionally saving or compiling (`--with-state` mode)
//! - `POST /api/navigate`: Moves the GUI editor caret, switches viewer pages/modes, or toggles panels (`--with-state` mode)

use crate::compiler::engine::{CompileOptions, CompilerEngine, EngineKind};
use crate::document::outline::OutlineParser;
use crate::document::packages::PackageManager;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use tiny_http::{Header, Response, Server};

/// Snapshot of the live GUI editor state exposed in `--with-state` mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveEditorSnapshot {
    /// Current LaTeX document source code in the editor buffer
    pub source: String,
    /// Display file name (e.g. `"main.tex"` or `"untitled.tex"`)
    pub file_name: String,
    /// Full path on disk if the document is associated with a file
    pub file_path: Option<PathBuf>,
    /// Active project working directory
    pub working_dir: PathBuf,
    /// Configured master document path for multi-file projects, if any
    pub master_document: Option<PathBuf>,
    /// Whether the editor buffer has unsaved modifications
    pub is_dirty: bool,
    /// Total line count
    pub line_count: usize,
    /// Total word count
    pub word_count: usize,
    /// Total character count
    pub char_count: usize,
    /// Active compilation engine backend
    pub engine: EngineKind,
    /// Active viewer display mode (`"NativeVector"` or `"PdfRaster"`)
    pub view_mode: String,
    /// Current 1-based page number in the viewer
    pub current_page: usize,
    /// Current viewer zoom factor (e.g. `1.0` = 100%)
    pub zoom: f32,
    /// Latest compiler diagnostics
    pub diagnostics: Vec<crate::compiler::Diagnostic>,
    /// Latest compilation duration in milliseconds
    pub last_duration_ms: Option<u128>,
    /// Monotonically increasing revision number incremented on each state update
    pub revision: u64,
}

impl Default for LiveEditorSnapshot {
    fn default() -> Self {
        let default_doc = crate::document::Document::default();
        Self {
            line_count: default_doc.line_count(),
            word_count: default_doc.word_count(),
            char_count: default_doc.char_count(),
            file_name: default_doc.file_name(),
            source: default_doc.content,
            file_path: None,
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            master_document: None,
            is_dirty: false,
            engine: EngineKind::Native,
            view_mode: "NativeVector".to_string(),
            current_page: 1,
            zoom: 1.0,
            diagnostics: Vec::new(),
            last_duration_ms: None,
            revision: 1,
        }
    }
}

/// Command queued from the REST API thread to be applied on the next GUI frame in `--with-state` mode.
#[derive(Debug, Clone)]
pub enum StatefulUiCommand {
    /// Replace the live editor document content (and optionally trigger compilation)
    SetDocumentContent {
        /// New full LaTeX source code
        source: String,
        /// Updated dirty status (`false` if saved to disk)
        is_dirty: bool,
        /// Whether to trigger compilation/layout refresh
        trigger_compile: bool,
        /// Status bar message to display in the GUI
        status_message: String,
    },
    /// Apply a completed compilation result directly to the GUI viewer & diagnostics drawer
    ApplyCompileResult {
        /// Optional new LaTeX source code that was compiled
        source: Option<String>,
        /// Optional updated dirty state
        is_dirty: Option<bool>,
        /// Engine used for compilation
        engine: EngineKind,
        /// Completed compilation output
        result: crate::compiler::CompileResult,
    },
    /// Switch the active compilation engine in the GUI
    SetEngine(EngineKind),
    /// Navigate or control the GUI editor and viewer panels
    Navigate {
        /// Optional 1-based line number to jump the editor caret to
        line: Option<usize>,
        /// Optional 1-based page number to jump the viewer to
        page: Option<usize>,
        /// Optional viewer mode (`"NativeVector"` / `"native"` or `"PdfRaster"` / `"pdf"`)
        view_mode: Option<String>,
        /// Optional viewer zoom factor (`0.25..=4.0`)
        zoom: Option<f32>,
        /// Optional toggle for the bottom drawer
        show_drawer: Option<bool>,
        /// Optional bottom drawer tab (`"diagnostics"`, `"audit"`, `"logs"`, `"api"`, or `"audit_log"`)
        drawer_tab: Option<String>,
        /// Optional toggle for the left sidebar
        show_sidebar: Option<bool>,
    },
}

/// Thread-safe bridge between the background REST API server and the live GUI editor (`--with-api --with-state`).
pub struct SharedAppState {
    snapshot: RwLock<LiveEditorSnapshot>,
    commands: Mutex<VecDeque<StatefulUiCommand>>,
    has_commands: AtomicBool,
    egui_ctx: Mutex<Option<eframe::egui::Context>>,
}

impl Default for SharedAppState {
    fn default() -> Self {
        Self::new(LiveEditorSnapshot::default())
    }
}

impl SharedAppState {
    /// Creates a new [`SharedAppState`] initialized with the given editor snapshot.
    pub fn new(initial: LiveEditorSnapshot) -> Self {
        Self {
            snapshot: RwLock::new(initial),
            commands: Mutex::new(VecDeque::new()),
            has_commands: AtomicBool::new(false),
            egui_ctx: Mutex::new(None),
        }
    }

    /// Registers the active `egui::Context` so background API calls can wake the GUI window immediately.
    pub fn set_egui_context(&self, ctx: eframe::egui::Context) {
        if let Ok(mut guard) = self.egui_ctx.lock() {
            *guard = Some(ctx);
        }
    }

    /// Requests an immediate GUI frame repaint if an `egui::Context` is attached.
    pub fn request_repaint(&self) {
        if let Ok(guard) = self.egui_ctx.lock() {
            if let Some(ref ctx) = *guard {
                ctx.request_repaint();
            }
        }
    }

    /// Returns a cloned snapshot of the current live editor state.
    pub fn snapshot(&self) -> LiveEditorSnapshot {
        self.snapshot
            .read()
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    /// Inspects the current snapshot by reference under a read lock without cloning.
    pub fn with_snapshot<R, F: FnOnce(&LiveEditorSnapshot) -> R>(&self, f: F) -> Option<R> {
        self.snapshot.read().ok().map(|guard| f(&guard))
    }

    /// Mutates the live editor snapshot in-place from the GUI thread without cloning the snapshot back out.
    pub fn sync_from_gui<F: FnOnce(&mut LiveEditorSnapshot)>(&self, f: F) {
        if let Ok(mut guard) = self.snapshot.write() {
            f(&mut guard);
            guard.revision = guard.revision.wrapping_add(1);
        }
    }

    /// Mutates the live editor snapshot in-place and increments its revision counter.
    pub fn update_snapshot<F: FnOnce(&mut LiveEditorSnapshot)>(&self, f: F) -> LiveEditorSnapshot {
        if let Ok(mut guard) = self.snapshot.write() {
            f(&mut guard);
            guard.line_count = guard.source.lines().count().max(1);
            guard.word_count = guard.source.split_whitespace().count();
            guard.char_count = guard.source.chars().count();
            guard.revision = guard.revision.wrapping_add(1);
            guard.clone()
        } else {
            LiveEditorSnapshot::default()
        }
    }

    /// Enqueues a UI command for the GUI thread and requests an immediate repaint.
    pub fn push_command(&self, cmd: StatefulUiCommand) {
        if let Ok(mut queue) = self.commands.lock() {
            queue.push_back(cmd);
            self.has_commands.store(true, Ordering::Release);
        }
        self.request_repaint();
    }

    /// Drains all pending UI commands queued by the REST API server.
    pub fn drain_commands(&self) -> Vec<StatefulUiCommand> {
        if !self.has_commands.load(Ordering::Acquire) {
            return Vec::new();
        }
        if let Ok(mut queue) = self.commands.lock() {
            self.has_commands.store(false, Ordering::Release);
            queue.drain(..).collect()
        } else {
            Vec::new()
        }
    }

    /// Returns `true` if there are queued UI commands waiting to be processed by the GUI thread.
    pub fn has_pending_commands(&self) -> bool {
        self.has_commands.load(Ordering::Acquire)
    }
}

/// A single targeted edit operation in `POST /api/document`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DocumentEditOperation {
    /// Replace a 1-based inclusive line range `[start_line, end_line]` with `content`.
    LineRange {
        /// 1-based starting line number (inclusive)
        start_line: usize,
        /// 1-based ending line number (inclusive)
        end_line: usize,
        /// Replacement text for the specified line range
        content: String,
    },
    /// Search for `search` and replace with `replace` (first occurrence unless `all` is true).
    SearchReplace {
        /// Exact substring to locate in the document
        search: String,
        /// Replacement string
        replace: String,
        /// If true, replaces all occurrences; otherwise replaces only the first occurrence
        #[serde(default)]
        all: bool,
    },
}

/// Applies a sequence of [`DocumentEditOperation`] items to `source`.
pub fn apply_edit_operations(
    source: &str,
    edits: &[DocumentEditOperation],
) -> Result<String, String> {
    let mut current = source.replace("\r\n", "\n");
    for (idx, edit) in edits.iter().enumerate() {
        match edit {
            DocumentEditOperation::SearchReplace {
                search,
                replace,
                all,
            } => {
                let norm_search = search.replace("\r\n", "\n");
                let norm_replace = replace.replace("\r\n", "\n");
                if norm_search.is_empty() {
                    return Err(format!("Edit #{}: 'search' string cannot be empty", idx + 1));
                }
                if !current.contains(&norm_search) {
                    return Err(format!(
                        "Edit #{}: search target not found in document",
                        idx + 1
                    ));
                }
                current = if *all {
                    current.replace(&norm_search, &norm_replace)
                } else {
                    current.replacen(&norm_search, &norm_replace, 1)
                };
            }
            DocumentEditOperation::LineRange {
                start_line,
                end_line,
                content,
            } => {
                if *start_line == 0 || *end_line < *start_line {
                    return Err(format!(
                        "Edit #{}: invalid 1-based line range {}..={}",
                        idx + 1,
                        start_line,
                        end_line
                    ));
                }
                let mut lines: Vec<&str> = current.split('\n').collect();
                if *start_line > lines.len() + 1 || *end_line > lines.len() {
                    return Err(format!(
                        "Edit #{}: line range {}..={} exceeds document line count ({})",
                        idx + 1,
                        start_line,
                        end_line,
                        lines.len()
                    ));
                }
                let norm_content = content.replace("\r\n", "\n");
                let replacement_lines: Vec<&str> = if norm_content.is_empty() {
                    Vec::new()
                } else {
                    norm_content.split('\n').collect()
                };
                lines.splice((start_line - 1)..*end_line, replacement_lines);
                current = lines.join("\n");
            }
        }
    }
    Ok(current)
}

/// Request payload for `POST /api/document` in `--with-state` mode.
#[derive(Debug, Default, Deserialize)]
pub struct UpdateDocumentApiRequest {
    /// Optional full LaTeX source code replacement
    pub source: Option<String>,
    /// Optional sequence of surgical search-replace or line-range edits
    pub edits: Option<Vec<DocumentEditOperation>>,
    /// Whether to save the updated buffer to its bound file on disk (default: false)
    #[serde(default)]
    pub save: bool,
    /// Whether to trigger compilation/layout refresh in the GUI (default: true when content changes)
    pub compile: Option<bool>,
    /// Optional compiler engine override ("native", "tectonic", "pdflatex", "xelatex", "lualatex")
    pub engine: Option<String>,
    /// Optional 1-based line number to jump the editor caret to
    pub jump_to_line: Option<usize>,
    /// Optional viewer mode override ("NativeVector" / "native" or "PdfRaster" / "pdf")
    pub view_mode: Option<String>,
}

/// Request payload for `POST /api/navigate` in `--with-state` mode.
#[derive(Debug, Default, Deserialize)]
pub struct NavigateApiRequest {
    /// Optional 1-based line number to jump the editor caret to
    pub line: Option<usize>,
    /// Optional 1-based page number to jump the PDF/vector viewer to
    pub page: Option<usize>,
    /// Optional viewer mode ("NativeVector" / "native" or "PdfRaster" / "pdf")
    pub view_mode: Option<String>,
    /// Optional viewer zoom factor (e.g. 1.0 for 100%, clamped to 0.25..=4.0)
    pub zoom: Option<f32>,
    /// Optional toggle for the bottom diagnostics/audit/logs drawer
    pub show_drawer: Option<bool>,
    /// Optional bottom drawer tab selection ("diagnostics", "audit", "logs", "api", or "audit_log")
    pub drawer_tab: Option<String>,
    /// Optional toggle for the left navigation sidebar
    pub show_sidebar: Option<bool>,
}

/// Request payload for `/api/compile`.
#[derive(Debug, Default, Deserialize)]
pub struct CompileApiRequest {
    /// LaTeX document source code (optional in `--with-state` mode; defaults to active editor buffer)
    #[serde(default)]
    pub source: Option<String>,
    /// Compiler engine backend (native, tectonic, pdflatex, xelatex, lualatex)
    pub engine: Option<String>,
    /// Optional opt-in request to allow `-shell-escape` (honored only if server allows shell-escape)
    #[serde(default)]
    pub allow_shell_escape: bool,
    /// Optional flag in `--with-state` mode to also save the updated source to its bound file on disk
    #[serde(default)]
    pub save: bool,
}

/// Response payload from `/api/compile`.
#[derive(Debug, Serialize)]
pub struct CompileApiResponse {
    /// True if compilation succeeded
    pub success: bool,
    /// Elapsed compile time in milliseconds
    pub duration_ms: u128,
    /// Parsed compiler warnings and errors
    pub diagnostics: Vec<crate::compiler::Diagnostic>,
    /// Base64 encoded PDF bytes if generated
    pub pdf_base64: Option<String>,
    /// Raw compiler standard output
    pub stdout: String,
    /// Raw compiler standard error
    pub stderr: String,
    /// Markdown error report if errors occurred
    pub error_report_markdown: Option<String>,
}

/// Request payload for `/api/check`, `/api/outline`, and `/api/packages`.
#[derive(Debug, Default, Deserialize)]
pub struct SourceApiRequest {
    /// LaTeX document source code (optional in `--with-state` mode; defaults to active editor buffer)
    #[serde(default)]
    pub source: Option<String>,
}

/// Request payload for `/api/dependencies/install`.
#[derive(Debug, Deserialize)]
pub struct InstallDependencyApiRequest {
    /// Pip or system dependency package name (e.g. "pygments")
    pub package: String,
    /// Explicit user consent flag
    pub consent: bool,
}

/// Embedded HTTP server handle.
#[allow(dead_code)]
pub struct HeadlessServer {
    /// Atomic flag controlling the server listening loop
    pub running: Arc<AtomicBool>,
}

impl Default for HeadlessServer {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadlessServer {
    /// Constructs a new [`HeadlessServer`] instance with active running state.
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Spawns the HTTP REST API server on a dedicated background thread,
    /// returning an [`Arc<AtomicBool>`] handle that can be used to signal graceful shutdown.
    pub fn start_background(host: &str, port: u16) -> std::io::Result<Arc<AtomicBool>> {
        Self::start_background_with_options(host, port, false)
    }

    /// Spawns the HTTP REST API server on a dedicated background thread with explicit shell-escape policy.
    pub fn start_background_with_options(
        host: &str,
        port: u16,
        server_allow_shell_escape: bool,
    ) -> std::io::Result<Arc<AtomicBool>> {
        Self::start_background_with_state(host, port, server_allow_shell_escape, None)
    }

    /// Spawns the HTTP REST API server on a dedicated background thread with optional live GUI state synchronization (`--with-state`).
    pub fn start_background_with_state(
        host: &str,
        port: u16,
        server_allow_shell_escape: bool,
        shared_state: Option<Arc<SharedAppState>>,
    ) -> std::io::Result<Arc<AtomicBool>> {
        let address = format!("{host}:{port}");
        let bind_host = host.to_string();
        let server = Server::http(&address).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Server start error: {e}"))
        })?;

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        thread::spawn(move || {
            let mode_str = if shared_state.is_some() {
                "stateful live-GUI mode"
            } else {
                "stateless mode"
            };
            println!("[Laketex Headless API] Listening on http://{address} ({mode_str})");

            while running_clone.load(Ordering::SeqCst) {
                if let Ok(Some(mut request)) = server.recv_timeout(std::time::Duration::from_millis(200)) {
                    // Security validation: block CSRF, cross-origin browser requests, and DNS rebinding attacks
                    if let Err((status_code, err_msg)) = validate_request_security(&request, &bind_host) {
                        let body = serde_json::json!({ "error": err_msg }).to_string();
                        let content_type = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
                        let response = Response::from_string(body)
                            .with_status_code(status_code)
                            .with_header(content_type);
                        let _ = request.respond(response);
                        continue;
                    }

                    let url = request.url().to_string();
                    let method = request.method().as_str().to_uppercase();

                    let (status, body) = match (method.as_str(), url.as_str()) {
                        ("GET", "/health") | ("GET", "/api/status") => {
                            let engines = CompilerEngine::detect_engines();
                            crate::ui::drawer::record_api_audit(
                                crate::ui::drawer::AuditActor::ApiRead,
                                format!("GET {url}"),
                                format!(
                                    "Probed server health & engines (stateful={})",
                                    shared_state.is_some()
                                ),
                                None,
                            );
                            let json = serde_json::json!({
                                "status": "online",
                                "app": "Laketex",
                                "version": env!("CARGO_PKG_VERSION"),
                                "stateful": shared_state.is_some(),
                                "api_spec_endpoint": "/api/spec",
                                "help_endpoint": "/api/help",
                                "api_spec_cli_command": "laketex api-spec",
                                "engines": engines,
                            });
                            (200, json.to_string())
                        }
                        ("GET", "/") | ("GET", "/help") | ("GET", "/api/help") => {
                            crate::ui::drawer::record_api_audit(
                                crate::ui::drawer::AuditActor::ApiRead,
                                format!("GET {url}"),
                                "Retrieved REST API help & endpoint directory".to_string(),
                                None,
                            );
                            let help_json = serde_json::json!({
                                "app": "Laketex",
                                "version": env!("CARGO_PKG_VERSION"),
                                "stateful_gui_mode_active": shared_state.is_some(),
                                "agent_api_spec": {
                                    "description": "Complete machine-readable JSON specification of all endpoints, schemas, security invariants, and agent workflows.",
                                    "http_endpoint": "GET /api/spec (aliases: /api/schema, /api/agent)",
                                    "cli_command": "laketex api-spec (aliases: agent-spec, schema, endpoints; pass --compact for single-line JSON)"
                                },
                                "launch_modes": {
                                    "headless_daemon": "laketex serve --port 3030",
                                    "gui_with_stateless_api": "laketex --with-api --api-port 3030",
                                    "gui_with_stateful_live_api": "laketex --with-api --with-state --api-port 3030"
                                },
                                "endpoints": [
                                    { "method": "GET", "path": "/api/spec", "aliases": ["/api/schema", "/api/agent"], "summary": "Full machine-readable agent API specification (same output as CLI `laketex api-spec`)" },
                                    { "method": "GET", "path": "/api/help", "aliases": ["/help", "/"], "summary": "Quick endpoint overview and CLI command reference" },
                                    { "method": "GET", "path": "/health", "aliases": ["/api/status"], "summary": "Server health, stateful mode flag, and detected TeX engines" },
                                    { "method": "GET", "path": "/api/document", "summary": "[--with-state] Read active GUI editor buffer, file path, dirty status, and diagnostics" },
                                    { "method": "POST", "path": "/api/document", "aliases": ["PUT /api/document", "PATCH /api/document"], "summary": "[--with-state] Replace or surgically patch active GUI editor buffer (line_range / search_replace)" },
                                    { "method": "POST", "path": "/api/navigate", "summary": "[--with-state] Move editor caret to line, switch viewer page/mode/zoom, or open console drawer tabs (including audit_log)" },
                                    { "method": "POST", "path": "/api/compile", "summary": "Compile LaTeX source to base64 PDF and diagnostics (updates live GUI when in --with-state mode)" },
                                    { "method": "POST", "path": "/api/check", "summary": "Fast Tectonic syntax & diagnostic linting" },
                                    { "method": "POST", "path": "/api/outline", "summary": "Extract hierarchical section, figure, table, and equation outline" },
                                    { "method": "POST", "path": "/api/packages", "summary": "Detect imported LaTeX packages and metadata" },
                                    { "method": "POST", "path": "/api/debug", "aliases": ["/debug"], "summary": "Deep typographic & math/matrix render audit" },
                                    { "method": "GET", "path": "/api/dependencies", "summary": "List external binary/Python dependencies and installation status" },
                                    { "method": "POST", "path": "/api/dependencies/check", "summary": "Check external dependencies required by a specific document" },
                                    { "method": "POST", "path": "/api/dependencies/install", "summary": "Install external dependency (requires explicit consent: true)" },
                                    { "method": "GET", "path": "/api/docs", "summary": "List built-in offline LaTeX documentation manuals" },
                                    { "method": "POST", "path": "/api/docs", "summary": "Retrieve full source of a built-in LaTeX documentation manual" }
                                ]
                            });
                            (200, help_json.to_string())
                        }
                        ("GET", "/api/spec")
                        | ("GET", "/api/schema")
                        | ("GET", "/api/agent") => {
                            crate::ui::drawer::record_api_audit(
                                crate::ui::drawer::AuditActor::ApiRead,
                                format!("GET {url}"),
                                "Retrieved machine-readable agent API specification".to_string(),
                                None,
                            );
                            let spec = Self::agent_api_spec_json(
                                port,
                                Some(shared_state.is_some()),
                                Some(server_allow_shell_escape),
                            );
                            (200, spec.to_string())
                        }
                        ("GET", "/api/document") => {
                            if let Some(ref shared) = shared_state {
                                let snap = shared.snapshot();
                                crate::ui::drawer::record_api_audit(
                                    crate::ui::drawer::AuditActor::ApiRead,
                                    "GET /api/document",
                                    format!(
                                        "Read live document '{}' (rev {}, {} lines)",
                                        snap.file_name, snap.revision, snap.line_count
                                    ),
                                    None,
                                );
                                (200, serde_json::to_string(&snap).unwrap_or_default())
                            } else {
                                crate::ui::drawer::record_api_audit(
                                    crate::ui::drawer::AuditActor::ApiRead,
                                    "GET /api/document [400]",
                                    "Rejected: server not running in --with-state mode".to_string(),
                                    None,
                                );
                                (
                                    400,
                                    serde_json::json!({
                                        "error": "Endpoint /api/document requires stateful GUI mode (launch with --with-api --with-state)"
                                    })
                                    .to_string(),
                                )
                            }
                        }
                        ("POST", "/api/document")
                        | ("PUT", "/api/document")
                        | ("PATCH", "/api/document") => {
                            if let Some(ref shared) = shared_state {
                                let body_str = read_json_body(&mut request);
                                match serde_json::from_str::<UpdateDocumentApiRequest>(&body_str) {
                                    Ok(req) => {
                                        let snap = shared.snapshot();
                                        let mut updated_source = req
                                            .source
                                            .clone()
                                            .unwrap_or_else(|| snap.source.clone());

                                        let mut inferred_line = req.jump_to_line;
                                        if let Some(ref edits) = req.edits {
                                            if inferred_line.is_none() {
                                                for op in edits {
                                                    match op {
                                                        DocumentEditOperation::LineRange {
                                                            start_line,
                                                            ..
                                                        } => {
                                                            inferred_line = Some(*start_line);
                                                            break;
                                                        }
                                                        DocumentEditOperation::SearchReplace {
                                                            search,
                                                            ..
                                                        } => {
                                                            if let Some(byte_pos) =
                                                                updated_source.find(search.as_str())
                                                            {
                                                                let line_num = updated_source
                                                                    [..byte_pos]
                                                                    .chars()
                                                                    .filter(|&c| c == '\n')
                                                                    .count()
                                                                    + 1;
                                                                inferred_line = Some(line_num);
                                                                break;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            match apply_edit_operations(&updated_source, edits) {
                                                Ok(edited) => updated_source = edited,
                                                Err(err_msg) => {
                                                    crate::ui::drawer::record_api_audit(
                                                        crate::ui::drawer::AuditActor::ApiMutation,
                                                        format!("{method} /api/document [Failed]"),
                                                        format!("Edit rejected: {err_msg}"),
                                                        inferred_line,
                                                    );
                                                    let resp_body =
                                                        serde_json::json!({ "error": err_msg })
                                                            .to_string();
                                                    let content_type = Header::from_bytes(
                                                        &b"Content-Type"[..],
                                                        &b"application/json"[..],
                                                    )
                                                    .unwrap();
                                                    let response = Response::from_string(resp_body)
                                                        .with_status_code(400)
                                                        .with_header(content_type);
                                                    let _ = request.respond(response);
                                                    continue;
                                                }
                                            }
                                        }

                                        let content_modified =
                                            req.source.is_some() || req.edits.is_some();

                                        let new_dirty = if req.save {
                                            if let Some(ref path) = snap.file_path {
                                                if let Err(e) =
                                                    std::fs::write(path, &updated_source)
                                                {
                                                    let resp_body = serde_json::json!({
                                                        "error": format!("Failed to save file: {e}")
                                                    })
                                                    .to_string();
                                                    let content_type = Header::from_bytes(
                                                        &b"Content-Type"[..],
                                                        &b"application/json"[..],
                                                    )
                                                    .unwrap();
                                                    let response = Response::from_string(resp_body)
                                                        .with_status_code(500)
                                                        .with_header(content_type);
                                                    let _ = request.respond(response);
                                                    continue;
                                                }
                                                false
                                            } else {
                                                let resp_body = serde_json::json!({
                                                    "error": "Cannot save untitled document: no file path is bound in the editor"
                                                })
                                                .to_string();
                                                let content_type = Header::from_bytes(
                                                    &b"Content-Type"[..],
                                                    &b"application/json"[..],
                                                )
                                                .unwrap();
                                                let response = Response::from_string(resp_body)
                                                    .with_status_code(400)
                                                    .with_header(content_type);
                                                let _ = request.respond(response);
                                                continue;
                                            }
                                        } else if content_modified {
                                            true
                                        } else {
                                            snap.is_dirty
                                        };

                                        let new_engine =
                                            req.engine.as_deref().map(parse_engine_str);
                                        if let Some(eng) = new_engine {
                                            shared.push_command(StatefulUiCommand::SetEngine(eng));
                                        }

                                        if req.jump_to_line.is_some() || req.view_mode.is_some() {
                                            shared.push_command(StatefulUiCommand::Navigate {
                                                line: req.jump_to_line,
                                                page: None,
                                                view_mode: req.view_mode.clone(),
                                                zoom: None,
                                                show_drawer: None,
                                                drawer_tab: None,
                                                show_sidebar: None,
                                            });
                                        }

                                        if content_modified
                                            || req.save
                                            || req.compile == Some(true)
                                        {
                                            let trigger_compile =
                                                req.compile.unwrap_or(content_modified);
                                            let status_message = if req.save {
                                                format!("API saved {}", snap.file_name)
                                            } else {
                                                "Updated via REST API".to_string()
                                            };
                                            shared.push_command(
                                                StatefulUiCommand::SetDocumentContent {
                                                    source: updated_source.clone(),
                                                    is_dirty: new_dirty,
                                                    trigger_compile,
                                                    status_message,
                                                },
                                            );
                                        }

                                        let edit_desc = if let Some(ref edits) = req.edits {
                                            format!("Applied {} surgical edit op(s)", edits.len())
                                        } else if req.source.is_some() {
                                            "Replaced full document buffer".to_string()
                                        } else {
                                            "Updated document state".to_string()
                                        };

                                        let updated_snap = shared.update_snapshot(|s| {
                                            s.source = updated_source;
                                            s.is_dirty = new_dirty;
                                            if let Some(eng) = new_engine {
                                                s.engine = eng;
                                            }
                                            if let Some(ref vm) = req.view_mode {
                                                s.view_mode =
                                                    normalize_view_mode_label(vm).to_string();
                                            }
                                        });

                                        crate::ui::drawer::record_api_audit(
                                            crate::ui::drawer::AuditActor::ApiMutation,
                                            format!("{method} /api/document"),
                                            format!(
                                                "{edit_desc} on '{}' → rev {} ({} lines, save={})",
                                                updated_snap.file_name,
                                                updated_snap.revision,
                                                updated_snap.line_count,
                                                req.save
                                            ),
                                            inferred_line,
                                        );

                                        (
                                            200,
                                            serde_json::to_string(&updated_snap)
                                                .unwrap_or_default(),
                                        )
                                    }
                                    Err(e) => (
                                        400,
                                        serde_json::json!({ "error": format!("Invalid JSON: {e}") })
                                            .to_string(),
                                    ),
                                }
                            } else {
                                (
                                    400,
                                    serde_json::json!({
                                        "error": "Endpoint /api/document requires stateful GUI mode (launch with --with-api --with-state)"
                                    })
                                    .to_string(),
                                )
                            }
                        }
                        ("POST", "/api/navigate") => {
                            if let Some(ref shared) = shared_state {
                                let body_str = read_json_body(&mut request);
                                match serde_json::from_str::<NavigateApiRequest>(&body_str) {
                                    Ok(req) => {
                                        let mut parts = Vec::new();
                                        if let Some(l) = req.line {
                                            parts.push(format!("line={l}"));
                                        }
                                        if let Some(p) = req.page {
                                            parts.push(format!("page={p}"));
                                        }
                                        if let Some(ref vm) = req.view_mode {
                                            parts.push(format!("view_mode={vm}"));
                                        }
                                        if let Some(z) = req.zoom {
                                            parts.push(format!("zoom={z:.2}"));
                                        }
                                        if let Some(ref tab) = req.drawer_tab {
                                            parts.push(format!("drawer_tab={tab}"));
                                        }
                                        let nav_summary = if parts.is_empty() {
                                            "Updated GUI panel navigation".to_string()
                                        } else {
                                            format!("Navigated GUI ({})", parts.join(", "))
                                        };
                                        crate::ui::drawer::record_api_audit(
                                            crate::ui::drawer::AuditActor::ApiMutation,
                                            "POST /api/navigate",
                                            nav_summary,
                                            req.line,
                                        );

                                        let updated_snap = shared.update_snapshot(|s| {
                                            if let Some(p) = req.page {
                                                s.current_page = p.max(1);
                                            }
                                            if let Some(z) = req.zoom {
                                                s.zoom = z.clamp(0.25, 4.0);
                                            }
                                            if let Some(ref vm) = req.view_mode {
                                                s.view_mode =
                                                    normalize_view_mode_label(vm).to_string();
                                            }
                                        });

                                        shared.push_command(StatefulUiCommand::Navigate {
                                            line: req.line,
                                            page: req.page,
                                            view_mode: req.view_mode,
                                            zoom: req.zoom,
                                            show_drawer: req.show_drawer,
                                            drawer_tab: req.drawer_tab,
                                            show_sidebar: req.show_sidebar,
                                        });

                                        (
                                            200,
                                            serde_json::json!({
                                                "success": true,
                                                "snapshot": updated_snap
                                            })
                                            .to_string(),
                                        )
                                    }
                                    Err(e) => (
                                        400,
                                        serde_json::json!({ "error": format!("Invalid JSON: {e}") })
                                            .to_string(),
                                    ),
                                }
                            } else {
                                (
                                    400,
                                    serde_json::json!({
                                        "error": "Endpoint /api/navigate requires stateful GUI mode (launch with --with-api --with-state)"
                                    })
                                    .to_string(),
                                )
                            }
                        }
                        ("POST", "/api/compile") => {
                            let body_str = read_json_body(&mut request);

                            match serde_json::from_str::<CompileApiRequest>(&body_str) {
                                Ok(req) => {
                                    if let Some(ref shared) = shared_state {
                                        let snap = shared.snapshot();
                                        let engine = req
                                            .engine
                                            .as_deref()
                                            .map(parse_engine_str)
                                            .unwrap_or(snap.engine);
                                        let source_to_compile = req
                                            .source
                                            .clone()
                                            .unwrap_or_else(|| snap.source.clone());

                                        let new_dirty = if req.save {
                                            if let Some(ref path) = snap.file_path {
                                                if let Err(e) =
                                                    std::fs::write(path, &source_to_compile)
                                                {
                                                    let resp_body = serde_json::json!({
                                                        "error": format!("Failed to save file: {e}")
                                                    })
                                                    .to_string();
                                                    let content_type = Header::from_bytes(
                                                        &b"Content-Type"[..],
                                                        &b"application/json"[..],
                                                    )
                                                    .unwrap();
                                                    let response = Response::from_string(resp_body)
                                                        .with_status_code(500)
                                                        .with_header(content_type);
                                                    let _ = request.respond(response);
                                                    continue;
                                                }
                                                false
                                            } else {
                                                let resp_body = serde_json::json!({
                                                    "error": "Cannot save untitled document: no file path is bound in the editor"
                                                })
                                                .to_string();
                                                let content_type = Header::from_bytes(
                                                    &b"Content-Type"[..],
                                                    &b"application/json"[..],
                                                )
                                                .unwrap();
                                                let response = Response::from_string(resp_body)
                                                    .with_status_code(400)
                                                    .with_header(content_type);
                                                let _ = request.respond(response);
                                                continue;
                                            }
                                        } else if req.source.is_some() {
                                            true
                                        } else {
                                            snap.is_dirty
                                        };

                                        let options = CompileOptions {
                                            work_dir: Some(snap.working_dir.clone()),
                                            doc_name: Some(snap.file_name.clone()),
                                            allow_shell_escape: server_allow_shell_escape
                                                && req.allow_shell_escape,
                                            shadow_subdocument: None,
                                            cancel_token: None,
                                        };

                                        match CompilerEngine::compile_with_options(
                                            &source_to_compile,
                                            engine,
                                            &options,
                                        ) {
                                            Ok(res) => {
                                                crate::ui::drawer::record_api_audit(
                                                    crate::ui::drawer::AuditActor::ApiMutation,
                                                    "POST /api/compile",
                                                    format!(
                                                        "Compiled live document '{}' with {} in {} ms (success={}, {} diagnostics)",
                                                        snap.file_name,
                                                        engine.display_name(),
                                                        res.duration_ms,
                                                        res.success,
                                                        res.diagnostics.len()
                                                    ),
                                                    res.diagnostics.iter().find_map(|d| d.line),
                                                );
                                                shared.update_snapshot(|s| {
                                                    s.source = source_to_compile.clone();
                                                    s.is_dirty = new_dirty;
                                                    s.engine = engine;
                                                    s.diagnostics = res.diagnostics.clone();
                                                    s.last_duration_ms = Some(res.duration_ms);
                                                });
                                                shared.push_command(
                                                    StatefulUiCommand::ApplyCompileResult {
                                                        source: req.source.clone(),
                                                        is_dirty: Some(new_dirty),
                                                        engine,
                                                        result: res.clone(),
                                                    },
                                                );

                                                let pdf_b64 = res
                                                    .pdf_bytes
                                                    .as_ref()
                                                    .map(|b| BASE64.encode(b));
                                                let report_md =
                                                    if !res.success || !res.diagnostics.is_empty() {
                                                        let r =
                                                            crate::compiler::ErrorReport::generate(
                                                                &source_to_compile,
                                                                &snap.file_name,
                                                                engine,
                                                                &res.diagnostics,
                                                                &res.stdout,
                                                                &res.stderr,
                                                            );
                                                        Some(r.to_markdown())
                                                    } else {
                                                        None
                                                    };
                                                let response = CompileApiResponse {
                                                    success: res.success,
                                                    duration_ms: res.duration_ms,
                                                    diagnostics: res.diagnostics,
                                                    pdf_base64: pdf_b64,
                                                    stdout: res.stdout,
                                                    stderr: res.stderr,
                                                    error_report_markdown: report_md,
                                                };
                                                (
                                                    200,
                                                    serde_json::to_string(&response)
                                                        .unwrap_or_default(),
                                                )
                                            }
                                            Err(e) => (
                                                500,
                                                serde_json::json!({ "error": e.to_string() })
                                                    .to_string(),
                                            ),
                                        }
                                    } else {
                                        let Some(source_str) = req.source else {
                                            let resp_body = serde_json::json!({
                                                "error": "Missing required field 'source' (or launch with --with-api --with-state to compile the active editor buffer)"
                                            })
                                            .to_string();
                                            let content_type = Header::from_bytes(
                                                &b"Content-Type"[..],
                                                &b"application/json"[..],
                                            )
                                            .unwrap();
                                            let response = Response::from_string(resp_body)
                                                .with_status_code(400)
                                                .with_header(content_type);
                                            let _ = request.respond(response);
                                            continue;
                                        };
                                        let engine = req
                                            .engine
                                            .as_deref()
                                            .map(parse_engine_str)
                                            .unwrap_or(EngineKind::Native);
                                        let options = CompileOptions {
                                            allow_shell_escape: server_allow_shell_escape
                                                && req.allow_shell_escape,
                                            ..Default::default()
                                        };

                                        match CompilerEngine::compile_with_options(
                                            &source_str,
                                            engine,
                                            &options,
                                        ) {
                                            Ok(res) => {
                                                crate::ui::drawer::record_api_audit(
                                                    crate::ui::drawer::AuditActor::ApiRead,
                                                    "POST /api/compile [Stateless]",
                                                    format!(
                                                        "Stateless compile with {} in {} ms (success={}, {} diagnostics)",
                                                        engine.display_name(),
                                                        res.duration_ms,
                                                        res.success,
                                                        res.diagnostics.len()
                                                    ),
                                                    res.diagnostics.iter().find_map(|d| d.line),
                                                );
                                                let pdf_b64 = res
                                                    .pdf_bytes
                                                    .as_ref()
                                                    .map(|b| BASE64.encode(b));
                                                let report_md =
                                                    if !res.success || !res.diagnostics.is_empty() {
                                                        let r =
                                                            crate::compiler::ErrorReport::generate(
                                                                &source_str,
                                                                "api_document.tex",
                                                                engine,
                                                                &res.diagnostics,
                                                                &res.stdout,
                                                                &res.stderr,
                                                            );
                                                        Some(r.to_markdown())
                                                    } else {
                                                        None
                                                    };
                                                let response = CompileApiResponse {
                                                    success: res.success,
                                                    duration_ms: res.duration_ms,
                                                    diagnostics: res.diagnostics,
                                                    pdf_base64: pdf_b64,
                                                    stdout: res.stdout,
                                                    stderr: res.stderr,
                                                    error_report_markdown: report_md,
                                                };
                                                (
                                                    200,
                                                    serde_json::to_string(&response)
                                                        .unwrap_or_default(),
                                                )
                                            }
                                            Err(e) => (
                                                500,
                                                serde_json::json!({ "error": e.to_string() })
                                                    .to_string(),
                                            ),
                                        }
                                    }
                                }
                                Err(e) => (
                                    400,
                                    serde_json::json!({ "error": format!("Invalid JSON: {e}") })
                                        .to_string(),
                                ),
                            }
                        }
                        ("POST", "/api/outline") => {
                            let body_str = read_json_body(&mut request);

                            match serde_json::from_str::<SourceApiRequest>(&body_str) {
                                Ok(req) => {
                                    match resolve_readonly_source(req.source, &shared_state) {
                                        Ok((source_str, doc_name, _)) => {
                                            let items = OutlineParser::parse(&source_str);
                                            crate::ui::drawer::record_api_audit(
                                                crate::ui::drawer::AuditActor::ApiRead,
                                                "POST /api/outline",
                                                format!(
                                                    "Extracted {} outline section(s) from '{doc_name}'",
                                                    items.len()
                                                ),
                                                items.first().map(|i| i.line),
                                            );
                                            (200, serde_json::json!({ "outline": items }).to_string())
                                        }
                                        Err(err_resp) => err_resp,
                                    }
                                }
                                Err(e) => (
                                    400,
                                    serde_json::json!({ "error": format!("Invalid JSON: {e}") }).to_string(),
                                ),
                            }
                        }
                        ("POST", "/api/packages") => {
                            let body_str = read_json_body(&mut request);

                            match serde_json::from_str::<SourceApiRequest>(&body_str) {
                                Ok(req) => {
                                    match resolve_readonly_source(req.source, &shared_state) {
                                        Ok((source_str, doc_name, _)) => {
                                            let pkgs = PackageManager::detect_packages(&source_str);
                                            crate::ui::drawer::record_api_audit(
                                                crate::ui::drawer::AuditActor::ApiRead,
                                                "POST /api/packages",
                                                format!(
                                                    "Detected {} imported package(s) in '{doc_name}'",
                                                    pkgs.len()
                                                ),
                                                pkgs.first().map(|p| p.line),
                                            );
                                            (200, serde_json::json!({ "packages": pkgs }).to_string())
                                        }
                                        Err(err_resp) => err_resp,
                                    }
                                }
                                Err(e) => (
                                    400,
                                    serde_json::json!({ "error": format!("Invalid JSON: {e}") }).to_string(),
                                ),
                            }
                        }
                        ("POST", "/api/check") => {
                            let body_str = read_json_body(&mut request);

                            match serde_json::from_str::<SourceApiRequest>(&body_str) {
                                Ok(req) => {
                                    match resolve_readonly_source(req.source, &shared_state) {
                                        Ok((source_str, doc_name, work_dir)) => {
                                            match CompilerEngine::compile(
                                                &source_str,
                                                EngineKind::Tectonic,
                                                work_dir.as_deref(),
                                            ) {
                                                Ok(res) => {
                                                    crate::ui::drawer::record_api_audit(
                                                        crate::ui::drawer::AuditActor::ApiRead,
                                                        "POST /api/check",
                                                        format!(
                                                            "Linted '{doc_name}' in {} ms (success={}, {} diagnostics)",
                                                            res.duration_ms,
                                                            res.success,
                                                            res.diagnostics.len()
                                                        ),
                                                        res.diagnostics.iter().find_map(|d| d.line),
                                                    );
                                                    let report =
                                                        crate::compiler::ErrorReport::generate(
                                                            &source_str,
                                                            &doc_name,
                                                            EngineKind::Tectonic,
                                                            &res.diagnostics,
                                                            &res.stdout,
                                                            &res.stderr,
                                                        );
                                                    (
                                                        200,
                                                        serde_json::json!({
                                                            "success": res.success,
                                                            "diagnostics": res.diagnostics,
                                                            "duration_ms": res.duration_ms,
                                                            "error_report_markdown": report.to_markdown(),
                                                            "error_report": report
                                                        })
                                                        .to_string(),
                                                    )
                                                }
                                                Err(e) => (
                                                    500,
                                                    serde_json::json!({ "error": e.to_string() })
                                                        .to_string(),
                                                ),
                                            }
                                        }
                                        Err(err_resp) => err_resp,
                                    }
                                }
                                Err(e) => (
                                    400,
                                    serde_json::json!({ "error": format!("Invalid JSON: {e}") }).to_string(),
                                ),
                            }
                        }
                        ("POST", "/api/debug") | ("POST", "/debug") => {
                            let body_str = read_json_body(&mut request);

                            match serde_json::from_str::<CompileApiRequest>(&body_str) {
                                Ok(req) => {
                                    match resolve_readonly_source(req.source, &shared_state) {
                                        Ok((source_str, doc_name, work_dir)) => {
                                            let engine = match req.engine.as_deref() {
                                                Some("tectonic") => EngineKind::Tectonic,
                                                _ => EngineKind::Native,
                                            };
                                            let options = CompileOptions {
                                                work_dir,
                                                doc_name: Some(doc_name.clone()),
                                                allow_shell_escape: server_allow_shell_escape
                                                    && req.allow_shell_escape,
                                                shadow_subdocument: None,
                                                cancel_token: None,
                                            };

                                            let compile_res = CompilerEngine::compile_with_options(
                                                &source_str,
                                                engine,
                                                &options,
                                            )
                                            .ok();
                                            let pdf_bytes = compile_res
                                                .as_ref()
                                                .and_then(|r| r.pdf_bytes.as_deref());

                                            let report =
                                                crate::engine::render_debugger::RenderDebugger::scan(
                                                    &source_str,
                                                    &doc_name,
                                                    pdf_bytes,
                                                );
                                            crate::ui::drawer::record_api_audit(
                                                crate::ui::drawer::AuditActor::ApiRead,
                                                "POST /api/debug",
                                                format!(
                                                    "Ran render audit on '{doc_name}' ({} issue(s) found)",
                                                    report.total_issues
                                                ),
                                                report.issues.first().map(|i| i.line),
                                            );
                                            (200, report.to_json())
                                        }
                                        Err(err_resp) => err_resp,
                                    }
                                }
                                Err(e) => (
                                    400,
                                    serde_json::json!({ "error": format!("Invalid JSON: {e}") }).to_string(),
                                ),
                            }
                        }
                        ("GET", "/api/dependencies") => {
                            let py_env = PackageManager::check_python_environment();
                            let deps = PackageManager::get_known_dependencies();
                            crate::ui::drawer::record_api_audit(
                                crate::ui::drawer::AuditActor::ApiRead,
                                "GET /api/dependencies",
                                format!("Inspected {} external tool dependencies", deps.len()),
                                None,
                            );
                            (
                                200,
                                serde_json::json!({
                                    "python_environment": py_env,
                                    "dependencies": deps,
                                })
                                .to_string(),
                            )
                        }
                        ("POST", "/api/dependencies/check") => {
                            let body_str = read_json_body(&mut request);

                            match serde_json::from_str::<SourceApiRequest>(&body_str) {
                                Ok(req) => {
                                    match resolve_readonly_source(req.source, &shared_state) {
                                        Ok((source_str, doc_name, _)) => {
                                            let py_env = PackageManager::check_python_environment();
                                            let deps =
                                                PackageManager::detect_dependencies(&source_str);
                                            crate::ui::drawer::record_api_audit(
                                                crate::ui::drawer::AuditActor::ApiRead,
                                                "POST /api/dependencies/check",
                                                format!(
                                                    "Checked external dependencies for '{doc_name}' ({} required)",
                                                    deps.len()
                                                ),
                                                None,
                                            );
                                            (
                                                200,
                                                serde_json::json!({
                                                    "python_environment": py_env,
                                                    "dependencies": deps,
                                                })
                                                .to_string(),
                                            )
                                        }
                                        Err(err_resp) => err_resp,
                                    }
                                }
                                Err(e) => (
                                    400,
                                    serde_json::json!({ "error": format!("Invalid JSON: {e}") }).to_string(),
                                ),
                            }
                        }
                        ("POST", "/api/dependencies/install") => {
                            let mut body_str = String::new();
                            let _ = request.as_reader().take(10 * 1024 * 1024).read_to_string(&mut body_str);

                            match serde_json::from_str::<InstallDependencyApiRequest>(&body_str) {
                                Ok(req) => {
                                    crate::ui::drawer::record_api_audit(
                                        crate::ui::drawer::AuditActor::ApiMutation,
                                        "POST /api/dependencies/install",
                                        format!(
                                            "Requested dependency installation for '{}' (consent={})",
                                            req.package, req.consent
                                        ),
                                        None,
                                    );
                                    if !req.consent {
                                        (
                                            403,
                                            serde_json::json!({
                                                "success": false,
                                                "error": "Explicit user consent is required to install dependencies (set 'consent': true)."
                                            })
                                            .to_string(),
                                        )
                                    } else {
                                        match PackageManager::install_dependency(&req.package, true) {
                                            Ok(msg) => (
                                                200,
                                                serde_json::json!({
                                                    "success": true,
                                                    "package": req.package,
                                                    "output": msg
                                                })
                                                .to_string(),
                                            ),
                                            Err(e) => (
                                                500,
                                                serde_json::json!({
                                                    "success": false,
                                                    "error": e.to_string()
                                                })
                                                .to_string(),
                                            ),
                                        }
                                    }
                                }
                                Err(e) => (
                                    400,
                                    serde_json::json!({ "error": format!("Invalid JSON: {e}") }).to_string(),
                                ),
                            }
                        }
                        ("GET", "/api/docs") => {
                            let docs = crate::document::BuiltinDoc::all();
                            let items: Vec<serde_json::Value> = docs
                                .iter()
                                .map(|d| {
                                    serde_json::json!({
                                        "kind": format!("{:?}", d.kind),
                                        "title": d.title,
                                        "filename": d.filename,
                                        "description": d.description,
                                        "lines": d.content.lines().count(),
                                    })
                                })
                                .collect();
                            (200, serde_json::json!({ "docs": items }).to_string())
                        }
                        ("POST", "/api/docs") => {
                            let mut body_str = String::new();
                            let _ = request.as_reader().take(10 * 1024 * 1024).read_to_string(&mut body_str);

                            #[derive(Deserialize)]
                            struct DocReq {
                                doc: String,
                            }

                            match serde_json::from_str::<DocReq>(&body_str) {
                                Ok(req) => {
                                    let docs = crate::document::BuiltinDoc::all();
                                    let found = docs.iter().find(|d| {
                                        let k = format!("{:?}", d.kind).to_lowercase();
                                        k == req.doc.to_lowercase()
                                            || d.filename.to_lowercase() == req.doc.to_lowercase()
                                            || d.filename.to_lowercase().starts_with(&req.doc.to_lowercase())
                                    });

                                    if let Some(d) = found {
                                        (
                                            200,
                                            serde_json::json!({
                                                "kind": format!("{:?}", d.kind),
                                                "title": d.title,
                                                "filename": d.filename,
                                                "description": d.description,
                                                "content": d.content
                                            })
                                            .to_string(),
                                        )
                                    } else {
                                        (
                                            404,
                                            serde_json::json!({
                                                "error": format!("Document '{}' not found", req.doc)
                                            })
                                            .to_string(),
                                        )
                                    }
                                }
                                Err(e) => (
                                    400,
                                    serde_json::json!({ "error": format!("Invalid JSON: {e}") }).to_string(),
                                ),
                            }
                        }
                        _ => (
                            404,
                            serde_json::json!({
                                "error": "Endpoint not found",
                                "help_endpoint": "GET /api/help",
                                "api_spec_endpoint": "GET /api/spec",
                                "cli_spec_command": "laketex api-spec"
                            })
                            .to_string(),
                        ),
                    };

                    let content_type = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
                    let response = Response::from_string(body)
                        .with_status_code(status)
                        .with_header(content_type);
                    let _ = request.respond(response);
                }
            }
        });

        Ok(running)
    }

    /// Runs the HTTP REST API server on the calling thread, blocking until interrupted (e.g. via `Ctrl+C`).
    pub fn run_blocking(&self, host: &str, port: u16) -> std::io::Result<()> {
        self.run_blocking_with_options(host, port, false)
    }

    /// Runs the HTTP REST API server on the calling thread with explicit shell-escape policy.
    pub fn run_blocking_with_options(
        &self,
        host: &str,
        port: u16,
        server_allow_shell_escape: bool,
    ) -> std::io::Result<()> {
        let address = format!("{host}:{port}");
        let _handle = Self::start_background_with_options(host, port, server_allow_shell_escape)?;
        println!("[Laketex Headless Daemon] Running on http://{address}. Press Ctrl+C to stop.");
        loop {
            thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    /// JSON endpoint schema for the REST API.
    pub fn agent_api_spec_json(
        port: u16,
        runtime_stateful: Option<bool>,
        runtime_allow_shell_escape: Option<bool>,
    ) -> serde_json::Value {
        let base_url = format!("http://127.0.0.1:{port}");
        let endpoints = vec![
            serde_json::json!({
                "id": "health_status",
                "method": "GET",
                "path": "/health",
                "aliases": ["/api/status"],
                "availability": "both_stateless_and_stateful",
                "mutates_gui_state": false,
                "writes_to_disk": "never",
                "purpose": "Probes server liveness, version, whether stateful GUI synchronization (--with-state) is active, and which LaTeX engines are installed on the host.",
                "user_effect": "None (background status check).",
                "request_body": null,
                "response_200": {
                    "status": "\"online\"",
                    "app": "\"Laketex\"",
                    "version": "string (e.g. \"0.2.3\")",
                    "stateful": "boolean (true only when launched with --with-api --with-state)",
                    "api_spec_endpoint": "\"/api/spec\"",
                    "help_endpoint": "\"/api/help\"",
                    "api_spec_cli_command": "\"laketex api-spec\"",
                    "engines": "[{\"kind\": \"Native|Tectonic|PdfLatex|XeLatex|LuaLatex\", \"available\": bool, \"version_info\": \"string|null\"}]"
                }
            }),
            serde_json::json!({
                "id": "api_help",
                "method": "GET",
                "path": "/api/help",
                "aliases": ["/help", "/"],
                "availability": "both_stateless_and_stateful",
                "mutates_gui_state": false,
                "writes_to_disk": "never",
                "purpose": "Returns a concise endpoint directory, launch flags, and pointers to GET /api/spec and `laketex api-spec`.",
                "user_effect": "None (logged in Console Drawer Audit Log).",
                "request_body": null,
                "response_200": "Help summary object listing all endpoints and CLI commands."
            }),
            serde_json::json!({
                "id": "agent_api_spec",
                "method": "GET",
                "path": "/api/spec",
                "aliases": ["/api/schema", "/api/agent"],
                "availability": "both_stateless_and_stateful",
                "mutates_gui_state": false,
                "writes_to_disk": "never",
                "purpose": "Returns this machine-parseable JSON specification of all endpoints, schemas, security rules, and agent workflows (identical to CLI `laketex api-spec`).",
                "user_effect": "None (logged in Console Drawer Audit Log).",
                "request_body": null,
                "response_200": "This specification object."
            }),
            serde_json::json!({
                "id": "get_live_document",
                "method": "GET",
                "path": "/api/document",
                "aliases": [],
                "availability": "stateful_only (--with-api --with-state)",
                "mutates_gui_state": false,
                "writes_to_disk": "never",
                "purpose": "Reads the complete snapshot of the document currently open in the user's GUI editor.",
                "user_effect": "None (non-intrusive read of the live editor state).",
                "request_body": null,
                "response_200": {
                    "source": "string (full LaTeX buffer in editor)",
                    "file_name": "string (e.g. \"main.tex\" or \"untitled.tex\")",
                    "file_path": "string|null (absolute disk path if bound to a file)",
                    "working_dir": "string (active project root / compilation working directory)",
                    "master_document": "string|null (configured master .tex path for multi-file projects)",
                    "is_dirty": "boolean (true if buffer has unsaved edits)",
                    "line_count": "usize",
                    "word_count": "usize",
                    "char_count": "usize",
                    "engine": "\"Native\" | \"Tectonic\" | \"PdfLatex\" | \"XeLatex\" | \"LuaLatex\"",
                    "view_mode": "\"NativeVector\" | \"PdfRaster\"",
                    "current_page": "usize (1-based)",
                    "zoom": "f32 (e.g. 1.0 = 100%)",
                    "diagnostics": "Array<Diagnostic>",
                    "last_duration_ms": "u128|null",
                    "revision": "u64 (monotonic state counter)"
                },
                "errors": {
                    "400": "Returned if server was started without --with-state."
                }
            }),
            serde_json::json!({
                "id": "update_live_document",
                "method": "POST",
                "path": "/api/document",
                "aliases": ["PUT /api/document", "PATCH /api/document"],
                "availability": "stateful_only (--with-api --with-state)",
                "mutates_gui_state": true,
                "writes_to_disk": "only_when_save_true",
                "purpose": "Replaces or surgically edits the LaTeX document open in the GUI editor, optionally jumping the caret to a target line, switching engines/view modes, triggering live recompilation, or saving to disk.",
                "user_effect": "Immediately updates the code in the user's GUI editor, refreshes the live vector/PDF preview, moves the editor scroll/caret if 'jump_to_line' is set, and marks the document dirty ('●') unless 'save': true is passed.",
                "request_body": {
                    "source": "string (optional — full replacement LaTeX source; if omitted, edits apply to current GUI buffer)",
                    "edits": "Array<DocumentEditOperation> (optional — applied sequentially after 'source' if both provided). Each item is EITHER SearchReplace: {\"search\": \"string (non-empty)\", \"replace\": \"string\", \"all\": bool (default false)} OR LineRange: {\"start_line\": usize (1-based inclusive), \"end_line\": usize (1-based inclusive), \"content\": \"string\"}.",
                    "save": "boolean (optional, default false — if true, writes the updated buffer to its bound 'file_path' on disk and clears 'is_dirty'; returns 400 if document is untitled/unbound)",
                    "compile": "boolean (optional, defaults to true whenever 'source' or 'edits' is provided — triggers live GUI layout & background compilation)",
                    "engine": "string (optional — \"native\", \"tectonic\", \"pdflatex\", \"xelatex\", \"lualatex\")",
                    "jump_to_line": "usize (optional — 1-based line number to scroll and move the editor caret to)",
                    "view_mode": "string (optional — \"NativeVector\" / \"native\" or \"PdfRaster\" / \"pdf\")"
                },
                "response_200": "Updated LiveEditorSnapshot JSON object (same schema as GET /api/document).",
                "errors": {
                    "400": "Returned if not in --with-state mode, if an edit 'search' string is not found, if 'start_line'..'end_line' is out of bounds, or if 'save': true is requested on an untitled document without a bound file_path.",
                    "415": "Returned if Content-Type: application/json header is missing."
                }
            }),
            serde_json::json!({
                "id": "navigate_gui",
                "method": "POST",
                "path": "/api/navigate",
                "aliases": [],
                "availability": "stateful_only (--with-api --with-state)",
                "mutates_gui_state": true,
                "writes_to_disk": "never",
                "purpose": "Controls the user's GUI viewport, editor caret line, active preview page, zoom level, view mode, and collapsible panels without modifying document text.",
                "user_effect": "Scrolls the editor to 'line', switches viewer to 'page'/'view_mode'/'zoom', or opens a specific drawer tab (e.g. 'diagnostics' or 'audit') on the user's screen.",
                "request_body": {
                    "line": "usize (optional — 1-based source line number to jump the editor caret to)",
                    "page": "usize (optional — 1-based page number to display in the viewer)",
                    "view_mode": "string (optional — \"NativeVector\" / \"native\" or \"PdfRaster\" / \"pdf\")",
                    "zoom": "f32 (optional — viewer zoom factor, clamped to 0.25..=4.0)",
                    "show_drawer": "boolean (optional — open or close the bottom console drawer)",
                    "drawer_tab": "string (optional — \"diagnostics\", \"audit\", \"logs\", \"api\", or \"audit_log\"; automatically opens drawer)",
                    "show_sidebar": "boolean (optional — open or close the left project/outline sidebar)"
                },
                "response_200": {
                    "success": true,
                    "snapshot": "Updated LiveEditorSnapshot object"
                }
            }),
            serde_json::json!({
                "id": "compile_document",
                "method": "POST",
                "path": "/api/compile",
                "aliases": [],
                "availability": "both_stateless_and_stateful (altered behavior in --with-state mode)",
                "mutates_gui_state": "true when --with-state is active; false in stateless mode",
                "writes_to_disk": "only_when_save_true in --with-state mode",
                "purpose": "Compiles LaTeX source code using the requested engine and returns Base64 PDF bytes, structured diagnostics, raw logs, and a Markdown error report.",
                "user_effect": "In stateless mode: none. In --with-state mode: if 'source' is provided, replaces the open GUI editor buffer (marking it dirty unless 'save': true); if 'source' is omitted ({}), compiles the currently open GUI editor buffer. In both cases under --with-state, immediately updates the GUI's live PDF/vector preview, diagnostics drawer, and compile timer.",
                "request_body": {
                    "source": "string (required in stateless mode; optional in --with-state mode where omitting it compiles the active GUI editor buffer)",
                    "engine": "string (optional — \"native\", \"tectonic\", \"pdflatex\", \"xelatex\", \"lualatex\"; defaults to \"native\" in stateless mode or the active GUI engine in --with-state mode)",
                    "allow_shell_escape": "boolean (optional, default false — requests -shell-escape; only honored if server also allows shell-escape)",
                    "save": "boolean (optional, default false, --with-state only — if true, saves the compiled buffer to its bound file_path on disk)"
                },
                "response_200": {
                    "success": "boolean",
                    "duration_ms": "u128",
                    "diagnostics": "Array<{severity: \"Error\"|\"Warning\"|\"Info\", message: string, line: usize|null, file: string|null, details: string|null}>",
                    "pdf_base64": "string|null (Base64-encoded PDF binary)",
                    "stdout": "string",
                    "stderr": "string",
                    "error_report_markdown": "string|null (present when errors or warnings occur)"
                }
            }),
            serde_json::json!({
                "id": "check_syntax",
                "method": "POST",
                "path": "/api/check",
                "aliases": [],
                "availability": "both_stateless_and_stateful",
                "mutates_gui_state": false,
                "writes_to_disk": "never",
                "purpose": "Runs a non-mutating Tectonic lint/syntax check (with shell-escape strictly disabled) and returns structured diagnostics plus a contextual ErrorReport.",
                "user_effect": "None (even in --with-state mode, passing a custom 'source' to /api/check does NOT overwrite the user's open editor buffer).",
                "request_body": {
                    "source": "string (required in stateless mode; optional in --with-state mode where omitting it checks the active GUI editor buffer)"
                },
                "response_200": {
                    "success": "boolean",
                    "diagnostics": "Array<Diagnostic>",
                    "duration_ms": "u128",
                    "error_report_markdown": "string",
                    "error_report": "Structured ErrorReport JSON object"
                }
            }),
            serde_json::json!({
                "id": "render_debug_audit",
                "method": "POST",
                "path": "/api/debug",
                "aliases": ["/debug"],
                "availability": "both_stateless_and_stateful",
                "mutates_gui_state": false,
                "writes_to_disk": "never",
                "purpose": "Runs the RenderDebugger static and PDF preview analyzer to detect unrendered LaTeX commands, matrix column mismatches, math-outside-math-mode bugs, and missing package declarations.",
                "user_effect": "None (non-mutating inspection).",
                "request_body": {
                    "source": "string (required in stateless mode; optional in --with-state mode where omitting it audits the active GUI editor buffer)",
                    "engine": "string (optional — \"native\" or \"tectonic\", default \"native\")",
                    "allow_shell_escape": "boolean (optional, default false)"
                },
                "response_200": "RenderAuditReport JSON object containing total_issues, error_count, warning_count, matrix_issue_count, math_issue_count, pdf_inspected, pdf_page_count, and issues[]."
            }),
            serde_json::json!({
                "id": "extract_outline",
                "method": "POST",
                "path": "/api/outline",
                "aliases": [],
                "availability": "both_stateless_and_stateful",
                "mutates_gui_state": false,
                "writes_to_disk": "never",
                "purpose": "Extracts the hierarchical section/subsection/equation/table outline with 1-based line numbers.",
                "user_effect": "None (non-mutating inspection).",
                "request_body": {
                    "source": "string (required in stateless mode; optional in --with-state mode where omitting it parses the active GUI editor buffer)"
                },
                "response_200": {
                    "outline": "Array<{title: string, level: u8, line: usize}>"
                }
            }),
            serde_json::json!({
                "id": "detect_packages",
                "method": "POST",
                "path": "/api/packages",
                "aliases": [],
                "availability": "both_stateless_and_stateful",
                "mutates_gui_state": false,
                "writes_to_disk": "never",
                "purpose": "Parses all \\usepackage[options]{name} declarations and enriches them with built-in CTAN package descriptions.",
                "user_effect": "None (non-mutating inspection).",
                "request_body": {
                    "source": "string (required in stateless mode; optional in --with-state mode where omitting it inspects the active GUI editor buffer)"
                },
                "response_200": {
                    "packages": "Array<{name: string, options: string|null, line: usize, description: string}>"
                }
            }),
            serde_json::json!({
                "id": "get_dependencies",
                "method": "GET",
                "path": "/api/dependencies",
                "aliases": [],
                "availability": "both_stateless_and_stateful",
                "mutates_gui_state": false,
                "writes_to_disk": "never",
                "purpose": "Detects the host Python/Conda/venv environment and checks installation status of known external LaTeX tool dependencies (e.g. Pygments/pygmentize for minted).",
                "user_effect": "None.",
                "request_body": null,
                "response_200": {
                    "python_environment": "PythonEnvironmentInfo object",
                    "dependencies": "Array<PackageDependency>"
                }
            }),
            serde_json::json!({
                "id": "check_source_dependencies",
                "method": "POST",
                "path": "/api/dependencies/check",
                "aliases": [],
                "availability": "both_stateless_and_stateful",
                "mutates_gui_state": false,
                "writes_to_disk": "never",
                "purpose": "Scans LaTeX source code for packages requiring external tools (e.g. minted -> pygmentize) and reports whether each required tool is installed.",
                "user_effect": "None.",
                "request_body": {
                    "source": "string (required in stateless mode; optional in --with-state mode)"
                },
                "response_200": {
                    "python_environment": "PythonEnvironmentInfo object",
                    "dependencies": "Array<PackageDependency>"
                }
            }),
            serde_json::json!({
                "id": "install_dependency",
                "method": "POST",
                "path": "/api/dependencies/install",
                "aliases": [],
                "availability": "both_stateless_and_stateful",
                "mutates_gui_state": false,
                "writes_to_disk": "installs whitelisted Python package into active Python/Conda environment",
                "purpose": "Installs a whitelisted external dependency (currently 'pygments') via pip/conda when explicit user consent ('consent': true) is provided.",
                "user_effect": "Installs Pygments so minted code blocks can be highlighted.",
                "request_body": {
                    "package": "\"pygments\" (strictly whitelisted)",
                    "consent": "boolean (MUST be true; false returns 403 Forbidden)"
                },
                "response_200": {
                    "success": true,
                    "package": "string",
                    "output": "string"
                }
            }),
            serde_json::json!({
                "id": "list_builtin_docs",
                "method": "GET",
                "path": "/api/docs",
                "aliases": [],
                "availability": "both_stateless_and_stateful",
                "mutates_gui_state": false,
                "writes_to_disk": "never",
                "purpose": "Lists the offline LaTeX documentation manuals bundled inside the Laketex binary (manual, theming, api, architecture).",
                "user_effect": "None.",
                "request_body": null,
                "response_200": {
                    "docs": "Array<{kind: string, title: string, filename: string, description: string, lines: usize}>"
                }
            }),
            serde_json::json!({
                "id": "fetch_builtin_doc",
                "method": "POST",
                "path": "/api/docs",
                "aliases": [],
                "availability": "both_stateless_and_stateful",
                "mutates_gui_state": false,
                "writes_to_disk": "never",
                "purpose": "Retrieves the full LaTeX source content of a specific built-in documentation manual.",
                "user_effect": "None.",
                "request_body": {
                    "doc": "\"manual\" | \"theming\" | \"api\" | \"architecture\""
                },
                "response_200": {
                    "kind": "string",
                    "title": "string",
                    "filename": "string",
                    "description": "string",
                    "content": "string (full LaTeX source)"
                }
            }),
        ];

        serde_json::json!({
            "schema_version": "1.0.0",
            "app": "Laketex",
            "app_version": env!("CARGO_PKG_VERSION"),
            "default_base_url": base_url,
            "runtime_state": {
                "is_live_server_response": runtime_stateful.is_some(),
                "stateful_mode_active": runtime_stateful,
                "server_allow_shell_escape": runtime_allow_shell_escape,
            },
            "architecture_and_operating_modes": {
                "summary": "Laketex is a dual-engine LaTeX IDE and headless compilation daemon in Rust. It provides both a pure-Rust instant layout/PDF engine ('native', <5ms) and an embedded zero-setup Tectonic XeTeX engine ('tectonic'), plus external TeX Live/MiKTeX engine support ('pdflatex', 'xelatex', 'lualatex').",
                "modes": {
                    "gui_default_no_api": {
                        "launch_command": "laketex [FILE]",
                        "behavior": "Runs the desktop GUI with the HTTP REST API server completely disabled ('REST API: Off')."
                    },
                    "stateless_api": {
                        "launch_commands": [
                            "laketex serve [--host 127.0.0.1] [--port 3030] [--allow-shell-escape]",
                            "laketex --with-api [--api-port 3030] [FILE]"
                        ],
                        "behavior": "Runs the HTTP REST API server in stateless mode. Every endpoint operates exclusively on the 'source' string provided in the request JSON payload. Calls do not read or modify any open GUI editor buffer, and 'source' is strictly required on all compilation/analysis endpoints."
                    },
                    "stateful_live_gui_api": {
                        "launch_command": "laketex --with-api --with-state [--api-port 3030] [FILE]",
                        "behavior": "Couples the HTTP REST API server directly to the live desktop GUI editor and preview canvas via a lock-free/RwLock SharedAppState bridge and egui::Context::request_repaint() wakeup. Mutating calls (POST /api/document, POST /api/compile, POST /api/navigate) immediately update what the human user sees on screen (editor text buffer, dirty indicator '●', native vector preview, PDF raster preview, diagnostics drawer, and caret line position). Read/lint calls (POST /api/check, /api/outline, /api/packages, /api/debug, /api/dependencies/check) default to inspecting the user's currently open editor buffer when 'source' is omitted ({}), or inspect a provided 'source' string without overwriting the user's open editor buffer."
                    }
                }
            },
            "protocol_and_security_invariants": {
                "required_headers_for_mutations": {
                    "Content-Type": "application/json",
                    "enforcement": "All POST, PUT, and PATCH requests MUST include 'Content-Type: application/json'. Omitting it returns HTTP 415 Unsupported Media Type (CSRF protection)."
                },
                "host_and_origin_validation": {
                    "allowed_hosts": ["127.0.0.1", "localhost", "[::1]", "::1"],
                    "enforcement": "Requests with external Host, Origin, or Referer headers (e.g. 'localhost.evil.com') are rejected with HTTP 403 Forbidden (DNS rebinding and cross-origin browser protection)."
                },
                "empty_body_handling": "On POST endpoints, an empty body ('') is automatically normalized to '{}' before JSON deserialization.",
                "max_payload_bytes": 10485760,
                "shell_escape_policy": "Disabled by default to prevent arbitrary command execution from untrusted LaTeX sources (e.g. \\write18). When disabled, documents using '\\usepackage{minted}' automatically inject '\\PassOptionsToPackage{draft}{minted}' so compilation still succeeds. Real pygmentize shell-escape only runs if BOTH the server permits it (--allow-shell-escape or GUI Settings 'allow_shell_escape: true') AND the request sets '\"allow_shell_escape\": true'. /api/check never enables shell-escape."
            },
            "recommended_agent_workflows": [
                {
                    "workflow": "live_collaborative_editing_in_gui",
                    "precondition": "Server running with --with-api --with-state (verify via GET /health -> 'stateful': true).",
                    "steps": [
                        "1. Call GET /api/document to read the current 'source', 'file_name', 'file_path', 'is_dirty', 'engine', 'diagnostics', and 'revision'.",
                        "2. Apply targeted modifications via POST /api/document using 'edits' (LineRange or SearchReplace) and set 'jump_to_line' to the edited region. Leave 'save': false while iterating so the user sees live changes in the editor and preview with the dirty indicator ('●') while their disk file remains untouched.",
                        "3. Call POST /api/debug with '{}' (and/or POST /api/check with '{}') to audit the active buffer for unrendered math commands, matrix column mismatches, or TeX syntax errors without overwriting the editor.",
                        "4. Once verified clean, call POST /api/compile with '{\"save\": true}' (or POST /api/document with '{\"save\": true}') if the user requested saving to disk."
                    ]
                },
                {
                    "workflow": "stateless_batch_verification_and_rendering",
                    "precondition": "Any API mode (stateless or stateful).",
                    "steps": [
                        "1. Call POST /api/debug with '{\"source\": \"...\", \"engine\": \"native\"}' to detect structural math/matrix/environment issues.",
                        "2. Call POST /api/check with '{\"source\": \"...\"}' to run full Tectonic XeTeX linting and receive structured line-level diagnostics plus a contextual Markdown error report.",
                        "3. Call POST /api/compile with '{\"source\": \"...\", \"engine\": \"tectonic\"}' to generate Base64-encoded PDF bytes ('pdf_base64')."
                    ]
                }
            ],
            "endpoints": endpoints
        })
    }
}

fn read_json_body(request: &mut tiny_http::Request) -> String {
    let mut body_str = String::new();
    let _ = request
        .as_reader()
        .take(10 * 1024 * 1024)
        .read_to_string(&mut body_str);
    if body_str.trim().is_empty() {
        "{}".to_string()
    } else {
        body_str
    }
}

fn parse_engine_str(s: &str) -> EngineKind {
    match s.trim().to_ascii_lowercase().as_str() {
        "tectonic" => EngineKind::Tectonic,
        "pdflatex" => EngineKind::PdfLatex,
        "xelatex" => EngineKind::XeLatex,
        "lualatex" => EngineKind::LuaLatex,
        _ => EngineKind::Native,
    }
}

fn normalize_view_mode_label(s: &str) -> &'static str {
    match s.trim().to_ascii_lowercase().as_str() {
        "pdf" | "raster" | "pdfraster" => "PdfRaster",
        _ => "NativeVector",
    }
}

fn resolve_readonly_source(
    req_source: Option<String>,
    shared_state: &Option<Arc<SharedAppState>>,
) -> Result<(String, String, Option<PathBuf>), (u16, String)> {
    if let Some(src) = req_source {
        if let Some(shared) = shared_state {
            let snap = shared.snapshot();
            Ok((src, snap.file_name, Some(snap.working_dir)))
        } else {
            Ok((src, "api_document.tex".to_string(), None))
        }
    } else if let Some(shared) = shared_state {
        let snap = shared.snapshot();
        Ok((snap.source, snap.file_name, Some(snap.working_dir)))
    } else {
        Err((
            400,
            serde_json::json!({
                "error": "Missing required field 'source' (or launch with --with-api --with-state to inspect the active editor buffer)"
            })
            .to_string(),
        ))
    }
}

fn extract_authority_host(input: &str) -> &str {
    let s = input.trim();
    let after_scheme = if let Some(idx) = s.find("://") {
        &s[idx + 3..]
    } else {
        s
    };
    let authority = after_scheme.split('/').next().unwrap_or("");
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    if authority.starts_with('[') {
        if let Some(end_bracket) = authority.find(']') {
            return &authority[..=end_bracket];
        }
        authority
    } else {
        authority.split(':').next().unwrap_or("")
    }
}

fn is_allowed_host(host: &str, bind_host: &str) -> bool {
    host.eq_ignore_ascii_case("127.0.0.1")
        || host.eq_ignore_ascii_case("localhost")
        || host.eq_ignore_ascii_case("[::1]")
        || host.eq_ignore_ascii_case("::1")
        || (!bind_host.is_empty()
            && bind_host != "0.0.0.0"
            && host.eq_ignore_ascii_case(bind_host))
}

/// Validates incoming HTTP requests to protect against Cross-Origin (CORS/CSRF) attacks
/// and DNS rebinding attacks from web browsers.
fn validate_request_security(
    request: &tiny_http::Request,
    bind_host: &str,
) -> std::result::Result<(), (u16, &'static str)> {
    let mut has_json_content_type = false;

    for header in request.headers() {
        let field = header.field.as_str().as_str();
        let value = header.value.as_str();

        // 1. Origin header check: browsers send this on cross-origin requests
        if field.eq_ignore_ascii_case("origin") {
            let host = extract_authority_host(value);
            if !is_allowed_host(host, bind_host) {
                return Err((403, "Forbidden: Cross-origin browser requests are blocked"));
            }
        }

        // 2. Referer header check: if present, must originate from allowed host
        if field.eq_ignore_ascii_case("referer") {
            let host = extract_authority_host(value);
            if !is_allowed_host(host, bind_host) {
                return Err((403, "Forbidden: Untrusted referer header blocked"));
            }
        }

        // 3. Host header check: DNS rebinding protection
        if field.eq_ignore_ascii_case("host") {
            let host = extract_authority_host(value);
            if !is_allowed_host(host, bind_host) {
                return Err((403, "Forbidden: Host header validation failed (DNS rebinding protection)"));
            }
        }

        // 4. Content-Type check for CSRF protection on POST requests
        if field.eq_ignore_ascii_case("content-type") {
            let media_type = value.split(';').next().unwrap_or("").trim();
            if media_type.eq_ignore_ascii_case("application/json") {
                has_json_content_type = true;
            }
        }
    }

    let method_str = request.method().as_str();
    if (method_str.eq_ignore_ascii_case("POST")
        || method_str.eq_ignore_ascii_case("PUT")
        || method_str.eq_ignore_ascii_case("PATCH"))
        && !has_json_content_type
    {
        return Err((
            415,
            "Unsupported Media Type: POST/PUT/PATCH requests require Content-Type: application/json",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_authority_host_and_allowed_host() {
        assert_eq!(extract_authority_host("http://127.0.0.1:3030/api"), "127.0.0.1");
        assert_eq!(extract_authority_host("http://localhost:3030"), "localhost");
        assert_eq!(extract_authority_host("http://[::1]:3030/health"), "[::1]");
        assert_eq!(extract_authority_host("[::1]:3030"), "[::1]");
        assert_eq!(
            extract_authority_host("http://localhost.evil.com:3030/attack"),
            "localhost.evil.com"
        );
        assert_eq!(
            extract_authority_host("http://127.0.0.1.attacker.com"),
            "127.0.0.1.attacker.com"
        );

        assert!(is_allowed_host(extract_authority_host("http://127.0.0.1:3030"), "127.0.0.1"));
        assert!(is_allowed_host(extract_authority_host("http://localhost:3030"), "127.0.0.1"));
        assert!(is_allowed_host(extract_authority_host("http://[::1]:3030"), "127.0.0.1"));
        assert!(!is_allowed_host(
            extract_authority_host("http://localhost.evil.com:3030"),
            "127.0.0.1"
        ));
        assert!(!is_allowed_host(
            extract_authority_host("http://127.0.0.1.attacker.com:3030"),
            "127.0.0.1"
        ));
    }

    #[test]
    fn test_apply_edit_operations_search_replace_and_line_range() {
        let initial = "\\documentclass{article}\n\\begin{document}\n\\section{Intro}\nOld text\n\\end{document}";
        let edits = vec![
            DocumentEditOperation::SearchReplace {
                search: "\\section{Intro}".to_string(),
                replace: "\\section{Introduction}".to_string(),
                all: false,
            },
            DocumentEditOperation::LineRange {
                start_line: 4,
                end_line: 4,
                content: "Updated line 1\nUpdated line 2".to_string(),
            },
        ];

        let out = apply_edit_operations(initial, &edits).expect("edits should succeed");
        assert_eq!(
            out,
            "\\documentclass{article}\n\\begin{document}\n\\section{Introduction}\nUpdated line 1\nUpdated line 2\n\\end{document}"
        );

        let bad_edit = vec![DocumentEditOperation::SearchReplace {
            search: "Nonexistent string".to_string(),
            replace: "x".to_string(),
            all: false,
        }];
        assert!(apply_edit_operations(initial, &bad_edit).is_err());
    }

    #[test]
    fn test_agent_api_spec_completeness() {
        let spec = HeadlessServer::agent_api_spec_json(3030, Some(true), Some(false));
        assert_eq!(spec["app"], "Laketex");
        assert_eq!(spec["runtime_state"]["stateful_mode_active"], true);
        let endpoints = spec["endpoints"].as_array().expect("endpoints array");
        assert!(endpoints.len() >= 14);
        assert!(endpoints.iter().any(|e| e["path"] == "/api/document" && e["method"] == "GET"));
        assert!(endpoints.iter().any(|e| e["path"] == "/api/document" && e["method"] == "POST"));
        assert!(endpoints.iter().any(|e| e["path"] == "/api/navigate" && e["method"] == "POST"));
        let nav = endpoints.iter().find(|e| e["path"] == "/api/navigate").expect("navigate endpoint");
        let tab_desc = nav["request_body"]["drawer_tab"].as_str().expect("drawer_tab string");
        assert!(tab_desc.contains("\"audit_log\""), "drawer_tab description must include audit_log");
        assert!(endpoints.iter().any(|e| e["path"] == "/api/compile" && e["method"] == "POST"));
    }
}
