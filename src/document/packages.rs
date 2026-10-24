//! # CTAN Package Detection, Dependency Consent & Conda/Python Management
//!
//! Scans LaTeX preambles for declared packages (`\usepackage{...}`, `\RequirePackage{...}`),
//! maps packages to friendly descriptions and categories, and provides auto-injection
//! utilities for missing dependencies.
//!
//! Also manages external host tool dependencies (e.g., Pygments for the `minted` package,
//! PythonTeX, dot2tex) with explicit user consent workflows and deep environment discovery:
//! - Auto-discovery of active Conda (`CONDA_PREFIX`), virtual environments (`VIRTUAL_ENV`),
//!   and registered Conda environments in `~/.conda/environments.txt`.
//! - Support for standard Python installations as well as Miniconda, Anaconda, Miniforge,
//!   Mambaforge, and Micromamba across Windows, macOS, and Linux.
//! - Multi-strategy package installation trying standard `pip`, user-level install (`--user`),
//!   PEP 668 `--break-system-packages`, and falling back to `conda install -y -c conda-forge`
//!   if `pip` is absent in a Conda environment.
//! - Post-install binary verification to guarantee that required binaries are located on disk
//!   and automatically synchronized into the compiler process's `PATH`.

use crate::error::{LakeTexError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

static CACHED_PYTHON_ENV: RwLock<Option<PythonEnvironment>> = RwLock::new(None);

/// Static specification of a known LaTeX package external tool dependency.
pub struct DependencySpec {
    /// Name of the LaTeX package requiring this external tool (e.g., `"minted"`).
    pub latex_package: &'static str,
    /// Human-readable display name of the tool (e.g., `"Pygments"`).
    pub name: &'static str,
    /// Executable CLI command name searched in system `PATH` (e.g., `"pygmentize"`).
    pub binary_name: &'static str,
    /// Corresponding package name on PyPI if installable via pip (e.g., `Some("pygments")`).
    pub pip_package: Option<&'static str>,
    /// Classification of the dependency (system binary, Python package, Perl script).
    pub kind: DependencyKind,
    /// Explanation of why the LaTeX package requires this external dependency.
    pub description: &'static str,
    /// Suggested installation command shown to the user or executed with consent.
    pub install_command: &'static str,
}

/// Known registry of LaTeX packages with external tool dependencies.
pub const KNOWN_DEPENDENCY_SPECS: &[DependencySpec] = &[
    DependencySpec {
        latex_package: "minted",
        name: "Pygments",
        binary_name: "pygmentize",
        pip_package: Some("pygments"),
        kind: DependencyKind::PythonPackage,
        description: "Pygments syntax highlighting engine required by LaTeX minted package",
        install_command: "python -m pip install pygments",
    },
    DependencySpec {
        latex_package: "pythontex",
        name: "PythonTeX",
        binary_name: "pythontex",
        pip_package: Some("pythontex"),
        kind: DependencyKind::PythonPackage,
        description: "Executes Python code blocks embedded within LaTeX documents",
        install_command: "python -m pip install pythontex",
    },
    DependencySpec {
        latex_package: "dot2tex",
        name: "dot2tex",
        binary_name: "dot2tex",
        pip_package: Some("dot2tex"),
        kind: DependencyKind::PythonPackage,
        description: "Converts Graphviz DOT graphs into native PSTricks or TikZ code",
        install_command: "python -m pip install dot2tex",
    },
    DependencySpec {
        latex_package: "gnuplottex",
        name: "Gnuplot",
        binary_name: "gnuplot",
        pip_package: None,
        kind: DependencyKind::SystemBinary,
        description: "Generates high-precision scientific plots directly in LaTeX",
        install_command: if cfg!(windows) {
            "winget install gnuplot"
        } else {
            "brew install gnuplot"
        },
    },
];

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
    /// Path to user-level Python Scripts directory (e.g. for `pip install --user`)
    #[serde(default)]
    pub user_scripts_dir: Option<String>,
    /// Whether this environment is managed by Conda / Miniconda / Miniforge / Mamba
    #[serde(default)]
    pub is_conda: bool,
    /// Conda prefix path if this is a Conda environment
    #[serde(default)]
    pub conda_prefix: Option<String>,
    /// Path to conda CLI executable if available for fallback installation
    #[serde(default)]
    pub conda_cli_path: Option<String>,
    /// Friendly environment name (e.g. "base", "ecor1041", or virtualenv name)
    #[serde(default)]
    pub environment_name: Option<String>,
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

    /// Invalidates the cached Python environment probe, forcing a refresh on next access.
    pub fn invalidate_python_environment() {
        if let Ok(mut lock) = CACHED_PYTHON_ENV.write() {
            *lock = None;
        }
    }

    /// Probes the system Python environment and detects version, path, scripts directory, and pip.
    /// Results are cached statically to avoid spawning subprocesses repeatedly.
    pub fn check_python_environment() -> PythonEnvironment {
        if let Ok(read) = CACHED_PYTHON_ENV.read() {
            if let Some(env) = read.as_ref() {
                return env.clone();
            }
        }
        let probed = Self::probe_python_environment();
        if let Ok(mut write) = CACHED_PYTHON_ENV.write() {
            *write = Some(probed.clone());
        }
        probed
    }

    /// Discovers all available Conda, Miniconda, Miniforge, and Mamba environments on the system.
    pub fn discover_conda_environments() -> Vec<PathBuf> {
        let mut envs: Vec<PathBuf> = Vec::new();

        // 1. Check active CONDA_PREFIX
        if let Ok(prefix) = std::env::var("CONDA_PREFIX") {
            let p = PathBuf::from(prefix);
            if p.is_dir() && !envs.contains(&p) {
                envs.push(p);
            }
        }

        // 2. Check ~/.conda/environments.txt registry
        let env_file = if cfg!(windows) {
            std::env::var("USERPROFILE")
                .ok()
                .map(|up| PathBuf::from(up).join(".conda").join("environments.txt"))
        } else {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".conda").join("environments.txt"))
        };

        if let Some(file_path) = env_file {
            if file_path.is_file() {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            let p = PathBuf::from(trimmed);
                            if p.is_dir() && !envs.contains(&p) {
                                envs.push(p);
                            }
                        }
                    }
                }
            }
        }

        // 3. Known standard base directories for Conda, Miniconda, Miniforge, Mambaforge, Micromamba
        let mut base_dirs: Vec<PathBuf> = Vec::new();
        if cfg!(windows) {
            if let Ok(user_profile) = std::env::var("USERPROFILE") {
                let up = PathBuf::from(user_profile);
                for name in &[
                    "miniconda3",
                    "miniforge3",
                    "anaconda3",
                    "mambaforge",
                    "micromamba",
                ] {
                    base_dirs.push(up.join(name));
                }
            }
            if let Ok(program_data) = std::env::var("ProgramData") {
                let pd = PathBuf::from(program_data);
                for name in &["miniconda3", "miniforge3", "anaconda3", "mambaforge"] {
                    base_dirs.push(pd.join(name));
                }
            }
            for drive_root in &[
                "C:\\miniconda3",
                "C:\\miniforge3",
                "C:\\anaconda3",
                "C:\\mambaforge",
            ] {
                base_dirs.push(PathBuf::from(drive_root));
            }
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                let lad = PathBuf::from(local_app_data);
                for name in &["miniconda3", "miniforge3", "anaconda3"] {
                    base_dirs.push(lad.join(name));
                }
            }
        } else {
            if let Ok(home) = std::env::var("HOME") {
                let h = PathBuf::from(home);
                for name in &[
                    "miniconda3",
                    "miniforge3",
                    "anaconda3",
                    "mambaforge",
                    ".micromamba",
                ] {
                    base_dirs.push(h.join(name));
                }
            }
            for root in &[
                "/opt/conda",
                "/opt/miniconda3",
                "/opt/miniforge3",
                "/opt/anaconda3",
                "/opt/homebrew/Caskroom/miniforge/base",
                "/opt/homebrew/Caskroom/miniconda/base",
                "/usr/local/miniconda3",
                "/usr/local/anaconda3",
            ] {
                base_dirs.push(PathBuf::from(root));
            }
        }

        for base in base_dirs {
            if base.is_dir() {
                if !envs.contains(&base) {
                    envs.push(base.clone());
                }
                // Also check sub-environments in base/envs/
                let envs_subdir = base.join("envs");
                if envs_subdir.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(&envs_subdir) {
                        for entry in entries.flatten() {
                            let sub_p = entry.path();
                            if sub_p.is_dir() && !envs.contains(&sub_p) {
                                envs.push(sub_p);
                            }
                        }
                    }
                }
            }
        }

        envs
    }

    /// Locates the `conda` (or `mamba`/`micromamba`) CLI executable.
    pub fn find_conda_cli(prefix: Option<&Path>) -> Option<String> {
        // 1. Direct check in prefix condabin, Scripts, or bin
        if let Some(p) = prefix {
            let candidates = if cfg!(windows) {
                vec![
                    p.join("condabin").join("conda.bat"),
                    p.join("Scripts").join("conda.exe"),
                    p.join("Library").join("bin").join("conda.bat"),
                ]
            } else {
                vec![
                    p.join("bin").join("conda"),
                    p.join("condabin").join("conda"),
                    p.join("bin").join("mamba"),
                    p.join("bin").join("micromamba"),
                ]
            };
            for cand in candidates {
                if cand.is_file() {
                    return Some(cand.to_string_lossy().to_string());
                }
            }

            // If prefix is an env (e.g. miniconda3/envs/myenv), check root conda base
            if let Some(parent) = p.parent() {
                if parent.file_name().and_then(|s| s.to_str()) == Some("envs") {
                    if let Some(root) = parent.parent() {
                        let root_cand = if cfg!(windows) {
                            root.join("condabin").join("conda.bat")
                        } else {
                            root.join("bin").join("conda")
                        };
                        if root_cand.is_file() {
                            return Some(root_cand.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        // 2. Check CONDA_EXE environment variable
        if let Ok(exe) = std::env::var("CONDA_EXE") {
            let p = PathBuf::from(exe);
            if p.is_file() {
                return Some(p.to_string_lossy().to_string());
            }
        }

        // 3. Check system PATH
        if let Ok(path_var) = std::env::var("PATH") {
            let exe_names: &[&str] = if cfg!(windows) {
                &[
                    "conda.bat",
                    "conda.exe",
                    "mamba.bat",
                    "mamba.exe",
                    "micromamba.exe",
                ]
            } else {
                &["conda", "mamba", "micromamba"]
            };
            for dir in std::env::split_paths(&path_var) {
                for exe in exe_names {
                    let cand = dir.join(exe);
                    if cand.is_file() {
                        return Some(cand.to_string_lossy().to_string());
                    }
                }
            }
        }

        None
    }

    fn probe_python_environment() -> PythonEnvironment {
        let mut candidates: Vec<String> = Vec::new();

        // 1. Active virtualenv (VIRTUAL_ENV)
        if let Ok(venv) = std::env::var("VIRTUAL_ENV") {
            let py = if cfg!(windows) {
                PathBuf::from(&venv).join("Scripts").join("python.exe")
            } else {
                PathBuf::from(&venv).join("bin").join("python")
            };
            if py.is_file() {
                candidates.push(py.to_string_lossy().to_string());
            }
        }

        // 2. Active Conda environment (CONDA_PREFIX)
        if let Ok(prefix) = std::env::var("CONDA_PREFIX") {
            let py = if cfg!(windows) {
                PathBuf::from(&prefix).join("python.exe")
            } else {
                PathBuf::from(&prefix).join("bin").join("python")
            };
            if py.is_file() {
                let s = py.to_string_lossy().to_string();
                if !candidates.contains(&s) {
                    candidates.push(s);
                }
            }
        }

        // 3. Discovered Conda environments
        for env in Self::discover_conda_environments() {
            let py = if cfg!(windows) {
                env.join("python.exe")
            } else {
                env.join("bin").join("python")
            };
            if py.is_file() {
                let s = py.to_string_lossy().to_string();
                if !candidates.contains(&s) {
                    candidates.push(s);
                }
            }
        }

        // 4. Standard Windows locations
        if cfg!(windows) {
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                let py_programs = PathBuf::from(&local_app_data).join("Programs").join("Python");
                if py_programs.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(py_programs) {
                        for entry in entries.flatten() {
                            let cand = entry.path().join("python.exe");
                            if cand.is_file() {
                                let s = cand.to_string_lossy().to_string();
                                if !candidates.contains(&s) {
                                    candidates.push(s);
                                }
                            }
                        }
                    }
                }
            }
            for drive in [
                "C:\\Python313",
                "C:\\Python312",
                "C:\\Python311",
                "C:\\Python310",
                "C:\\Python39",
            ] {
                let cand = PathBuf::from(drive).join("python.exe");
                if cand.is_file() {
                    let s = cand.to_string_lossy().to_string();
                    if !candidates.contains(&s) {
                        candidates.push(s);
                    }
                }
            }
        }

        // 5. Standard system command names
        let default_names: &[&str] = if cfg!(windows) {
            &["python", "py", "python3"]
        } else {
            &["python3", "python"]
        };
        for name in default_names {
            let s = name.to_string();
            if !candidates.contains(&s) {
                candidates.push(s);
            }
        }

        struct ProbedCandidate {
            env: PythonEnvironment,
            score: i32,
        }

        let mut probed_list: Vec<ProbedCandidate> = Vec::new();

        for candidate in &candidates {
            let output = crate::create_hidden_command(candidate)
                .args([
                    "-c",
                    "import sys, sysconfig, os, site; print(sys.executable); print(sys.version.split()[0]); print(sysconfig.get_path('scripts') or ''); ub = getattr(site, 'USER_BASE', ''); print(os.path.join(ub, 'Scripts' if os.name == 'nt' else 'bin') if ub else ''); is_conda = ('conda' in sys.version.lower()) or ('continuum' in sys.version.lower()) or os.path.exists(os.path.join(sys.prefix, 'conda-meta')); print('1' if is_conda else '0'); print(sys.prefix if is_conda else ''); print(os.path.basename(sys.prefix) if is_conda else '')",
                ])
                .output();

            if let Ok(out) = output {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout);
                    let mut lines = text.lines();
                    let exe_path = lines
                        .next()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    let ver = lines
                        .next()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    let scripts = lines
                        .next()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    let user_scripts = lines
                        .next()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    let is_conda = lines.next().map(|s| s.trim() == "1").unwrap_or(false);
                    let conda_prefix = lines
                        .next()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    let env_name = lines
                        .next()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());

                    // Check if pip is available with this interpreter
                    let pip_status = crate::create_hidden_command(candidate)
                        .args(["-m", "pip", "--version"])
                        .output()
                        .map(|o| o.status.success())
                        .unwrap_or(false);

                    let conda_cli_path = if is_conda {
                        Self::find_conda_cli(conda_prefix.as_deref().map(Path::new))
                    } else {
                        None
                    };

                    let mut score: i32 = 10;
                    if let Some(exe) = &exe_path {
                        // Heavily penalize Windows Store zero-byte redirector stubs
                        if exe.to_lowercase().contains("windowsapps") {
                            score -= 200;
                        }
                    }
                    if pip_status {
                        score += 50;
                    }
                    if is_conda && conda_cli_path.is_some() {
                        score += 25;
                    }
                    if let Ok(cur_conda) = std::env::var("CONDA_PREFIX") {
                        if let Some(pfx) = &conda_prefix {
                            if pfx.eq_ignore_ascii_case(&cur_conda) {
                                score += 100;
                            }
                        }
                    }
                    if let Ok(cur_venv) = std::env::var("VIRTUAL_ENV") {
                        if let Some(exe) = &exe_path {
                            if exe.to_lowercase().contains(&cur_venv.to_lowercase()) {
                                score += 100;
                            }
                        }
                    }
                    // Bonus if pygmentize is already in this candidate's scripts
                    if let Some(sc) = &scripts {
                        let cand_pyg = if cfg!(windows) {
                            PathBuf::from(sc).join("pygmentize.exe")
                        } else {
                            PathBuf::from(sc).join("pygmentize")
                        };
                        if cand_pyg.is_file() {
                            score += 40;
                        }
                    }

                    let env = PythonEnvironment {
                        python_available: true,
                        pip_available: pip_status,
                        python_version: ver,
                        python_path: exe_path,
                        scripts_dir: scripts,
                        user_scripts_dir: user_scripts,
                        is_conda,
                        conda_prefix,
                        conda_cli_path,
                        environment_name: env_name,
                    };

                    probed_list.push(ProbedCandidate { env, score });
                }
            }
        }

        // Sort descending by score
        probed_list.sort_by(|a, b| b.score.cmp(&a.score));

        if let Some(best) = probed_list.into_iter().next() {
            if best.score > 0 {
                if let Some(scripts_dir) = &best.env.scripts_dir {
                    Self::add_to_process_path(Path::new(scripts_dir));
                }
                if let Some(user_scripts_dir) = &best.env.user_scripts_dir {
                    let p = Path::new(user_scripts_dir);
                    if p.is_dir() {
                        Self::add_to_process_path(p);
                    }
                }
                if let Some(prefix) = &best.env.conda_prefix {
                    if cfg!(windows) {
                        let p = Path::new(prefix);
                        Self::add_to_process_path(&p.join("Library").join("bin"));
                        Self::add_to_process_path(&p.join("Scripts"));
                    }
                }
                return best.env;
            }
        }

        PythonEnvironment {
            python_available: false,
            pip_available: false,
            python_version: None,
            python_path: None,
            scripts_dir: None,
            user_scripts_dir: None,
            is_conda: false,
            conda_prefix: None,
            conda_cli_path: None,
            environment_name: None,
        }
    }

    /// Checks if an executable exists on PATH, Conda environments, virtual environments,
    /// or Python scripts directories.
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

        // 2. Check detected Python scripts directory
        let env = Self::check_python_environment();
        if let Some(scripts_dir) = &env.scripts_dir {
            let scripts_path = PathBuf::from(scripts_dir);
            let candidate = scripts_path.join(&exe_name);
            if candidate.is_file() {
                Self::add_to_process_path(&scripts_path);
                return Some(candidate);
            }
        }

        // 3. Check detected Python user-level scripts directory (e.g. from pip install --user)
        if let Some(user_scripts_dir) = &env.user_scripts_dir {
            let user_scripts_path = PathBuf::from(user_scripts_dir);
            let candidate = user_scripts_path.join(&exe_name);
            if candidate.is_file() {
                Self::add_to_process_path(&user_scripts_path);
                return Some(candidate);
            }
        }

        // 4. Check adjacent to detected Python executable
        if let Some(py_path) = &env.python_path {
            let py_buf = PathBuf::from(py_path);
            if let Some(py_dir) = py_buf.parent() {
                let scripts = py_dir.join("Scripts");
                let candidate = scripts.join(&exe_name);
                if candidate.is_file() {
                    Self::add_to_process_path(&scripts);
                    return Some(candidate);
                }
                let bin = py_dir.join("bin");
                let cand_bin = bin.join(&exe_name);
                if cand_bin.is_file() {
                    Self::add_to_process_path(&bin);
                    return Some(cand_bin);
                }
                let lib_bin = py_dir.join("Library").join("bin");
                let cand_lib = lib_bin.join(&exe_name);
                if cand_lib.is_file() {
                    Self::add_to_process_path(&lib_bin);
                    return Some(cand_lib);
                }
            }
        }

        // 5. Check active virtualenv (VIRTUAL_ENV)
        if let Ok(venv) = std::env::var("VIRTUAL_ENV") {
            let venv_path = PathBuf::from(&venv);
            let check_dirs = if cfg!(windows) {
                vec![venv_path.join("Scripts"), venv_path.join("bin")]
            } else {
                vec![venv_path.join("bin")]
            };
            for dir in check_dirs {
                let cand = dir.join(&exe_name);
                if cand.is_file() {
                    Self::add_to_process_path(&dir);
                    return Some(cand);
                }
            }
        }

        // 6. Check active Conda prefix (CONDA_PREFIX)
        if let Ok(prefix) = std::env::var("CONDA_PREFIX") {
            let prefix_path = PathBuf::from(&prefix);
            let check_dirs = if cfg!(windows) {
                vec![
                    prefix_path.join("Scripts"),
                    prefix_path.join("Library").join("bin"),
                    prefix_path.join("bin"),
                ]
            } else {
                vec![prefix_path.join("bin")]
            };
            for dir in check_dirs {
                let cand = dir.join(&exe_name);
                if cand.is_file() {
                    Self::add_to_process_path(&dir);
                    return Some(cand);
                }
            }
        }

        // 7. Check all discovered Conda environments (base and named envs in ~/.conda/environments.txt)
        for env_path in Self::discover_conda_environments() {
            let check_dirs = if cfg!(windows) {
                vec![
                    env_path.join("Scripts"),
                    env_path.join("Library").join("bin"),
                    env_path.join("bin"),
                ]
            } else {
                vec![env_path.join("bin")]
            };
            for dir in check_dirs {
                let cand = dir.join(&exe_name);
                if cand.is_file() {
                    Self::add_to_process_path(&dir);
                    return Some(cand);
                }
            }
        }

        // 8. Check common Python scripts paths on Windows
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
                    // Check directly inside APPDATA\Python\Scripts
                    let direct_scripts = python_user_scripts.join("Scripts");
                    let direct_cand = direct_scripts.join(&exe_name);
                    if direct_cand.is_file() {
                        Self::add_to_process_path(&direct_scripts);
                        return Some(direct_cand);
                    }
                    if let Ok(entries) = std::fs::read_dir(&python_user_scripts) {
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
        } else {
            // Check ~/.local/bin and /usr/local/bin on Unix
            if let Ok(home) = std::env::var("HOME") {
                let local_bin = PathBuf::from(home).join(".local").join("bin");
                let cand = local_bin.join(&exe_name);
                if cand.is_file() {
                    Self::add_to_process_path(&local_bin);
                    return Some(cand);
                }
            }
            let usr_local_bin = PathBuf::from("/usr/local/bin");
            let cand = usr_local_bin.join(&exe_name);
            if cand.is_file() {
                Self::add_to_process_path(&usr_local_bin);
                return Some(cand);
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
                // SAFETY: Updating process PATH for external tool discovery.
                unsafe {
                    std::env::set_var("PATH", new_path);
                }
            }
        }
    }

    /// Returns true if a binary is available on PATH or in python scripts.
    pub fn is_binary_available(binary: &str) -> bool {
        Self::find_binary_path(binary).is_some()
    }

    /// Returns catalog of known external tool dependencies for LaTeX packages.
    pub fn get_known_dependencies() -> Vec<PackageDependency> {
        KNOWN_DEPENDENCY_SPECS
            .iter()
            .map(|spec| {
                let bin_path = Self::find_binary_path(spec.binary_name);
                let installed = bin_path.is_some() || Self::is_binary_available(spec.binary_name);
                PackageDependency {
                    latex_package: spec.latex_package.to_string(),
                    name: spec.name.to_string(),
                    binary_name: spec.binary_name.to_string(),
                    pip_package: spec.pip_package.map(|s| s.to_string()),
                    kind: spec.kind.clone(),
                    description: spec.description.to_string(),
                    installed,
                    install_command: spec.install_command.to_string(),
                    binary_path: bin_path.map(|p| p.to_string_lossy().to_string()),
                }
            })
            .collect()
    }

    /// Detects external dependencies required by packages declared in the document source.
    /// Fast: only performs binary lookups for packages actually declared in `source`.
    pub fn detect_dependencies(source: &str) -> Vec<PackageDependency> {
        let declared = Self::detect_packages(source);
        let mut deps = Vec::new();

        for pkg in &declared {
            if let Some(spec) = KNOWN_DEPENDENCY_SPECS
                .iter()
                .find(|s| s.latex_package == pkg.name)
            {
                let bin_path = Self::find_binary_path(spec.binary_name);
                let installed = bin_path.is_some() || Self::is_binary_available(spec.binary_name);
                deps.push(PackageDependency {
                    latex_package: spec.latex_package.to_string(),
                    name: spec.name.to_string(),
                    binary_name: spec.binary_name.to_string(),
                    pip_package: spec.pip_package.map(|s| s.to_string()),
                    kind: spec.kind.clone(),
                    description: spec.description.to_string(),
                    installed,
                    install_command: spec.install_command.to_string(),
                    binary_path: bin_path.map(|p| p.to_string_lossy().to_string()),
                });
            }
        }

        deps
    }

    /// Installs an external package dependency (e.g. Pygments for minted) with explicit user consent.
    ///
    /// # Arguments
    /// * `dep_name` - Name or alias of the dependency (e.g. `"pygments"`, `"minted"`).
    /// * `user_consented` - Boolean flag indicating that the end user has explicitly authorized the install.
    ///
    /// # Errors
    /// Returns [`LakeTexError::Dependency`] if:
    /// - User consent was not granted (`user_consented = false`)
    /// - `dep_name` is not in the approved static [`KNOWN_DEPENDENCY_SPECS`] whitelist
    /// - The registered pip package name fails character validation
    /// - The Python interpreter or `pip` is unavailable
    /// - `pip install` subprocess execution fails
    ///
    /// # Security Policy
    /// Strictly limits installation targets to trusted, statically defined package specifications.
    /// Arbitrary package names or command-line flags cannot be injected into the `pip` command.
    pub fn install_dependency(dep_name: &str, user_consented: bool) -> Result<String> {
        if !user_consented {
            return Err(LakeTexError::Dependency(format!(
                "Installation of external dependency '{dep_name}' aborted: user consent was not granted."
            )));
        }

        // Whitelist validation: only allow pre-registered, known dependencies
        let spec = KNOWN_DEPENDENCY_SPECS
            .iter()
            .find(|s| {
                s.pip_package == Some(dep_name)
                    || s.latex_package == dep_name
                    || s.binary_name == dep_name
                    || s.name.eq_ignore_ascii_case(dep_name)
            })
            .ok_or_else(|| {
                LakeTexError::Dependency(format!(
                    "Security policy violation: '{dep_name}' is not in the approved package registry."
                ))
            })?;

        let pip_pkg = spec.pip_package.ok_or_else(|| {
            LakeTexError::Dependency(format!(
                "Dependency '{}' cannot be installed via pip (install command: {})",
                spec.name, spec.install_command
            ))
        })?;

        // Strict character verification (prevent flag injection or arbitrary pip arguments)
        if !pip_pkg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(LakeTexError::Dependency(format!(
                "Security policy violation: package name '{pip_pkg}' contains illegal characters."
            )));
        }

        let env = Self::check_python_environment();
        if !env.python_available {
            return Err(LakeTexError::Dependency(
                "Python was not detected on system. Please install Python (https://www.python.org) or Conda, or use Native mode."
                    .to_string(),
            ));
        }

        let python_exe = env.python_path.as_deref().unwrap_or("python");
        let mut install_log: Vec<String> = Vec::new();
        let mut install_success = false;

        // Strategy 1: Standard pip install
        if env.pip_available {
            let res = crate::create_hidden_command(python_exe)
                .args(["-m", "pip", "install", pip_pkg])
                .output();
            if let Ok(out) = res {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let log_entry = format!("pip install:\n{}\n{}", stdout.trim(), stderr.trim());
                if out.status.success() {
                    install_success = true;
                    install_log.push(log_entry);
                } else {
                    install_log.push(format!(
                        "pip install (exit {:?}):\n{}\n{}",
                        out.status.code(),
                        stdout.trim(),
                        stderr.trim()
                    ));
                }
            }

            // Strategy 2: If standard pip failed (e.g. system permissions), retry with --user
            if !install_success {
                let res_user = crate::create_hidden_command(python_exe)
                    .args(["-m", "pip", "install", "--user", pip_pkg])
                    .output();
                if let Ok(out) = res_user {
                    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                    let log_entry = format!("pip install --user:\n{}\n{}", stdout.trim(), stderr.trim());
                    if out.status.success() {
                        install_success = true;
                        install_log.push(log_entry);
                    } else {
                        install_log.push(format!(
                            "pip install --user (exit {:?}):\n{}\n{}",
                            out.status.code(),
                            stdout.trim(),
                            stderr.trim()
                        ));
                    }
                }
            }

            // Strategy 3: If still failed (e.g. PEP 668 externally-managed-environment), retry with --break-system-packages
            if !install_success {
                let res_break = crate::create_hidden_command(python_exe)
                    .args(["-m", "pip", "install", "--break-system-packages", pip_pkg])
                    .output();
                if let Ok(out) = res_break {
                    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                    let log_entry = format!(
                        "pip install --break-system-packages:\n{}\n{}",
                        stdout.trim(),
                        stderr.trim()
                    );
                    if out.status.success() {
                        install_success = true;
                        install_log.push(log_entry);
                    } else {
                        install_log.push(format!(
                            "pip install --break-system-packages (exit {:?}):\n{}\n{}",
                            out.status.code(),
                            stdout.trim(),
                            stderr.trim()
                        ));
                    }
                }
            }
        }

        // Strategy 4: If pip failed or was unavailable, and this is a Conda environment, fallback to conda / mamba / micromamba
        if !install_success && env.is_conda {
            if let Some(conda_cli) = &env.conda_cli_path {
                let mut cmd = crate::create_hidden_command(conda_cli);
                cmd.arg("install").arg("-y").arg("-c").arg("conda-forge");
                if let Some(prefix) = &env.conda_prefix {
                    cmd.arg("-p").arg(prefix);
                }
                cmd.arg(pip_pkg);
                if let Ok(out) = cmd.output() {
                    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                    let log_entry = format!("conda install:\n{}\n{}", stdout.trim(), stderr.trim());
                    if out.status.success() {
                        install_success = true;
                        install_log.push(log_entry);
                    } else {
                        install_log.push(format!(
                            "conda install (exit {:?}):\n{}\n{}",
                            out.status.code(),
                            stdout.trim(),
                            stderr.trim()
                        ));
                    }
                }
            }
        }

        if !install_success {
            return Err(LakeTexError::Dependency(format!(
                "Failed to install package '{pip_pkg}'. Attempted installer output:\n{}",
                install_log.join("\n---\n")
            )));
        }

        // Post-installation verification gate: ensure binary actually exists on disk
        Self::invalidate_python_environment();
        let _ = Self::check_python_environment();
        let found_path = Self::find_binary_path(spec.binary_name);

        match found_path {
            Some(path) => {
                if let Some(parent) = path.parent() {
                    Self::add_to_process_path(parent);
                }
                Ok(format!(
                    "Successfully installed and verified '{dep_name}' at: {}\n{}",
                    path.display(),
                    install_log.last().cloned().unwrap_or_default().trim()
                ))
            }
            None => {
                Err(LakeTexError::Dependency(format!(
                    "Package installation for '{pip_pkg}' completed, but the expected binary '{}' was not located in PATH or any Python scripts directory.\nLogs:\n{}",
                    spec.binary_name,
                    install_log.join("\n")
                )))
            }
        }
    }
}
