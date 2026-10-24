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
            Self::Native => "LakeTex",
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
                if let Ok(meta) = local_path.metadata() {
                    if meta.len() > 1_000_000 || kind != EngineKind::Tectonic {
                        return Some(local_path);
                    }
                }
            }
        }

        // 2. Check Laketex cache directory (%LOCALAPPDATA%\Laketex\bin or ~/.cache/laketex/bin)
        if let Some(cache_bin) = Self::laketex_bin_dir() {
            let cached_path = cache_bin.join(&exe_name);
            if cached_path.is_file() {
                if let Ok(meta) = cached_path.metadata() {
                    if meta.len() > 1_000_000 || kind != EngineKind::Tectonic {
                        return Some(cached_path);
                    }
                }
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
                .or_else(|| Some(std::env::temp_dir().join("laketex").join("bin")))
        }
    }

    fn probe_engine(kind: EngineKind) -> (bool, Option<PathBuf>, Option<String>) {
        if let Some(path) = Self::find_engine_path(kind) {
            let output = crate::create_hidden_command(&path).arg("--version").output();
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
        Self::compile_named(source, kind, work_dir, None)
    }

    /// Compiles LaTeX source code to PDF using the specified engine backend,
    /// with an optional working directory and document file name.
    pub fn compile_named(
        source: &str,
        kind: EngineKind,
        work_dir: Option<&Path>,
        doc_name: Option<&str>,
    ) -> Result<CompileResult> {
        let start = Instant::now();

        let clean_stem = doc_name
            .map(Path::new)
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty() && *s != "untitled")
            .unwrap_or("laketex_doc");

        let is_untitled = clean_stem == "laketex_doc";

        // 1. Handle Native Pure-Rust Engine
        if kind == EngineKind::Native {
            let ast = NativeLatexParser::parse(source);
            let pages = DocumentLayout::layout_document(&ast);

            match NativePdfExporter::export_to_pdf_bytes(&ast) {
                Ok(bytes) => {
                    let duration_ms = start.elapsed().as_millis();
                    if let Some(p) = work_dir {
                        if !is_untitled {
                            let target_pdf = p.join(format!("{clean_stem}.pdf"));
                            let _ = std::fs::write(&target_pdf, &bytes);
                        }
                    }
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
        let engine_path = match Self::find_engine_path(kind) {
            Some(p) => p,
            None if kind == EngineKind::Tectonic => {
                match crate::compiler::provisioner::Provisioner::provision_tectonic(|_, _| {}) {
                    Ok(p) => p,
                    Err(e) => {
                        return Err(LakeTexError::EngineNotFound(format!(
                            "{} not found and automated download failed: {e}",
                            kind.display_name()
                        )));
                    }
                }
            }
            None => {
                return Err(LakeTexError::EngineNotFound(format!(
                    "{} not found in PATH or Laketex bin directory",
                    kind.display_name()
                )));
            }
        };

        // Create a temporary workspace if no working directory provided or if document is untitled
        let temp_dir = tempfile::tempdir().map_err(LakeTexError::Io)?;
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        let abs_run_dir = match work_dir {
            Some(p) if !p.as_os_str().is_empty() && !is_untitled => {
                if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    cwd.join(p).canonicalize().unwrap_or_else(|_| cwd.join(p))
                }
            }
            _ => temp_dir.path().to_path_buf(),
        };
        let run_dir = &abs_run_dir;

        let compile_stem = if is_untitled {
            "laketex_doc".to_string()
        } else {
            format!(".laketex_{clean_stem}")
        };

        let tex_file_path = abs_run_dir.join(format!("{compile_stem}.tex"));
        let pdf_file_path = abs_run_dir.join(format!("{compile_stem}.pdf"));
        let final_pdf_path = abs_run_dir.join(format!("{clean_stem}.pdf"));

        let clean_intermediates = |dir: &Path, stem: &str| {
            for ext in &[
                "aux", "log", "out", "xdv", "aex", "toc", "nav", "snm", "bbl", "blg", "fls",
                "fdb_latexmk", "synctex.gz",
            ] {
                let _ = std::fs::remove_file(dir.join(format!("{stem}.{ext}")));
            }
        };

        // Clean up any pre-existing files to prevent picking up stale PDFs from prior runs
        let _ = std::fs::remove_file(&pdf_file_path);
        let _ = std::fs::remove_file(&tex_file_path);

        // If pygmentize is not available and source uses minted, pre-inject draft mode
        // to avoid unrecoverable XeTeX crashes when pygmentize cannot be executed
        let has_pygmentize = crate::document::packages::PackageManager::is_binary_available("pygmentize");
        let pre_draft_minted = !has_pygmentize
            && source.contains("minted")
            && !source.contains("\\PassOptionsToPackage{draft}{minted}");

        // If pictures are referenced via \includegraphics that do not exist on disk,
        // pre-inject draft mode for graphicx so the compiler defaults to inserting
        // a filepath placeholder box instead of failing compilation (like Overleaf)
        let missing_graphics = Self::detect_missing_graphics(source, &abs_run_dir);
        let pre_draft_graphics = !missing_graphics.is_empty()
            && (source.contains("\\includegraphics") || source.contains("graphicx"))
            && !source.contains("\\PassOptionsToPackage{draft}{graphicx}")
            && !source.contains("[draft]");

        // If document uses pdfTeX glyphtounicode primitives (\input{glyphtounicode} or \pdfgentounicode),
        // pre-inject compatibility shims for XeTeX/Tectonic engines where these primitives are not built-in
        let pre_glyphtounicode = (kind == EngineKind::Tectonic || kind == EngineKind::XeLatex)
            && (source.contains("glyphtounicode") || source.contains("pdfgentounicode"));

        let mut pre_injections = Vec::new();
        if pre_glyphtounicode {
            pre_injections.push(
                "\\makeatletter\\@ifundefined{pdfglyphtounicode}{\\let\\pdfglyphtounicode\\@gobbletwo}{}\\@ifundefined{pdfgentounicode}{\\newcount\\pdfgentounicode}{}\\makeatother"
            );
        }
        if pre_draft_minted {
            pre_injections.push("\\PassOptionsToPackage{draft}{minted}");
        }
        if pre_draft_graphics {
            pre_injections.push("\\PassOptionsToPackage{draft}{graphicx}\n\\makeatletter\\AtBeginDocument{\\@ifundefined{Gin@nat@width}{}{\\setkeys{Gin}{natwidth=100,natheight=100}}}\\makeatother");
        }

        let effective_source = if !pre_injections.is_empty() {
            format!("{}\n{source}", pre_injections.join("\n"))
        } else {
            source.to_string()
        };

        std::fs::write(&tex_file_path, &effective_source).map_err(LakeTexError::Io)?;

        let mut cmd = crate::create_hidden_command(&engine_path);
        cmd.current_dir(&abs_run_dir);
        if let Ok(cur_path) = std::env::var("PATH") {
            cmd.env("PATH", cur_path);
        }

        match kind {
            EngineKind::Tectonic => {
                cmd.arg("-Z").arg("continue-on-errors");
                if has_pygmentize && source.contains("minted") {
                    cmd.arg("-Z").arg("shell-escape");
                    cmd.arg("-Z").arg(format!("shell-escape-cwd={}", abs_run_dir.display()));
                }
                cmd.arg("--outdir")
                    .arg(&abs_run_dir)
                    .arg(&tex_file_path);
            }
            EngineKind::PdfLatex | EngineKind::XeLatex | EngineKind::LuaLatex => {
                let shell_flag = if has_pygmentize && source.contains("minted") {
                    "-shell-escape"
                } else {
                    "-no-shell-escape"
                };
                cmd.arg("-interaction=nonstopmode")
                    .arg(shell_flag)
                    .arg("-output-directory")
                    .arg(&abs_run_dir)
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

        // If compilation failed specifically due to minted or missing pictures, attempt draft fallback
        let combined_log = format!("{stdout}\n{stderr}");
        let compilation_failed = !output.status.success() || !pdf_file_path.exists();
        let is_minted_failure = compilation_failed
            && (combined_log.contains("failed to execute the shell-escape command")
                || combined_log.contains("Package minted Error")
                || combined_log.contains("You must invoke LaTeX with the -shell-escape flag"))
            && source.contains("minted")
            && !source.contains("\\PassOptionsToPackage{draft}{minted}")
            && !pre_draft_minted;

        let is_graphics_failure = (!output.status.success()
            || !pdf_file_path.exists()
            || combined_log.contains("Unable to load picture")
            || combined_log.contains("Image inclusion failed")
            || combined_log.contains("the xdvipdfmx engine had an unrecoverable error")
            || combined_log.contains("pdftex.def Error: File")
            || (combined_log.contains("LaTeX Error: File") && combined_log.contains("not found"))
            || (combined_log.contains("File `") && combined_log.contains("not found"))
            || (combined_log.contains("File '") && combined_log.contains("not found"))
            || combined_log.contains("Cannot find file"))
            && (source.contains("\\includegraphics") || source.contains("graphicx"))
            && !source.contains("\\PassOptionsToPackage{draft}{graphicx}")
            && !pre_draft_graphics;

        if is_minted_failure || is_graphics_failure {
            let mut retry_prefixes = Vec::new();
            if pre_glyphtounicode {
                retry_prefixes.push(
                    "\\makeatletter\\@ifundefined{pdfglyphtounicode}{\\let\\pdfglyphtounicode\\@gobbletwo}{}\\@ifundefined{pdfgentounicode}{\\newcount\\pdfgentounicode}{}\\makeatother"
                );
            }
            if is_minted_failure || pre_draft_minted {
                retry_prefixes.push("\\PassOptionsToPackage{draft}{minted}");
            }
            if is_graphics_failure || pre_draft_graphics {
                retry_prefixes.push("\\PassOptionsToPackage{draft}{graphicx}\n\\makeatletter\\AtBeginDocument{\\@ifundefined{Gin@nat@width}{}{\\setkeys{Gin}{natwidth=100,natheight=100}}}\\makeatother");
            }
            let fallback_source = format!("{}\n{source}", retry_prefixes.join("\n"));
            let _ = std::fs::write(&tex_file_path, &fallback_source);
            let _ = std::fs::remove_file(&pdf_file_path);

            let mut retry_cmd = crate::create_hidden_command(&engine_path);
            retry_cmd.current_dir(&abs_run_dir);
            if let Ok(cur_path) = std::env::var("PATH") {
                retry_cmd.env("PATH", cur_path);
            }
            match kind {
                EngineKind::Tectonic => {
                    retry_cmd
                        .arg("-Z")
                        .arg("continue-on-errors")
                        .arg("--outdir")
                        .arg(&abs_run_dir)
                        .arg(&tex_file_path);
                }
                EngineKind::PdfLatex | EngineKind::XeLatex | EngineKind::LuaLatex => {
                    retry_cmd
                        .arg("-interaction=nonstopmode")
                        .arg("-no-shell-escape")
                        .arg("-output-directory")
                        .arg(&abs_run_dir)
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

                if retry_pdf.is_some() && !retry_has_no_pages {
                    let mut retry_diags = DiagnosticParser::parse(&retry_combined);

                    if is_graphics_failure || pre_draft_graphics {
                        let missing_pics = Self::extract_missing_pictures_from_log(&retry_combined);
                        let missing_desc = if !missing_pics.is_empty() {
                            format!(" [{}]", missing_pics.join(", "))
                        } else if !missing_graphics.is_empty() {
                            format!(" [{}]", missing_graphics.join(", "))
                        } else {
                            String::new()
                        };

                        retry_diags.retain(|d| {
                            !d.message.contains("Unable to load picture")
                                && !d.message.contains("Image inclusion failed")
                                && !d.message.contains("the xdvipdfmx engine had an unrecoverable error")
                                && !d.message.contains("something bad happened inside xdvipdfmx")
                                && !d.message.contains("File `")
                                && !d.message.contains("Cannot find file")
                                && !d.message.contains("Division by 0")
                        });

                        retry_diags.push(Diagnostic {
                            severity: crate::compiler::diagnostic::DiagnosticSeverity::Warning,
                            message: format!("Missing picture(s){missing_desc} replaced with filepath placeholder box (Overleaf-style draft mode). Place the image file in your project folder to render the full graphic."),
                            line: None,
                            file: None,
                            details: Some("LaTeX compiled with '\\PassOptionsToPackage{draft}{graphicx}'".to_string()),
                        });
                    }

                    if is_minted_failure || pre_draft_minted {
                        retry_diags.retain(|d| d.severity != crate::compiler::diagnostic::DiagnosticSeverity::Error);
                        let minted_msg = if has_pygmentize {
                            "Package 'minted' compiled in fallback draft mode: Pygments ('pygmentize') was found, but shell-escape execution failed. The document was typeset without syntax highlighting.".to_string()
                        } else {
                            let py_env = crate::document::packages::PackageManager::check_python_environment();
                            if py_env.is_conda {
                                "Package 'minted' compiled in fallback draft mode: Pygments is not installed or shell-escape was restricted. Run 'conda install -c conda-forge pygments' or 'pip install pygments' to enable full Pygments syntax coloring in external TeX engines.".to_string()
                            } else {
                                "Package 'minted' compiled in fallback draft mode: Pygments is not installed or shell-escape was restricted. Run 'pip install pygments' to enable full Pygments syntax coloring in external TeX engines.".to_string()
                            }
                        };
                        retry_diags.push(Diagnostic {
                            severity: crate::compiler::diagnostic::DiagnosticSeverity::Warning,
                            message: minted_msg,
                            line: None,
                            file: None,
                            details: Some("LaTeX compiled with '\\PassOptionsToPackage{draft}{minted}'".to_string()),
                        });
                    }

                    if retry_pdf.is_some() && !is_untitled {
                        if pdf_file_path.exists() && pdf_file_path != final_pdf_path {
                            let _ = std::fs::copy(&pdf_file_path, &final_pdf_path);
                        }
                    }

                    let _ = std::fs::remove_file(&tex_file_path);
                    let _ = std::fs::remove_file(&pdf_file_path);
                    clean_intermediates(run_dir, &compile_stem);
                    clean_intermediates(run_dir, clean_stem);
                    let _ = std::fs::remove_file(run_dir.join("laketex_doc.tex"));
                    let _ = std::fs::remove_file(run_dir.join("laketex_doc.pdf"));
                    if run_dir == temp_dir.path() {
                        let _ = std::fs::remove_file(&final_pdf_path);
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

        let mut stdout = stdout;
        let mut stderr = stderr;
        let mut combined_log = format!("{stdout}\n{stderr}");

        // If cross-references or labels are unresolved and a draft/initial output was generated, run pass 2
        let needs_rerun = (combined_log.contains("Rerun to get cross-references right")
            || combined_log.contains("Rerun to get /PageLabels entry")
            || combined_log.contains("undefined on input line"))
            && pdf_file_path.exists();

        if needs_rerun {
            let mut pass2_cmd = crate::create_hidden_command(&engine_path);
            pass2_cmd.current_dir(run_dir);
            if let Ok(cur_path) = std::env::var("PATH") {
                pass2_cmd.env("PATH", cur_path);
            }
            match kind {
                EngineKind::Tectonic => {
                    pass2_cmd.arg("-Z").arg("continue-on-errors");
                    if has_pygmentize && source.contains("minted") {
                        pass2_cmd.arg("-Z").arg("shell-escape");
                        pass2_cmd.arg("-Z").arg(format!("shell-escape-cwd={}", abs_run_dir.display()));
                    }
                    pass2_cmd
                        .arg("--outdir")
                        .arg(run_dir)
                        .arg(&tex_file_path);
                }
                EngineKind::PdfLatex | EngineKind::XeLatex | EngineKind::LuaLatex => {
                    let shell_flag = if has_pygmentize && source.contains("minted") {
                        "-shell-escape"
                    } else {
                        "-no-shell-escape"
                    };
                    pass2_cmd
                        .arg("-interaction=nonstopmode")
                        .arg(shell_flag)
                        .arg("-output-directory")
                        .arg(run_dir)
                        .arg(&tex_file_path);
                }
                _ => {}
            }
            if let Ok(pass2_out) = pass2_cmd.output() {
                stdout = String::from_utf8_lossy(&pass2_out.stdout).to_string();
                stderr = String::from_utf8_lossy(&pass2_out.stderr).to_string();
                combined_log = format!("{stdout}\n{stderr}");
            }
        }

        let mut diagnostics = DiagnosticParser::parse(&combined_log);

        let raw_pdf = if pdf_file_path.exists() {
            std::fs::read(&pdf_file_path).ok()
        } else {
            None
        };
        let pdf_bytes = raw_pdf.filter(|b| b.len() >= 50 && b.starts_with(b"%PDF-"));

        if pdf_bytes.is_some() && !is_untitled {
            if pdf_file_path.exists() && pdf_file_path != final_pdf_path {
                let _ = std::fs::copy(&pdf_file_path, &final_pdf_path);
            }
        }

        let _ = std::fs::remove_file(&tex_file_path);
        let _ = std::fs::remove_file(&pdf_file_path);
        clean_intermediates(run_dir, &compile_stem);
        clean_intermediates(run_dir, clean_stem);
        let _ = std::fs::remove_file(run_dir.join("laketex_doc.tex"));
        let _ = std::fs::remove_file(run_dir.join("laketex_doc.pdf"));
        if run_dir == temp_dir.path() {
            let _ = std::fs::remove_file(&final_pdf_path);
        }

        let has_no_pages = combined_log.contains("No pages of output");
        let only_missing_pic_errors = !diagnostics.is_empty()
            && diagnostics
                .iter()
                .filter(|d| d.severity == crate::compiler::diagnostic::DiagnosticSeverity::Error)
                .all(|d| {
                    d.message.contains("Unable to load picture")
                        || d.message.contains("Image inclusion failed")
                        || d.message.contains("the xdvipdfmx engine had an unrecoverable error")
                        || d.message.contains("something bad happened inside xdvipdfmx")
                        || (d.message.contains("File") && d.message.contains("not found"))
                        || d.message.contains("Cannot find file")
                        || d.message.contains("Division by 0")
                });

        let is_draft_active = pre_draft_graphics || source.contains("draft");
        let has_errors = if only_missing_pic_errors && is_draft_active {
            diagnostics.retain(|d| {
                !d.message.contains("Unable to load picture")
                    && !d.message.contains("Image inclusion failed")
                    && !d.message.contains("the xdvipdfmx engine had an unrecoverable error")
                    && !d.message.contains("something bad happened inside xdvipdfmx")
                    && !d.message.contains("File `")
                    && !d.message.contains("Cannot find file")
                    && !d.message.contains("Division by 0")
                    && !d.message.contains("Division by 0")
            });
            let missing_names = if !missing_graphics.is_empty() {
                missing_graphics.join(", ")
            } else {
                Self::extract_missing_pictures_from_log(&combined_log).join(", ")
            };
            let missing_desc = if !missing_names.is_empty() {
                format!(" [{missing_names}]")
            } else {
                String::new()
            };
            diagnostics.push(Diagnostic {
                severity: crate::compiler::diagnostic::DiagnosticSeverity::Warning,
                message: format!("Missing picture(s){missing_desc} replaced with filepath placeholder box (Overleaf-style draft mode). Place the image file in your project folder to render the full graphic."),
                line: None,
                file: None,
                details: Some("LaTeX compiled with '\\PassOptionsToPackage{draft}{graphicx}'".to_string()),
            });
            false
        } else {
            diagnostics
                .iter()
                .any(|d| d.severity == crate::compiler::diagnostic::DiagnosticSeverity::Error)
        };

        let success = (output.status.success() || (only_missing_pic_errors && is_draft_active))
            && pdf_bytes.is_some()
            && !has_errors
            && !has_no_pages;

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

        if pre_draft_graphics && success {
            diagnostics.retain(|d| {
                !d.message.contains("Unable to load picture")
                    && !d.message.contains("Image inclusion failed")
                    && !d.message.contains("the xdvipdfmx engine had an unrecoverable error")
                    && !d.message.contains("something bad happened inside xdvipdfmx")
                    && !d.message.contains("File `")
                    && !d.message.contains("Cannot find file")
                    && !d.message.contains("Division by 0")
            });
            let missing_desc = if !missing_graphics.is_empty() {
                format!(" [{}]", missing_graphics.join(", "))
            } else {
                String::new()
            };
            if !diagnostics.iter().any(|d| d.message.contains("Missing picture(s)")) {
                diagnostics.push(Diagnostic {
                    severity: crate::compiler::diagnostic::DiagnosticSeverity::Warning,
                    message: format!("Missing picture(s){missing_desc} replaced with filepath placeholder box (Overleaf-style draft mode). Place the image file in your project folder to render the full graphic."),
                    line: None,
                    file: None,
                    details: Some("LaTeX compiled with '\\PassOptionsToPackage{draft}{graphicx}'".to_string()),
                });
            }
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

    /// Extracts a list of referenced graphic paths from `\includegraphics` commands in the LaTeX source.
    pub fn extract_graphic_paths(source: &str) -> Vec<String> {
        let mut paths = Vec::new();
        let mut search_idx = 0;
        while let Some(pos) = source[search_idx..].find("\\includegraphics") {
            let abs_pos = search_idx + pos;
            let rest = &source[abs_pos + "\\includegraphics".len()..];
            search_idx = abs_pos + "\\includegraphics".len();

            let mut slice = rest.trim_start_matches('*').trim_start();
            if slice.starts_with('[') {
                if let Some(bracket_end) = slice.find(']') {
                    slice = slice[bracket_end + 1..].trim_start();
                }
            }
            if slice.starts_with('{') {
                if let Some(end) = slice.find('}') {
                    let path_str = slice[1..end].trim();
                    if !path_str.is_empty() && !paths.contains(&path_str.to_string()) {
                        paths.push(path_str.to_string());
                    }
                }
            }
        }
        paths
    }

    /// Checks whether a referenced graphic file exists on disk, probing relative directories and standard extensions.
    pub fn check_graphic_exists(path_str: &str, run_dir: &std::path::Path) -> bool {
        let clean_path = path_str.trim().replace('\\', "/");
        if clean_path.is_empty() {
            return false;
        }

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let search_roots = [run_dir, &cwd];

        let extensions = ["", ".png", ".jpg", ".jpeg", ".pdf", ".eps", ".PNG", ".JPG", ".JPEG", ".PDF"];
        let subfolders = ["", "images/", "figures/", "img/", "plots/", "assets/"];

        for root in &search_roots {
            for sub in &subfolders {
                for ext in &extensions {
                    let candidate = if clean_path.ends_with(ext) && !ext.is_empty() {
                        root.join(sub).join(&clean_path)
                    } else {
                        root.join(sub).join(format!("{clean_path}{ext}"))
                    };
                    if candidate.is_file() {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Returns a list of graphic files referenced in `\includegraphics` that are missing from disk.
    pub fn detect_missing_graphics(source: &str, run_dir: &std::path::Path) -> Vec<String> {
        let all_paths = Self::extract_graphic_paths(source);
        all_paths
            .into_iter()
            .filter(|p| !Self::check_graphic_exists(p, run_dir))
            .collect()
    }

    /// Extracts filenames of missing pictures from compiler standard error / stdout logs.
    pub fn extract_missing_pictures_from_log(log: &str) -> Vec<String> {
        let mut missing = Vec::new();
        for line in log.lines() {
            if line.contains("Unable to load picture or PDF file") {
                if let Some(start) = line.find('\'') {
                    if let Some(end) = line[start + 1..].find('\'') {
                        let name = line[start + 1..start + 1 + end].trim();
                        if !name.is_empty() && !missing.contains(&name.to_string()) {
                            missing.push(name.to_string());
                        }
                    }
                }
            } else if line.contains("Image inclusion failed for") {
                if let Some(start) = line.find('"') {
                    if let Some(end) = line[start + 1..].find('"') {
                        let name = line[start + 1..start + 1 + end].trim();
                        if !name.is_empty() && !missing.contains(&name.to_string()) {
                            missing.push(name.to_string());
                        }
                    }
                }
            } else if line.contains("not found") && (line.contains("File `") || line.contains("File '")) {
                let quote_char = if line.contains("File `") { '`' } else { '\'' };
                if let Some(start) = line.find(quote_char) {
                    if let Some(end) = line[start + 1..].find('\'') {
                        let name = line[start + 1..start + 1 + end].trim();
                        if !name.is_empty() && !missing.contains(&name.to_string()) {
                            missing.push(name.to_string());
                        }
                    }
                }
            }
        }
        missing
    }
}
