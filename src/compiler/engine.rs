//! # Compiler Engine Abstraction & Execution
//!
//! Provides a unified interface ([`CompilerEngine`]) for compiling LaTeX documents across
//! multiple backend engines:
//! - Pure-Rust Native typesetter (instant, portable, zero-install)
//! - Tectonic (embedded TeX engine with automatic CTAN downloading)
//! - Traditional TeX distributions (pdfLaTeX, XeLaTeX, LuaLaTeX)

use crate::compiler::diagnostic::{Diagnostic, DiagnosticParser};
use crate::engine::layout::DocumentLayout;
use crate::engine::parser::NativeLatexParser;
use crate::engine::pdf_exporter::NativePdfExporter;
use crate::error::{LakeTexError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// Supported LaTeX compiler backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineKind {
    /// Pure Rust Native Engine (Zero-Install, Instant, Built-in)
    Native,
    /// Tectonic engine with automated CTAN resource downloading
    Tectonic,
    /// Standard pdfLaTeX executable from system TeX Live or MiKTeX
    PdfLatex,
    /// XeLaTeX executable with native system font support
    XeLatex,
    /// LuaLaTeX executable with Lua scripting extensions
    LuaLatex,
}

impl Default for EngineKind {
    fn default() -> Self {
        Self::Native
    }
}

impl EngineKind {
    /// Returns the human-readable display label shown in UI selectors
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Native => "Native Rust (Zero-Install, Instant)",
            Self::Tectonic => "Tectonic (Auto-CTAN)",
            Self::PdfLatex => "pdfLaTeX",
            Self::XeLatex => "XeLaTeX",
            Self::LuaLatex => "LuaLaTeX",
        }
    }

    /// Returns the standard executable command name on disk
    pub fn command_name(&self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Tectonic => "tectonic",
            Self::PdfLatex => "pdflatex",
            Self::XeLatex => "xelatex",
            Self::LuaLatex => "lualatex",
        }
    }
}

/// Metadata and detection status for a compiler backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineInfo {
    /// Engine kind
    pub kind: EngineKind,
    /// Whether this engine is installed and executable on the local system
    pub available: bool,
    /// Absolute path to the detected executable on disk
    pub path: Option<PathBuf>,
    /// Version string reported by `--version`
    pub version: Option<String>,
}

/// Result of a completed LaTeX compilation invocation.
#[derive(Debug, Clone)]
pub struct CompileResult {
    /// True if compilation succeeded with exit code 0 and generated valid PDF output
    pub success: bool,
    /// Raw generated PDF byte buffer
    pub pdf_bytes: Option<Vec<u8>>,
    /// Standard output stream captured from the compiler
    pub stdout: String,
    /// Standard error stream captured from the compiler
    pub stderr: String,
    /// Parsed compiler warnings and errors
    pub diagnostics: Vec<Diagnostic>,
    /// Elapsed wall-clock compilation duration in milliseconds
    pub duration_ms: u128,
}

/// Compilation orchestrator that detects, locates, and invokes LaTeX engines.
pub struct CompilerEngine;

impl CompilerEngine {
    /// Detects available engines on the local system by probing PATH and application cache directories.
    pub fn detect_engines() -> Vec<EngineInfo> {
        let mut engines = vec![EngineInfo {
            kind: EngineKind::Native,
            available: true,
            path: None,
            version: Some("Laketex Native Typesetter 0.2.0 (Pure Rust)".to_string()),
        }];

        let kinds = [
            EngineKind::Tectonic,
            EngineKind::PdfLatex,
            EngineKind::XeLatex,
            EngineKind::LuaLatex,
        ];

        for kind in kinds {
            let (available, path, version) = Self::probe_engine(kind);
            engines.push(EngineInfo {
                kind,
                available,
                path,
                version,
            });
        }

        engines
    }

