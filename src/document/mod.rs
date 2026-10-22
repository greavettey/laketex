//! # Document Representation & In-Memory LaTeX Buffer
//!
//! Provides the primary [`Document`] model, which holds the active LaTeX source string,
//! cached structural section outline ([`outline::OutlineItem`]), detected CTAN packages
//! ([`packages::PackageInfo`]), dirty state tracking, and file I/O operations.

pub mod lexer;
pub mod outline;
pub mod packages;

use outline::{OutlineItem, OutlineParser};
use packages::{PackageInfo, PackageManager};
use std::path::{Path, PathBuf};

/// Default modern LaTeX document template initialized when creating a new file.
pub const DEFAULT_DOCUMENT_TEMPLATE: &str = r#"\documentclass[11pt,a4paper]{article}

% --- Essential Packages (Auto-imported by Laketex) ---
\usepackage{amsmath}
\usepackage{amssymb}
\usepackage{graphicx}
\usepackage{booktabs}
\usepackage{xcolor}
\usepackage{hyperref}

\hypersetup{
    colorlinks=true,
    linkcolor=blue,
    filecolor=magenta,      
    urlcolor=cyan,
    pdftitle={Laketex Document},
    pdfpagemode=FullScreen,
}

\title{\textbf{Laketex}: High-Performance Modern \LaTeX}
\author{Research \& Engineering Group}
\date{\today}

\begin{document}

\maketitle

\begin{abstract}
Laketex is a lightweight, zero-installation, information-dense \LaTeX\ editor and live viewer written in Rust. It features real-time debounced compilation, automatic CTAN package provisioning, crisp high-DPI PDF previewing, and a headless API.
\end{abstract}

\section{Introduction}
Modern scientific communication requires precision, aesthetic elegance, and rapid iteration. Laketex replaces gigabyte-heavy distributions with a sub-second, memory-efficient single binary.

\subsection{Key Capabilities}
\begin{itemize}
    \item \textbf{Sub-second Live Recompile}: Experience live updates as you write.
    \item \textbf{Automatic CTAN Fetch}: No more missing package errors.
    \item \textbf{Information Density}: Code editor, outline navigator, package inspector, and live PDF canvas in one unified view.
    \item \textbf{Headless REST API}: Fully scriptable via HTTP endpoints and CLI commands.
\end{itemize}

\section{Mathematical Formulations}
Laketex renders standard AMS mathematics with vector fidelity:
\begin{equation}
\label{eq:fourier}
\hat{f}(\xi) = \int_{-\infty}^{\infty} f(x)\,e^{-2\pi i x \xi}\,dx
\end{equation}

In Equation~\ref{eq:fourier}, the continuous Fourier transform maps time-domain signals into the frequency domain.

\subsection{Matrix Computations}
Matrices and tensor expressions compile seamlessly:
\begin{equation}
\mathbf{H} = \begin{pmatrix}
\frac{\partial^2 f}{\partial x_1^2} & \cdots & \frac{\partial^2 f}{\partial x_1 \partial x_n} \\
\vdots & \ddots & \vdots \\
\frac{\partial^2 f}{\partial x_n \partial x_1} & \cdots & \frac{\partial^2 f}{\partial x_n^2}
\end{pmatrix}
\end{equation}

\section{Publication Tables}
Clean tabular structures using standard \texttt{booktabs}:
\begin{table}[htbp]
\centering
\begin{tabular}{llrr}
\toprule
\textbf{Tool} & \textbf{Language} & \textbf{Binary Size} & \textbf{Startup (ms)} \\
\midrule
TeXstudio & C++/Qt & 180 MB & 3,200 \\
VS Code + LaTeX & Electron/JS & 520 MB & 4,500 \\
Overleaf & Cloud/Node & Web App & 1,800 \\
\textbf{Laketex} & \textbf{Rust} & \textbf{16 MB} & \textbf{75} \\
\bottomrule
\end{tabular}
\caption{Comparative performance metrics.}
\label{tab:benchmarks}
\end{table}

\section{Conclusion}
Laketex brings the modern Rust tooling ethos to \LaTeX\ typesetting.

\end{document}
"#;

/// In-memory representation of an open LaTeX document.
pub struct Document {
    /// Document LaTeX source code
    pub content: String,
    /// Absolute or relative path on disk if saved
    pub path: Option<PathBuf>,
    /// Whether unsaved changes exist
    pub is_dirty: bool,
    /// Cached hierarchical outline tree
    pub outline_cache: Vec<OutlineItem>,
    /// Cached list of declared CTAN packages
    pub package_cache: Vec<PackageInfo>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new(DEFAULT_DOCUMENT_TEMPLATE.to_string(), None)
    }
}

impl Document {
    /// Creates a new document from source text and an optional filesystem path
    pub fn new(content: String, path: Option<PathBuf>) -> Self {
        let outline_cache = OutlineParser::parse(&content);
        let package_cache = PackageManager::detect_packages(&content);
        Self {
            content,
            path,
            is_dirty: false,
            outline_cache,
            package_cache,
        }
    }

    /// Replaces the document source content, marking it dirty and updating caches
    #[allow(dead_code)]
    pub fn set_content(&mut self, new_content: String) {
        if self.content != new_content {
            self.content = new_content;
            self.is_dirty = true;
            self.refresh_caches();
        }
    }

    /// Re-parses the outline and package imports from the current content buffer
    pub fn refresh_caches(&mut self) {
        self.outline_cache = OutlineParser::parse(&self.content);
        self.package_cache = PackageManager::detect_packages(&self.content);
    }

    /// Returns the basename of the document file or `"untitled.tex"`
    pub fn file_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("untitled.tex")
            .to_string()
    }

    /// Returns total whitespace-separated word count
    pub fn word_count(&self) -> usize {
        self.content.split_whitespace().count()
    }

    /// Returns total unicode character count
    pub fn char_count(&self) -> usize {
        self.content.chars().count()
    }

    /// Returns total line count
    pub fn line_count(&self) -> usize {
        self.content.lines().count().max(1)
    }

    /// Writes document content to disk at the given path
    pub fn save_to_file(&mut self, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, &self.content)?;
        self.path = Some(path.to_path_buf());
        self.is_dirty = false;
        Ok(())
    }

    /// Reads a LaTeX document from disk
    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::new(content, Some(path.to_path_buf())))
    }
}
