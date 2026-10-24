//! # Headless HTTP REST API Daemon
//!
//! Exposes an embedded, lightweight HTTP web server providing programmatic endpoints
//! for remote LaTeX compilation, syntax linting, outline extraction, and health checks.
//!
//! ## Endpoints
//! - `GET  /health` or `/api/status`: Probes engine availability and returns JSON status
//! - `POST /api/compile`: Compiles LaTeX code, returning base64 PDF bytes and error reports
//! - `POST /api/check`: Fast syntax linting returning structured diagnostics
//! - `POST /api/outline`: Extracts document section hierarchy
//! - `POST /api/packages`: Detects imported packages

use crate::compiler::engine::{CompilerEngine, EngineKind};
use crate::document::outline::OutlineParser;
use crate::document::packages::PackageManager;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tiny_http::{Header, Response, Server};

/// Request payload for `/api/compile`.
#[derive(Debug, Deserialize)]
pub struct CompileApiRequest {
    /// LaTeX document source code
    pub source: String,
    /// Compiler engine backend (native, tectonic, pdflatex, xelatex, lualatex)
    pub engine: Option<String>,
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
#[derive(Debug, Deserialize)]
pub struct SourceApiRequest {
    /// LaTeX document source code
    pub source: String,
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
        let address = format!("{host}:{port}");
        let server = Server::http(&address).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Server start error: {e}"))
        })?;

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        thread::spawn(move || {
            println!("[Laketex Headless API] Listening on http://{address}");

            while running_clone.load(Ordering::SeqCst) {
                if let Ok(Some(mut request)) = server.recv_timeout(std::time::Duration::from_millis(200)) {
                    // Security validation: block CSRF, cross-origin browser requests, and DNS rebinding attacks
                    if let Err((status_code, err_msg)) = validate_request_security(&request) {
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
                            let json = serde_json::json!({
                                "status": "online",
                                "app": "Laketex",
                                "version": "0.1.0",
                                "engines": engines,
                            });
                            (200, json.to_string())
                        }
                        ("POST", "/api/compile") => {
                            let mut body_str = String::new();
                            let _ = request.as_reader().take(10 * 1024 * 1024).read_to_string(&mut body_str);

                            match serde_json::from_str::<CompileApiRequest>(&body_str) {
                                Ok(req) => {
                                    let engine = match req.engine.as_deref() {
                                        Some("tectonic") => EngineKind::Tectonic,
                                        Some("pdflatex") => EngineKind::PdfLatex,
                                        Some("xelatex") => EngineKind::XeLatex,
                                        Some("lualatex") => EngineKind::LuaLatex,
                                        _ => EngineKind::Native,
                                    };

                                    match CompilerEngine::compile(&req.source, engine, None) {
                                        Ok(res) => {
                                            let pdf_b64 = res.pdf_bytes.as_ref().map(|b| BASE64.encode(b));
                                            let report_md = if !res.success || !res.diagnostics.is_empty() {
                                                let r = crate::compiler::ErrorReport::generate(
                                                    &req.source,
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
                                            (200, serde_json::to_string(&response).unwrap_or_default())
                                        }
                                        Err(e) => (
                                            500,
                                            serde_json::json!({ "error": e.to_string() }).to_string(),
                                        ),
                                    }
                                }
                                Err(e) => (
                                    400,
                                    serde_json::json!({ "error": format!("Invalid JSON: {e}") }).to_string(),
                                ),
                            }
                        }
                        ("POST", "/api/outline") => {
                            let mut body_str = String::new();
                            let _ = request.as_reader().take(10 * 1024 * 1024).read_to_string(&mut body_str);

                            match serde_json::from_str::<SourceApiRequest>(&body_str) {
                                Ok(req) => {
                                    let items = OutlineParser::parse(&req.source);
                                    (200, serde_json::json!({ "outline": items }).to_string())
                                }
                                Err(e) => (
                                    400,
                                    serde_json::json!({ "error": format!("Invalid JSON: {e}") }).to_string(),
                                ),
                            }
                        }
                        ("POST", "/api/packages") => {
                            let mut body_str = String::new();
                            let _ = request.as_reader().take(10 * 1024 * 1024).read_to_string(&mut body_str);

                            match serde_json::from_str::<SourceApiRequest>(&body_str) {
                                Ok(req) => {
                                    let pkgs = PackageManager::detect_packages(&req.source);
                                    (200, serde_json::json!({ "packages": pkgs }).to_string())
                                }
                                Err(e) => (
                                    400,
                                    serde_json::json!({ "error": format!("Invalid JSON: {e}") }).to_string(),
                                ),
                            }
                        }
                        ("POST", "/api/check") => {
                            let mut body_str = String::new();
                            let _ = request.as_reader().take(10 * 1024 * 1024).read_to_string(&mut body_str);

                            match serde_json::from_str::<SourceApiRequest>(&body_str) {
                                Ok(req) => {
                                    match CompilerEngine::compile(&req.source, EngineKind::Tectonic, None) {
                                        Ok(res) => {
                                            let report = crate::compiler::ErrorReport::generate(
                                                &req.source,
                                                "api_document.tex",
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
                                                }).to_string(),
                                            )
                                        }
                                        Err(e) => (
                                            500,
                                            serde_json::json!({ "error": e.to_string() }).to_string(),
                                        ),
                                    }
                                }
                                Err(e) => (
                                    400,
                                    serde_json::json!({ "error": format!("Invalid JSON: {e}") }).to_string(),
                                ),
                            }
                        }
                        ("POST", "/api/debug") | ("POST", "/debug") => {
                            let mut body_str = String::new();
                            let _ = request.as_reader().take(10 * 1024 * 1024).read_to_string(&mut body_str);

                            match serde_json::from_str::<CompileApiRequest>(&body_str) {
                                Ok(req) => {
                                    let engine = match req.engine.as_deref() {
                                        Some("tectonic") => EngineKind::Tectonic,
                                        _ => EngineKind::Native,
                                    };

                                    let compile_res = CompilerEngine::compile(&req.source, engine, None).ok();
                                    let pdf_bytes = compile_res.as_ref().and_then(|r| r.pdf_bytes.as_deref());

                                    let report = crate::engine::render_debugger::RenderDebugger::scan(
                                        &req.source,
                                        "api_document.tex",
                                        pdf_bytes,
                                    );
                                    (200, report.to_json())
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
                            let mut body_str = String::new();
                            let _ = request.as_reader().take(10 * 1024 * 1024).read_to_string(&mut body_str);

                            match serde_json::from_str::<SourceApiRequest>(&body_str) {
                                Ok(req) => {
                                    let py_env = PackageManager::check_python_environment();
                                    let deps = PackageManager::detect_dependencies(&req.source);
                                    (
                                        200,
                                        serde_json::json!({
                                            "python_environment": py_env,
                                            "dependencies": deps,
                                        })
                                        .to_string(),
                                    )
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
                            serde_json::json!({ "error": "Endpoint not found" }).to_string(),
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
        let address = format!("{host}:{port}");
        let _handle = Self::start_background(host, port)?;
        println!("[Laketex Headless Daemon] Running on http://{address}. Press Ctrl+C to stop.");
        loop {
            thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}

/// Validates incoming HTTP requests to protect against Cross-Origin (CORS/CSRF) attacks
/// and DNS rebinding attacks from web browsers.
///
/// # Arguments
/// * `request` - The incoming HTTP request to inspect.
///
/// # Returns
/// * `Ok(())` if the request satisfies all security invariants.
/// * `Err((status_code, error_message))` if the request violates security policy.
///
/// # Security Policy
/// - Rejects cross-origin browser requests with external `Origin` headers (`403 Forbidden`).
/// - Rejects requests containing external `Referer` headers (`403 Forbidden`).
/// - Enforces that `Host` headers point exclusively to local loopback interfaces (`127.0.0.1`,
///   `localhost`, `[::1]`) to mitigate DNS rebinding exploits.
fn validate_request_security(request: &tiny_http::Request) -> std::result::Result<(), (u16, &'static str)> {
    for header in request.headers() {
        let field = header.field.as_str().as_str();
        let value = header.value.as_str();

        // 1. Origin header check: browsers send this on cross-origin requests
        if field.eq_ignore_ascii_case("origin") {
            let val_lower = value.to_ascii_lowercase();
            let is_local = val_lower.starts_with("http://127.0.0.1")
                || val_lower.starts_with("http://localhost")
                || val_lower.starts_with("http://[::1]");
            if !is_local {
                return Err((403, "Forbidden: Cross-origin browser requests are blocked"));
            }
        }

        // 2. Referer header check: if present, must originate from localhost
        if field.eq_ignore_ascii_case("referer") {
            let val_lower = value.to_ascii_lowercase();
            let is_local = val_lower.starts_with("http://127.0.0.1")
                || val_lower.starts_with("http://localhost")
                || val_lower.starts_with("http://[::1]");
            if !is_local {
                return Err((403, "Forbidden: Untrusted referer header blocked"));
            }
        }

        // 3. Host header check: DNS rebinding protection
        if field.eq_ignore_ascii_case("host") {
            let host_lower = value.to_ascii_lowercase();
            let host_part = host_lower.split(':').next().unwrap_or("");
            let is_valid_host = host_part == "127.0.0.1" || host_part == "localhost" || host_part == "[::1]";
            if !is_valid_host {
                return Err((403, "Forbidden: Host header validation failed (DNS rebinding protection)"));
            }
        }
    }
    Ok(())
}
