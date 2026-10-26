//! Command line argument parsing and subcommand dispatch.

use crate::compiler::engine::{CompilerEngine, EngineKind};
use crate::document::outline::OutlineParser;
use crate::document::packages::PackageManager;
use crate::headless::server::HeadlessServer;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Helper macro to write formatted text to stdout, catching and ignoring BrokenPipe errors
/// (e.g. when piping output into PowerShell Measure-Command, head, or Select-Object).
macro_rules! safe_println {
    () => {
        safe_print!("\n");
    };
    ($($arg:tt)*) => {{
        use std::io::Write;
        if let Err(e) = writeln!(std::io::stdout(), $($arg)*) {
            if e.kind() != std::io::ErrorKind::BrokenPipe {
                eprintln!("Warning: failed writing to stdout: {e}");
            }
        }
    }};
}

/// Helper macro to write text to stdout without newline, catching and ignoring BrokenPipe errors.
macro_rules! safe_print {
    ($($arg:tt)*) => {{
        use std::io::Write;
        if let Err(e) = write!(std::io::stdout(), $($arg)*) {
            if e.kind() != std::io::ErrorKind::BrokenPipe {
                eprintln!("Warning: failed writing to stdout: {e}");
            }
        }
    }};
}

#[derive(Parser, Debug)]
#[command(
    name = "laketex",
    about = "Laketex: Lightweight, portable LaTeX editor and viewer in Rust",
    version,
    after_help = "Run `laketex api-spec` for the JSON API schema, or `laketex serve` to start the HTTP server."
)]
pub struct CliArgs {
    /// Optional headless CLI subcommand to execute instead of launching the desktop GUI.
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to a LaTeX file to open directly in the GUI editor
    pub file: Option<PathBuf>,

    /// Run headless API server in the background alongside the GUI
    #[arg(long, default_value_t = false)]
    pub with_api: bool,

    /// Enable stateful REST API mode (syncs API calls with the live GUI editor and viewer in real time; requires --with-api)
    #[arg(long, default_value_t = false)]
    pub with_state: bool,

    /// API port if background server is enabled
    #[arg(long, default_value_t = 3030)]
    pub api_port: u16,
}

