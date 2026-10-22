//! # In-Application Built-in LaTeX Documentation Library
//!
//! Provides embedded, portable LaTeX documentation files bundled directly within the
//! Laketex binary. These documents can be opened, edited, typeset, or exported at any time
//! without requiring an active internet connection.

/// The comprehensive welcome manual and user guide explaining Laketex, its features,
/// dual typesetting engines, and custom themes.
pub const LAKETEX_MANUAL: &str = include_str!("../../docs/LAKETEX_MANUAL.tex");

/// Dedicated guide to the key-value theming engine, color tokens, and custom themes in `themes/`.
pub const THEMING_GUIDE: &str = include_str!("../../docs/THEMING_GUIDE.tex");

/// Dedicated reference manual for the headless CLI subcommands and REST API endpoints.
pub const API_REFERENCE: &str = include_str!("../../docs/API_REFERENCE.tex");

/// Deep dive whitepaper into the internal pure-Rust typesetting pipeline and AST engine.
pub const ARCHITECTURE_GUIDE: &str = include_str!("../../docs/ARCHITECTURE.tex");

/// Enumeration of available built-in documentation manuals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinDocKind {
    /// Comprehensive program manual and overview
    Manual,
    /// Theming system and custom themes folder guide
    Theming,
    /// Headless CLI and REST API reference
    ApiReference,
    /// Rust internals and typesetting pipeline architecture
    Architecture,
}

/// Metadata and content handle for an in-app documentation manual.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinDoc {
    /// Document identifier
    pub kind: BuiltinDocKind,
    /// Human-friendly display title
    pub title: &'static str,
    /// Canonical file name on disk
    pub filename: &'static str,
    /// Brief explanatory summary
    pub description: &'static str,
    /// Raw embedded LaTeX source text
    pub content: &'static str,
}

impl BuiltinDoc {
    /// Returns catalog of all bundled LaTeX documentation files.
    pub fn all() -> &'static [BuiltinDoc] {
        &[
            BuiltinDoc {
                kind: BuiltinDocKind::Manual,
                title: "Laketex Program Manual & Welcome Guide",
                filename: "LAKETEX_MANUAL.tex",
                description: "Comprehensive overview of Laketex features, dual engines, SyncTeX, and shortcuts.",
                content: LAKETEX_MANUAL,
            },
            BuiltinDoc {
                kind: BuiltinDocKind::Theming,
                title: "Custom Theming & Themes Folder Guide",
                filename: "THEMING_GUIDE.tex",
                description: "Complete guide to key-value UI theming, color tokens, and JSON/YAML custom themes.",
                content: THEMING_GUIDE,
            },
            BuiltinDoc {
                kind: BuiltinDocKind::ApiReference,
                title: "Headless REST API & CLI Reference",
                filename: "API_REFERENCE.tex",
                description: "Full reference for terminal subcommands and embedded HTTP REST endpoints.",
                content: API_REFERENCE,
            },
            BuiltinDoc {
                kind: BuiltinDocKind::Architecture,
                title: "Pure-Rust Architecture & Engine Internals",
                filename: "ARCHITECTURE.tex",
                description: "Deep dive into the native AST parser, layout pipeline, and PDFium integration.",
                content: ARCHITECTURE_GUIDE,
            },
        ]
    }

    /// Finds a built-in document by kind
    pub fn get(kind: BuiltinDocKind) -> &'static BuiltinDoc {
        Self::all()
            .iter()
            .find(|d| d.kind == kind)
            .unwrap_or(&Self::all()[0])
    }
}
