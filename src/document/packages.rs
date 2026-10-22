//! # CTAN Package Detection & Auto-Import Management
//!
//! Scans LaTeX preambles for declared packages (`\usepackage{...}`, `\RequirePackage{...}`),
//! maps packages to friendly descriptions and categories, and provides auto-injection
//! utilities for missing dependencies. Also manages external package tool dependencies
//! (e.g., Pygments for the `minted` package) with explicit user consent workflows.

use crate::error::{LakeTexError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// Classification of package external dependencies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyKind {
    /// Python package installed via pip (e.g. Pygments for minted, pythontex)
    PythonPackage,
    /// System binary required on PATH (e.g. gnuplot, dot, asy)
    SystemBinary,
}

/// Information about an external dependency required by a LaTeX package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDependency {
    /// Associated LaTeX package (e.g. "minted")
    pub latex_package: String,
    /// Human-friendly name of the dependency (e.g. "Pygments")
    pub name: String,
    /// Executable binary expected on PATH or in scripts (e.g. "pygmentize")
    pub binary_name: String,
    /// Pip package name (e.g. "pygments") if installed via Python
    pub pip_package: Option<String>,
    /// Classification of dependency
    pub kind: DependencyKind,
    /// Friendly explanation of why this dependency is needed
    pub description: String,
    /// Whether the binary is currently installed and executable
    pub installed: bool,
    /// Suggested installation command (e.g. "python -m pip install pygments")
    pub install_command: String,
    /// Path to installed binary if found
    pub binary_path: Option<String>,
}

/// Detected Python environment details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PythonEnvironment {
    /// Whether Python interpreter is detected
    pub python_available: bool,
    /// Whether pip package manager is available
    pub pip_available: bool,
    /// Python version string (e.g. "3.12.0")
    pub python_version: Option<String>,
    /// Absolute path to python executable
    pub python_path: Option<String>,
    /// Path to Python Scripts directory
    pub scripts_dir: Option<String>,
}

/// Metadata regarding a declared or detected CTAN package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    /// Package name (e.g. `"amsmath"`, `"graphicx"`)
    pub name: String,
    /// Optional configuration arguments passed in brackets (e.g. `"margin=1in"`)
    pub options: Option<String>,
    /// 1-based source line where package was declared
    pub line: usize,
    /// Friendly explanation of package purpose
    pub description: String,
    /// Broad category tag (e.g. "Math", "Graphics", "Typography")
    pub category: String,
}

/// Package dependency manager and catalog.
pub struct PackageManager;

