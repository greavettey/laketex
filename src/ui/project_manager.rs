//! # Project Files & Working Directory Manager
//!
//! Provides an Overleaf-style Working Directory modal and sidebar file explorer:
//! - Visual directory tree of files and subfolders (`images/`, `sections/`, etc.)
//! - Folder creation (e.g. `images`, `sections`, `figures`)
//! - File creation with LaTeX starter templates (.tex, .bib, .sty, etc.)
//! - Import / upload files from local filesystem into the project root or subfolders
//! - One-click opening of any `.tex` file directly into the active editor
//! - One-click insertion or clipboard copying of `\includegraphics{...}` snippets for images
//! - One-click insertion or clipboard copying of `\input{...}` snippets for subfiles
//! - Rename and delete operations with confirmation
//! - "Open in Explorer" system integration
//! - Working directory switching (browse project folder)
//! - Search/filter files across the project

use egui::{vec2, Color32, RichText, ScrollArea};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Fixed row height in points for virtualized tree rendering.
pub const ROW_HEIGHT: f32 = 22.0;

/// Maximum directory recursion depth during filesystem scanning to prevent stack overflow.
pub const MAX_SCAN_DEPTH: usize = 30;

/// Cleans Windows extended length path prefix (`\\?\`) for normalized path comparisons.
fn dunce_clean_path(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        p.to_path_buf()
    }
}


/// Classification of file types within a LaTeX project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectFileType {
    /// Directory or subfolder.
    Directory,
    /// LaTeX source files (`.tex`, `.ltx`, `.sty`, `.cls`).
    Tex,
    /// Image and graphic assets (`.png`, `.jpg`, `.jpeg`, `.pdf`, `.eps`, `.svg`).
    Image,
    /// BibTeX/BibLaTeX bibliography databases (`.bib`).
    Bib,
    /// Configuration, package stylesheets, or document class definitions.
    StyleOrConfig,
    /// Tabular or external data files (`.csv`, `.tsv`, `.dat`, `.json`).
    Data,
    /// Miscellaneous unclassified project assets.
    Other,
}

impl ProjectFileType {
    /// Classifies an entry based on its base name string and directory flag with zero heap allocations.
    pub fn from_name_and_is_dir(name: &str, is_dir: bool) -> Self {
        if is_dir {
            return Self::Directory;
        }
        let ext = name.rfind('.').map(|i| &name[i + 1..]).unwrap_or("");
        match ext {
            "tex" | "ltx" | "TEX" | "LTX" => Self::Tex,
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp" | "pdf" | "eps"
            | "PNG" | "JPG" | "JPEG" | "GIF" | "SVG" | "WEBP" | "BMP" | "PDF" | "EPS" => Self::Image,
            "bib" | "BIB" => Self::Bib,
            "sty" | "cls" | "cfg" | "dtx" | "ins"
            | "STY" | "CLS" | "CFG" | "DTX" | "INS" => Self::StyleOrConfig,
            "csv" | "tsv" | "dat" | "json" | "txt" | "yaml" | "yml"
            | "CSV" | "TSV" | "DAT" | "JSON" | "TXT" | "YAML" | "YML" => Self::Data,
            _ => {
                let lower = ext.to_ascii_lowercase();
                match lower.as_str() {
                    "tex" | "ltx" => Self::Tex,
                    "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp" | "pdf" | "eps" => Self::Image,
                    "bib" => Self::Bib,
                    "sty" | "cls" | "cfg" | "dtx" | "ins" => Self::StyleOrConfig,
                    "csv" | "tsv" | "dat" | "json" | "txt" | "yaml" | "yml" => Self::Data,
                    _ => Self::Other,
                }
            }
        }
    }

    /// Classifies a path based on a known directory flag and file extension, avoiding redundant filesystem calls.
    pub fn from_path_with_is_dir(path: &Path, is_dir: bool) -> Self {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        Self::from_name_and_is_dir(name, is_dir)
    }

    /// Classifies a path based on filesystem metadata and file extension.
    pub fn from_path(path: &Path) -> Self {
        Self::from_path_with_is_dir(path, path.is_dir())
    }

    /// Returns an emoji icon representing the file type.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Directory => "📁",
            Self::Tex => "📄",
            Self::Image => "🖼",
            Self::Bib => "📚",
            Self::StyleOrConfig => "⚙",
            Self::Data => "📊",
            Self::Other => "📎",
        }
    }

    /// Returns a human-readable description of the file type.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Directory => "Folder",
            Self::Tex => "LaTeX Document",
            Self::Image => "Image / Graphic",
            Self::Bib => "BibTeX Bibliography",
            Self::StyleOrConfig => "LaTeX Style / Class",
            Self::Data => "Data File",
            Self::Other => "File",
        }
    }

    /// Returns true if this file is a graphic/image that can be included via `\includegraphics`.
    pub fn is_image(&self) -> bool {
        matches!(self, Self::Image)
    }

    /// Returns true if this file is a LaTeX document (.tex).
    pub fn is_tex(&self) -> bool {
        matches!(self, Self::Tex)
    }
}

/// A node in the project file hierarchy (directory or file).
#[derive(Debug, Clone)]
pub struct ProjectNode {
    /// Absolute filesystem path
    pub path: PathBuf,
    /// Path relative to project root
    pub relative_path: PathBuf,
    /// File or directory base name
    pub name: String,
    /// Pre-lowercased base name for zero-allocation fast filter searching
    pub name_lower: String,
    /// Categorized file type
    pub file_type: ProjectFileType,
    /// File size in bytes (0 for directories)
    pub size_bytes: u64,
    /// Last modification timestamp
    pub modified: Option<SystemTime>,
    /// Child items if this node is a directory
    pub children: Vec<ProjectNode>,
    /// Whether this directory node is expanded in the tree view
    pub is_expanded: bool,
}

impl ProjectNode {
    /// Fast zero-allocation match against an already-lowercased query string.
    pub fn matches_filter_lower(&self, query_lower: &str) -> bool {
        if query_lower.is_empty() {
            return true;
        }
        self.name_lower.contains(query_lower)
    }

    /// Fast recursive match against an already-lowercased query string.
    pub fn contains_match_lower(&self, query_lower: &str) -> bool {
        if self.matches_filter_lower(query_lower) {
            return true;
        }
        self.children.iter().any(|c| c.contains_match_lower(query_lower))
    }

    /// Checks if this node matches the search filter (case-insensitive).
    pub fn matches_filter(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let q_lower = query.to_lowercase();
        self.matches_filter_lower(&q_lower)
    }

    /// Checks if this node or any of its recursive descendants matches the filter.
    pub fn contains_match(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let q_lower = query.to_lowercase();
        self.contains_match_lower(&q_lower)
    }
}

/// User actions emitted by the Project Manager.
#[derive(Debug, Clone)]
pub enum ProjectAction {
    /// No action
    None,
    /// Switch active editor to a different `.tex` document
    OpenTexFile(PathBuf),
    /// Insert an `\includegraphics` figure environment at the cursor
    InsertIncludeGraphics {
        /// Relative path from the working directory to the target image file.
        relative_path: String,
        /// LaTeX reference label for cross-referencing (e.g. `fig:diagram`).
        label: String,
    },
    /// Insert an `\input{...}` statement at the cursor
    InsertInput {
        /// Relative path from the working directory to the referenced `.tex` file.
        relative_path: String,
    },
    /// Copy text (e.g. snippet or relative path) to clipboard
    CopyToClipboard(String),
    /// Active working directory was changed by user
    WorkingDirChanged(PathBuf),
    /// Set a file as the project's Master/Root Document for compilation
    SetMasterDocument(PathBuf),
    /// Cleared the Master Document (compiles active buffer instead)
    ClearMasterDocument,
}

/// Internal UI interaction intents collected during tree traversal.
enum UiIntent {
    ToggleExpand(PathBuf),
    SetTargetFolder(PathBuf),
    OpenTex(PathBuf),
    InsertFigure { rel_path: String, label: String },
    InsertInput { rel_path: String },
    CopyClipboard(String),
    PromptRename(PathBuf, String),
    PromptDelete(PathBuf),
    ImportInto(PathBuf),
    SetMaster(PathBuf),
    ClearMaster,
    CollapseAll,
    ExpandAll,
}

/// Reference to a visible node for virtualized row rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatRow {
    /// Indices path down the tree to locate the node (e.g. [0, 2, 1])
    pub indices: Vec<usize>,
    /// Indentation depth level
    pub depth: usize,
}

/// Overleaf-style Working Directory and Project Files Manager.
pub struct ProjectManager {
    /// Active project root directory
    pub root_dir: PathBuf,
    /// Whether the floating modal window is open
    pub show_modal: bool,
    /// Cached hierarchical project tree
    pub root_nodes: Vec<ProjectNode>,
    /// Search filter string
    pub search_query: String,
    /// Active folder selected as destination for new files/folders or imports
    pub target_folder: PathBuf,
    /// State: New File dialog input bar open
    pub new_file_open: bool,
    /// Pending filename input for newly created files
    pub new_file_name: String,
    /// State: New Folder dialog input bar open
    pub new_folder_open: bool,
    /// Pending folder name input for newly created directories
    pub new_folder_name: String,
    /// State: Rename target item
    pub rename_target: Option<PathBuf>,
    /// Pending new name for the item currently being renamed
    pub rename_new_name: String,
    /// State: Delete confirmation item
    pub delete_target: Option<PathBuf>,
    /// Optional designated Master / Root Document for compilation
    pub master_document: Option<PathBuf>,
    /// Temporary feedback notification
    pub status_message: Option<(String, Instant)>,
    /// Cached metric totals (file count, directory count, total bytes) for O(1) status queries
    pub cached_metrics: (usize, usize, u64),
    /// Set of user-expanded folder paths preserved across refreshes
    pub expanded_folders: HashSet<PathBuf>,
    /// Flag indicating whether the next refresh is the initial workspace scan
    pub is_initial_scan: bool,
    /// Cached flattened visible rows for O(1) per-frame rendering
    pub cached_visible_nodes: Vec<FlatRow>,
    /// Last search query string used to build cached visible nodes
    pub last_cached_query: String,
    /// Dirty flag indicating cached visible rows must be recalculated
    pub visible_nodes_dirty: bool,
    /// Cached raw active file path to detect file switches
    pub cached_active_raw: Option<PathBuf>,
    /// Cached canonical active file path to eliminate per-frame canonicalize syscalls
    pub cached_active_path: Option<PathBuf>,
    /// Cached cleaned active path (stripped Windows \\?\ prefix)
    pub cached_active_clean: Option<PathBuf>,
}

