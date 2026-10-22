//! # Command Line Interface (CLI) Dispatcher
//!
//! Handles terminal arguments using `clap` for standalone compilation, syntax checking,
//! visual parity testing, and launching the headless REST API service.

use crate::compiler::engine::{CompilerEngine, EngineKind};
use crate::document::outline::OutlineParser;
use crate::document::packages::PackageManager;
use crate::headless::server::HeadlessServer;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Command line arguments parsed by LakeTex.
#[derive(Parser, Debug)]
#[command(
    name = "laketex",
    about = "Laketex: Lightweight, portable LaTeX editor and viewer in Rust",
    version = "0.1.0"
)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to a LaTeX file to open directly in the GUI editor
    pub file: Option<PathBuf>,

    /// Run headless API server in the background alongside the GUI
    #[arg(long, default_value_t = false)]
    pub with_api: bool,

    /// API port if background server is enabled
    #[arg(long, default_value_t = 3030)]
    pub api_port: u16,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Launch the headless REST API server
    Serve {
        #[arg(short, long, default_value = "127.0.0.1")]
        host: String,

        #[arg(short, long, default_value_t = 3030)]
        port: u16,
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
}

impl CliArgs {
    pub fn execute_subcommand(cmd: Commands) -> Result<(), Box<dyn std::error::Error>> {
        match cmd {
            Commands::Serve { host, port } => {
                let server = HeadlessServer::new();
                server.run_blocking(&host, port)?;
            }
            Commands::Compile {
                input,
                output,
                engine,
                install_deps,
                export_log,
            } => {
                let source = std::fs::read_to_string(&input)?;

                if install_deps {
                    let missing = PackageManager::detect_dependencies(&source);
                    for dep in missing.into_iter().filter(|d| !d.installed) {
                        let pkg = dep.pip_package.as_deref().unwrap_or(&dep.name);
                        println!("Installing missing dependency '{}' via pip...", pkg);
                        if let Err(e) = PackageManager::install_dependency(pkg, true) {
                            eprintln!("Warning: Failed to install dependency {}: {}", pkg, e);
                        } else {
                            println!("✓ Installed '{}'", pkg);
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

                println!("Compiling {} with {}...", input.display(), engine_kind.display_name());
                let work_dir = input.parent().filter(|p| !p.as_os_str().is_empty());
                let result = CompilerEngine::compile(&source, engine_kind, work_dir)?;

                for diag in &result.diagnostics {
                    println!(
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
                    println!("📄 Exported contextual error log to {}", log_path.display());
                }

                if result.success {
                    let out_path = output.unwrap_or_else(|| input.with_extension("pdf"));
                    if let Some(bytes) = result.pdf_bytes {
                        std::fs::write(&out_path, bytes)?;
                        println!("✓ Successfully compiled to {} in {} ms", out_path.display(), result.duration_ms);
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
                    println!("📄 Exported contextual error log to {}", log_path.display());
                }

                if format.to_lowercase() == "json" {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!("Diagnostics for {}:", input.display());
                    for diag in &report.diagnostics {
                        println!(
                            "\n[{:?}] {} (Line {})",
                            diag.severity,
                            diag.message,
                            diag.line.map(|l| l.to_string()).unwrap_or_else(|| "?".to_string())
                        );
                        println!("  Activity/Intent: {}", diag.environment_intent);
                        if !diag.surrounding_context.is_empty() {
                            for ctx in &diag.surrounding_context {
                                let m = if ctx.is_target { ">>>" } else { "   " };
                                println!("  {} {:>4} | {}", m, ctx.line_number, ctx.content);
                            }
                        }
                        println!("  💡 Suggested Fix: {}", diag.suggested_fix);
                    }
                    if report.diagnostics.is_empty() {
                        println!("  ✓ No errors or warnings found!");
                    }
                }
            }
            Commands::Outline { input } => {
                let source = std::fs::read_to_string(&input)?;
                let items = OutlineParser::parse(&source);
                println!("{}", serde_json::to_string_pretty(&items)?);
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
                    println!("{}", serde_json::to_string_pretty(&packages)?);
                    return Ok(());
                }

                println!("======================================================================");
                println!("📦 LAKETEX PACKAGE DEPENDENCY AUDIT: {}", input.display());
                println!("======================================================================");

                let py_env = PackageManager::check_python_environment();
                println!("System Environment:");
                if py_env.python_available {
                    let ver = py_env.python_version.as_deref().unwrap_or("unknown");
                    let path = py_env.python_path.as_deref().unwrap_or("python");
                    println!("  ✓ Python: {} ({})", ver, path);
                    if py_env.pip_available {
                        println!("  ✓ pip: Available");
                    } else {
                        println!("  ✗ pip: Missing");
                    }
                } else {
                    println!("  ✗ Python: Not detected on system PATH");
                }
                println!("----------------------------------------------------------------------");

                if dependencies.is_empty() {
                    println!("✓ None of the declared packages require external host tools.");
                } else {
                    println!("Detected External Tool Dependencies ({}):", dependencies.len());
                    for (i, dep) in dependencies.iter().enumerate() {
                        let status_label = if dep.installed {
                            format!("✓ Installed ({})", dep.binary_path.as_deref().unwrap_or(&dep.binary_name))
                        } else {
                            "✗ MISSING".to_string()
                        };
                        println!(
                            "[{}] Package '{}' -> Dependency '{}' (binary: '{}')",
                            i + 1,
                            dep.latex_package,
                            dep.name,
                            dep.binary_name
                        );
                        println!("    Status:          {}", status_label);
                        println!("    Description:     {}", dep.description);
                        println!("    Install Command: {}", dep.install_command);
                    }
                }

                if install_deps {
                    println!("----------------------------------------------------------------------");
                    let missing_deps: Vec<_> = dependencies
                        .into_iter()
                        .filter(|d| !d.installed)
                        .collect();
                    if missing_deps.is_empty() {
                        println!("✓ All required external tool dependencies are already installed!");
                    } else {
                        for dep in missing_deps {
                            let pkg_to_install = dep
                                .pip_package
                                .as_deref()
                                .unwrap_or(&dep.name);
                            let mut consented = yes;

                            if !consented {
                                print!(
                                    "\nLaTeX package '{}' requires '{}'. Do you consent to installing it via '{}'? [y/N]: ",
                                    dep.latex_package, dep.name, dep.install_command
                                );
                                std::io::Write::flush(&mut std::io::stdout())?;
                                let mut input_line = String::new();
                                std::io::stdin().read_line(&mut input_line)?;
                                let trimmed = input_line.trim().to_lowercase();
                                if trimmed == "y" || trimmed == "yes" {
                                    consented = true;
                                }
                            }

                            if consented {
                                println!("Installing '{}' via pip...", pkg_to_install);
                                match PackageManager::install_dependency(pkg_to_install, true) {
                                    Ok(msg) => println!("✓ {}", msg.trim()),
                                    Err(e) => {
                                        eprintln!("✗ Failed to install {}: {}", pkg_to_install, e)
                                    }
                                }
                            } else {
                                println!("Installation skipped: user consent was not granted.");
                            }
                        }
                    }
                }

                println!("======================================================================");
            }
            Commands::Compare {
                input,
                page,
                width,
                output_dir,
            } => {
                use crate::viewer::parity::VisualParityComparator;
                let source = std::fs::read_to_string(&input)?;
                println!(
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

                println!("========================================================");
                println!("           LAKETEX VISUAL PARITY BENCHMARK REPORT       ");
                println!("========================================================");
                println!("Document:                 {}", report.document_name);
                println!("Page Analyzed:            Page {}", report.page_number);
                println!(
                    "Canvas Dimensions:        {} x {} px",
                    report.rendered_width_px, report.rendered_height_px
                );
                println!("Total Pixels Evaluated:   {}", report.total_pixels);
                println!(
                    "Matching Pixels:          {} ({:.2}%)",
                    report.matching_pixels, report.visual_parity_score
                );
                println!("Mismatch Pixels:          {}", report.mismatch_pixels);
                println!("Mean Squared Error (MSE): {:.2}", report.mean_squared_error);
                println!(
                    "PSNR (Fidelity):          {:.2} dB",
                    report.peak_signal_to_noise_ratio
                );
                println!(
                    "Tectonic Compilation:     {} ms ({} bytes)",
                    report.tectonic_compile_ms, report.tectonic_pdf_bytes
                );
                println!(
                    "Native Rust Compilation:  {} ms ({} bytes)",
                    report.native_compile_ms, report.native_pdf_bytes
                );
                println!("Output Artifacts Saved:   {}", output_dir.display());
                println!("  -> {}/tectonic_render.png", output_dir.display());
                println!("  -> {}/native_render.png", output_dir.display());
                println!("  -> {}/diff_comparison.png", output_dir.display());
                println!("  -> {}/visual_parity_report.json", output_dir.display());
                println!("========================================================");
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
                    println!("{}", report.to_json());
                } else {
                    print!("{}", report.to_text_summary());
                }

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
                    println!("============================================================");
                    println!("  Laketex Built-in LaTeX Documentation Library");
                    println!("============================================================");
                    println!();
                    for d in docs {
                        println!("  * [{:?}] {} ({})", d.kind, d.title, d.filename);
                        println!("    Summary: {}", d.description);
                        println!("    Lines:   {}", d.content.lines().count());
                        println!();
                    }
                    println!("CLI Usage Examples:");
                    println!("  laketex docs --list                      # List available docs");
                    println!("  laketex docs --print --doc manual        # Print manual to stdout");
                    println!("  laketex docs --export ./docs             # Export all docs to folder");
                    println!("  laketex docs --export ./docs --doc theming");
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
                            println!("✓ Exported {} -> {}", d.filename, out_path.display());
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
                        print!("{}", d.content);
                    } else {
                        eprintln!(
                            "Document '{}' not found. Available: manual, theming, api, architecture",
                            doc
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
