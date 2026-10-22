//! # CTAN Package Detection & Auto-Import Management
//!
//! Scans LaTeX preambles for declared packages (`\usepackage{...}`, `\RequirePackage{...}`),
//! maps packages to friendly descriptions and categories, and provides auto-injection
//! utilities for missing dependencies.

use serde::{Deserialize, Serialize};

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
}
