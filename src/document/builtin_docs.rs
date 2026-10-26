//! Embedded LaTeX documentation files included in the binary.

/// Program manual and user guide.
pub const LAKETEX_MANUAL: &str = include_str!("../../docs/LAKETEX_MANUAL.tex");

/// Dedicated guide to the key-value theming engine, color tokens, and custom themes in `themes/`.
pub const THEMING_GUIDE: &str = include_str!("../../docs/THEMING_GUIDE.tex");

/// Dedicated reference manual for the headless CLI subcommands and REST API endpoints.
pub const API_REFERENCE: &str = include_str!("../../docs/API_REFERENCE.tex");

/// Architecture notes on the internal pure-Rust typesetting pipeline and AST engine.
pub const ARCHITECTURE_GUIDE: &str = include_str!("../../docs/ARCHITECTURE.tex");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinDocKind {
    Manual,
    Theming,
    ApiReference,
    Architecture,
}

#[derive(Debug, Clone, Copy)]
pub struct BuiltinDoc {
    pub kind: BuiltinDocKind,
    pub title: &'static str,
    pub filename: &'static str,
    pub description: &'static str,
    pub content: &'static str,
}

impl BuiltinDoc {
    pub fn all() -> &'static [BuiltinDoc] {
        &[
            BuiltinDoc {
                kind: BuiltinDocKind::Manual,
                title: "Laketex Program Manual & Welcome Guide",
                filename: "LAKETEX_MANUAL.tex",
                description: "Overview of Laketex features, engines, SyncTeX, and shortcuts.",
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
