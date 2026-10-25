//! # LakeTex — Lightweight, Portable LaTeX Viewer & Typesetting Engine in Rust
//!
//! **LakeTex** is an ultra-fast, zero-installation, portable LaTeX viewer, editor, and
//! typesetting system written entirely in Rust.
//!
//! ## Core Architecture
//!
//! LakeTex operates across several coordinated layers:
//!
//! 1. **Pure-Rust Native Typesetting Engine ([`engine`])**:
//!    - An in-process, sub-millisecond LaTeX parser and typesetting engine.
//!    - Native layout computation with balanced-brace fraction typesetting,
//!      multi-row bracketed matrix formatting (`pmatrix`, `bmatrix`), superscript/subscript
//!      placement, and academic margin handling.
//!    - Standalone vector PDF export ([`engine::pdf_exporter`]) powered by `printpdf`.
//!
//! 2. **Compiler & Engine Orchestration ([`compiler`])**:
//!    - Multi-engine backend supporting Pure-Rust Native, embedded Tectonic,
//!      pdfLaTeX, XeLaTeX, and LuaLaTeX ([`compiler::engine::EngineKind`]).
//!    - Non-blocking background worker thread ([`compiler::CompilerWorker`]) preventing UI freezes.
//!    - Automated CTAN package dependency detection and on-demand injection ([`compiler::provisioner`]).
//!    - Deep contextual error reporting with snippet extraction ([`compiler::ErrorReport`]).
//!
//! 3. **Document Representation & SyncTeX ([`document`])**:
//!    - Synchronous LaTeX lexer, token stream, and comment/math stripper ([`document::lexer`]).
//!    - Hierarchical section outline tree parser ([`document::outline`]).
//!    - Bidirectional SyncTeX line $\leftrightarrow$ coordinate navigation between editor and preview.
//!
//! 4. **Visual Document Viewer & Parity Harness ([`viewer`])**:
//!    - Smooth GPU-accelerated interactive canvas ([`viewer::canvas::PdfCanvas`]) with pan, zoom,
//!      dark-mode color inversion, and double-click forward/backward SyncTeX jumping.
//!    - Visual Parity Benchmark Harness ([`viewer::parity::VisualParityComparator`]) comparing native
//!      in-house rendering against TeX reference engines.
//!
//! 5. **Key-Value Theming Engine ([`ui::theme`])**:
//!    - Simple key-value hex color control (`#RRGGBB`, `#RGB`).
//!    - Dynamic discovery of custom themes from `.json` and `.yaml` files placed alongside the executable.
//!    - Ships with an authentic JetBrains-styled **Dracula** theme built-in, alongside Academic Dark,
//!      Slate Minimal, Nord Frost, and Paper White.
//!
//! 6. **Headless CLI & REST API ([`headless`])**:
//!    - Batch CLI compilation, file watching, and visual parity comparisons ([`headless::cli`]).
//!    - Headless HTTP REST API daemon ([`headless::server`]) exposing endpoints for automation.

#![warn(missing_docs)]

pub mod compiler;
pub mod document;
pub mod engine;
pub mod error;
pub mod headless;
pub mod i18n;
pub mod ui;
pub mod viewer;

pub use error::{LakeTexError, Result};
pub use i18n::{current_locale, set_locale, tr, tr_args};

/// Creates a standard `std::process::Command` configured with `CREATE_NO_WINDOW` on Windows
/// to prevent console window popups when invoking external subprocesses, and populates `PATH`
/// with any discovered Python/Conda/local tool directories.
pub fn create_hidden_command<S: AsRef<std::ffi::OsStr>>(program: S) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    if let Some(aug_path) = crate::document::packages::PackageManager::augmented_path_env() {
        cmd.env("PATH", aug_path);
    }
    cmd
}
