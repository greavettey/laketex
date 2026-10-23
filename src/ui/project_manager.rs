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
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

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
    /// Classifies a path based on filesystem metadata and file extension.
    pub fn from_path(path: &Path) -> Self {
        if path.is_dir() {
            return Self::Directory;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        match ext.as_str() {
            "tex" | "ltx" => Self::Tex,
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp" | "pdf" | "eps" => Self::Image,
            "bib" => Self::Bib,
            "sty" | "cls" | "cfg" | "dtx" | "ins" => Self::StyleOrConfig,
            "csv" | "tsv" | "dat" | "json" | "txt" | "yaml" | "yml" => Self::Data,
            _ => Self::Other,
        }
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
    /// Checks if this node matches the search filter (case-insensitive).
    pub fn matches_filter(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        self.name.to_lowercase().contains(&query.to_lowercase())
    }

    /// Checks if this node or any of its recursive descendants matches the filter.
    pub fn contains_match(&self, query: &str) -> bool {
        if self.matches_filter(query) {
            return true;
        }
        self.children.iter().any(|c| c.contains_match(query))
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
}

impl ProjectManager {
    /// Initializes a new ProjectManager rooted at `initial_dir` or the current directory.
    pub fn new(initial_dir: Option<PathBuf>) -> Self {
        let root = initial_dir
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
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
        };
        mgr.auto_detect_master_document();
        mgr.refresh();
        mgr
    }

    /// Automatically detects a candidate master document (e.g. `main.tex`, `document.tex`) in the root directory.
    pub fn auto_detect_master_document(&mut self) {
        if let Some(ref m) = self.master_document {
            if m.is_file() {
                return;
            }
        }
        for candidate in &["main.tex", "document.tex", "thesis.tex", "paper.tex", "index.tex"] {
            let p = self.root_dir.join(candidate);
            if p.is_file() {
                if let Ok(content) = std::fs::read_to_string(&p) {
                    if content.contains("\\documentclass") {
                        self.master_document = Some(p);
                        return;
                    }
                }
            }
        }
    }

    /// Sets a new project root directory and rescans its contents.
    pub fn set_root_dir(&mut self, new_root: PathBuf) {
        self.root_dir = new_root.clone();
        self.target_folder = new_root;
        self.auto_detect_master_document();
        self.refresh();
        self.set_status(format!("Working directory: {}", self.root_dir.display()));
    }

    /// Sets a status feedback message displayed inside the modal and sidebar.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now()));
    }

    /// Rescans the filesystem starting from `root_dir`.
    pub fn refresh(&mut self) {
        self.root_nodes = Self::scan_dir(&self.root_dir, &self.root_dir);
    }

    /// Recursively scans a directory and builds the project tree.
    fn scan_dir(dir: &Path, root: &Path) -> Vec<ProjectNode> {
        let mut nodes = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return nodes,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if Self::is_ignored(&name) {
                continue;
            }

            let file_type = ProjectFileType::from_path(&path);
            let metadata = entry.metadata().ok();
            let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = metadata.and_then(|m| m.modified().ok());

            let relative_path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();

            let children = if file_type == ProjectFileType::Directory {
                Self::scan_dir(&path, root)
            } else {
                Vec::new()
            };

            nodes.push(ProjectNode {
                path,
                relative_path,
                name,
                file_type,
                size_bytes,
                modified,
                children,
                is_expanded: true,
            });
        }

        // Sort: directories first (alphabetically), then files (alphabetically)
        nodes.sort_by(|a, b| {
            match (
                a.file_type == ProjectFileType::Directory,
                b.file_type == ProjectFileType::Directory,
            ) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
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
        {
            return true;
        }
        matches!(
            name,
            "target"
                | "node_modules"
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

    /// Creates a new subfolder inside `target_folder`.
    pub fn create_folder(&mut self, folder_name: &str) -> Result<PathBuf, String> {
        let trimmed = folder_name.trim();
        if trimmed.is_empty() {
            return Err("Folder name cannot be empty".to_string());
        }
        let folder_path = self.target_folder.join(trimmed);
        if folder_path.exists() {
            return Err("A file or folder with that name already exists".to_string());
        }
        std::fs::create_dir_all(&folder_path).map_err(|e| e.to_string())?;
        self.refresh();
        self.set_status(format!("Created folder: {}", trimmed));
        Ok(folder_path)
    }

    /// Creates a new file inside `target_folder`, populated with a starter template.
    pub fn create_file(&mut self, file_name: &str) -> Result<PathBuf, String> {
        let trimmed = file_name.trim();
        if trimmed.is_empty() {
            return Err("File name cannot be empty".to_string());
        }
        let file_path = self.target_folder.join(trimmed);
        if file_path.exists() {
            return Err("A file with that name already exists".to_string());
        }

        let is_root = self.target_folder == self.root_dir;
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
        self.refresh();
        self.set_status(format!("Created file: {}", trimmed));
        Ok(file_path)
    }

    /// Prompts a file dialog to import one or more files into `target_folder`.
    pub fn import_files_dialog(&mut self) -> Result<usize, String> {
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
                let dest_path = self.target_folder.join(file_name);
                if let Err(e) = std::fs::copy(src_path, &dest_path) {
                    return Err(format!("Failed to copy {}: {e}", src_path.display()));
                }
                count += 1;
            }
        }

        self.refresh();
        let target_display = self.relative_target_display();
        self.set_status(format!("Imported {count} file(s) into {target_display}"));
        Ok(count)
    }

    /// Deletes a file or directory from disk.
    pub fn delete_item(&mut self, path: &Path) -> Result<(), String> {
        if path.is_dir() {
            std::fs::remove_dir_all(path).map_err(|e| e.to_string())?;
        } else {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        self.refresh();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        self.set_status(format!("Deleted: {name}"));
        Ok(())
    }

    /// Renames a file or directory on disk.
    pub fn rename_item(&mut self, old_path: &Path, new_name: &str) -> Result<PathBuf, String> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err("Name cannot be empty".to_string());
        }
        let parent = old_path.parent().unwrap_or(&self.root_dir);
        let new_path = parent.join(trimmed);
        if new_path.exists() {
            return Err("Destination name already exists".to_string());
        }
        std::fs::rename(old_path, &new_path).map_err(|e| e.to_string())?;
        self.refresh();
        self.set_status(format!("Renamed to: {trimmed}"));
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

    /// Calculates total count of files, directories, and total byte size.
    pub fn metrics(&self) -> (usize, usize, u64) {
        Self::calculate_metrics(&self.root_nodes)
    }

    fn calculate_metrics(nodes: &[ProjectNode]) -> (usize, usize, u64) {
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
        let query = self.search_query.clone();
        ScrollArea::vertical()
            .id_salt("project_tree_scroll")
            .auto_shrink([false, false])
            .max_height(if is_modal { 340.0 } else { 420.0 })
            .show(ui, |ui| {
                if self.root_nodes.is_empty() {
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
                } else {
                    for node in &self.root_nodes {
                        Self::render_node(ui, node, 0, &query, active_file, self.master_document.as_deref(), &mut intents);
                    }
                }
            });

        // Apply collected UI intents after tree traversal
        for intent in intents {
            match intent {
                UiIntent::ToggleExpand(path) => {
                    Self::toggle_expand_in_nodes(&mut self.root_nodes, &path);
                }
                UiIntent::SetTargetFolder(path) => {
                    self.target_folder = path;
                    self.new_file_open = true;
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
                    self.target_folder = path;
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

    /// Renders a single node (and its recursive children) in the tree.
    fn render_node(
        ui: &mut egui::Ui,
        node: &ProjectNode,
        depth: usize,
        query: &str,
        active_file: Option<&Path>,
        master_file: Option<&Path>,
        intents: &mut Vec<UiIntent>,
    ) {
        if !query.is_empty() && !node.contains_match(query) {
            return;
        }

        ui.horizontal(|ui| {
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
                        || std::fs::canonicalize(p).ok().map_or(false, |cp| {
                            Some(cp) == std::fs::canonicalize(&node.path).ok()
                        })
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

                let forward_rel_path = node
                    .relative_path
                    .to_string_lossy()
                    .replace('\\', "/");

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
                            let is_master = master_file.map_or(false, |m| m == node.path);
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
                                intents.push(UiIntent::InsertInput {
                                    rel_path: forward_rel_path.clone(),
                                });
                            }
                        }
                        ProjectFileType::Image => {
                            let stem = node
                                .path
                                .file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            if ui
                                .small_button("➕ Figure")
                                .on_hover_text("Insert \\begin{figure}...\\includegraphics environment")
                                .clicked()
                            {
                                intents.push(UiIntent::InsertFigure {
                                    rel_path: forward_rel_path.clone(),
                                    label: stem.clone(),
                                });
                            }
                            if ui
                                .small_button("📋 Snippet")
                                .on_hover_text("Copy \\includegraphics snippet to clipboard")
                                .clicked()
                            {
                                let snippet = format!(
                                    "\\includegraphics[width=0.8\\linewidth]{{{forward_rel_path}}}"
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
                                let bib_stem = forward_rel_path.trim_end_matches(".bib");
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

        // Recursively render expanded directory children
        if node.file_type == ProjectFileType::Directory && node.is_expanded {
            for child in &node.children {
                Self::render_node(ui, child, depth + 1, query, active_file, master_file, intents);
            }
        }
    }

    /// Toggles the `is_expanded` state of a folder matching `target_path`.
    fn toggle_expand_in_nodes(nodes: &mut [ProjectNode], target_path: &Path) {
        for node in nodes {
            if node.path == target_path {
                node.is_expanded = !node.is_expanded;
                return;
            }
            if !node.children.is_empty() {
                Self::toggle_expand_in_nodes(&mut node.children, target_path);
            }
        }
    }
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
}