impl ProjectManager {
    /// Resolves the default working directory for the session.
    ///
    /// When the executable is launched directly by double-clicking inside a Cargo build directory
    /// (e.g., `target/release` or `target/debug`), automatically steps up to the parent workspace root
    /// containing `Cargo.toml` so startup does not scan build artifacts.
    pub fn resolve_default_working_dir(initial_file: Option<&Path>) -> PathBuf {
        if let Some(file_path) = initial_file {
            if let Some(parent) = file_path.parent() {
                if !parent.as_os_str().is_empty() {
                    return parent.to_path_buf();
                }
            }
        }
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::escape_cargo_target_dir(cwd)
    }

    fn escape_cargo_target_dir(dir: PathBuf) -> PathBuf {
        if let Some(parent) = dir.parent() {
            let parent_name = parent.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if parent_name.eq_ignore_ascii_case("target") {
                if let Some(grandparent) = parent.parent() {
                    if grandparent.join("Cargo.toml").is_file() {
                        return grandparent.to_path_buf();
                    }
                }
            }
            let dir_name = dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if dir_name.eq_ignore_ascii_case("target") && parent.join("Cargo.toml").is_file() {
                return parent.to_path_buf();
            }
        }
        dir
    }

    /// Initializes a new ProjectManager rooted at `initial_dir` or the current directory.
    pub fn new(initial_dir: Option<PathBuf>) -> Self {
        let root = initial_dir.unwrap_or_else(|| Self::resolve_default_working_dir(None));
        let mut mgr = Self {
            root_dir: root.clone(),
            show_modal: false,
            root_nodes: Vec::new(),
            search_query: String::new(),
            target_folder: root,
            new_file_open: false,
            new_file_name: String::new(),
            new_folder_open: false,
            new_folder_name: String::new(),
            rename_target: None,
            rename_new_name: String::new(),
            delete_target: None,
            master_document: None,
            status_message: None,
            cached_metrics: (0, 0, 0),
            expanded_folders: HashSet::new(),
            is_initial_scan: true,
            cached_visible_nodes: Vec::new(),
            last_cached_query: String::new(),
            visible_nodes_dirty: true,
            cached_active_raw: None,
            cached_active_path: None,
            cached_active_clean: None,
        };
        mgr.auto_detect_master_document();
        mgr.refresh();
        mgr.rebuild_cached_visible_nodes();
        mgr
    }

    /// Automatically detects a candidate master document (e.g. `main.tex`, `document.tex`) in the root directory.
    pub fn auto_detect_master_document(&mut self) {
        if let Some(ref m) = self.master_document {
            if m.is_file() {
                return;
            }
        }
        for candidate in &["main.tex"] {
            let p = self.root_dir.join(candidate);
            if p.is_file() {
                // Read up to 8 KB only to avoid loading massive files into RAM during startup
                if let Ok(file) = std::fs::File::open(&p) {
                    use std::io::Read;
                    let mut buffer = [0u8; 8192];
                    let mut reader = std::io::BufReader::new(file);
                    if let Ok(bytes_read) = reader.read(&mut buffer) {
                        let prefix = String::from_utf8_lossy(&buffer[..bytes_read]);
                        if prefix.contains("\\documentclass") {
                            self.master_document = Some(p);
                            return;
                        }
                    }
                }
            }
        }
    }

    /// Sets a new project root directory and rescans its contents.
    pub fn set_root_dir(&mut self, new_root: PathBuf) {
        self.root_dir = new_root.clone();
        self.target_folder = new_root;
        self.expanded_folders.clear();
        self.is_initial_scan = true;
        self.auto_detect_master_document();
        self.refresh();
        self.rebuild_cached_visible_nodes();
        self.set_status(format!("Working directory: {}", self.root_dir.display()));
    }