/// Headless CLI subcommands supported by Laketex.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Launch the headless REST API server
    Serve {
        /// Host IP address to bind to (e.g. `"127.0.0.1"`).
        #[arg(short, long, default_value = "127.0.0.1")]
        host: String,

        /// Network port to listen on.
        #[arg(short, long, default_value_t = 3030)]
        port: u16,

        /// Permit `-shell-escape` when requested by `/api/compile` callers
        #[arg(long, default_value_t = false)]
        allow_shell_escape: bool,
    },

    /// Compile a LaTeX file to PDF directly from the terminal
    Compile {
        /// Input .tex file
        input: PathBuf,

        /// Output .pdf file path (defaults to same name with .pdf)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Engine to use: native (default, pure Rust), tectonic, pdflatex, xelatex, lualatex
        #[arg(short, long, default_value = "native")]
        engine: String,

        /// Automatically install missing external dependencies with consent before compiling
        #[arg(long)]
        install_deps: bool,

        /// Permit external TeX engines to run with `-shell-escape` (e.g. for `minted`)
        #[arg(long, default_value_t = false)]
        allow_shell_escape: bool,

        /// Export a contextual error report file (.md or .json) if errors occur
        #[arg(long)]
        export_log: Option<PathBuf>,
    },

    /// Check and lint a LaTeX file for syntax errors and warnings
    Check {
        /// Input .tex file
        input: PathBuf,

        /// Output format: text or json
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Export a contextual error report file (.md or .json)
        #[arg(long)]
        export_log: Option<PathBuf>,
    },

    /// Extract document outline (sections, equations, tables) as JSON
    Outline {
        /// Input .tex file
        input: PathBuf,
    },

    /// Inspect imported packages, check external tool dependencies, and install missing dependencies with consent
    Packages {
        /// Input .tex file
        input: PathBuf,

        /// Check external tool dependencies (e.g. Pygments for minted)
        #[arg(long)]
        check_deps: bool,

        /// Install missing external tool dependencies via pip/system package manager
        #[arg(long)]
        install_deps: bool,

        /// Automatically grant consent for package installation without interactive prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Visually compare Tectonic rendering to the Native Pure-Rust engine and compute parity metrics
    Compare {
        /// Input .tex file
        input: PathBuf,

        /// Page index to compare (0-indexed, default: 0)
        #[arg(short, long, default_value_t = 0)]
        page: usize,

        /// Target pixel render width
        #[arg(short, long, default_value_t = 1000)]
        width: u16,

        /// Output directory for visual diff artifacts and parity report JSON
        #[arg(short, long, default_value = "./parity_report")]
        output_dir: PathBuf,
    },

    /// Audit and debug LaTeX source code and PDF preview to identify unrendered commands, matrix flaws, and math notation bugs
    Debug {
        /// Input .tex file
        input: PathBuf,

        /// Output format: text (human-readable summary table) or json
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Output file to export audit report (.txt, .md, or .json)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Engine to compile preview with: native (default) or tectonic
        #[arg(short, long, default_value = "native")]
        engine: String,
    },

    /// View, list, or export built-in LaTeX documentation manuals bundled within Laketex
    Docs {
        /// Document name or kind: manual, theming, api, architecture, or all (default)
        #[arg(short, long, default_value = "all")]
        doc: String,

        /// Print the LaTeX source code directly to stdout
        #[arg(short, long)]
        print: bool,

        /// Export the LaTeX documentation files to a target directory
        #[arg(short, long)]
        export: Option<PathBuf>,

        /// List available built-in documentation manuals
        #[arg(short, long)]
        list: bool,
    },

    /// Emit a complete machine-readable JSON specification of all REST API endpoints, schemas, and stateful/stateless agent workflows
    #[command(visible_aliases = ["agent-spec", "schema", "endpoints"])]
    ApiSpec {
        /// Target API port to embed in example URLs
        #[arg(short, long, default_value_t = 3030)]
        port: u16,

        /// Emit compact single-line JSON instead of pretty-printed JSON
        #[arg(short, long, default_value_t = false)]
        compact: bool,

        /// Optional file path to write the JSON specification to
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

impl CliArgs {
    /// Dispatches and executes the specified headless CLI subcommand, returning any execution errors.
    pub fn execute_subcommand(cmd: Commands) -> Result<(), Box<dyn std::error::Error>> {
        match cmd {
            Commands::Serve {
                host,
                port,
                allow_shell_escape,
            } => {
                let server = HeadlessServer::new();
                server.run_blocking_with_options(&host, port, allow_shell_escape)?;
            }
            Commands::Compile {
                input,
                output,
                engine,
                install_deps,
                allow_shell_escape,
                export_log,
            } => {
                let source = std::fs::read_to_string(&input)?;

                if install_deps {
                    let missing = PackageManager::detect_dependencies(&source);
                    for dep in missing.into_iter().filter(|d| !d.installed) {
                        let pkg = dep.pip_package.as_deref().unwrap_or(&dep.name);
                        safe_println!("Installing missing dependency '{}' via pip...", pkg);
                        if let Err(e) = PackageManager::install_dependency(pkg, true) {
                            eprintln!("Warning: Failed to install dependency {}: {}", pkg, e);
                        } else {
                            safe_println!("✓ Installed '{}'", pkg);
                        }
                    }
                }

                let engine_kind = match engine.to_lowercase().as_str() {
                    "tectonic" => EngineKind::Tectonic,
                    "pdflatex" => EngineKind::PdfLatex,
                    "xelatex" => EngineKind::XeLatex,
                    "lualatex" => EngineKind::LuaLatex,
                    _ => EngineKind::Native, // Default to Native Pure-Rust!
                };

                safe_println!("Compiling {} with {}...", input.display(), engine_kind.display_name());
                let work_dir = input.parent().filter(|p| !p.as_os_str().is_empty()).map(|p| p.to_path_buf());
                let options = crate::compiler::CompileOptions {
                    work_dir,
                    doc_name: input.file_name().and_then(|s| s.to_str()).map(str::to_string),
                    allow_shell_escape,
                    shadow_subdocument: None,
                    cancel_token: None,
                };
                let result = CompilerEngine::compile_with_options(&source, engine_kind, &options)?;

                for diag in &result.diagnostics {
                    safe_println!(
                        "[{:?}] line {}: {}",
                        diag.severity,
                        diag.line.map(|l| l.to_string()).unwrap_or_else(|| "?".to_string()),
                        diag.message
                    );
                }

                if let Some(log_path) = export_log {
                    let report = crate::compiler::ErrorReport::generate(
                        &source,
                        &input.display().to_string(),
                        engine_kind,
                        &result.diagnostics,
                        &result.stdout,
                        &result.stderr,
                    );
                    let content = if log_path.extension().and_then(|s| s.to_str()) == Some("json") {
                        report.to_json()
                    } else {
                        report.to_markdown()
                    };
                    std::fs::write(&log_path, content)?;
                    safe_println!("📄 Exported contextual error log to {}", log_path.display());
                }

                if result.success {
                    let out_path = output.unwrap_or_else(|| input.with_extension("pdf"));
                    if let Some(bytes) = result.pdf_bytes {
                        std::fs::write(&out_path, bytes)?;
                        safe_println!("✓ Successfully compiled to {} in {} ms", out_path.display(), result.duration_ms);
                    }
                } else {
                    eprintln!("✗ Compilation failed in {} ms", result.duration_ms);
                    std::process::exit(1);
                }
            }
            Commands::Check {
                input,
                format,
                export_log,
            } => {
                let source = std::fs::read_to_string(&input)?;
                let work_dir = input.parent().filter(|p| !p.as_os_str().is_empty());
                let result = CompilerEngine::compile(&source, EngineKind::Tectonic, work_dir)?;
                let report = crate::compiler::ErrorReport::generate(
                    &source,
                    &input.display().to_string(),
                    EngineKind::Tectonic,
                    &result.diagnostics,
                    &result.stdout,
                    &result.stderr,
                );

                if let Some(log_path) = export_log {
                    let content = if log_path.extension().and_then(|s| s.to_str()) == Some("json") {
                        report.to_json()
                    } else {
                        report.to_markdown()
                    };
                    std::fs::write(&log_path, content)?;
                    safe_println!("📄 Exported contextual error log to {}", log_path.display());
                }

                if format.to_lowercase() == "json" {
                    safe_println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    safe_println!("Diagnostics for {}:", input.display());
                    for diag in &report.diagnostics {
                        safe_println!(
                            "\n[{:?}] {} (Line {})",
                            diag.severity,
                            diag.message,
                            diag.line.map(|l| l.to_string()).unwrap_or_else(|| "?".to_string())
                        );
                        safe_println!("  Activity/Intent: {}", diag.environment_intent);
                        if !diag.surrounding_context.is_empty() {
                            for ctx in &diag.surrounding_context {
                                let m = if ctx.is_target { ">>>" } else { "   " };
                                safe_println!("  {} {:>4} | {}", m, ctx.line_number, ctx.content);
                            }
                        }
                        safe_println!("  💡 Suggested Fix: {}", diag.suggested_fix);
                    }
                    if report.diagnostics.is_empty() {
                        safe_println!("  ✓ No errors or warnings found!");
                    }
                }
            }
            Commands::Outline { input } => {
                let source = std::fs::read_to_string(&input)?;
                let items = OutlineParser::parse(&source);
                safe_println!("{}", serde_json::to_string_pretty(&items)?);
            }
            Commands::Packages {
                input,
                check_deps,
                install_deps,
                yes,
            } => {
                let source = std::fs::read_to_string(&input)?;
                let packages = PackageManager::detect_packages(&source);
                let dependencies = PackageManager::detect_dependencies(&source);

                if !check_deps && !install_deps {
                    safe_println!("{}", serde_json::to_string_pretty(&packages)?);
                    return Ok(());
                }

                safe_println!("======================================================================");
                safe_println!("📦 LAKETEX PACKAGE DEPENDENCY AUDIT: {}", input.display());
                safe_println!("======================================================================");

                let py_env = PackageManager::check_python_environment();
                safe_println!("System Environment:");
                if py_env.python_available {
                    let ver = py_env.python_version.as_deref().unwrap_or("unknown");
                    let path = py_env.python_path.as_deref().unwrap_or("python");
                    let conda_tag = if py_env.is_conda {
                        let name = py_env.environment_name.as_deref().unwrap_or("env");
                        format!(" [Conda: {name}]")
                    } else {
                        String::new()
                    };
                    safe_println!("  ✓ Python: {} ({}){}", ver, path, conda_tag);
                    if py_env.pip_available {
                        safe_println!("  ✓ pip: Available");
                    } else if py_env.conda_cli_path.is_some() {
                        safe_println!("  ✓ conda: Available (conda-forge fallback ready)");
                    } else {
                        safe_println!("  ✗ pip / conda: Missing");
                    }
                } else {
                    safe_println!("  ✗ Python: Not detected on system PATH");
                }
                safe_println!("----------------------------------------------------------------------");

                if dependencies.is_empty() {
                    safe_println!("✓ None of the declared packages require external host tools.");
                } else {
                    safe_println!("Detected External Tool Dependencies ({}):", dependencies.len());
                    for (i, dep) in dependencies.iter().enumerate() {
                        let status_label = if dep.installed {
                            format!("✓ Installed ({})", dep.binary_path.as_deref().unwrap_or(&dep.binary_name))
                        } else {
                            "✗ MISSING".to_string()
                        };
                        safe_println!(
                            "[{}] Package '{}' -> Dependency '{}' (binary: '{}')",
                            i + 1,
                            dep.latex_package,
                            dep.name,
                            dep.binary_name
                        );
                        safe_println!("    Status:          {}", status_label);
                        safe_println!("    Description:     {}", dep.description);
                        safe_println!("    Install Command: {}", dep.install_command);
                    }
                }

                if install_deps {
                    safe_println!("----------------------------------------------------------------------");
                    let missing_deps: Vec<_> = dependencies
                        .into_iter()
                        .filter(|d| !d.installed)
                        .collect();
                    if missing_deps.is_empty() {
                        safe_println!("✓ All required external tool dependencies are already installed!");
                    } else {
                        for dep in missing_deps {
                            let pkg_to_install = dep
                                .pip_package
                                .as_deref()
                                .unwrap_or(&dep.name);
                            let mut consented = yes;

                            if !consented {
                                safe_print!(
                                    "\nLaTeX package '{}' requires '{}'. Do you consent to installing it via '{}'? [y/N]: ",
                                    dep.latex_package, dep.name, dep.install_command
                                );
                                let _ = std::io::Write::flush(&mut std::io::stdout());
                                let mut input_line = String::new();
                                std::io::stdin().read_line(&mut input_line)?;
                                let trimmed = input_line.trim().to_lowercase();
                                if trimmed == "y" || trimmed == "yes" {
                                    consented = true;
                                }
                            }

                            if consented {
                                safe_println!("Installing '{}' via pip...", pkg_to_install);
                                match PackageManager::install_dependency(pkg_to_install, true) {
                                    Ok(msg) => safe_println!("✓ {}", msg.trim()),
                                    Err(e) => {
                                        eprintln!("✗ Failed to install {}: {}", pkg_to_install, e)
                                    }
                                }
                            } else {
                                safe_println!("Installation skipped: user consent was not granted.");
                            }
                        }
                    }
                }

                safe_println!("======================================================================");
            }
            Commands::Compare {
                input,
                page,
                width,
                output_dir,
            } => {
                use crate::viewer::parity::VisualParityComparator;
                let source = std::fs::read_to_string(&input)?;
                safe_println!(
                    "Comparing Tectonic vs Native Engine visual rendering for {} (page {})...",
                    input.display(),
                    page + 1
                );
                let work_dir = input.parent().filter(|p| !p.as_os_str().is_empty());
                let report = VisualParityComparator::compare_latex_source_with_dir(
                    &source,
                    &input.display().to_string(),
                    page,
                    width,
                    Some(&output_dir),
                    work_dir,
                )?;

                safe_println!("========================================================");
                safe_println!("           LAKETEX VISUAL PARITY BENCHMARK REPORT       ");
                safe_println!("========================================================");
                safe_println!("Document:                 {}", report.document_name);
                safe_println!("Page Analyzed:            Page {}", report.page_number);
                safe_println!(
                    "Canvas Dimensions:        {} x {} px",
                    report.rendered_width_px, report.rendered_height_px
                );
                safe_println!("Total Pixels Evaluated:   {}", report.total_pixels);
                safe_println!(
                    "Matching Pixels:          {} ({:.2}%)",
                    report.matching_pixels, report.visual_parity_score
                );
                safe_println!("Mismatch Pixels:          {}", report.mismatch_pixels);
                safe_println!("Mean Squared Error (MSE): {:.2}", report.mean_squared_error);
                safe_println!(
                    "PSNR (Fidelity):          {:.2} dB",
                    report.peak_signal_to_noise_ratio
                );
                safe_println!(
                    "Tectonic Compilation:     {} ms ({} bytes)",
                    report.tectonic_compile_ms, report.tectonic_pdf_bytes
                );
                safe_println!(
                    "Native Rust Compilation:  {} ms ({} bytes)",
                    report.native_compile_ms, report.native_pdf_bytes
                );
                safe_println!("Output Artifacts Saved:   {}", output_dir.display());
                safe_println!("  -> {}/tectonic_render.png", output_dir.display());
                safe_println!("  -> {}/native_render.png", output_dir.display());
                safe_println!("  -> {}/diff_comparison.png", output_dir.display());
                safe_println!("  -> {}/visual_parity_report.json", output_dir.display());
                safe_println!("========================================================");
            }
            Commands::Debug {
                input,
                format,
                output,
                engine,
            } => {
                use crate::engine::render_debugger::RenderDebugger;
                let source = std::fs::read_to_string(&input)?;
                let engine_kind = match engine.to_lowercase().as_str() {
                    "tectonic" => EngineKind::Tectonic,
                    _ => EngineKind::Native,
                };

                let compile_res = CompilerEngine::compile(&source, engine_kind, input.parent()).ok();
                let pdf_bytes = compile_res.as_ref().and_then(|r| r.pdf_bytes.as_deref());

                let report = RenderDebugger::scan(&source, &input.display().to_string(), pdf_bytes);

                if let Some(out_path) = &output {
                    let content = if out_path.extension().and_then(|s| s.to_str()) == Some("json") {
                        report.to_json()
                    } else if out_path.extension().and_then(|s| s.to_str()) == Some("md") {
                        report.to_markdown()
                    } else {
                        report.to_text_summary()
                    };
                    std::fs::write(out_path, content)?;
                    eprintln!("Exported render audit report to {}", out_path.display());
                }

                if format.to_lowercase() == "json" {
                    safe_println!("{}", report.to_json());
                } else {
                    safe_println!("{}", report.to_text_summary());
                }
                use std::io::Write;
                let _ = std::io::stdout().flush();

                if report.error_count > 0 {
                    std::process::exit(1);
                }
            }
            Commands::Docs {
                doc,
                print,
                export,
                list,
            } => {
                let docs = crate::document::BuiltinDoc::all();
                if list || (!print && export.is_none()) {
                    safe_println!("============================================================");
                    safe_println!("  Laketex Built-in LaTeX Documentation Library");
                    safe_println!("============================================================");
                    safe_println!();
                    for d in docs {
                        safe_println!("  * [{:?}] {} ({})", d.kind, d.title, d.filename);
                        safe_println!("    Summary: {}", d.description);
                        safe_println!("    Lines:   {}", d.content.lines().count());
                        safe_println!();
                    }
                    safe_println!("CLI Usage Examples:");
                    safe_println!("  laketex docs --list                      # List available docs");
                    safe_println!("  laketex docs --print --doc manual        # Print manual to stdout");
                    safe_println!("  laketex docs --export ./docs             # Export all docs to folder");
                    safe_println!("  laketex docs --export ./docs --doc theming");
                    if !print && export.is_none() {
                        return Ok(());
                    }
                }

                if let Some(target_dir) = export {
                    std::fs::create_dir_all(&target_dir)?;
                    let to_export: Vec<&crate::document::BuiltinDoc> =
                        if doc.eq_ignore_ascii_case("all") {
                            docs.iter().collect()
                        } else {
                            docs.iter()
                                .filter(|d| {
                                    let k = format!("{:?}", d.kind).to_lowercase();
                                    k.contains(&doc.to_lowercase())
                                        || d.filename.to_lowercase().contains(&doc.to_lowercase())
                                })
                                .collect()
                        };

                    if to_export.is_empty() {
                        eprintln!("Warning: No documentation manuals matched '{}'", doc);
                    } else {
                        for d in to_export {
                            let out_path = target_dir.join(d.filename);
                            std::fs::write(&out_path, d.content)?;
                            safe_println!("✓ Exported {} -> {}", d.filename, out_path.display());
                        }
                    }
                }

                if print {
                    let matching = docs.iter().find(|d| {
                        let k = format!("{:?}", d.kind).to_lowercase();
                        k.contains(&doc.to_lowercase())
                            || d.filename.to_lowercase().contains(&doc.to_lowercase())
                    });

                    if let Some(d) = matching {
                        safe_print!("{}", d.content);
                    } else {
                        eprintln!(
                            "Document '{}' not found. Available: manual, theming, api, architecture",
                            doc
                        );
                    }
                }
            }
            Commands::ApiSpec {
                port,
                compact,
                output,
            } => {
                let spec = HeadlessServer::agent_api_spec_json(port, None, None);
                let json_out = if compact {
                    serde_json::to_string(&spec)?
                } else {
                    serde_json::to_string_pretty(&spec)?
                };
                if let Some(out_path) = output {
                    std::fs::write(&out_path, &json_out)?;
                    eprintln!("Exported agent API specification to {}", out_path.display());
                } else {
                    safe_println!("{json_out}");
                }
            }
        }
        Ok(())
    }
}
