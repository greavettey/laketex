//! The open document: source text, cached outline/packages, dirty flag, file io.


pub mod builtin_docs;
pub mod lexer;
pub mod outline;
pub mod packages;

pub use builtin_docs::{BuiltinDoc, BuiltinDocKind};
use outline::{OutlineItem, OutlineParser};
use packages::{PackageInfo, PackageManager};
use std::path::{Path, PathBuf};

pub const DEFAULT_DOCUMENT_TEMPLATE: &str = builtin_docs::LAKETEX_MANUAL;

pub struct Document {
    pub content: String,
    pub path: Option<PathBuf>,
    pub is_dirty: bool,
    pub outline_cache: Vec<OutlineItem>,
    pub package_cache: Vec<PackageInfo>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new(DEFAULT_DOCUMENT_TEMPLATE.to_string(), None)
    }
}

impl Document {
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

    #[allow(dead_code)]
    pub fn set_content(&mut self, new_content: String) {
        if self.content != new_content {
            self.content = new_content;
            self.is_dirty = true;
            self.refresh_caches();
        }
    }

    pub fn refresh_caches(&mut self) {
        self.outline_cache = OutlineParser::parse(&self.content);
        self.package_cache = PackageManager::detect_packages(&self.content);
    }

    pub fn file_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("untitled.tex")
            .to_string()
    }

    pub fn word_count(&self) -> usize {
        self.content.split_whitespace().count()
    }

    pub fn char_count(&self) -> usize {
        self.content.chars().count()
    }

    pub fn line_count(&self) -> usize {
        self.content.lines().count().max(1)
    }

    pub fn save_to_file(&mut self, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, &self.content)?;
        self.path = Some(path.to_path_buf());
        self.is_dirty = false;
        Ok(())
    }

    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::new(content, Some(path.to_path_buf())))
    }
}