    /// Sets a status feedback message displayed inside the modal and sidebar.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now()));
    }

    /// Rescans the filesystem starting from `root_dir`.
    pub fn refresh(&mut self) {
        let mut visited_ancestors = HashSet::new();
        self.root_nodes = Self::scan_dir(
            &self.root_dir,
            &self.root_dir,
            Path::new(""),
            0,
            true,
            &self.expanded_folders,
            self.is_initial_scan,
            &mut visited_ancestors,
        );
        if self.is_initial_scan {
            Self::collect_expanded_paths(&self.root_nodes, &mut self.expanded_folders);
        }
        self.cached_metrics = Self::calculate_metrics(&self.root_nodes);
        self.is_initial_scan = false;
        self.rebuild_cached_visible_nodes();
    }

    /// Returns true if a path represents an OS user/container root (Desktop, Downloads, UserProfile, Root drive)
    /// where child folders should not be eagerly recursed into during startup.
    pub fn is_system_or_user_root(dir: &Path) -> bool {
        if let Some(name) = dir.file_name().and_then(|s| s.to_str()) {
            if name.eq_ignore_ascii_case("Desktop")
                || name.eq_ignore_ascii_case("Downloads")
                || name.eq_ignore_ascii_case("Documents")
            {
                return true;
            }
        }
        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            if Path::new(&home) == dir {
                return true;
            }
        }
        dir.parent().is_none()
    }

    /// Recursively scans a directory and builds the project tree.
    fn scan_dir(
        dir: &Path,
        root: &Path,
        rel_dir: &Path,
        depth: usize,
        check_canonical: bool,
        expanded_folders: &HashSet<PathBuf>,
        is_initial_scan: bool,
        visited_ancestors: &mut HashSet<PathBuf>,
    ) -> Vec<ProjectNode> {
        let mut nodes = Vec::new();
        if depth > MAX_SCAN_DEPTH {
            return nodes;
        }

        // Symlink cycle detection: only canonicalize root or symlink/junction directories
        let canonical_dir = if check_canonical {
            std::fs::canonicalize(dir).ok()
        } else {
            None
        };
        if let Some(ref c) = canonical_dir {
            if !visited_ancestors.insert(c.clone()) {
                // Cycle detected in directory tree; stop recursion
                return nodes;
            }
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => {
                if let Some(ref c) = canonical_dir {
                    visited_ancestors.remove(c);
                }
                return nodes;
            }
        };

        let is_root_container = depth == 0 && Self::is_system_or_user_root(dir);

        let mut raw_items = Vec::new();
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();

            if Self::is_ignored(&name_str) {
                continue;
            }

            let path = dir.join(&*file_name);
            let name = name_str.to_string();

            let file_type_res = entry.file_type();
            let is_symlink = file_type_res.as_ref().map(|ft| ft.is_symlink()).unwrap_or(false);
            let is_dir = if is_symlink {
                path.is_dir()
            } else {
                file_type_res.as_ref().map(|ft| ft.is_dir()).unwrap_or_else(|_| path.is_dir())
            };

            let size_bytes = if is_dir {
                0
            } else {
                entry.metadata().map(|m| m.len()).unwrap_or(0)
            };

            let relative_path = if rel_dir.as_os_str().is_empty() {
                PathBuf::from(&name)
            } else {
                rel_dir.join(&name)
            };

            let name_lower = name.to_lowercase();
            let file_type = ProjectFileType::from_name_and_is_dir(&name, is_dir);

            raw_items.push((name, name_lower, path, relative_path, file_type, size_bytes, is_dir, is_symlink));
        }

        for (name, name_lower, path, relative_path, file_type, size_bytes, is_dir, is_symlink) in raw_items {
            let is_expanded = if is_dir {
                if is_initial_scan {
                    // For root container directories (like Desktop), keep folders collapsed by default.
                    // For regular LaTeX project folders, expand top-level folders.
                    depth == 0 && !is_root_container
                } else {
                    expanded_folders.contains(&path)
                }
            } else {
                false
            };

            let should_recurse = if is_dir {
                if is_root_container {
                    // Container directories (like Desktop) are never eagerly recursed into on startup
                    false
                } else if is_initial_scan {
                    // For normal projects, recurse into directories up to depth 3
                    depth < 3
                } else {
                    // On refresh / manual expansion, recurse if expanded or depth 0
                    expanded_folders.contains(&path)
                }
            } else {
                false
            };

            let children = if should_recurse {
                Self::scan_dir(
                    &path,
                    root,
                    &relative_path,
                    depth + 1,
                    is_symlink,
                    expanded_folders,
                    is_initial_scan,
                    visited_ancestors,
                )
            } else {
                Vec::new()
            };

            nodes.push(ProjectNode {
                path,
                relative_path,
                name,
                name_lower,
                file_type,
                size_bytes,
                modified: None,
                children,
                is_expanded,
            });
        }

        // Cycle protection cleanup
        if let Some(ref c) = canonical_dir {
            visited_ancestors.remove(c);
        }

        // Fast zero-allocation sort: directories first (alphabetically), then files (alphabetically)
        nodes.sort_by(|a, b| {
            match (
                a.file_type == ProjectFileType::Directory,
                b.file_type == ProjectFileType::Directory,
            ) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name_lower.cmp(&b.name_lower),
            }
        });

        nodes
    }

    /// Returns true if a file or directory should be hidden from the project tree.
    pub fn is_ignored(name: &str) -> bool {
        if name.starts_with('.')
            || name.starts_with("_minted-")
            || name.ends_with('~')
            || name.ends_with(".tmp")
            || name.ends_with(".rlib")
            || name.ends_with(".rmeta")
            || name.ends_with(".pdb")
            || name.ends_with(".exp")
            || name.ends_with(".obj")
        {
            return true;
        }
        matches!(
            name,
            "target"
                | "node_modules"
                | "incremental"
                | "__pycache__"
                | ".git"
                | ".svn"
                | ".hg"
                | "venv"
                | ".venv"
                | "env"
                | ".env"
                | "dist"
                | "build"
                | "bin"
                | "obj"
                | ".gradle"
                | ".idea"
                | ".vscode"
                | "laketex_doc.tex"
                | "laketex_doc.pdf"
                | "laketex_doc.aux"
                | "laketex_doc.log"
                | "laketex_doc.fls"
                | "laketex_doc.synctex.gz"
                | "laketex_doc.out"
                | "laketex_doc.toc"
        )
    }

    /// Validates a single file or directory name against path traversal, forbidden characters,
    /// and Windows reserved device names.
    ///
    /// # Arguments
    /// * `name` - The base name of the file or folder to validate (must not be a path with separators).
    ///
    /// # Returns
    /// * `Ok(())` if the name is secure and valid.
    /// * `Err(String)` describing the validation or security violation.
    ///
    /// # Security & Validation Checks
    /// - Rejects empty strings or whitespace-only names
    /// - Rejects directory navigation tokens (`.` and `..`)
    /// - Rejects path separators (`/`, `\`) and null bytes (`\0`)
    /// - Rejects prohibited filesystem characters (`<`, `>`, `:`, `"`, `|`, `?`, `*`)
    /// - Rejects DOS/Windows reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`)
    pub fn validate_entry_name(name: &str) -> Result<(), String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Name cannot be empty".to_string());
        }
        if trimmed == "." || trimmed == ".." {
            return Err("Invalid name: cannot be '.' or '..'".to_string());
        }
        if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains('\0') {
            return Err("Name cannot contain path separators ('/', '\\') or null bytes".to_string());
        }
        // Disallow dangerous Windows/POSIX filesystem characters (<, >, :, ", |, ?, *)
        if trimmed.chars().any(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*')) {
            return Err("Name contains prohibited characters (<, >, :, \", |, ?, *)".to_string());
        }
        // Disallow Windows reserved device names (e.g. CON, PRN, AUX, NUL, COM1..9, LPT1..9)
        let stem = Path::new(trimmed)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(trimmed);
        let upper = stem.to_ascii_uppercase();
        let reserved = [
            "CON", "PRN", "AUX", "NUL",
            "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
            "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        if reserved.contains(&upper.as_str()) {
            return Err(format!("'{trimmed}' is a reserved system device name"));
        }
        Ok(())
    }

    /// Validates that `candidate` resides strictly inside `self.root_dir`.
    ///
    /// Resolves canonical paths to prevent symlink traversal and relative path escaping.
    ///
    /// # Arguments
    /// * `candidate` - The filesystem path to verify.
    /// * `allow_root` - If `false`, operations targeting the project root itself are rejected.
    ///
    /// # Returns
    /// * `Ok(PathBuf)` containing the canonicalized path within the root.
    /// * `Err(String)` if canonicalization fails or the path escapes the project root.
    ///
    /// # Security
    /// Guarantees that no caller can create, read, modify, or delete files outside of the
    /// active project directory boundary.
    pub fn ensure_within_root(&self, candidate: &Path, allow_root: bool) -> Result<PathBuf, String> {
        let canonical_root = std::fs::canonicalize(&self.root_dir)
            .map_err(|e| format!("Cannot resolve project root directory: {e}"))?;

        let canonical_candidate = if candidate.exists() {
            std::fs::canonicalize(candidate)
                .map_err(|e| format!("Cannot resolve path {}: {e}", candidate.display()))?
        } else {
            let parent = candidate
                .parent()
                .ok_or_else(|| "Target path has no parent directory".to_string())?;
            let canonical_parent = std::fs::canonicalize(parent)
                .map_err(|e| format!("Cannot resolve parent directory {}: {e}", parent.display()))?;
            let file_name = candidate
                .file_name()
                .ok_or_else(|| "Target path has no file name".to_string())?;
            canonical_parent.join(file_name)
        };

        if !canonical_candidate.starts_with(&canonical_root) {
            return Err(format!(
                "Security violation: path '{}' escapes project root",
                candidate.display()
            ));
        }

        if !allow_root && canonical_candidate == canonical_root {
            return Err("Security violation: operation not permitted on project root directory".to_string());
        }

        Ok(canonical_candidate)
    }

    /// Creates a new subfolder inside `target_folder`.
    ///
    /// Validates the folder name and ensures the destination remains strictly within
    /// the project root directory.
    ///
    /// # Arguments
    /// * `folder_name` - Base name of the folder to create.
    ///
    /// # Errors
    /// Returns `Err` if the name is invalid, escapes the project root, or already exists.
    pub fn create_folder(&mut self, folder_name: &str) -> Result<PathBuf, String> {
        Self::validate_entry_name(folder_name)?;
        let trimmed = folder_name.trim();

        let _ = self.ensure_within_root(&self.target_folder, true)?;
        let folder_path = self.target_folder.join(trimmed);
        let _ = self.ensure_within_root(&folder_path, true)?;
        if folder_path.exists() {
            return Err("A file or folder with that name already exists".to_string());
        }
        std::fs::create_dir_all(&folder_path).map_err(|e| e.to_string())?;
        self.expanded_folders.insert(self.target_folder.clone());
        self.expanded_folders.insert(folder_path.clone());
        self.refresh();
        self.set_status(format!("Created folder: {}", trimmed));
        crate::ui::drawer::record_user_audit(
            "Folder Created",
            format!("Created folder '{trimmed}'"),
            None,
        );
        Ok(folder_path)
    }

    /// Creates a new file inside `target_folder`, populated with an appropriate starter template.
    ///
    /// Validates the file name and ensures the destination remains strictly within
    /// the project root directory.
    ///
    /// # Arguments
    /// * `file_name` - Base name of the file to create (e.g. `document.tex`, `references.bib`).
    ///
    /// # Errors
    /// Returns `Err` if the name is invalid, escapes the project root, or already exists.
    pub fn create_file(&mut self, file_name: &str) -> Result<PathBuf, String> {
        Self::validate_entry_name(file_name)?;
        let trimmed = file_name.trim();

        let _ = self.ensure_within_root(&self.target_folder, true)?;
        let file_path = self.target_folder.join(trimmed);
        let _ = self.ensure_within_root(&file_path, true)?;
        if file_path.exists() {
            return Err("A file with that name already exists".to_string());
        }

        let canonical_root = std::fs::canonicalize(&self.root_dir).unwrap_or_else(|_| self.root_dir.clone());
        let is_root = self.target_folder == self.root_dir
            || std::fs::canonicalize(&self.target_folder).map(|p| p == canonical_root).unwrap_or(false);
        let content = if trimmed.ends_with(".tex") || trimmed.ends_with(".ltx") {
            if is_root {
                Self::root_tex_template(trimmed)
            } else {
                Self::subfile_tex_template(trimmed)
            }
        } else if trimmed.ends_with(".bib") {
            Self::bib_template()
        } else {
            String::new()
        };

        std::fs::write(&file_path, content).map_err(|e| e.to_string())?;
        self.expanded_folders.insert(self.target_folder.clone());
        self.refresh();
        self.set_status(format!("Created file: {}", trimmed));
        crate::ui::drawer::record_user_audit(
            "File Created",
            format!("Created project file '{trimmed}'"),
            None,
        );
        Ok(file_path)
    }

    /// Prompts a native file dialog to import one or more files into `target_folder`.
    ///
    /// Validates destination paths and file names to prevent directory escape during import.
    pub fn import_files_dialog(&mut self) -> Result<usize, String> {
        let _ = self.ensure_within_root(&self.target_folder, true)?;
        let files = rfd::FileDialog::new()
            .set_directory(&self.target_folder)
            .pick_files();

        let files = match files {
            Some(f) if !f.is_empty() => f,
            _ => return Ok(0),
        };

        let mut count = 0;
        for src_path in &files {
            if let Some(file_name) = src_path.file_name() {
                let name_str = file_name.to_string_lossy();
                if Self::validate_entry_name(&name_str).is_err() {
                    continue;
                }
                let dest_path = self.target_folder.join(file_name);
                if self.ensure_within_root(&dest_path, true).is_err() {
                    continue;
                }
                if let Err(e) = std::fs::copy(src_path, &dest_path) {
                    return Err(format!("Failed to copy {}: {e}", src_path.display()));
                }
                count += 1;
            }
        }

        self.refresh();
        let target_display = self.relative_target_display();
        self.set_status(format!("Imported {count} file(s) into {target_display}"));
        crate::ui::drawer::record_user_audit(
            "Files Imported",
            format!("Imported {count} file(s) into {target_display}"),
            None,
        );
        Ok(count)
    }

    /// Deletes a file or directory from disk.
    ///
    /// Validates that `path` is strictly contained within `self.root_dir` and forbids
    /// deleting the project root directory itself.
    ///
    /// # Arguments
    /// * `path` - The path to the file or directory to delete.
    ///
    /// # Errors
    /// Returns `Err` if `path` does not exist, resolves outside `self.root_dir`, or is `self.root_dir`.
    pub fn delete_item(&mut self, path: &Path) -> Result<(), String> {
        let _ = self.ensure_within_root(path, false)?;
        if path.is_dir() {
            std::fs::remove_dir_all(path).map_err(|e| e.to_string())?;
        } else {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        self.refresh();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        self.set_status(format!("Deleted: {name}"));
        crate::ui::drawer::record_user_audit(
            "Entry Deleted",
            format!("Deleted '{name}' from project"),
            None,
        );
        Ok(())
    }

    /// Renames a file or directory on disk.
    ///
    /// Validates the new name and ensures neither the source nor the destination escapes
    /// the project root directory.
    ///
    /// # Arguments
    /// * `old_path` - The current path of the file or directory.
    /// * `new_name` - The new base name for the item.
    ///
    /// # Errors
    /// Returns `Err` if `new_name` contains path traversal tokens or invalid characters,
    /// if `old_path` is outside the root directory or is the root directory itself, or if
    /// the destination already exists.
    pub fn rename_item(&mut self, old_path: &Path, new_name: &str) -> Result<PathBuf, String> {
        Self::validate_entry_name(new_name)?;
        let trimmed = new_name.trim();

        let _ = self.ensure_within_root(old_path, false)?;
        let parent = old_path.parent().unwrap_or(&self.root_dir);
        let new_path = parent.join(trimmed);
        let _ = self.ensure_within_root(&new_path, false)?;
        if new_path.exists() {
            return Err("Destination name already exists".to_string());
        }
        let old_name = old_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        std::fs::rename(old_path, &new_path).map_err(|e| e.to_string())?;
        self.refresh();
        self.set_status(format!("Renamed to: {trimmed}"));
        crate::ui::drawer::record_user_audit(
            "Entry Renamed",
            format!("Renamed '{old_name}' to '{trimmed}'"),
            None,
        );
        Ok(new_path)
    }

    /// Opens the current project root in the system file manager (e.g. Windows Explorer).
    pub fn open_in_system_explorer(&self) {
        let path = if self.root_dir.is_dir() {
            &self.root_dir
        } else if let Some(parent) = self.root_dir.parent() {
            parent
        } else {
            &self.root_dir
        };

        #[cfg(windows)]
        {
            let _ = crate::create_hidden_command("explorer").arg(path).spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(path).spawn();
        }
        #[cfg(all(not(windows), not(target_os = "macos")))]
        {
            let _ = std::process::Command::new("xdg-open").arg(path).spawn();
        }
    }

    /// Returns a human-friendly string of the active target folder relative to root.
    pub fn relative_target_display(&self) -> String {
        if self.target_folder == self.root_dir {
            "Project Root /".to_string()
        } else {
            let rel = self
                .target_folder
                .strip_prefix(&self.root_dir)
                .unwrap_or(&self.target_folder);
            format!("{}/", rel.to_string_lossy().replace('\\', "/"))
        }
    }

    /// Returns cached total count of files, directories, and total byte size in O(1) time.
    pub fn metrics(&self) -> (usize, usize, u64) {
        self.cached_metrics
    }

    /// Calculates total count of files, directories, and total byte size from a node list.
    pub fn calculate_metrics(nodes: &[ProjectNode]) -> (usize, usize, u64) {
        let mut files = 0;
        let mut dirs = 0;
        let mut bytes = 0;
        for node in nodes {
            if node.file_type == ProjectFileType::Directory {
                dirs += 1;
                let (f, d, b) = Self::calculate_metrics(&node.children);
                files += f;
                dirs += d;
                bytes += b;
            } else {
                files += 1;
                bytes += node.size_bytes;
            }
        }
        (files, dirs, bytes)
    }

    /// Collapses all directory nodes in the project tree.
    pub fn collapse_all(&mut self) {
        self.expanded_folders.clear();
        Self::set_all_expanded(&mut self.root_nodes, false);
        self.visible_nodes_dirty = true;
        self.rebuild_cached_visible_nodes();
    }

    /// Expands all directory nodes in the project tree.
    pub fn expand_all(&mut self) {
        Self::populate_all_children(&mut self.root_nodes, &self.root_dir, &self.expanded_folders, 0);
        Self::set_all_expanded(&mut self.root_nodes, true);
        Self::collect_expanded_paths(&self.root_nodes, &mut self.expanded_folders);
        self.cached_metrics = Self::calculate_metrics(&self.root_nodes);
        self.visible_nodes_dirty = true;
        self.rebuild_cached_visible_nodes();
    }

    fn populate_all_children(
        nodes: &mut [ProjectNode],
        root: &Path,
        expanded_folders: &HashSet<PathBuf>,
        depth: usize,
    ) {
        if depth > MAX_SCAN_DEPTH {
            return;
        }
        for node in nodes {
            if node.file_type == ProjectFileType::Directory {
                if node.children.is_empty() {
                    let mut visited = HashSet::new();
                    node.children = Self::scan_dir(
                        &node.path,
                        root,
                        &node.relative_path,
                        depth + 1,
                        false,
                        expanded_folders,
                        false,
                        &mut visited,
                    );
                }
                Self::populate_all_children(&mut node.children, root, expanded_folders, depth + 1);
            }
        }
    }

    /// Sets expansion state of all nodes recursively.
    pub fn set_all_expanded(nodes: &mut [ProjectNode], expanded: bool) {
        for node in nodes {
            if node.file_type == ProjectFileType::Directory {
                node.is_expanded = expanded;
                Self::set_all_expanded(&mut node.children, expanded);
            }
        }
    }

    /// Collects paths of currently expanded directory nodes.
    pub fn collect_expanded_paths(nodes: &[ProjectNode], set: &mut HashSet<PathBuf>) {
        for node in nodes {
            if node.file_type == ProjectFileType::Directory {
                if node.is_expanded {
                    set.insert(node.path.clone());
                }
                Self::collect_expanded_paths(&node.children, set);
            }
        }
    }

    /// Sets the expansion state of a specific folder and updates expanded_folders.
    pub fn set_folder_expanded(&mut self, target_path: &Path, expanded: bool) -> bool {
        if expanded {
            self.expanded_folders.insert(target_path.to_path_buf());
            let _ = Self::populate_node_children_if_empty(
                &mut self.root_nodes,
                target_path,
                &self.root_dir,
                &self.expanded_folders,
            );
        } else {
            self.expanded_folders.remove(target_path);
        }
        let res = Self::set_expanded_in_nodes(&mut self.root_nodes, target_path, expanded);
        self.cached_metrics = Self::calculate_metrics(&self.root_nodes);
        self.visible_nodes_dirty = true;
        self.rebuild_cached_visible_nodes();
        res
    }

    /// Lazily populates the children of a specific directory node matching `target_path` if empty.
    pub fn populate_node_children_if_empty(
        nodes: &mut [ProjectNode],
        target_path: &Path,
        root: &Path,
        expanded_folders: &HashSet<PathBuf>,
    ) -> bool {
        for node in nodes {
            if node.path == target_path {
                if node.file_type == ProjectFileType::Directory && node.children.is_empty() {
                    let mut visited = HashSet::new();
                    node.children = Self::scan_dir(
                        &node.path,
                        root,
                        &node.relative_path,
                        1,
                        false,
                        expanded_folders,
                        false,
                        &mut visited,
                    );
                }
                return true;
            }
            if !node.children.is_empty() {
                if Self::populate_node_children_if_empty(&mut node.children, target_path, root, expanded_folders) {
                    return true;
                }
            }
        }
        false
    }

    /// Sets the expansion state of a specific directory matching `target_path`.
    pub fn set_expanded_in_nodes(nodes: &mut [ProjectNode], target_path: &Path, expanded: bool) -> bool {
        for node in nodes {
            if node.path == target_path {
                node.is_expanded = expanded;
                return true;
            }
            if !node.children.is_empty() {
                if Self::set_expanded_in_nodes(&mut node.children, target_path, expanded) {
                    return true;
                }
            }
        }
        false
    }



    /// Formats a byte count into a readable string (e.g. `24.5 KB`, `1.2 MB`).
    pub fn format_bytes(bytes: u64) -> String {
        if bytes < 1024 {
            format!("{bytes} B")
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        }
    }

    /// Generates a starter document template for a root-level `.tex` file.
    fn root_tex_template(name: &str) -> String {
        let title = name.trim_end_matches(".tex").replace(['_', '-'], " ");
        format!(
            r#"\documentclass[12pt,a4paper]{{article}}
\usepackage{{amsmath, amssymb, graphicx, booktabs, hyperref}}

\title{{{title}}}
\author{{LakeTex Author}}
\date{{\today}}

\begin{{document}}
\maketitle

\section{{Introduction}}
Start typing your document here.

\end{{document}}
"#
        )
    }

    /// Generates a starter section template for a subfolder `.tex` file.
    fn subfile_tex_template(name: &str) -> String {
        let title = name.trim_end_matches(".tex").replace(['_', '-'], " ");
        format!(
            r#"\section{{{title}}}

Write your section content here.
"#
        )
    }

    /// Generates a starter BibTeX file template.
    fn bib_template() -> String {
        r#"@article{sample2026,
  author  = {Doe, Jane and Smith, John},
  title   = {Modern Typesetting with LakeTex},
  journal = {Journal of Portable Software},
  year    = {2026},
  volume  = {1},
  pages   = {1--12}
}
"#
        .to_string()
    }

    /// Renders the Overleaf-style Working Directory floating modal window.
    pub fn show_modal(&mut self, ctx: &egui::Context, active_file: Option<&Path>) -> ProjectAction {
        if !self.show_modal {
            return ProjectAction::None;
        }

        let mut action = ProjectAction::None;
        let mut is_open = self.show_modal;

        egui::Window::new("📁 Project Files — Working Directory")
            .open(&mut is_open)
            .default_size(vec2(680.0, 520.0))
            .min_size(vec2(480.0, 360.0))
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                action = self.render_ui(ui, active_file, true);
            });

        self.show_modal = is_open;
        action
    }

    /// Renders the file manager embedded inside the collapsible sidebar tab.
    pub fn show_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        active_file: Option<&Path>,
    ) -> ProjectAction {
        self.render_ui(ui, active_file, false)
    }

    /// Internal rendering core supporting both modal and sidebar presentations.
    fn render_ui(
        &mut self,
        ui: &mut egui::Ui,
        active_file: Option<&Path>,
        is_modal: bool,
    ) -> ProjectAction {
        let mut project_action = ProjectAction::None;
        let mut intents = Vec::new();

        // 1. Directory Bar
        ui.horizontal(|ui| {
            if is_modal {
                let dir_label = format!("📂 {}", self.root_dir.display());
                ui.label(RichText::new(dir_label).strong().size(13.0));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🔄 Refresh").clicked() {
                        self.refresh();
                    }
                    if ui
                        .button("🗁 Explorer")
                        .on_hover_text("Open folder in Windows Explorer")
                        .clicked()
                    {
                        self.open_in_system_explorer();
                    }
                    if ui
                        .button("📂 Change Folder...")
                        .on_hover_text("Select a different project root folder")
                        .clicked()
                    {
                        if let Some(folder) = rfd::FileDialog::new()
                            .set_directory(&self.root_dir)
                            .pick_folder()
                        {
                            self.set_root_dir(folder);
                            project_action =
                                ProjectAction::WorkingDirChanged(self.root_dir.clone());
                        }
                    }
                });
            } else {
                ui.label(RichText::new("📁 Project Explorer").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🔄").on_hover_text("Refresh project files").clicked() {
                        self.refresh();
                    }
                    if ui
                        .button("⛶ Pop-out")
                        .on_hover_text("Open full Project Directory modal (Ctrl+P)")
                        .clicked()
                    {
                        self.show_modal = true;
                    }
                });
            }
        });

        ui.separator();

        // 2. Action Toolbar: New File, New Folder, Import Files, Search
        ui.horizontal(|ui| {
            if ui
                .button("📄 + File")
                .on_hover_text("Create a new .tex or asset file")
                .clicked()
            {
                self.new_file_open = !self.new_file_open;
                self.new_folder_open = false;
                if self.new_file_open && self.new_file_name.is_empty() {
                    self.new_file_name = "section.tex".to_string();
                }
            }

            if ui
                .button("📁 + Folder")
                .on_hover_text("Create a new folder (e.g. images, sections)")
                .clicked()
            {
                self.new_folder_open = !self.new_folder_open;
                self.new_file_open = false;
                if self.new_folder_open && self.new_folder_name.is_empty() {
                    self.new_folder_name = "images".to_string();
                }
            }

            if ui
                .button("⬆ Upload")
                .on_hover_text("Import images or .tex files from computer")
                .clicked()
            {
                let _ = self.import_files_dialog();
            }

            // Quick Collapse / Expand All
            if ui
                .small_button("⊟")
                .on_hover_text("Collapse all folders")
                .clicked()
            {
                intents.push(UiIntent::CollapseAll);
            }
            if ui
                .small_button("⊞")
                .on_hover_text("Expand all folders")
                .clicked()
            {
                intents.push(UiIntent::ExpandAll);
            }

            // Search Filter
            ui.add_space(8.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text("🔍 Filter files...")
                    .desired_width(if is_modal { 160.0 } else { 100.0 }),
            );
            if !self.search_query.is_empty() && ui.small_button("✖").clicked() {
                self.search_query.clear();
            }
        });

        // Target Folder Banner (if not root)
        if self.target_folder != self.root_dir {
            ui.horizontal(|ui| {
                ui.colored_label(
                    Color32::from_rgb(97, 175, 239),
                    format!("Destination: {}", self.relative_target_display()),
                );
                if ui.small_button("✖ Reset to Root").clicked() {
                    self.target_folder = self.root_dir.clone();
                }
            });
        }

        // 3. Transient Input: New File
        if self.new_file_open {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("New file name:");
                    let edit = ui.text_edit_singleline(&mut self.new_file_name);
                    if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Ok(path) = self.create_file(&self.new_file_name.clone()) {
                            self.new_file_open = false;
                            if path.extension().map_or(false, |e| e == "tex") {
                                project_action = ProjectAction::OpenTexFile(path);
                            }
                        }
                    }
                    if ui.button("Create").clicked() {
                        if let Ok(path) = self.create_file(&self.new_file_name.clone()) {
                            self.new_file_open = false;
                            if path.extension().map_or(false, |e| e == "tex") {
                                project_action = ProjectAction::OpenTexFile(path);
                            }
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        self.new_file_open = false;
                    }
                });
            });
        }

        // Transient Input: New Folder
        if self.new_folder_open {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("New folder name:");
                    let edit = ui.text_edit_singleline(&mut self.new_folder_name);
                    if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let _ = self.create_folder(&self.new_folder_name.clone());
                        self.new_folder_open = false;
                    }
                    if ui.button("Create").clicked() {
                        let _ = self.create_folder(&self.new_folder_name.clone());
                        self.new_folder_open = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.new_folder_open = false;
                    }
                });
            });
        }

        // Transient Input: Rename Item
        if let Some(target) = &self.rename_target.clone() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Rename to:");
                    ui.text_edit_singleline(&mut self.rename_new_name);
                    if ui.button("Save").clicked() {
                        let _ = self.rename_item(target, &self.rename_new_name.clone());
                        self.rename_target = None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.rename_target = None;
                    }
                });
            });
        }

        // Transient Input: Delete Confirmation
        if let Some(target) = &self.delete_target.clone() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    let name = target.file_name().unwrap_or_default().to_string_lossy();
                    ui.colored_label(
                        Color32::from_rgb(235, 87, 87),
                        format!("Delete \"{name}\"?"),
                    );
                    if ui.button("Confirm Delete").clicked() {
                        let _ = self.delete_item(target);
                        self.delete_target = None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.delete_target = None;
                    }
                });
            });
        }

        ui.separator();

        // 4. File Tree View
        let query_trimmed = self.search_query.trim();
        if self.visible_nodes_dirty || query_trimmed != self.last_cached_query {
            self.rebuild_cached_visible_nodes();
        }

        // Cache active canonical path to eliminate per-frame filesystem canonicalize syscalls
        if self.cached_active_raw.as_deref() != active_file {
            self.cached_active_raw = active_file.map(|p| p.to_path_buf());
            self.cached_active_path = active_file.and_then(|p| std::fs::canonicalize(p).ok());
            self.cached_active_clean = self.cached_active_path.as_ref().map(|p| dunce_clean_path(p));
        }
        let active_clean = self.cached_active_clean.as_deref();
        let active_canonical = self.cached_active_path.as_deref();

        let scroll_id = if is_modal {
            "project_tree_scroll_modal"
        } else {
            "project_tree_scroll_sidebar"
        };

        if self.root_nodes.is_empty() {
            ScrollArea::vertical()
                .id_salt(scroll_id)
                .auto_shrink([false, false])
                .max_height(if is_modal { 340.0 } else { 420.0 })
                .show(ui, |ui| {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.colored_label(
                            Color32::from_gray(140),
                            "No files found in working directory.",
                        );
                        if ui.button("📄 Create main.tex").clicked() {
                            let _ = self.create_file("main.tex");
                        }
                    });
                });
        } else if self.cached_visible_nodes.is_empty() {
            ScrollArea::vertical()
                .id_salt(scroll_id)
                .auto_shrink([false, false])
                .max_height(if is_modal { 340.0 } else { 420.0 })
                .show(ui, |ui| {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.colored_label(
                            Color32::from_gray(140),
                            format!("No files matching \"{}\"", self.search_query),
                        );
                    });
                });
        } else {
            let total_rows = self.cached_visible_nodes.len();
            ScrollArea::vertical()
                .id_salt(scroll_id)
                .auto_shrink([false, false])
                .max_height(if is_modal { 340.0 } else { 420.0 })
                .show_rows(ui, ROW_HEIGHT, total_rows, |ui, row_range| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for idx in row_range {
                        if let Some(row) = self.cached_visible_nodes.get(idx) {
                            if let Some(node) = self.get_node(&row.indices) {
                                Self::render_node_row(
                                    ui,
                                    node,
                                    row.depth,
                                    active_file,
                                    active_clean,
                                    active_canonical,
                                    self.master_document.as_deref(),
                                    &mut intents,
                                );
                            }
                        }
                    }
                });
        }

        // Apply collected UI intents after tree traversal
        for intent in intents {
            match intent {
                UiIntent::ToggleExpand(path) => {
                    if Self::toggle_expand_in_nodes(
                        &mut self.root_nodes,
                        &path,
                        &self.root_dir,
                        &self.expanded_folders,
                    ) {
                        if self.expanded_folders.contains(&path) {
                            self.expanded_folders.remove(&path);
                        } else {
                            self.expanded_folders.insert(path);
                        }
                    }
                    self.cached_metrics = Self::calculate_metrics(&self.root_nodes);
                    self.visible_nodes_dirty = true;
                    self.rebuild_cached_visible_nodes();
                }
                UiIntent::CollapseAll => {
                    self.collapse_all();
                }
                UiIntent::ExpandAll => {
                    self.expand_all();
                }
                UiIntent::SetTargetFolder(path) => {
                    self.expanded_folders.insert(path.clone());
                    let _ = Self::set_expanded_in_nodes(&mut self.root_nodes, &path, true);
                    self.target_folder = path;
                    self.new_file_open = true;
                    self.visible_nodes_dirty = true;
                    self.rebuild_cached_visible_nodes();
                }
                UiIntent::OpenTex(path) => {
                    project_action = ProjectAction::OpenTexFile(path);
                }
                UiIntent::InsertFigure { rel_path, label } => {
                    project_action = ProjectAction::InsertIncludeGraphics {
                        relative_path: rel_path,
                        label,
                    };
                }
                UiIntent::InsertInput { rel_path } => {
                    project_action = ProjectAction::InsertInput {
                        relative_path: rel_path,
                    };
                }
                UiIntent::CopyClipboard(text) => {
                    project_action = ProjectAction::CopyToClipboard(text);
                }
                UiIntent::PromptRename(path, current_name) => {
                    self.rename_target = Some(path);
                    self.rename_new_name = current_name;
                }
                UiIntent::PromptDelete(path) => {
                    self.delete_target = Some(path);
                }
                UiIntent::ImportInto(path) => {
                    self.expanded_folders.insert(path.clone());
                    let _ = Self::set_expanded_in_nodes(&mut self.root_nodes, &path, true);
                    self.target_folder = path;
                    self.visible_nodes_dirty = true;
                    self.rebuild_cached_visible_nodes();
                    let _ = self.import_files_dialog();
                }
                UiIntent::SetMaster(path) => {
                    self.master_document = Some(path.clone());
                    self.set_status(format!("Set master: {}", path.file_name().unwrap_or_default().to_string_lossy()));
                    project_action = ProjectAction::SetMasterDocument(path);
                }
                UiIntent::ClearMaster => {
                    self.master_document = None;
                    self.set_status("Master document cleared");
                    project_action = ProjectAction::ClearMasterDocument;
                }
            }
        }

        ui.separator();

        // 5. Bottom Status and Metrics Bar
        ui.horizontal(|ui| {
            let (files, dirs, bytes) = self.metrics();
            ui.colored_label(
                Color32::from_gray(140),
                format!(
                    "📄 {} file{} · 📁 {} folder{} · {}",
                    files,
                    if files == 1 { "" } else { "s" },
                    dirs,
                    if dirs == 1 { "" } else { "s" },
                    Self::format_bytes(bytes)
                ),
            );

            // Ephemeral status message
            if let Some((msg, ts)) = &self.status_message {
                if ts.elapsed() < Duration::from_secs(6) {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.colored_label(Color32::from_rgb(80, 250, 123), msg);
                    });
                }
            }
        });

        project_action
    }

    /// Rebuilds the cached list of visible rows for virtualized rendering.
    pub fn rebuild_cached_visible_nodes(&mut self) {
        self.cached_visible_nodes.clear();
        let query_lower = self.search_query.trim().to_lowercase();
        let mut path = Vec::new();
        Self::collect_flat_indices(
            &self.root_nodes,
            &mut path,
            0,
            &query_lower,
            false,
            &mut self.cached_visible_nodes,
        );
        self.last_cached_query = self.search_query.trim().to_string();
        self.visible_nodes_dirty = false;
    }

    fn collect_flat_indices(
        nodes: &[ProjectNode],
        current_path: &mut Vec<usize>,
        depth: usize,
        query_lower: &str,
        parent_matched: bool,
        out: &mut Vec<FlatRow>,
    ) {
        for (idx, node) in nodes.iter().enumerate() {
            current_path.push(idx);

            if !query_lower.is_empty() {
                let this_matches = parent_matched || node.matches_filter_lower(query_lower);
                let subtree_matches = this_matches || node.contains_match_lower(query_lower);
                if subtree_matches {
                    out.push(FlatRow {
                        indices: current_path.clone(),
                        depth,
                    });
                    if node.file_type == ProjectFileType::Directory {
                        Self::collect_flat_indices(
                            &node.children,
                            current_path,
                            depth + 1,
                            query_lower,
                            this_matches,
                            out,
                        );
                    }
                }
            } else {
                out.push(FlatRow {
                    indices: current_path.clone(),
                    depth,
                });
                if node.file_type == ProjectFileType::Directory && node.is_expanded {
                    Self::collect_flat_indices(
                        &node.children,
                        current_path,
                        depth + 1,
                        query_lower,
                        false,
                        out,
                    );
                }
            }

            current_path.pop();
        }
    }

    /// Retrieves a reference to a node given its hierarchical tree indices.
    pub fn get_node(&self, indices: &[usize]) -> Option<&ProjectNode> {
        let mut curr_slice = self.root_nodes.as_slice();
        let mut curr_node = None;
        for &idx in indices {
            curr_node = curr_slice.get(idx);
            if let Some(node) = curr_node {
                curr_slice = &node.children;
            } else {
                return None;
            }
        }
        curr_node
    }

    /// Recursively flattens visible nodes in display order for virtualized row rendering.
    pub fn flatten_visible_nodes<'a>(
        nodes: &'a [ProjectNode],
        depth: usize,
        query_lower: &str,
        parent_matched: bool,
        out: &mut Vec<FlattenedNode<'a>>,
    ) {
        for node in nodes {
            if !query_lower.is_empty() {
                let this_matches = parent_matched || node.matches_filter_lower(query_lower);
                let subtree_matches = this_matches || node.contains_match_lower(query_lower);
                if !subtree_matches {
                    continue;
                }
                out.push(FlattenedNode { node, depth });
                if node.file_type == ProjectFileType::Directory {
                    Self::flatten_visible_nodes(&node.children, depth + 1, query_lower, this_matches, out);
                }
            } else {
                out.push(FlattenedNode { node, depth });
                if node.file_type == ProjectFileType::Directory && node.is_expanded {
                    Self::flatten_visible_nodes(&node.children, depth + 1, query_lower, false, out);
                }
            }
        }
    }

    /// Renders a single row in the virtualized file tree.
    fn render_node_row(
        ui: &mut egui::Ui,
        node: &ProjectNode,
        depth: usize,
        active_file: Option<&Path>,
        active_clean: Option<&Path>,
        active_canonical: Option<&Path>,
        master_file: Option<&Path>,
        intents: &mut Vec<UiIntent>,
    ) {
        ui.horizontal(|ui| {
            ui.set_min_height(ROW_HEIGHT);

            // Indentation
            ui.add_space(depth as f32 * 14.0);

            if node.file_type == ProjectFileType::Directory {
                // Directory row
                let arrow = if node.is_expanded { "▾" } else { "▸" };
                if ui.small_button(arrow).clicked() {
                    intents.push(UiIntent::ToggleExpand(node.path.clone()));
                }

                ui.label(node.file_type.icon());
                let folder_name = RichText::new(&node.name)
                    .strong()
                    .color(Color32::from_rgb(230, 219, 116));
                ui.label(folder_name);

                ui.colored_label(
                    Color32::from_gray(120),
                    format!("({})", node.children.len()),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("🗑")
                        .on_hover_text("Delete folder")
                        .clicked()
                    {
                        intents.push(UiIntent::PromptDelete(node.path.clone()));
                    }
                    if ui
                        .small_button("⬆")
                        .on_hover_text("Import files into this folder")
                        .clicked()
                    {
                        intents.push(UiIntent::ImportInto(node.path.clone()));
                    }
                    if ui
                        .small_button("+ File")
                        .on_hover_text("Create file in this folder")
                        .clicked()
                    {
                        intents.push(UiIntent::SetTargetFolder(node.path.clone()));
                    }
                });
            } else {
                // File row
                ui.add_space(18.0); // Align with directory items
                ui.label(node.file_type.icon());

                let is_active = active_file.map_or(false, |p| {
                    p == node.path
                        || p == node.relative_path
                        || active_clean == Some(&node.path)
                        || active_canonical == Some(&node.path)
                        || active_clean == Some(&node.relative_path)
                });

                if is_active {
                    let name_text = RichText::new(&node.name)
                        .strong()
                        .color(Color32::from_rgb(80, 250, 123));
                    ui.label(name_text);
                    ui.colored_label(Color32::from_rgb(80, 250, 123), "● Active");
                } else {
                    ui.label(&node.name);
                }

                ui.colored_label(
                    Color32::from_gray(110),
                    Self::format_bytes(node.size_bytes),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("🗑").on_hover_text("Delete file").clicked() {
                        intents.push(UiIntent::PromptDelete(node.path.clone()));
                    }
                    if ui
                        .small_button("✏")
                        .on_hover_text("Rename file")
                        .clicked()
                    {
                        intents.push(UiIntent::PromptRename(
                            node.path.clone(),
                            node.name.clone(),
                        ));
                    }

                    match node.file_type {
                        ProjectFileType::Tex => {
                            let is_master = master_file.map_or(false, |m| m == node.path || m == node.relative_path);
                            if is_master {
                                if ui
                                    .small_button(RichText::new("👑 Master").color(Color32::from_rgb(229, 192, 123)).strong())
                                    .on_hover_text("Root Master Document for compilation. Click to unset.")
                                    .clicked()
                                {
                                    intents.push(UiIntent::ClearMaster);
                                }
                            } else {
                                if ui
                                    .small_button("👑 Set Master")
                                    .on_hover_text("Set this file as Master/Root Document for project compilation")
                                    .clicked()
                                {
                                    intents.push(UiIntent::SetMaster(node.path.clone()));
                                }
                            }

                            if !is_active
                                && ui
                                    .small_button("📖 Open")
                                    .on_hover_text("Open document in editor")
                                    .clicked()
                            {
                                intents.push(UiIntent::OpenTex(node.path.clone()));
                            }
                            if ui
                                .small_button("➕ \\input")
                                .on_hover_text("Insert \\input{...} into document")
                                .clicked()
                            {
                                let forward_rel = node.relative_path.to_string_lossy().replace('\\', "/");
                                intents.push(UiIntent::InsertInput {
                                    rel_path: forward_rel,
                                });
                            }
                        }
                        ProjectFileType::Image => {
                            if ui
                                .small_button("➕ Figure")
                                .on_hover_text("Insert \\begin{figure}...\\includegraphics environment")
                                .clicked()
                            {
                                let forward_rel = node.relative_path.to_string_lossy().replace('\\', "/");
                                let stem = node
                                    .path
                                    .file_stem()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string();
                                intents.push(UiIntent::InsertFigure {
                                    rel_path: forward_rel,
                                    label: stem,
                                });
                            }
                            if ui
                                .small_button("📋 Snippet")
                                .on_hover_text("Copy \\includegraphics snippet to clipboard")
                                .clicked()
                            {
                                let forward_rel = node.relative_path.to_string_lossy().replace('\\', "/");
                                let snippet = format!(
                                    "\\includegraphics[width=0.8\\linewidth]{{{forward_rel}}}"
                                );
                                intents.push(UiIntent::CopyClipboard(snippet));
                            }
                        }
                        ProjectFileType::Bib => {
                            if ui
                                .small_button("➕ \\bibliography")
                                .on_hover_text("Insert \\bibliography statement")
                                .clicked()
                            {
                                let forward_rel = node.relative_path.to_string_lossy().replace('\\', "/");
                                let bib_stem = forward_rel.trim_end_matches(".bib");
                                intents.push(UiIntent::CopyClipboard(format!(
                                    "\\bibliography{{{bib_stem}}}"
                                )));
                            }
                        }
                        _ => {}
                    }
                });
            }
        });
    }

    /// Toggles the `is_expanded` state of a folder matching `target_path`.
    /// Lazily scans and populates children on demand if they have not been loaded yet.
    fn toggle_expand_in_nodes(
        nodes: &mut [ProjectNode],
        target_path: &Path,
        root: &Path,
        expanded_folders: &HashSet<PathBuf>,
    ) -> bool {
        for node in nodes {
            if node.path == target_path {
                node.is_expanded = !node.is_expanded;
                if node.is_expanded && node.file_type == ProjectFileType::Directory && node.children.is_empty() {
                    let mut visited = HashSet::new();
                    node.children = Self::scan_dir(
                        &node.path,
                        root,
                        &node.relative_path,
                        1,
                        false,
                        expanded_folders,
                        false,
                        &mut visited,
                    );
                }
                return true;
            }
            if !node.children.is_empty() {
                if Self::toggle_expand_in_nodes(&mut node.children, target_path, root, expanded_folders) {
                    return true;
                }
            }
        }
        false
    }
}

