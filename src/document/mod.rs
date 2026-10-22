//! # Document Representation & In-Memory LaTeX Buffer
//!
//! Provides the primary [`Document`] model, which holds the active LaTeX source string,
//! cached structural section outline ([`outline::OutlineItem`]), detected CTAN packages
//! ([`packages::PackageInfo`]), dirty state tracking, and file I/O operations.

pub mod builtin_docs;
pub mod lexer;
pub mod outline;
pub mod packages;

pub use builtin_docs::{BuiltinDoc, BuiltinDocKind};
use outline::{OutlineItem, OutlineParser};
use packages::{PackageInfo, PackageManager};
use std::path::{Path, PathBuf};

/// Default modern LaTeX document template initialized when creating a new file.
/// Points to the comprehensive Laketex Welcome Manual and User Guide.
pub const DEFAULT_DOCUMENT_TEMPLATE: &str = builtin_docs::LAKETEX_MANUAL;

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
