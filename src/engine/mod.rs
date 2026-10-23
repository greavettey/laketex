//! # Pure-Rust Native Typesetting Engine
//!
//! An independent, in-process LaTeX parser, mathematical formula typesetter, page layout
//! generator, and PDF producer written from the ground up in 100% safe Rust.
//!
//! ## Subsystems
//! - [`ast`]: Rich typed Abstract Syntax Tree representing LaTeX document structure
//! - [`parser`]: Recursive descent parser extracting blocks, macros, equations, and tables
//! - [`math`]: Mathematical formula rendering engine featuring balanced-brace fraction formatting,
//!   bracketed 2D matrices (`pmatrix`, `bmatrix`), superscripts, subscripts, and Greek symbols
//! - [`layout`]: Physical page layout manager with margin calculations, line wrapping,
//!   page breaks, running headers, and forward/backward SyncTeX coordinate bounding boxes
//! - [`native_renderer`]: Egui vector drawing implementation rendering crisp vector preview
//!   with interactive double-click SyncTeX caret positioning
//! - [`pdf_exporter`]: Academic PDF producer generating publication-quality PDFs via `printpdf`
//! - [`render_debugger`]: Command and preview render auditor identifying flawed LaTeX math and matrix commands

pub mod ast;
pub mod layout;
pub mod math;
pub mod native_renderer;
pub mod parser;
pub mod pdf_exporter;
pub mod render_debugger;
pub mod syntax;

pub use ast::{DocumentBlock, InlineSpan, LatexAst, PaperSize};
pub use layout::{DocumentLayout, TypesetElement, TypesetPage};
pub use math::MathRenderer;
pub use native_renderer::NativePageRenderer;
pub use parser::NativeLatexParser;
pub use pdf_exporter::NativePdfExporter;
pub use render_debugger::{RenderDebugItem, RenderDebugReport, RenderDebugger, RenderIssueSeverity};
pub use syntax::SyntaxHighlighter;