impl PackageManager {
    /// Extracts all declared packages from LaTeX source code.
    pub fn detect_packages(source: &str) -> Vec<PackageInfo> {
        let mut packages = Vec::new();

        for (line_idx, line) in source.lines().enumerate() {
            let line_num = line_idx + 1;
            let trimmed = line.trim();

            if trimmed.starts_with('%') {
                continue;
            }

            for prefix in ["\\usepackage", "\\RequirePackage"] {
                if let Some(pos) = trimmed.find(prefix) {
                    let mut rest = &trimmed[pos + prefix.len()..];
                    let mut options = None;

                    // Parse optional bracket options [options]
                    let trimmed_rest = rest.trim_start();
                    if trimmed_rest.starts_with('[') {
                        if let Some(end_bracket) = trimmed_rest.find(']') {
                            options = Some(trimmed_rest[1..end_bracket].trim().to_string());
                            rest = &trimmed_rest[end_bracket + 1..];
                        }
                    }

                    // Parse brace contents {pkg1, pkg2}
                    let trimmed_rest = rest.trim_start();
                    if trimmed_rest.starts_with('{') {
                        if let Some(end_brace) = trimmed_rest.find('}') {
                            let pkg_list = &trimmed_rest[1..end_brace];
                            for pkg in pkg_list.split(',') {
                                let pkg_name = pkg.trim();
                                if !pkg_name.is_empty() {
                                    let desc = Self::lookup_description(pkg_name);
                                    let cat = Self::lookup_category(pkg_name);
                                    packages.push(PackageInfo {
                                        name: pkg_name.to_string(),
                                        options: options.clone(),
                                        line: line_num,
                                        description: desc.to_string(),
                                        category: cat.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        packages
    }

    /// Curated catalog of essential and popular CTAN packages
    pub fn catalog() -> &'static [(&'static str, &'static str, &'static str)] {
        &[
            ("amsmath", "AMS mathematical facilities & environments", "Math"),
            ("amssymb", "Extended AMS math symbol libraries", "Math"),
            ("mathtools", "Mathematical tools to use with amsmath", "Math"),
            ("graphicx", "Enhanced support for graphics & image inclusion", "Graphics"),
            ("tikz", "Create sophisticated vector graphics programmatically", "Graphics"),
            ("pgfplots", "Plots and graphs directly in LaTeX", "Graphics"),
            ("xcolor", "Driver-independent color extensions", "Formatting"),
            ("hyperref", "Extensive hypertext cross-referencing and links", "Navigation"),
            ("booktabs", "Publication-quality clean tabular rules", "Tables"),
            ("tabularx", "Tabulars with adjustable width columns", "Tables"),
            ("geometry", "Flexible and complete interface to document dimensions", "Layout"),
            ("microtype", "Subliminal typographic enhancements and micro-kerning", "Typography"),
            ("fontspec", "Advanced font selection in XeLaTeX / LuaLaTeX", "Typography"),
            ("biblatex", "Sophisticated modern bibliographies & citation styles", "Citations"),
            ("csquotes", "Context sensitive quotation facilities", "Typography"),
            ("listings", "Source code syntax printing and highlighting", "Code"),
            ("minted", "Highlighted source code using Pygments", "Code"),
            ("enumitem", "Control layout of enumerate, itemize and description", "Lists"),
            ("fancyhdr", "Extensive control over headers and footers", "Layout"),
            ("cleveref", "Intelligent cross-referencing with automatic names", "Navigation"),
            ("subcaption", "Support for sub-figures and sub-captions", "Figures"),
            ("siunitx", "A comprehensive (SI) units package", "Science"),
            ("algorithm2e", "Floating algorithm environment with formatting", "Algorithms"),
            ("tcolorbox", "Colored and framed text boxes with rounded corners", "Boxes"),
        ]
    }

    /// Looks up description for a package name
    pub fn lookup_description(name: &str) -> &'static str {
        for (pkg, desc, _) in Self::catalog() {
            if *pkg == name {
                return desc;
            }
        }
        "CTAN LaTeX Package (auto-imported on compile)"
    }

    /// Looks up category tag for a package name
    pub fn lookup_category(name: &str) -> &'static str {
        for (pkg, _, cat) in Self::catalog() {
            if *pkg == name {
                return cat;
            }
        }
        "General"
    }

    /// Injects `\usepackage{pkg_name}` into the LaTeX source if not already present.
    pub fn insert_package(source: &str, pkg_name: &str) -> String {
        if source.contains(&format!("{{{pkg_name}}}")) {
            return source.to_string();
        }

        // Find position of \documentclass or preamble
        if let Some(pos) = source.find("\\documentclass") {
            if let Some(newline_offset) = source[pos..].find('\n') {
                let insert_pos = pos + newline_offset + 1;
                let mut new_source = String::with_capacity(source.len() + pkg_name.len() + 32);
                new_source.push_str(&source[..insert_pos]);
                new_source.push_str(&format!("\\usepackage{{{pkg_name}}}\n"));
                new_source.push_str(&source[insert_pos..]);
                return new_source;
            }
        }

        // Fallback: prepend at beginning
        format!("\\usepackage{{{pkg_name}}}\n{source}")
    }

    /// Probes the system Python environment and detects version, path, scripts directory, and pip.
    pub fn check_python_environment() -> PythonEnvironment {
        let candidates = if cfg!(windows) {
            vec!["python", "py", "python3"]
        } else {
            vec!["python3", "python"]
        };

        for candidate in candidates {
            let output = Command::new(candidate)
                .args(["-c", "import sys, sysconfig; print(sys.executable); print(sys.version.split()[0]); print(sysconfig.get_path('scripts'))"])
                .output();

            if let Ok(out) = output {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout);
                    let mut lines = text.lines();
                    let exe_path = lines.next().map(|s| s.trim().to_string());
                    let ver = lines.next().map(|s| s.trim().to_string());
                    let scripts = lines.next().map(|s| s.trim().to_string());

                    // Check if pip is available with this interpreter
                    let pip_status = Command::new(candidate)
                        .args(["-m", "pip", "--version"])
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);

                    return PythonEnvironment {
                        python_available: true,
                        pip_available: pip_status,
                        python_version: ver,
                        python_path: exe_path,
                        scripts_dir: scripts,
                    };
                }
            }
        }

        PythonEnvironment {
            python_available: false,
            pip_available: false,
            python_version: None,
            python_path: None,
            scripts_dir: None,
        }
    }

    /// Checks if an executable exists on PATH or in Python scripts directories.
    pub fn find_binary_path(binary: &str) -> Option<PathBuf> {
        let exe_name = if cfg!(windows) {
            format!("{binary}.exe")
        } else {
            binary.to_string()
        };

        // 1. Check system PATH
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let candidate = dir.join(&exe_name);
                if candidate.is_file() {
                    return Some(candidate);
                }
                let raw_candidate = dir.join(binary);
                if raw_candidate.is_file() {
                    return Some(raw_candidate);
                }
            }
        }

        // 2. Check Python scripts directory
        let env = Self::check_python_environment();
        if let Some(scripts_dir) = env.scripts_dir {
            let scripts_path = PathBuf::from(&scripts_dir);
            let candidate = scripts_path.join(&exe_name);
            if candidate.is_file() {
                Self::add_to_process_path(&scripts_path);
                return Some(candidate);
            }
        }

        // 3. Check common Python scripts paths on Windows
        if cfg!(windows) {
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                let local_path = PathBuf::from(local_app_data).join("Programs").join("Python");
                if local_path.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(local_path) {
                        for entry in entries.flatten() {
                            let scripts = entry.path().join("Scripts");
                            let cand = scripts.join(&exe_name);
                            if cand.is_file() {
                                Self::add_to_process_path(&scripts);
                                return Some(cand);
                            }
                        }
                    }
                }
            }
            if let Ok(app_data) = std::env::var("APPDATA") {
                let python_user_scripts = PathBuf::from(app_data).join("Python");
                if python_user_scripts.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(python_user_scripts) {
                        for entry in entries.flatten() {
                            let scripts = entry.path().join("Scripts");
                            let cand = scripts.join(&exe_name);
                            if cand.is_file() {
                                Self::add_to_process_path(&scripts);
                                return Some(cand);
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Adds a directory to the current process's PATH environment variable if not already present.
    pub fn add_to_process_path(dir: &std::path::Path) {
        if let Ok(cur_path) = std::env::var("PATH") {
            let dir_str = dir.to_string_lossy();
            if !cur_path.to_lowercase().contains(&dir_str.to_lowercase()) {
                let sep = if cfg!(windows) { ";" } else { ":" };
                let new_path = format!("{dir_str}{sep}{cur_path}");
                std::env::set_var("PATH", new_path);
            }
        }
    }

    /// Returns true if a binary is available on PATH or in python scripts.
    pub fn is_binary_available(binary: &str) -> bool {
        Self::find_binary_path(binary).is_some()
    }

    /// Returns catalog of known external tool dependencies for LaTeX packages.
    pub fn get_known_dependencies() -> Vec<PackageDependency> {
        let env = Self::check_python_environment();
        let py_cmd = if env.python_available {
            "python -m pip install pygments".to_string()
        } else {
            "pip install pygments (Python required)".to_string()
        };

        vec![
            PackageDependency {
                latex_package: "minted".to_string(),
                name: "Pygments".to_string(),
                binary_name: "pygmentize".to_string(),
                pip_package: Some("pygments".to_string()),
                kind: DependencyKind::PythonPackage,
                description: "Pygments syntax highlighting engine required by LaTeX minted package"
                    .to_string(),
                installed: Self::is_binary_available("pygmentize"),
                install_command: py_cmd,
                binary_path: Self::find_binary_path("pygmentize")
                    .map(|p| p.to_string_lossy().to_string()),
            },
            PackageDependency {
                latex_package: "pythontex".to_string(),
                name: "PythonTeX".to_string(),
                binary_name: "pythontex".to_string(),
                pip_package: Some("pythontex".to_string()),
                kind: DependencyKind::PythonPackage,
                description: "Executes Python code blocks embedded within LaTeX documents"
                    .to_string(),
                installed: Self::is_binary_available("pythontex"),
                install_command: "python -m pip install pythontex".to_string(),
                binary_path: Self::find_binary_path("pythontex")
                    .map(|p| p.to_string_lossy().to_string()),
            },
            PackageDependency {
                latex_package: "dot2tex".to_string(),
                name: "dot2tex".to_string(),
                binary_name: "dot2tex".to_string(),
                pip_package: Some("dot2tex".to_string()),
                kind: DependencyKind::PythonPackage,
                description: "Converts Graphviz DOT graphs into native PSTricks or TikZ code"
                    .to_string(),
                installed: Self::is_binary_available("dot2tex"),
                install_command: "python -m pip install dot2tex".to_string(),
                binary_path: Self::find_binary_path("dot2tex")
                    .map(|p| p.to_string_lossy().to_string()),
            },
            PackageDependency {
                latex_package: "gnuplottex".to_string(),
                name: "Gnuplot".to_string(),
                binary_name: "gnuplot".to_string(),
                pip_package: None,
                kind: DependencyKind::SystemBinary,
                description: "Generates high-precision scientific plots directly in LaTeX"
                    .to_string(),
                installed: Self::is_binary_available("gnuplot"),
                install_command: if cfg!(windows) {
                    "winget install gnuplot".to_string()
                } else {
                    "brew install gnuplot".to_string()
                },
                binary_path: Self::find_binary_path("gnuplot")
                    .map(|p| p.to_string_lossy().to_string()),
            },
        ]
    }

    /// Detects external dependencies required by packages declared in the document source.
    pub fn detect_dependencies(source: &str) -> Vec<PackageDependency> {
        let declared = Self::detect_packages(source);
        let mut deps = Vec::new();
        let known = Self::get_known_dependencies();

        for pkg in &declared {
            for dep in &known {
                if dep.latex_package == pkg.name {
                    let mut d = dep.clone();
                    d.installed = Self::is_binary_available(&d.binary_name);
                    d.binary_path = Self::find_binary_path(&d.binary_name)
                        .map(|p| p.to_string_lossy().to_string());
                    deps.push(d);
                }
            }
        }

        deps
    }

    /// Installs an external package dependency (e.g. Pygments for minted) with explicit user consent.
    pub fn install_dependency(dep_name: &str, user_consented: bool) -> Result<String> {
        if !user_consented {
            return Err(LakeTexError::Dependency(format!(
                "Installation of external dependency '{dep_name}' aborted: user consent was not granted."
            )));
        }

        let env = Self::check_python_environment();
        if !env.python_available {
            return Err(LakeTexError::Dependency(
                "Python was not detected on system. Please install Python (https://www.python.org) or use Native mode."
                    .to_string(),
            ));
        }
        if !env.pip_available {
            return Err(LakeTexError::Dependency(
                "pip package manager was not found in the detected Python installation."
                    .to_string(),
            ));
        }

        let python_exe = env.python_path.as_deref().unwrap_or("python");
        let output = Command::new(python_exe)
            .args(["-m", "pip", "install", dep_name])
            .output()
            .map_err(|e| LakeTexError::Dependency(format!("Failed to spawn pip installer: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            if let Some(scripts) = &env.scripts_dir {
                Self::add_to_process_path(&PathBuf::from(scripts));
            }
            Ok(format!(
                "Successfully installed '{dep_name}' via pip.\n{}",
                stdout.trim()
            ))
        } else {
            Err(LakeTexError::Dependency(format!(
                "pip install {dep_name} failed with exit code {:?}:\n{}\n{}",
                output.status.code(),
                stdout,
                stderr
            )))
        }
    }
}