    /// Finds candidate paths for an external engine binary on disk.
    pub fn find_engine_path(kind: EngineKind) -> Option<PathBuf> {
        if kind == EngineKind::Native {
            return None;
        }

        let exe_name = if cfg!(windows) {
            format!("{}.exe", kind.command_name())
        } else {
            kind.command_name().to_string()
        };

        // 1. Check current executable directory
        if let Ok(mut exe_dir) = std::env::current_exe() {
            exe_dir.pop();
            let local_path = exe_dir.join(&exe_name);
            if local_path.is_file() {
                return Some(local_path);
            }
        }

        // 2. Check Laketex cache directory (%LOCALAPPDATA%\Laketex\bin or ~/.cache/laketex/bin)
        if let Some(cache_bin) = Self::laketex_bin_dir() {
            let cached_path = cache_bin.join(&exe_name);
            if cached_path.is_file() {
                return Some(cached_path);
            }
        }

        // 3. Check system PATH
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let candidate = dir.join(&exe_name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }

        None
    }

    /// Checks if an executable exists on the system PATH.
    pub fn has_executable(name: &str) -> bool {
        let exe_name = if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_string()
        };
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                if dir.join(&exe_name).is_file() || dir.join(name).is_file() {
                    return true;
                }
            }
        }
        false
    }

    /// User data / cache directory for Laketex binaries
    pub fn laketex_bin_dir() -> Option<PathBuf> {
        if cfg!(windows) {
            std::env::var("LOCALAPPDATA")
                .ok()
                .map(|p| PathBuf::from(p).join("Laketex").join("bin"))
        } else {
            std::env::var("HOME")
                .ok()
                .map(|p| PathBuf::from(p).join(".cache").join("laketex").join("bin"))
        }
    }

    fn probe_engine(kind: EngineKind) -> (bool, Option<PathBuf>, Option<String>) {
        if let Some(path) = Self::find_engine_path(kind) {
            let output = Command::new(&path).arg("--version").output();
            if let Ok(out) = output {
                if out.status.success() {
                    let ver = String::from_utf8_lossy(&out.stdout)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    return (true, Some(path), Some(ver));
                }
            }
            return (true, Some(path), None);
        }
        (false, None, None)
    }

    /// Compiles LaTeX source code to PDF using the specified engine backend.
    pub fn compile(
        source: &str,
        kind: EngineKind,
        work_dir: Option<&Path>,
    ) -> Result<CompileResult> {
        let start = Instant::now();

        // 1. Handle Native Pure-Rust Engine
        if kind == EngineKind::Native {
            let ast = NativeLatexParser::parse(source);
            let pages = DocumentLayout::layout_document(&ast);

            match NativePdfExporter::export_to_pdf_bytes(&ast) {
                Ok(bytes) => {
                    let duration_ms = start.elapsed().as_millis();
                    return Ok(CompileResult {
                        success: true,
                        pdf_bytes: Some(bytes),
                        stdout: format!(
                            "Laketex Native Typesetter: parsed {} blocks across {} pages.",
                            ast.blocks.len(),
                            pages.len()
                        ),
                        stderr: String::new(),
                        diagnostics: Vec::new(),
                        duration_ms,
                    });
                }
                Err(e) => {
                    let duration_ms = start.elapsed().as_millis();
                    return Ok(CompileResult {
                        success: false,
                        pdf_bytes: None,
                        stdout: String::new(),
                        stderr: e.clone(),
                        diagnostics: vec![Diagnostic {
                            severity: crate::compiler::diagnostic::DiagnosticSeverity::Error,
                            message: e,
                            line: None,
                            file: None,
                            details: None,
                        }],
                        duration_ms,
                    });
                }
            }
        }

        // 2. Handle External CLI Engines (Tectonic, pdfLaTeX, XeLaTeX, LuaLaTeX)
        let engine_path = Self::find_engine_path(kind).ok_or_else(|| {
            LakeTexError::EngineNotFound(format!(
                "{} not found in PATH or Laketex bin directory",
                kind.display_name()
            ))
        })?;

        // Create a temporary workspace if no working directory provided or if empty
        let temp_dir = tempfile::tempdir().map_err(LakeTexError::Io)?;
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        let run_dir: &Path = match work_dir {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => &cwd,
        };

        let tex_file_path = run_dir.join("laketex_doc.tex");
        let pdf_file_path = run_dir.join("laketex_doc.pdf");

        // Clean up any pre-existing files to prevent picking up stale PDFs from prior runs
        let _ = std::fs::remove_file(&pdf_file_path);
        let _ = std::fs::remove_file(&tex_file_path);

        // If pygmentize is not available and source uses minted, pre-inject draft mode
        // to avoid unrecoverable XeTeX crashes when pygmentize cannot be executed
        let has_pygmentize = crate::document::packages::PackageManager::is_binary_available("pygmentize");
        let pre_draft_minted = !has_pygmentize
            && source.contains("minted")
            && !source.contains("\\PassOptionsToPackage{draft}{minted}");

        let effective_source = if pre_draft_minted {
            format!("\\PassOptionsToPackage{{draft}}{{minted}}\n{source}")
        } else {
            source.to_string()
        };

        std::fs::write(&tex_file_path, &effective_source).map_err(LakeTexError::Io)?;

        let mut cmd = Command::new(&engine_path);
        cmd.current_dir(run_dir);

        match kind {
            EngineKind::Tectonic => {
                cmd.arg("-Z")
                    .arg("shell-escape")
                    .arg("--outdir")
                    .arg(run_dir)
                    .arg(&tex_file_path);
            }
            EngineKind::PdfLatex | EngineKind::XeLatex | EngineKind::LuaLatex => {
                cmd.arg("-interaction=nonstopmode")
                    .arg("-shell-escape")
                    .arg("-output-directory")
                    .arg(run_dir)
                    .arg(&tex_file_path);
            }
            EngineKind::Native => unreachable!(),
        }

        let output = cmd.output().map_err(|e| {
            LakeTexError::Engine(format!("Failed to execute compiler: {e}"))
        })?;

        let duration_ms = start.elapsed().as_millis();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // If compilation failed specifically due to minted / shell-escape / pygments, attempt draft fallback
        let combined_log = format!("{stdout}\n{stderr}");
        let is_minted_failure = (!output.status.success()
            || !pdf_file_path.exists()
            || combined_log.contains("failed to execute the shell-escape command")
            || combined_log.contains("Package minted Error"))
            && source.contains("minted")
            && !source.contains("\\PassOptionsToPackage{draft}{minted}");

        if is_minted_failure {
            let fallback_source = format!("\\PassOptionsToPackage{{draft}}{{minted}}\n{source}");
            let _ = std::fs::write(&tex_file_path, &fallback_source);
            let _ = std::fs::remove_file(&pdf_file_path);

            let mut retry_cmd = Command::new(&engine_path);
            retry_cmd.current_dir(run_dir);
            match kind {
                EngineKind::Tectonic => {
                    retry_cmd
                        .arg("-Z")
                        .arg("shell-escape")
                        .arg("--outdir")
                        .arg(run_dir)
                        .arg(&tex_file_path);
                }
                EngineKind::PdfLatex | EngineKind::XeLatex | EngineKind::LuaLatex => {
                    retry_cmd
                        .arg("-interaction=nonstopmode")
                        .arg("-shell-escape")
                        .arg("-output-directory")
                        .arg(run_dir)
                        .arg(&tex_file_path);
                }
                _ => {}
            }

            if let Ok(retry_output) = retry_cmd.output() {
                let retry_stdout = String::from_utf8_lossy(&retry_output.stdout).to_string();
                let retry_stderr = String::from_utf8_lossy(&retry_output.stderr).to_string();
                let retry_combined = format!("{retry_stdout}\n{retry_stderr}");
                let retry_pdf = if pdf_file_path.exists() {
                    std::fs::read(&pdf_file_path).ok()
                } else {
                    None
                };
                let retry_pdf = retry_pdf.filter(|b| b.len() >= 50 && b.starts_with(b"%PDF-"));
                let retry_has_no_pages = retry_combined.contains("No pages of output");

                if retry_output.status.success() && retry_pdf.is_some() && !retry_has_no_pages {
                    let mut retry_diags = DiagnosticParser::parse(&retry_combined);
                    retry_diags.retain(|d| d.severity != crate::compiler::diagnostic::DiagnosticSeverity::Error);
                    retry_diags.push(Diagnostic {
                        severity: crate::compiler::diagnostic::DiagnosticSeverity::Warning,
                        message: "Package 'minted' compiled in fallback draft mode: Pygments is not installed or shell-escape was restricted. Run 'pip install pygments' to enable full Pygments syntax coloring in external TeX engines.".to_string(),
                        line: None,
                        file: None,
                        details: Some("LaTeX compiled with '\\PassOptionsToPackage{draft}{minted}'".to_string()),
                    });

                    let _ = std::fs::remove_file(&tex_file_path);
                    let _ = std::fs::remove_file(run_dir.join("laketex_doc.aux"));
                    let _ = std::fs::remove_file(run_dir.join("laketex_doc.log"));
                    let _ = std::fs::remove_file(run_dir.join("laketex_doc.out"));
                    let _ = std::fs::remove_file(run_dir.join("laketex_doc.xdv"));
                    let _ = std::fs::remove_file(run_dir.join("laketex_doc.aex"));
                    if run_dir == temp_dir.path() {
                        let _ = std::fs::remove_file(&pdf_file_path);
                    }

                    return Ok(CompileResult {
                        success: true,
                        pdf_bytes: retry_pdf,
                        stdout: retry_stdout,
                        stderr: retry_stderr,
                        diagnostics: retry_diags,
                        duration_ms: start.elapsed().as_millis(),
                    });
                }
            }
        }

        let combined_log = format!("{stdout}\n{stderr}");
        let mut diagnostics = DiagnosticParser::parse(&combined_log);

        let raw_pdf = if pdf_file_path.exists() {
            std::fs::read(&pdf_file_path).ok()
        } else {
            None
        };
        let pdf_bytes = raw_pdf.filter(|b| b.len() >= 50 && b.starts_with(b"%PDF-"));

        let _ = std::fs::remove_file(&tex_file_path);
        let _ = std::fs::remove_file(run_dir.join("laketex_doc.aux"));
        let _ = std::fs::remove_file(run_dir.join("laketex_doc.log"));
        let _ = std::fs::remove_file(run_dir.join("laketex_doc.out"));
        let _ = std::fs::remove_file(run_dir.join("laketex_doc.xdv"));
        let _ = std::fs::remove_file(run_dir.join("laketex_doc.aex"));
        if run_dir == temp_dir.path() {
            let _ = std::fs::remove_file(&pdf_file_path);
        }

        let has_no_pages = combined_log.contains("No pages of output");
        let has_errors = diagnostics
            .iter()
            .any(|d| d.severity == crate::compiler::diagnostic::DiagnosticSeverity::Error);
        let success = output.status.success() && pdf_bytes.is_some() && !has_errors && !has_no_pages;

        if pre_draft_minted && success {
            diagnostics.retain(|d| d.severity != crate::compiler::diagnostic::DiagnosticSeverity::Error);
            diagnostics.push(Diagnostic {
                severity: crate::compiler::diagnostic::DiagnosticSeverity::Warning,
                message: "Package 'minted' compiled in draft mode: Pygments ('pygmentize') was not found on PATH. Run 'pip install pygments' to enable full Pygments syntax coloring in external TeX engines.".to_string(),
                line: None,
                file: None,
                details: Some("LaTeX compiled with '\\PassOptionsToPackage{draft}{minted}'".to_string()),
            });
        }

        Ok(CompileResult {
            success,
            pdf_bytes: if success { pdf_bytes } else { None },
            stdout,
            stderr,
            diagnostics,
            duration_ms,
        })
    }
}
