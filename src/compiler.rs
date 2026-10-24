//! # LakeTex Compilation Subsystem
//!
//! Manages LaTeX compilation across multiple execution backends, parses compiler diagnostics,
//! downloads missing CTAN packages dynamically, and provides an asynchronous worker thread
//! for non-blocking UI compilation.
//!
//! ## Submodules
//! - [`engine`]: Backend engine abstraction (`Native`, `Tectonic`, `pdfLaTeX`, `XeLaTeX`, `LuaLaTeX`)
//! - [`diagnostic`]: Compiler output parsing into structured file/line diagnostics
//! - [`error_report`]: Rich contextual error reports with source line context, markdown, and JSON export
//! - [`provisioner`]: Automatic on-demand CTAN package provisioning and cache management
//! - [`worker`]: Asynchronous non-blocking compilation thread communicating via crossbeam channels

pub mod diagnostic;
pub mod engine;
pub mod error_report;
pub mod provisioner;
pub mod worker;

pub use diagnostic::{Diagnostic, DiagnosticSeverity};
pub use engine::{CompileResult, CompilerEngine, EngineKind};
pub use error_report::ErrorReport;
pub use provisioner::Provisioner;
pub use worker::CompilerWorker;