/// Lightweight reference to a visible node for virtualized row rendering.
pub struct FlattenedNode<'a> {
    /// Reference to the project tree node
    pub node: &'a ProjectNode,
    /// Indentation depth level
    pub depth: usize,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_type_classification() {
        assert_eq!(
            ProjectFileType::from_path(Path::new("main.tex")),
            ProjectFileType::Tex
        );
        assert_eq!(
            ProjectFileType::from_path(Path::new("photo.png")),
            ProjectFileType::Image
        );
        assert_eq!(
            ProjectFileType::from_path(Path::new("diagram.pdf")),
            ProjectFileType::Image
        );
        assert_eq!(
            ProjectFileType::from_path(Path::new("refs.bib")),
            ProjectFileType::Bib
        );
        assert_eq!(
            ProjectFileType::from_path(Path::new("custom.sty")),
            ProjectFileType::StyleOrConfig
        );
        assert_eq!(
            ProjectFileType::from_path(Path::new("data.csv")),
            ProjectFileType::Data
        );
    }

    #[test]
    fn test_is_ignored() {
        assert!(ProjectManager::is_ignored(".git"));
        assert!(ProjectManager::is_ignored("target"));
        assert!(ProjectManager::is_ignored("_minted-laketex_doc"));
        assert!(ProjectManager::is_ignored("laketex_doc.tex"));
        assert!(ProjectManager::is_ignored("laketex_doc.pdf"));
        assert!(ProjectManager::is_ignored("laketex_doc.aux"));
        assert!(ProjectManager::is_ignored("laketex_doc.log"));
        assert!(ProjectManager::is_ignored("temp.tmp"));
        assert!(ProjectManager::is_ignored("backup.tex~"));

        assert!(!ProjectManager::is_ignored("main.tex"));
        assert!(!ProjectManager::is_ignored("images"));
        assert!(!ProjectManager::is_ignored("figure1.png"));
    }

    #[test]
    fn test_project_operations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path().to_path_buf();

        let mut pm = ProjectManager::new(Some(root.clone()));
        assert_eq!(pm.root_nodes.len(), 0);

        // 1. Create Folder
        let images_dir = pm.create_folder("images").unwrap();
        assert!(images_dir.exists());

        // 2. Create File in Root
        let root_tex = pm.create_file("main.tex").unwrap();
        assert!(root_tex.exists());
        let root_content = std::fs::read_to_string(&root_tex).unwrap();
        assert!(root_content.contains("\\documentclass"));
        assert!(root_content.contains("\\begin{document}"));

        // 3. Create File in Subfolder
        pm.target_folder = images_dir;
        let sub_tex = pm.create_file("notes.tex").unwrap();
        assert!(sub_tex.exists());
        let sub_content = std::fs::read_to_string(&sub_tex).unwrap();
        assert!(sub_content.contains("\\section{notes}"));

        // Refresh and verify tree metrics
        pm.refresh();
        let (files, dirs, _) = pm.metrics();
        assert_eq!(files, 2);
        assert_eq!(dirs, 1);

        // 4. Rename
        let renamed = pm.rename_item(&sub_tex, "renamed_notes.tex").unwrap();
        assert!(renamed.exists());
        assert!(!sub_tex.exists());

        // 5. Delete
        pm.delete_item(&renamed).unwrap();
        assert!(!renamed.exists());
    }

    #[test]
    fn test_validate_entry_name() {
        assert!(ProjectManager::validate_entry_name("").is_err());
        assert!(ProjectManager::validate_entry_name("   ").is_err());
        assert!(ProjectManager::validate_entry_name(".").is_err());
        assert!(ProjectManager::validate_entry_name("..").is_err());
        assert!(ProjectManager::validate_entry_name("../evil.tex").is_err());
        assert!(ProjectManager::validate_entry_name("sub/evil.tex").is_err());
        assert!(ProjectManager::validate_entry_name("sub\\evil.tex").is_err());
        assert!(ProjectManager::validate_entry_name("evil:stream").is_err());
        assert!(ProjectManager::validate_entry_name("evil*name").is_err());
        assert!(ProjectManager::validate_entry_name("evil?name").is_err());
        assert!(ProjectManager::validate_entry_name("evil<name>").is_err());
        assert!(ProjectManager::validate_entry_name("CON").is_err());
        assert!(ProjectManager::validate_entry_name("con.txt").is_err());
        assert!(ProjectManager::validate_entry_name("aux.tex").is_err());
        assert!(ProjectManager::validate_entry_name("NUL").is_err());
        assert!(ProjectManager::validate_entry_name("COM1.dat").is_err());

        assert!(ProjectManager::validate_entry_name("document.tex").is_ok());
        assert!(ProjectManager::validate_entry_name("my_folder").is_ok());
        assert!(ProjectManager::validate_entry_name("figure-1.png").is_ok());
        assert!(ProjectManager::validate_entry_name("references.bib").is_ok());
    }

    #[test]
    fn test_path_traversal_prevention() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path().to_path_buf();
        let mut pm = ProjectManager::new(Some(root.clone()));

        // Create a valid file first
        let valid_file = pm.create_file("valid.tex").unwrap();
        assert!(valid_file.exists());

        // Attempt traversal in create_file
        assert!(pm.create_file("../outside.tex").is_err());
        assert!(pm.create_file("subdir/outside.tex").is_err());

        // Attempt traversal in create_folder
        assert!(pm.create_folder("../outside_folder").is_err());
        assert!(pm.create_folder("subdir/outside_folder").is_err());

        // Attempt traversal in rename_item
        assert!(pm.rename_item(&valid_file, "../escaped.tex").is_err());
        assert!(pm.rename_item(&valid_file, "CON").is_err());

        // Attempt deleting root itself
        assert!(pm.delete_item(&root).is_err());

        // Attempt deleting outside path
        let outside_dir = tempfile::tempdir().unwrap();
        let outside_file = outside_dir.path().join("do_not_delete.txt");
        std::fs::write(&outside_file, "keep me").unwrap();
        assert!(pm.delete_item(&outside_file).is_err());
        assert!(outside_file.exists(), "Outside file should not have been deleted!");
    }

    #[test]
    fn test_large_directory_performance_and_scaling() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path().to_path_buf();

        // Create 20 folders with 30 files each = 600 files total, deeply nested
        for folder_idx in 0..20 {
            let sub = root.join(format!("folder_{folder_idx:02}"));
            std::fs::create_dir_all(&sub).unwrap();
            let nested = sub.join("nested_subfolder");
            std::fs::create_dir_all(&nested).unwrap();

            for file_idx in 0..15 {
                let p1 = sub.join(format!("file_{folder_idx:02}_{file_idx:02}.tex"));
                std::fs::write(&p1, "% sub file").unwrap();

                let p2 = nested.join(format!("data_{folder_idx:02}_{file_idx:02}.csv"));
                std::fs::write(&p2, "1,2,3").unwrap();
            }
        }

        // Add 50 root-level files
        for i in 0..50 {
            std::fs::write(root.join(format!("root_{i}.tex")), "\\section{test}").unwrap();
        }

        let start = std::time::Instant::now();
        let mut pm = ProjectManager::new(Some(root.clone()));
        let scan_duration = start.elapsed();
        // Initial scan of 650 files and 40 directories must complete well under 1 second
        assert!(scan_duration.as_millis() < 1000, "Scan took too long: {:?}", scan_duration);

        let (files, dirs, _) = pm.metrics();
        assert_eq!(files, 650);
        assert_eq!(dirs, 40);

        // Test collapse_all and expand_all
        pm.collapse_all();
        let mut flat_collapsed = Vec::new();
        ProjectManager::flatten_visible_nodes(&pm.root_nodes, 0, "", false, &mut flat_collapsed);
        // When collapsed, only root-level items (20 folders + 50 files = 70 items) should be visible!
        assert_eq!(flat_collapsed.len(), 70);

        pm.expand_all();
        let mut flat_expanded = Vec::new();
        ProjectManager::flatten_visible_nodes(&pm.root_nodes, 0, "", false, &mut flat_expanded);
        // When expanded, all 650 files + 40 directories = 690 items are visible
        assert_eq!(flat_expanded.len(), 690);

        // Test search filter on large tree: filter for a specific file
        let mut flat_search = Vec::new();
        ProjectManager::flatten_visible_nodes(&pm.root_nodes, 0, "file_00_01", false, &mut flat_search);
        assert_eq!(flat_search.len(), 2); // folder_00 and file_00_01.tex

        // Test search filter for non-existent file
        let mut flat_empty = Vec::new();
        ProjectManager::flatten_visible_nodes(&pm.root_nodes, 0, "non_existent_xyz", false, &mut flat_empty);
        assert_eq!(flat_empty.len(), 0);
    }

    #[test]
    fn test_expansion_state_preservation_on_refresh() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path().to_path_buf();
        let mut pm = ProjectManager::new(Some(root.clone()));

        let folder_a = pm.create_folder("folder_a").unwrap();
        let folder_b = pm.create_folder("folder_b").unwrap();
        pm.refresh();

        // Initially folders are expanded
        pm.set_folder_expanded(&folder_a, false);
        pm.set_folder_expanded(&folder_b, true);
        assert!(!pm.root_nodes.iter().find(|n| n.path == folder_a).unwrap().is_expanded);
        assert!(pm.root_nodes.iter().find(|n| n.path == folder_b).unwrap().is_expanded);

        // Trigger refresh (e.g. after adding another file)
        pm.target_folder = root.clone();
        let _ = pm.create_file("document.tex").unwrap();

        // Verify folder_a remained collapsed and folder_b remained expanded!
        assert!(!pm.root_nodes.iter().find(|n| n.path == folder_a).unwrap().is_expanded);
        assert!(pm.root_nodes.iter().find(|n| n.path == folder_b).unwrap().is_expanded);
    }

    #[test]
    fn test_fast_filter_methods() {
        let node = ProjectNode {
            path: PathBuf::from("test.tex"),
            relative_path: PathBuf::from("test.tex"),
            name: "MySpecialDocument.tex".to_string(),
            name_lower: "myspecialdocument.tex".to_string(),
            file_type: ProjectFileType::Tex,
            size_bytes: 100,
            modified: None,
            children: Vec::new(),
            is_expanded: false,
        };

        assert!(node.matches_filter_lower("special"));
        assert!(node.matches_filter_lower("myspecialdocument"));
        assert!(!node.matches_filter_lower("unknown"));
        assert!(node.matches_filter("SPECIAL"));
        assert!(node.contains_match("DOCUMENT"));
    }

    #[test]
    fn test_virtualized_cached_visible_rows_and_node_lookup() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path().to_path_buf();
        let mut pm = ProjectManager::new(Some(root.clone()));

        let folder = pm.create_folder("sections").unwrap();
        pm.target_folder = folder.clone();
        let _intro = pm.create_file("intro.tex").unwrap();
        let _methods = pm.create_file("methods.tex").unwrap();
        pm.target_folder = root.clone();
        let _main = pm.create_file("main.tex").unwrap();

        // Cached visible rows should initially contain root items:
        // sections/ (expanded by default in small project), intro.tex, methods.tex, and main.tex
        assert_eq!(pm.cached_visible_nodes.len(), 4);

        // Verify O(1) node lookup by tree indices
        for row in &pm.cached_visible_nodes {
            let node = pm.get_node(&row.indices);
            assert!(node.is_some());
        }

        // Test collapsing sections folder
        pm.set_folder_expanded(&folder, false);
        // When collapsed, only sections/ and main.tex should be visible (2 items)
        assert_eq!(pm.cached_visible_nodes.len(), 2);

        // Re-expand
        pm.set_folder_expanded(&folder, true);
        assert_eq!(pm.cached_visible_nodes.len(), 4);

        // Test search query caching
        pm.search_query = "methods".to_string();
        pm.rebuild_cached_visible_nodes();
        // sections/ (ancestor) and methods.tex should match (2 items)
        assert_eq!(pm.cached_visible_nodes.len(), 2);
        let matching_names: Vec<String> = pm
            .cached_visible_nodes
            .iter()
            .map(|r| pm.get_node(&r.indices).unwrap().name.clone())
            .collect();
        assert_eq!(matching_names, vec!["sections", "methods.tex"]);
    }

    #[test]
    fn test_active_file_caching_avoids_canonicalize() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path().to_path_buf();
        let mut pm = ProjectManager::new(Some(root.clone()));
        let file = pm.create_file("document.tex").unwrap();

        assert!(pm.cached_active_raw.is_none());
        assert!(pm.cached_active_path.is_none());

        // Simulate active file detection
        let active = Some(file.as_path());
        if pm.cached_active_raw.as_deref() != active {
            pm.cached_active_raw = active.map(|p| p.to_path_buf());
            pm.cached_active_path = active.and_then(|p| std::fs::canonicalize(p).ok());
            pm.cached_active_clean = pm.cached_active_path.as_ref().map(|p| dunce_clean_path(p));
        }

        assert_eq!(pm.cached_active_raw.as_deref(), active);
        assert!(pm.cached_active_path.is_some());
        assert!(pm.cached_active_clean.is_some());

        // Subsequent check with the same active file must not trigger re-canonicalization
        let had_canonical = pm.cached_active_path.clone();
        if pm.cached_active_raw.as_deref() != active {
            panic!("Should not execute when active file is unchanged!");
        }
        assert_eq!(pm.cached_active_path, had_canonical);
    }

    #[test]
    fn test_symlink_and_deep_recursion_safety() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path().to_path_buf();

        // Create a chain of deeply nested directories
        let mut current = root.clone();
        for i in 0..35 {
            current = current.join(format!("level_{i}"));
        }
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(current.join("deep.tex"), "deep").unwrap();

        let mut visited = HashSet::new();
        let nodes = ProjectManager::scan_dir(
            &root,
            &root,
            Path::new(""),
            0,
            true,
            &HashSet::new(),
            false,
            &mut visited,
        );
        // Recursion must terminate safely at MAX_SCAN_DEPTH without stack overflow
        assert!(!nodes.is_empty());
    }

    #[test]
    fn test_scaling_thousands_of_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path().to_path_buf();

        // Create 50 folders with 40 files each = 2,000 files total!
        for folder_idx in 0..50 {
            let sub = root.join(format!("dir_{folder_idx:02}"));
            std::fs::create_dir_all(&sub).unwrap();

            for file_idx in 0..40 {
                let file_path = sub.join(format!("item_{folder_idx:02}_{file_idx:02}.tex"));
                std::fs::write(&file_path, "\\input{test}").unwrap();
            }
        }

        let start = std::time::Instant::now();
        let mut pm = ProjectManager::new(Some(root.clone()));
        let scan_elapsed = start.elapsed();
        // Scanning 2,000 files across 50 folders must finish well under 2 seconds even in debug mode
        assert!(
            scan_elapsed.as_millis() < 2000,
            "Scanning 2000 files took too long: {:?}",
            scan_elapsed
        );

        let (files, dirs, _) = pm.metrics();
        assert_eq!(files, 2000);
        assert_eq!(dirs, 50);

        // Test collapse_all on 2,050 items
        pm.collapse_all();
        assert_eq!(pm.cached_visible_nodes.len(), 50);

        // Test expand_all on 2,050 items
        pm.expand_all();
        assert_eq!(pm.cached_visible_nodes.len(), 2050);

        // Test O(1) node lookup by tree indices
        let lookup_start = std::time::Instant::now();
        for idx in (0..2050).step_by(50) {
            let row = &pm.cached_visible_nodes[idx];
            let node = pm.get_node(&row.indices);
            assert!(node.is_some());
        }
        let lookup_elapsed = lookup_start.elapsed();
        assert!(lookup_elapsed.as_millis() < 5, "Node lookups took too long: {:?}", lookup_elapsed);

        // Test search filter across 2,000 files
        let search_start = std::time::Instant::now();
        pm.search_query = "item_42_17".to_string();
        pm.rebuild_cached_visible_nodes();
        let search_elapsed = search_start.elapsed();
        // Should find dir_42 and item_42_17.tex (2 items)
        assert_eq!(pm.cached_visible_nodes.len(), 2);
        assert!(search_elapsed.as_millis() < 50, "Search took too long: {:?}", search_elapsed);
    }

    #[test]
    fn test_escape_cargo_target_dir_and_rust_artifact_ignoring() {
        let temp_dir = tempfile::tempdir().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();
        std::fs::write(workspace_root.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        let target_release = workspace_root.join("target").join("release");
        let target_debug = workspace_root.join("target").join("debug");
        let normal_sub = workspace_root.join("docs").join("release");
        std::fs::create_dir_all(&target_release).unwrap();
        std::fs::create_dir_all(&target_debug).unwrap();
        std::fs::create_dir_all(&normal_sub).unwrap();

        // Double-clicking inside target/release or target/debug should step up to workspace_root
        assert_eq!(
            ProjectManager::escape_cargo_target_dir(target_release),
            workspace_root
        );
        assert_eq!(
            ProjectManager::escape_cargo_target_dir(target_debug),
            workspace_root
        );

        // Normal directories must not be altered
        assert_eq!(
            ProjectManager::escape_cargo_target_dir(normal_sub.clone()),
            normal_sub
        );

        // Rust build artifacts should be ignored
        assert!(ProjectManager::is_ignored("libfoo.rlib"));
        assert!(ProjectManager::is_ignored("libfoo.rmeta"));
        assert!(ProjectManager::is_ignored("laketex.pdb"));
        assert!(ProjectManager::is_ignored("incremental"));
    }
}

