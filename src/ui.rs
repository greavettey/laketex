//! # LakeTex Graphical User Interface (GUI)
//!
//! Provides the primary desktop application interface built using `eframe` and `egui`.
//!
//! Orchestrates:
//! - Top action toolbar with compilation triggers and dynamic theme selection ([`toolbar`])
//! - Split editor panel with syntax highlighting and SyncTeX positioning ([`editor_panel`])
//! - Collapsible navigation sidebar ([`sidebar`])
//! - Slide-up diagnostic console drawer ([`drawer`])
//! - Dual-mode vector/PDF live document viewer ([`viewer_panel`])
//! - Key-value color theming engine ([`theme`])

pub mod config;
pub mod drawer;
pub mod editor_panel;
pub mod logo;
pub mod project_manager;
pub mod settings;
pub mod sidebar;
pub mod theme;
pub mod toolbar;
pub mod viewer_panel;

use crate::compiler::engine::EngineKind;
use crate::compiler::{CompileOptions, CompilerWorker, Diagnostic};
use crate::document::packages::{PackageDependency, PackageManager};
use crate::document::{BuiltinDoc, BuiltinDocKind, Document};
use crate::engine::layout::{DocumentLayout, TypesetPage};
use crate::engine::parser::NativeLatexParser;
use crate::headless::{SharedAppState, StatefulUiCommand};
use crate::ui::config::AppConfig;
use crate::ui::drawer::{Drawer, DrawerAction, DrawerTab};
use crate::ui::editor_panel::EditorPanel;
use crate::ui::project_manager::{ProjectAction, ProjectManager};
use crate::ui::settings::{SettingsAction, SettingsModal};
use crate::ui::sidebar::{Sidebar, SidebarAction};
use crate::ui::theme::ThemeConfig;
use crate::ui::toolbar::Toolbar;
use crate::ui::viewer_panel::{ViewMode, ViewerAction, ViewerPanel};
use crate::viewer::canvas::PdfCanvas;
use crate::viewer::PdfRenderer;
use eframe::egui::{self, vec2, Color32, RichText, TopBottomPanel};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

/// Standardized height for panel header menus (sidebar tab bar, editor source view menu, preview pane menu, and top toolbar).
pub const PANEL_HEADER_HEIGHT: f32 = 25.0;

/// Holds state for user-consented external tool dependency installation.
pub struct DependencyPromptState {
    /// Whether the interactive consent dialog is currently displayed.
    pub show_modal: bool,
    /// Pending external package dependency awaiting user consent.
    pub pending_dependency: Option<PackageDependency>,
    /// Whether background package manager installation is actively running.
    pub is_installing: bool,
    /// Current feedback message during background installation.
    pub install_status: Option<String>,
    /// Whether the user dismissed or refused installation for this session.
    pub user_declined: bool,
    /// Whether the automated prompt has already fired once to avoid repeated popups.
    pub auto_prompted_once: bool,
    /// Channel sender transmitting installation completion or failure messages.
    pub tx: Sender<Result<String, String>>,
    /// Channel receiver listening for background installer completion events.
    pub rx: Receiver<Result<String, String>>,
}

impl Default for DependencyPromptState {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            show_modal: false,
            pending_dependency: None,
            is_installing: false,
            install_status: None,
            user_declined: false,
            auto_prompted_once: false,
            tx,
            rx,
        }
    }
}

/// The top-level LakeTex desktop application state container.
pub struct LakeTexApp {
    /// Active document buffer, undo/redo history, and file metadata.
    pub document: Document,
    /// Pure-Rust PDFium vector and raster PDF renderer.
    pub renderer: PdfRenderer,
    /// GPU interactive canvas managing zoom, pan, and texture caching.
    pub canvas: PdfCanvas,
    /// Asynchronous background compilation worker thread.
    pub compiler: CompilerWorker,
    /// Active LaTeX compilation engine backend (Tectonic, Native, pdfLaTeX, etc.).
    pub engine: EngineKind,
    /// Preview display mode (Pure-Rust Native live view vs PDF raster vector view).
    pub view_mode: ViewMode,
    /// Whether documents recompile automatically on keystroke edits.
    pub auto_compile: bool,
    /// Currently active visual color theme.
    pub current_theme: ThemeConfig,
    /// All discovered built-in and external themes loaded from disk.
    pub available_themes: Vec<ThemeConfig>,

    /// Central text editor view with syntax highlighting and line numbers.
    pub editor_panel: EditorPanel,
    /// Left-hand navigation sidebar (outline, project files, symbols).
    pub sidebar: Sidebar,
    /// Expandable bottom console drawer (diagnostics, audit, logs).
    pub drawer: Drawer,
    /// Project manager modal and working directory tree browser.
    pub project_manager: ProjectManager,

    /// Whether the left navigation sidebar is visible.
    pub show_sidebar: bool,
    /// Whether the bottom console drawer is open.
    pub show_drawer: bool,

    /// Pure-Rust typeset pages generated by the native layout engine.
    pub typeset_pages: Vec<TypesetPage>,
    /// Latest raw PDF binary bytes compiled by the active engine.
    pub latest_pdf_bytes: Option<Vec<u8>>,
    /// Structured compiler warnings, errors, and syntax diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Raw terminal standard output produced by the latest compilation.
    pub raw_stdout: String,
    /// Raw terminal standard error produced by the latest compilation.
    pub raw_stderr: String,
    /// Compilation duration in milliseconds.
    pub last_duration_ms: Option<u128>,
    /// Monotonically increasing task identifier of the latest submitted compile job.
    pub current_compile_task_id: u64,

    /// Port of the accompanying background headless REST API server (if enabled).
    pub api_port: Option<u16>,
    /// Shared bridge for stateful REST API live editor synchronization (`--with-api --with-state`).
    pub shared_api_state: Option<Arc<SharedAppState>>,
    /// Status bar notification message.
    pub status_message: String,
    /// State machine for user-consented external tool installs (e.g., Pygments).
    pub dependency_prompt: DependencyPromptState,

    /// Application preferences and persistent settings configuration.
    pub config: AppConfig,
    /// Preferences and configuration modal dialog manager.
    pub settings_modal: SettingsModal,
    /// Pending action awaiting user confirmation of unsaved document changes.
    pub pending_close_action: Option<PendingCloseAction>,
    /// Confirmed close flag to prevent re-intercepting window close after user confirms Discard Changes or Save and Continue.
    pub allow_close: bool,
    /// Name of the theme last applied to the egui context to avoid redundant per-frame style clones.
    last_applied_theme: String,
    /// Debounce deadline for persisting `Ctrl+Scroll` editor font size adjustments to disk.
    font_size_save_deadline: Option<std::time::Instant>,
    /// Coalesced debounce deadline `(deadline, last_edited_line)` for recording significant manual user typing bursts in the Audit Log
    user_edit_audit_deadline: Option<(std::time::Instant, Option<usize>)>,
}

/// Target action deferred while confirming unsaved document modifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingCloseAction {
    /// Exit the application window
    ExitApp,
    /// Create a new empty document
    NewDocument,
    /// Open the file picker dialog
    OpenFileDialog,
    /// Open a specific document file from disk
    OpenFile(PathBuf),
    /// Load an offline built-in manual into the editor
    LoadBuiltinDoc(BuiltinDocKind),
}

impl LakeTexApp {
    /// Initializes a new [`LakeTexApp`] desktop session, loading font fallbacks,
    /// discovery themes, persistent configuration, and any initially requested document.
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        initial_file: Option<PathBuf>,
        api_port: Option<u16>,
    ) -> Self {
        Self::new_with_state(cc, initial_file, api_port, None)
    }

    /// Initializes a new [`LakeTexApp`] desktop session with optional stateful REST API synchronization (`--with-state`).
    pub fn new_with_state(
        cc: &eframe::CreationContext<'_>,
        initial_file: Option<PathBuf>,
        api_port: Option<u16>,
        shared_api_state: Option<Arc<SharedAppState>>,
    ) -> Self {
        crate::log_startup("new_with_state: starting configure_fonts");
        // Configure math and symbol fallback fonts
        ThemeConfig::configure_fonts(&cc.egui_ctx);
        crate::log_startup("new_with_state: configure_fonts done");

        let document = if let Some(ref path) = initial_file {
            Document::load_from_file(path).unwrap_or_default()
        } else {
            Document::default()
        };
        crate::log_startup("new_with_state: document loaded");

        let config = AppConfig::load();
        crate::log_startup("new_with_state: AppConfig::load done");
        let renderer = PdfRenderer::new();
        crate::log_startup("new_with_state: PdfRenderer::new done");
        let mut canvas = PdfCanvas::new();
        canvas.set_color_mode(config.pdf_color_mode);
        canvas.set_base_render_width(config.pdf_render_width);
        canvas.fit_to_width = config.fit_to_width;

        let compiler = CompilerWorker::new();
        crate::log_startup("new_with_state: CompilerWorker::new done");
        PackageManager::warmup_in_background();
        crate::log_startup("new_with_state: PackageManager warmup spawned");
        let engine = config.default_engine;
        let view_mode = config.view_mode;
        let auto_compile = config.auto_compile;

        // Compute initial native layout for instant display
        let ast = NativeLatexParser::parse(&document.content);
        let typeset_pages = DocumentLayout::layout_document(&ast);
        let latest_pdf_bytes = None;
        crate::log_startup("new_with_state: native layout done");

        let initial_working_dir =
            ProjectManager::resolve_default_working_dir(initial_file.as_deref());

        // Submit initial compilation in background worker thread
        let current_compile_task_id = compiler.submit_with_options(
            document.content.clone(),
            engine,
            CompileOptions {
                work_dir: Some(initial_working_dir.clone()),
                doc_name: Some(document.file_name()),
                allow_shell_escape: config.allow_shell_escape,
                shadow_subdocument: None,
                cancel_token: None,
            },
        );
        crate::log_startup("new_with_state: initial compile submitted");

        let project_manager = ProjectManager::new(Some(initial_working_dir));
        crate::log_startup("new_with_state: ProjectManager::new done");

        let available_themes = ThemeConfig::discover_themes();
        crate::log_startup("new_with_state: discover_themes done");
        let current_theme = available_themes
            .iter()
            .find(|t| t.name == config.theme_name)
            .cloned()
            .unwrap_or_else(|| {
                available_themes
                    .iter()
                    .find(|t| t.name == "Dracula (JetBrains)")
                    .cloned()
                    .unwrap_or_else(ThemeConfig::dracula_jetbrains)
            });

        if let Some(ref shared) = shared_api_state {
            shared.set_egui_context(cc.egui_ctx.clone());
        }
        crate::ui::drawer::register_audit_egui_context(cc.egui_ctx.clone());

        let mut drawer = Drawer::new();
        drawer.log_user_event(
            "Session Started",
            format!(
                "Initialized '{}' ({} lines, engine={})",
                document.file_name(),
                document.line_count(),
                engine.display_name()
            ),
            Some(1),
        );
        if let Some(port) = api_port {
            if shared_api_state.is_some() {
                drawer.log_event(
                    crate::ui::drawer::AuditActor::ApiMutation,
                    "REST API Active",
                    format!(
                        "Stateful Live REST API listening on http://127.0.0.1:{port} (--with-api --with-state)"
                    ),
                    None,
                );
            } else {
                drawer.log_event(
                    crate::ui::drawer::AuditActor::ApiRead,
                    "REST API Active",
                    format!(
                        "Stateless Headless REST API listening on http://127.0.0.1:{port} (--with-api)"
                    ),
                    None,
                );
            }
        }

        let app = Self {
            document,
            renderer,
            canvas,
            compiler,
            engine,
            view_mode,
            auto_compile,
            current_theme,
            available_themes,
            editor_panel: {
                let mut ep = EditorPanel::new();
                ep.font_size = config.font_size;
                ep
            },
            sidebar: Sidebar::new(),
            drawer,
            project_manager,
            show_sidebar: true,
            show_drawer: false,
            typeset_pages,
            latest_pdf_bytes,
            diagnostics: Vec::new(),
            raw_stdout: String::new(),
            raw_stderr: String::new(),
            last_duration_ms: None,
            current_compile_task_id,
            api_port,
            shared_api_state,
            status_message: "Ready".to_string(),
            dependency_prompt: DependencyPromptState::default(),
            config,
            settings_modal: SettingsModal::new(),
            pending_close_action: None,
            allow_close: false,
            last_applied_theme: String::new(),
            font_size_save_deadline: None,
            user_edit_audit_deadline: None,
        };
        app.sync_shared_api_state();
        app
    }

    /// Applies any queued stateful REST API commands to the live GUI editor and viewer.
    pub fn process_stateful_api_commands(&mut self) {
        let Some(ref shared) = self.shared_api_state else {
            return;
        };
        if !shared.has_pending_commands() {
            return;
        }
        let shared = shared.clone();
        let commands = shared.drain_commands();
        for cmd in commands {
            match cmd {
                StatefulUiCommand::SetDocumentContent {
                    source,
                    is_dirty,
                    trigger_compile,
                    status_message,
                } => {
                    self.document.content = source;
                    self.document.is_dirty = is_dirty;
                    self.document.refresh_caches();
                    self.status_message = status_message;
                    if trigger_compile {
                        self.trigger_compilation();
                    } else {
                        self.recompute_native_layout();
                    }
                }
                StatefulUiCommand::ApplyCompileResult {
                    source,
                    is_dirty,
                    engine,
                    result,
                } => {
                    self.engine = engine;
                    if let Some(src) = source {
                        self.document.content = src;
                        self.document.refresh_caches();
                        self.recompute_native_layout();
                    }
                    if let Some(dirty) = is_dirty {
                        self.document.is_dirty = dirty;
                    }
                    self.last_duration_ms = Some(result.duration_ms);
                    self.diagnostics = result.diagnostics;
                    self.raw_stdout = result.stdout;
                    self.raw_stderr = result.stderr;
                    if result.success && result.pdf_bytes.is_some() {
                        self.latest_pdf_bytes = result.pdf_bytes;
                        self.canvas.invalidate();
                        self.status_message = format!("API compiled in {} ms", result.duration_ms);
                    } else if !result.success {
                        self.status_message = "API compilation failed with errors".to_string();
                    }
                }
                StatefulUiCommand::SetEngine(eng) => {
                    self.engine = eng;
                }
                StatefulUiCommand::Navigate {
                    line,
                    page,
                    view_mode,
                    zoom,
                    show_drawer,
                    drawer_tab,
                    show_sidebar,
                } => {
                    if let Some(l) = line {
                        self.editor_panel.jump_to_line(l);
                    }
                    if let Some(p) = page {
                        self.canvas.current_page = p.saturating_sub(1);
                    }
                    if let Some(vm) = view_mode {
                        self.view_mode = match vm.trim().to_ascii_lowercase().as_str() {
                            "pdf" | "raster" | "pdfraster" => ViewMode::PdfRaster,
                            _ => ViewMode::NativeVector,
                        };
                    }
                    if let Some(z) = zoom {
                        self.canvas.zoom = z.clamp(0.25, 4.0);
                    }
                    if let Some(sd) = show_drawer {
                        self.show_drawer = sd;
                    }
                    if let Some(tab) = drawer_tab {
                        self.show_drawer = true;
                        self.drawer.current_tab = match tab.trim().to_ascii_lowercase().as_str() {
                            "audit" | "renderaudit" => DrawerTab::RenderAudit,
                            "audit_log" | "auditlog" | "history" => DrawerTab::AuditLog,
                            "logs" | "log" => DrawerTab::Logs,
                            "api" => DrawerTab::Api,
                            _ => DrawerTab::Diagnostics,
                        };
                    }
                    if let Some(ss) = show_sidebar {
                        self.show_sidebar = ss;
                    }
                }
            }
        }
    }

    /// Synchronizes the current GUI editor state into [`SharedAppState`] when running in `--with-state` mode.
    pub fn sync_shared_api_state(&self) {
        let Some(ref shared) = self.shared_api_state else {
            return;
        };
        if shared.has_pending_commands() {
            return;
        }
        let view_mode_str = match self.view_mode {
            ViewMode::NativeVector => "NativeVector",
            ViewMode::PdfRaster => "PdfRaster",
        };
        let work_dir = self
            .document
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or(&self.project_manager.root_dir);
        let current_page = self.canvas.current_page + 1;
        let zoom = self.canvas.zoom;

        let changed = shared
            .with_snapshot(|cur| {
                cur.is_dirty != self.document.is_dirty
                    || cur.engine != self.engine
                    || cur.view_mode != view_mode_str
                    || cur.current_page != current_page
                    || (cur.zoom - zoom).abs() > 0.001
                    || cur.last_duration_ms != self.last_duration_ms
                    || cur.diagnostics.len() != self.diagnostics.len()
                    || cur.file_path != self.document.path
                    || cur.working_dir.as_path() != work_dir
                    || cur.master_document != self.project_manager.master_document
                    || cur.source.len() != self.document.content.len()
                    || cur.source != self.document.content
            })
            .unwrap_or(false);

        if changed {
            let file_name = self.document.file_name();
            let work_dir_buf = work_dir.to_path_buf();
            let line_count = self.document.line_count();
            let word_count = self.document.word_count();
            let char_count = self.document.char_count();
            shared.sync_from_gui(|s| {
                if s.source != self.document.content {
                    s.source.clone_from(&self.document.content);
                }
                s.line_count = line_count;
                s.word_count = word_count;
                s.char_count = char_count;
                s.file_name = file_name;
                s.file_path.clone_from(&self.document.path);
                s.working_dir = work_dir_buf;
                s.master_document
                    .clone_from(&self.project_manager.master_document);
                s.is_dirty = self.document.is_dirty;
                s.engine = self.engine;
                s.view_mode = view_mode_str.to_string();
                s.current_page = current_page;
                s.zoom = zoom;
                s.diagnostics.clone_from(&self.diagnostics);
                s.last_duration_ms = self.last_duration_ms;
            });
        }
    }

    /// Displays an interactive consent dialog asking the user whether to install a missing external tool.
    pub fn prompt_dependency_consent(&mut self, dep: PackageDependency) {
        self.dependency_prompt.pending_dependency = Some(dep);
        self.dependency_prompt.show_modal = true;
        self.dependency_prompt.install_status = None;
    }

    /// Re-parses the document AST and recalculates page layout coordinates for the native vector view.
    pub fn recompute_native_layout(&mut self) {
        let ast = NativeLatexParser::parse(&self.document.content);
        self.typeset_pages = DocumentLayout::layout_document(&ast);
    }

    /// Triggers compilation using the active engine backend (debounced on worker thread for auto-compile).
    pub fn trigger_compilation(&mut self) {
        self.trigger_compilation_with_mode(false);
    }

    /// Triggers compilation with explicit manual vs. background auto-compile mode.
    ///
    /// When editing a subdocument compiled via a configured Master Document:
    /// - Manual compiles (or when `config.auto_save_on_compile` is enabled) flush dirty child files to disk.
    /// - Background auto-compiles stage unsaved child edits into a hidden shadow file so the real file on disk is untouched.
    pub fn trigger_compilation_with_mode(&mut self, is_manual: bool) {
        // Check if a Master Document is configured for multi-file projects (only applies when editing sub-files without \documentclass)
        let is_subdocument = !self.document.content.contains("\\documentclass");
        if is_subdocument {
            if let Some(master_path) = self.project_manager.master_document.clone() {
                if let Some(current_path) = self.document.path.clone() {
                    if current_path != master_path {
                        let shadow_subdocument = if (is_manual || self.config.auto_save_on_compile)
                            && self.document.is_dirty
                        {
                            let _ = self.document.save_to_file(&current_path);
                            self.document.is_dirty = false;
                            None
                        } else if self.document.is_dirty {
                            Some((current_path.clone(), self.document.content.clone()))
                        } else {
                            None
                        };

                        if let Ok(master_source) = std::fs::read_to_string(&master_path) {
                            let master_dir = master_path.parent().map(|p| p.to_path_buf());
                            let master_name = master_path
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("main.tex");
                            let id = self.compiler.submit_with_options(
                                master_source.clone(),
                                self.engine,
                                CompileOptions {
                                    work_dir: master_dir,
                                    doc_name: Some(master_name.to_string()),
                                    allow_shell_escape: self.config.allow_shell_escape,
                                    shadow_subdocument,
                                    cancel_token: None,
                                },
                            );
                            self.current_compile_task_id = id;
                            let ast = NativeLatexParser::parse(&master_source);
                            self.typeset_pages = DocumentLayout::layout_document(&ast);
                            self.status_message = format!("Compiling master [{master_name}]...");
                            return;
                        }
                    }
                }
            }
        }

        let ast = NativeLatexParser::parse(&self.document.content);
        self.typeset_pages = DocumentLayout::layout_document(&ast);

        if self.engine == EngineKind::Native && is_manual {
            // Instant in-process compilation for manual Native builds
            let start = std::time::Instant::now();
            match crate::engine::pdf_exporter::NativePdfExporter::export_to_pdf_bytes(&ast) {
                Ok(bytes) => {
                    let ms = start.elapsed().as_millis();
                    self.last_duration_ms = Some(ms);
                    self.latest_pdf_bytes = Some(bytes);
                    self.diagnostics.clear();
                    self.status_message = format!("Native build: {ms} ms");
                }
                Err(e) => {
                    self.status_message = format!("Native error: {e}");
                }
            }
        } else {
            let work_dir = self
                .document
                .path
                .as_ref()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .or_else(|| Some(self.project_manager.root_dir.clone()));
            let doc_name = Some(self.document.file_name());
            let id = self.compiler.submit_with_options(
                self.document.content.clone(),
                self.engine,
                CompileOptions {
                    work_dir,
                    doc_name,
                    allow_shell_escape: self.config.allow_shell_escape,
                    shadow_subdocument: None,
                    cancel_token: None,
                },
            );
            self.current_compile_task_id = id;
            self.status_message = "Compiling...".to_string();
        }
    }

    /// Cancels active or frozen compilation, terminates any running compiler processes, and resets UI state.
    pub fn cancel_compilation(&mut self) {
        self.compiler.cancel();
        self.current_compile_task_id = self.compiler.next_task_id();
        self.status_message = "Compilation cancelled by user".to_string();
        self.raw_stderr = "Compilation aborted: compiler process terminated by user.".to_string();
        self.drawer.log_user_event(
            "Compile Cancelled",
            format!(
                "Aborted compilation of '{}' with {}",
                self.document.file_name(),
                self.engine.display_name()
            ),
            None,
        );
    }

    /// Evaluates keyboard shortcuts and triggers corresponding actions (save, compile, find, open, etc.).
    pub fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // While capturing a key combination in the settings modal, do not trigger normal shortcuts
        if self.settings_modal.is_capturing() {
            return;
        }

        // Cancel active or frozen compilation on Escape (when modals and find/replace are closed)
        if self.compiler.is_compiling()
            && !self.editor_panel.find_replace.is_open
            && !self.project_manager.show_modal
            && !self.settings_modal.is_open
            && ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.cancel_compilation();
        }

        if self.config.shortcuts.matches_compile(ctx) {
            self.canvas.invalidate();
            self.trigger_compilation_with_mode(true);
        }
        if self.config.shortcuts.matches_save(ctx) {
            self.save_document();
        }
        if self.config.shortcuts.matches_open(ctx) {
            self.open_file_dialog();
        }
        if self.config.shortcuts.matches_new(ctx) {
            self.new_document();
        }
        if self.config.shortcuts.matches_project_files(ctx) {
            self.project_manager.show_modal = !self.project_manager.show_modal;
        }
        if self.config.shortcuts.matches_find(ctx) {
            self.editor_panel.open_find();
        }
        if self.config.shortcuts.matches_replace(ctx) {
            self.editor_panel.open_replace();
        }
        if self.config.shortcuts.matches_settings(ctx) {
            self.settings_modal.toggle(&self.config);
        }
    }

    /// Saves the current editor content to its associated file on disk, or opens Save As if untitled.
    pub fn save_document(&mut self) {
        if let Some(path) = self.document.path.clone() {
            if let Err(e) = self.document.save_to_file(&path) {
                self.status_message = format!("Save error: {e}");
            } else {
                self.status_message = format!("Saved {}", path.display());
                self.drawer.log_user_event(
                    "File Saved",
                    format!(
                        "Saved '{}' ({} lines, {} bytes)",
                        self.document.file_name(),
                        self.document.line_count(),
                        self.document.content.len()
                    ),
                    None,
                );
            }
        } else {
            self.save_document_as();
        }
    }

    /// Prompts a native file dialog to choose a file path and saves document content.
    pub fn save_document_as(&mut self) {
        if let Some(mut path) = rfd::FileDialog::new()
            .add_filter("LaTeX Document", &["tex"])
            .save_file()
        {
            if path.extension().is_none() {
                path.set_extension("tex");
            }
            if let Err(e) = self.document.save_to_file(&path) {
                self.status_message = format!("Save error: {e}");
            } else {
                self.status_message = format!("Saved as {}", path.display());
                self.drawer.log_user_event(
                    "File Saved As",
                    format!(
                        "Saved document as '{}' ({} lines)",
                        self.document.file_name(),
                        self.document.line_count()
                    ),
                    None,
                );
                if let Some(parent) = path.parent() {
                    self.project_manager.set_root_dir(parent.to_path_buf());
                }
                self.trigger_compilation();
            }
        }
    }

    /// Displays an open-file dialog to select and load an existing `.tex` document (protects unsaved changes).
    pub fn open_file_dialog(&mut self) {
        if self.document.is_dirty {
            self.pending_close_action = Some(PendingCloseAction::OpenFileDialog);
        } else {
            self.perform_open_file_dialog();
        }
    }

    /// Performs the open file dialog directly.
    pub fn perform_open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("LaTeX Document", &["tex"])
            .pick_file()
        {
            self.perform_open_file_from_path(&path);
        }
    }

    /// Loads a `.tex` document from a filesystem path, re-indexes project root if needed, and triggers compile (protects unsaved changes).
    pub fn open_file_from_path(&mut self, path: &Path) {
        if self.document.is_dirty {
            self.pending_close_action = Some(PendingCloseAction::OpenFile(path.to_path_buf()));
        } else {
            self.perform_open_file_from_path(path);
        }
    }

    /// Loads a `.tex` document from a filesystem path directly without prompt.
    pub fn perform_open_file_from_path(&mut self, path: &Path) {
        match Document::load_from_file(path) {
            Ok(doc) => {
                self.document = doc;
                self.user_edit_audit_deadline = None;
                self.canvas.reset_cache();
                self.canvas.current_page = 0;
                let ast = NativeLatexParser::parse(&self.document.content);
                self.typeset_pages = DocumentLayout::layout_document(&ast);
                self.latest_pdf_bytes =
                    crate::engine::pdf_exporter::NativePdfExporter::export_to_pdf_bytes(&ast)
                        .ok();
                self.status_message = format!("Loaded {}", path.display());
                self.drawer.log_user_event(
                    "File Opened",
                    format!(
                        "Opened '{}' ({} lines, {} words)",
                        self.document.file_name(),
                        self.document.line_count(),
                        self.document.word_count()
                    ),
                    Some(1),
                );

                // If opened file lives in a different project directory, update working directory
                if let Some(parent) = path.parent() {
                    if !path.starts_with(&self.project_manager.root_dir) {
                        self.project_manager.set_root_dir(parent.to_path_buf());
                    }
                }

                self.trigger_compilation();
            }
            Err(e) => {
                self.status_message = format!("Open error: {e}");
            }
        }
    }

    /// Inserts LaTeX code snippet text into the editor (at the current caret position, or before `\end{document}` if not focused).
    pub fn insert_snippet_into_editor(&mut self, ctx: &egui::Context, snippet: &str) {
        let new_pos = self
            .editor_panel
            .insert_snippet_at_caret(ctx, &mut self.document.content, snippet);
        self.document.is_dirty = true;
        self.document.refresh_caches();
        let line_num = self
            .document
            .content
            .chars()
            .take(new_pos)
            .filter(|&c| c == '\n')
            .count()
            + 1;
        let first_line_preview = snippet
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("snippet")
            .trim();
        self.drawer.log_user_event(
            "Snippet Inserted",
            format!(
                "Inserted snippet '{}' ({} chars) into '{}'",
                first_line_preview,
                snippet.chars().count(),
                self.document.file_name()
            ),
            Some(line_num),
        );
        if self.auto_compile {
            self.trigger_compilation();
        } else {
            self.recompute_native_layout();
        }
    }

    /// Handles an action dispatched from the project manager modal (e.g. open file, insert figure).
    pub fn handle_project_action(&mut self, ctx: &egui::Context, action: ProjectAction) {
        match action {
            ProjectAction::None => {}
            ProjectAction::OpenTexFile(path) => {
                self.open_file_from_path(&path);
            }
            ProjectAction::InsertIncludeGraphics {
                relative_path,
                label,
            } => {
                let forward_path = relative_path.replace('\\', "/");
                let snippet = format!(
                    "\n\\begin{{figure}}[htbp]\n    \\centering\n    \\includegraphics[width=0.8\\linewidth]{{{forward_path}}}\n    \\caption{{{label}}}\n    \\label{{fig:{label}}}\n\\end{{figure}}\n"
                );
                ctx.copy_text(snippet.clone());
                self.insert_snippet_into_editor(ctx, &snippet);
                self.status_message = format!("Inserted figure: {forward_path}");
            }
            ProjectAction::InsertInput { relative_path } => {
                let forward_path = relative_path.replace('\\', "/");
                let snippet = format!("\n\\input{{{forward_path}}}\n");
                ctx.copy_text(snippet.clone());
                self.insert_snippet_into_editor(ctx, &snippet);
                self.status_message = format!("Inserted input: {forward_path}");
            }
            ProjectAction::CopyToClipboard(text) => {
                ctx.copy_text(text);
                self.status_message = "Copied to clipboard".to_string();
            }
            ProjectAction::WorkingDirChanged(dir) => {
                self.status_message = format!("Working directory: {}", dir.display());
                self.drawer.log_user_event(
                    "Workspace Changed",
                    format!("Switched project root to '{}'", dir.display()),
                    None,
                );
                if self.auto_compile {
                    self.trigger_compilation();
                }
            }
            ProjectAction::SetMasterDocument(path) => {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                self.status_message = format!("👑 Master Document: {name}");
                self.drawer.log_user_event(
                    "Master Document",
                    format!("Set master compilation document to '{name}'"),
                    None,
                );
                self.trigger_compilation();
            }
            ProjectAction::ClearMasterDocument => {
                self.status_message = "Master Document cleared".to_string();
                self.drawer.log_user_event(
                    "Master Document",
                    "Cleared master document setting",
                    None,
                );
                self.trigger_compilation();
            }
        }
    }

    /// Clears the editor and initializes a new blank document using default template (protects unsaved changes).
    pub fn new_document(&mut self) {
        if self.document.is_dirty {
            self.pending_close_action = Some(PendingCloseAction::NewDocument);
        } else {
            self.perform_new_document();
        }
    }

    /// Replaces the current editor content with a fresh starter document.
    pub fn perform_new_document(&mut self) {
        self.document = Document::default();
        self.user_edit_audit_deadline = None;
        self.canvas.reset_cache();
        self.canvas.current_page = 0;
        let ast = NativeLatexParser::parse(&self.document.content);
        self.typeset_pages = DocumentLayout::layout_document(&ast);
        self.latest_pdf_bytes =
            crate::engine::pdf_exporter::NativePdfExporter::export_to_pdf_bytes(&ast).ok();
        self.status_message = "New document created".to_string();
        self.drawer.log_user_event(
            "New Document",
            "Initialized new untitled document from template",
            Some(1),
        );
        self.trigger_compilation();
    }

    /// Loads an offline built-in documentation manual directly into editor (protects unsaved changes).
    pub fn load_builtin_doc(&mut self, kind: BuiltinDocKind) {
        if self.document.is_dirty {
            self.pending_close_action = Some(PendingCloseAction::LoadBuiltinDoc(kind));
        } else {
            self.perform_load_builtin_doc(kind);
        }
    }

    /// Loads an offline built-in documentation manual directly into the editor buffer.
    pub fn perform_load_builtin_doc(&mut self, kind: BuiltinDocKind) {
        let doc_info = BuiltinDoc::get(kind);
        self.document = Document::new(doc_info.content.to_string(), None);
        self.user_edit_audit_deadline = None;
        self.canvas.reset_cache();
        self.canvas.current_page = 0;
        let ast = NativeLatexParser::parse(&self.document.content);
        self.typeset_pages = DocumentLayout::layout_document(&ast);
        self.latest_pdf_bytes =
            crate::engine::pdf_exporter::NativePdfExporter::export_to_pdf_bytes(&ast).ok();
        self.status_message = format!("Loaded documentation: {}", doc_info.title);
        self.drawer.log_user_event(
            "Manual Loaded",
            format!("Loaded built-in documentation '{}'", doc_info.title),
            Some(1),
        );
        self.drawer.status_feedback = Some((
            format!("✓ Opened {}", doc_info.filename),
            std::time::Instant::now(),
        ));
        self.trigger_compilation();
    }

    /// Exports an offline built-in documentation manual as a standalone .tex file to user's disk.
    pub fn export_builtin_doc(&mut self, kind: BuiltinDocKind) {
        let doc_info = BuiltinDoc::get(kind);
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(doc_info.filename)
            .add_filter("LaTeX Document", &["tex"])
            .save_file()
        {
            if let Err(e) = std::fs::write(&path, doc_info.content) {
                self.status_message = format!("Export error: {e}");
            } else {
                self.status_message = format!("Exported {} to {}", doc_info.filename, path.display());
                self.drawer.status_feedback = Some((
                    format!("✓ Exported {}", doc_info.filename),
                    std::time::Instant::now(),
                ));
            }
        }
    }

    /// Exports a structured contextual error report (.md, .json, or .log) to user selected file.
    pub fn export_error_log(&mut self) {
        let report = crate::compiler::ErrorReport::generate(
            &self.document.content,
            &self.document.file_name(),
            self.engine,
            &self.diagnostics,
            &self.raw_stdout,
            &self.raw_stderr,
        );

        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("laketex_error_report.md")
            .add_filter("Markdown Report", &["md"])
            .add_filter("JSON Report", &["json"])
            .add_filter("Log File", &["log", "txt"])
            .save_file()
        {
            let content = if path.extension().and_then(|s| s.to_str()) == Some("json") {
                report.to_json()
            } else {
                report.to_markdown()
            };

            if let Err(e) = std::fs::write(&path, content) {
                self.status_message = format!("Export error: {e}");
            } else {
                self.status_message = format!("Exported error report to {}", path.display());
                self.drawer.status_feedback = Some((
                    format!("✓ Saved report to {}", path.file_name().unwrap_or_default().to_string_lossy()),
                    std::time::Instant::now(),
                ));
            }
        }
    }

    /// Exports an audit and rendering flaw diagnosis report (.md or .json) to user selected file.
    pub fn export_audit_report(&mut self) {
        let report = crate::engine::render_debugger::RenderDebugger::scan(
            &self.document.content,
            &self.document.file_name(),
            self.latest_pdf_bytes.as_deref(),
        );

        if let Some(path) = rfd::FileDialog::new()
            .set_file_name("laketex_render_audit.md")
            .add_filter("Markdown Report", &["md"])
            .add_filter("JSON Report", &["json"])
            .save_file()
        {
            let content = if path.extension().and_then(|s| s.to_str()) == Some("json") {
                report.to_json()
            } else {
                report.to_markdown()
            };

            if let Err(e) = std::fs::write(&path, content) {
                self.status_message = format!("Export error: {e}");
            } else {
                self.status_message = format!("Exported render audit report to {}", path.display());
                self.drawer.status_feedback = Some((
                    format!(
                        "✓ Saved audit report to {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    std::time::Instant::now(),
                ));
            }
        }
    }

    /// Renders the user consent modal dialog when a LaTeX package requires external tool installation.
    pub fn render_dependency_prompt(&mut self, ctx: &egui::Context) {
        if !self.dependency_prompt.show_modal {
            return;
        }

        let Some(dep) = self.dependency_prompt.pending_dependency.clone() else {
            return;
        };

        let mut close_modal = false;
        let mut trigger_install = false;
        let mut user_declined = false;

        egui::Window::new("📦 Package Dependency Consent")
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .default_height(340.0)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("📦").size(24.0));
                    ui.vertical(|ui| {
                        ui.heading(format!("Install External Dependency: {}", dep.name));
                        ui.label(
                            RichText::new(format!(
                                "Required by LaTeX package: \\usepackage{{{}}}",
                                dep.latex_package
                            ))
                            .size(11.5)
                            .color(Color32::from_gray(160)),
                        );
                    });
                });
                ui.separator();

                ui.label(RichText::new(&dep.description).color(Color32::from_gray(180)));
                ui.add_space(4.0);

                ui.group(|ui| {
                    ui.label(RichText::new("System Prerequisite Check:").strong());
                    let py_env = PackageManager::check_python_environment();
                    if py_env.python_available {
                        let ver = py_env.python_version.as_deref().unwrap_or("Ready");
                        let path = py_env.python_path.as_deref().unwrap_or("python");
                        let conda_tag = if py_env.is_conda {
                            let env_name = py_env.environment_name.as_deref().unwrap_or("env");
                            format!(" [Conda: {env_name}]")
                        } else {
                            String::new()
                        };
                        ui.colored_label(
                            Color32::from_rgb(152, 195, 121),
                            format!("✓ Python {ver}{conda_tag} ({path})"),
                        );
                        if py_env.pip_available {
                            ui.colored_label(
                                Color32::from_rgb(152, 195, 121),
                                "✓ pip package manager available",
                            );
                        } else if py_env.is_conda && py_env.conda_cli_path.is_some() {
                            ui.colored_label(
                                Color32::from_rgb(152, 195, 121),
                                "✓ conda package manager available (will install via conda-forge)",
                            );
                        } else {
                            ui.colored_label(
                                Color32::from_rgb(224, 108, 117),
                                "✗ pip / conda package manager missing",
                            );
                        }
                    } else {
                        ui.colored_label(
                            Color32::from_rgb(224, 108, 117),
                            "✗ Python not found on system PATH",
                        );
                    }

                    let is_installed = PackageManager::is_binary_available(&dep.binary_name);
                    if is_installed {
                        ui.colored_label(
                            Color32::from_rgb(152, 195, 121),
                            format!("✓ '{}' is currently installed", dep.binary_name),
                        );
                    } else {
                        ui.colored_label(
                            Color32::from_rgb(229, 192, 123),
                            format!("✗ '{}' is not installed", dep.binary_name),
                        );
                    }
                });

                let py_env = PackageManager::check_python_environment();
                let installer_tool = if !py_env.pip_available && py_env.is_conda && py_env.conda_cli_path.is_some() {
                    "conda"
                } else {
                    "pip"
                };
                let install_cmd_str = if installer_tool == "conda" {
                    let pkg = dep.pip_package.as_deref().unwrap_or(&dep.name);
                    format!("conda install -y -c conda-forge {pkg}")
                } else {
                    dep.install_command.clone()
                };

                ui.add_space(6.0);
                ui.label(format!("Would you like Laketex to install this package using {installer_tool}?"));
                ui.colored_label(
                    Color32::from_rgb(97, 175, 239),
                    format!("Command: {install_cmd_str}"),
                );

                if let Some(status) = &self.dependency_prompt.install_status {
                    ui.add_space(4.0);
                    if status.starts_with("Error:") {
                        egui::Frame::new()
                            .fill(Color32::from_rgba_unmultiplied(224, 108, 117, 30))
                            .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(224, 108, 117)))
                            .corner_radius(4.0)
                            .inner_margin(6.0)
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(80.0)
                                    .show(ui, |ui| {
                                        ui.colored_label(Color32::from_rgb(224, 108, 117), status);
                                    });
                            });
                    } else {
                        ui.colored_label(Color32::from_rgb(242, 201, 76), status);
                    }
                }

                ui.add_space(8.0);
                ui.separator();

                ui.horizontal(|ui| {
                    let py_env = PackageManager::check_python_environment();
                    let can_install = py_env.python_available
                        && (py_env.pip_available || (py_env.is_conda && py_env.conda_cli_path.is_some()))
                        && !self.dependency_prompt.is_installing;

                    if self.dependency_prompt.is_installing {
                        ui.spinner();
                        let wait_msg = if !py_env.pip_available && py_env.is_conda {
                            "Installing package via conda, please wait..."
                        } else {
                            "Installing package via pip, please wait..."
                        };
                        ui.label(wait_msg);
                    } else {
                        if ui
                            .add_enabled(
                                can_install,
                                egui::Button::new(
                                    RichText::new("✓ Install (Grant Consent)").strong(),
                                ),
                            )
                            .clicked()
                        {
                            trigger_install = true;
                        }

                        if ui.button("Use Draft Mode (Skip)").clicked() {
                            close_modal = true;
                        }

                        if ui.button("Don't Ask Again").clicked() {
                            user_declined = true;
                            close_modal = true;
                        }

                        if self.dependency_prompt.install_status.as_ref().map(|s| s.starts_with("Error:")).unwrap_or(false) {
                            if ui.button("Dismiss").clicked() {
                                close_modal = true;
                            }
                        }
                    }
                });
            });

        if close_modal {
            self.dependency_prompt.show_modal = false;
        }
        if user_declined {
            self.dependency_prompt.user_declined = true;
        }
        if trigger_install {
            self.dependency_prompt.is_installing = true;
            self.dependency_prompt.install_status =
                Some("Installing via pip in background...".to_string());
            let tx = self.dependency_prompt.tx.clone();
            let pip_pkg = dep
                .pip_package
                .clone()
                .unwrap_or_else(|| dep.name.to_lowercase());
            std::thread::spawn(move || {
                let res =
                    PackageManager::install_dependency(&pip_pkg, true).map_err(|e| e.to_string());
                let _ = tx.send(res);
            });
        }
    }

    /// Renders an unsaved changes confirmation dialog when a destructive action or app close is pending.
    pub fn render_unsaved_changes_modal(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_close_action.clone() else {
            return;
        };

        let mut execute_discard = false;
        let mut execute_save_and_continue = false;
        let mut cancel = false;

        egui::Window::new("⚠️ Unsaved Changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .fixed_size(vec2(420.0, 160.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⚠️").size(24.0));
                    ui.vertical(|ui| {
                        ui.heading("Do you want to save your changes?");
                        let doc_name = self.document.file_name();
                        ui.label(
                            RichText::new(format!(
                                "You have unsaved changes in \"{doc_name}\". If you don't save, changes will be lost."
                            ))
                            .size(12.0)
                            .color(Color32::from_gray(180)),
                        );
                    });
                });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new("Save and Continue").strong())
                        .clicked()
                    {
                        execute_save_and_continue = true;
                    }
                    if ui
                        .button(
                            RichText::new("Discard Changes")
                                .color(Color32::from_rgb(224, 108, 117)),
                        )
                        .clicked()
                    {
                        execute_discard = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        if cancel {
            self.pending_close_action = None;
        } else if execute_discard {
            self.pending_close_action = None;
            self.document.is_dirty = false;
            self.execute_pending_action(pending, false, ctx);
        } else if execute_save_and_continue {
            self.save_document();
            if !self.document.is_dirty {
                self.pending_close_action = None;
                self.execute_pending_action(pending, true, ctx);
            }
        }
    }

    /// Executes the deferred action after unsaved changes have been saved or discarded.
    pub fn execute_pending_action(
        &mut self,
        action: PendingCloseAction,
        _saved: bool,
        ctx: &egui::Context,
    ) {
        match action {
            PendingCloseAction::ExitApp => {
                self.document.is_dirty = false;
                self.allow_close = true;
                self.compiler.cancel();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            PendingCloseAction::NewDocument => {
                self.perform_new_document();
            }
            PendingCloseAction::OpenFileDialog => {
                self.perform_open_file_dialog();
            }
            PendingCloseAction::OpenFile(path) => {
                self.perform_open_file_from_path(&path);
            }
            PendingCloseAction::LoadBuiltinDoc(kind) => {
                self.perform_load_builtin_doc(kind);
            }
        }
    }
}

impl eframe::App for LakeTexApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        static FIRST_FRAME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
        if FIRST_FRAME.swap(false, std::sync::atomic::Ordering::Relaxed) {
            crate::log_startup("First frame update() entered!");
        }

        // Apply any pending live commands from the stateful REST API server (--with-api --with-state)
        self.process_stateful_api_commands();
        // Drain any background API or user audit events into the in-memory drawer buffer
        self.drawer.poll_api_audit_events();

        // Window close interception: if user clicks [X] while document is dirty and close not yet confirmed, cancel immediate close and show prompt
        if ctx.input(|i| i.viewport().close_requested()) && self.document.is_dirty && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending_close_action = Some(PendingCloseAction::ExitApp);
        }

        // Apply active theme only when changed
        if self.last_applied_theme != self.current_theme.name {
            self.current_theme.apply(ctx);
            self.last_applied_theme = self.current_theme.name.clone();
        }

        // Check for completed background compilation results
        if let Some(output) = self.compiler.try_receive() {
            // Discard stale compilation outputs from prior files or superseded debounce tasks
            if output.id >= self.current_compile_task_id {
                match output.result {
                    Ok(res) => {
                        self.last_duration_ms = Some(res.duration_ms);
                        self.diagnostics = res.diagnostics;
                        self.raw_stdout = res.stdout;
                        self.raw_stderr = res.stderr;

                        if res.success && res.pdf_bytes.is_some() {
                            self.latest_pdf_bytes = res.pdf_bytes;
                            self.status_message = format!("Compiled in {} ms", res.duration_ms);
                        } else if !res.success {
                            self.status_message = "Compilation failed with errors".to_string();
                        }
                    }
                    Err(err_msg) => {
                        self.status_message = format!("Compile failed: {err_msg}");
                        self.raw_stderr = err_msg;
                    }
                }
            }
        }

        // Check for completed external tool dependency installation
        if let Ok(res) = self.dependency_prompt.rx.try_recv() {
            self.dependency_prompt.is_installing = false;
            match res {
                Ok(msg) => {
                    self.status_message = "Dependency installed successfully!".to_string();
                    self.dependency_prompt.install_status = Some(msg);
                    self.dependency_prompt.show_modal = false;
                    self.document.refresh_caches();
                    self.trigger_compilation();
                }
                Err(err) => {
                    self.status_message = format!("Dependency install failed: {err}");
                    self.dependency_prompt.install_status = Some(format!("Error: {err}"));
                    self.dependency_prompt.show_modal = true;
                }
            }
        }

        // Auto-prompt user consent for missing package dependencies if needed (checked once on startup)
        if !self.dependency_prompt.auto_prompted_once
            && !self.dependency_prompt.user_declined
            && !self.dependency_prompt.is_installing
        {
            crate::log_startup("update: detecting dependencies");
            self.dependency_prompt.auto_prompted_once = true;
            let missing = PackageManager::detect_dependencies(&self.document.content);
            if let Some(first_missing) = missing.into_iter().find(|d| !d.installed) {
                self.prompt_dependency_consent(first_missing);
            }
            crate::log_startup("update: detecting dependencies done");
        }

        // Handle global keyboard shortcuts
        self.handle_shortcuts(ctx);

        // 1. Top Toolbar
        let toolbar_frame = egui::Frame::side_top_panel(&ctx.style())
            .inner_margin(egui::Margin::symmetric(8, 4));
        TopBottomPanel::top("top_toolbar")
            .frame(toolbar_frame)
            .show(ctx, |ui| {
            let error_count = self
                .diagnostics
                .iter()
                .filter(|d| d.severity == crate::compiler::DiagnosticSeverity::Error)
                .count();

            let prev_engine = self.engine;
            let prev_color_mode = self.canvas.color_mode;

            let action = Toolbar::show(
                ui,
                self.compiler.is_compiling(),
                self.last_duration_ms,
                error_count,
                &mut self.engine,
                &mut self.auto_compile,
                &mut self.show_sidebar,
                &mut self.show_drawer,
                &mut self.canvas.color_mode,
                &mut self.canvas.fit_to_width,
                &mut self.canvas.zoom,
            );

            if self.engine != prev_engine {
                self.config.default_engine = self.engine;
                let _ = self.config.save();
                self.drawer.log_user_event(
                    "Engine Changed",
                    format!(
                        "Switched compiler engine from {} to {}",
                        prev_engine.display_name(),
                        self.engine.display_name()
                    ),
                    None,
                );
                self.canvas.invalidate();
                self.trigger_compilation();
            }

            if self.canvas.color_mode != prev_color_mode {
                self.config.pdf_color_mode = self.canvas.color_mode;
                let _ = self.config.save();
            }

            if action.inspect_errors {
                self.show_drawer = true;
                self.drawer.current_tab = DrawerTab::Diagnostics;
            }

            if action.cancel_compile_requested {
                self.cancel_compilation();
            }

            if action.compile_requested {
                self.drawer.log_user_event(
                    "Manual Compile",
                    format!(
                        "Triggered manual build of '{}' with {}",
                        self.document.file_name(),
                        self.engine.display_name()
                    ),
                    None,
                );
                self.canvas.invalidate();
                self.trigger_compilation_with_mode(true);
            }
            if action.new_requested {
                self.new_document();
            }
            if action.open_requested {
                self.open_file_dialog();
            }
            if action.save_requested {
                self.save_document();
            }
            if action.save_as_requested {
                self.save_document_as();
            }
            if action.settings_requested {
                self.settings_modal.open(&self.config);
            }
            if let Some(kind) = action.load_builtin_doc {
                self.load_builtin_doc(kind);
            }
        });

        // 2. Bottom Status Bar
        TopBottomPanel::bottom("bottom_status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 0.0);

                let dirty_marker = if self.document.is_dirty { "● " } else { "" };
                ui.label(format!("{}{}", dirty_marker, self.document.file_name()));

                if let Some(master) = &self.project_manager.master_document {
                    let master_name = master.file_name().unwrap_or_default().to_string_lossy();
                    ui.separator();
                    ui.colored_label(
                        Color32::from_rgb(229, 192, 123),
                        format!("👑 Master: {master_name}"),
                    );
                }

                ui.separator();

                ui.colored_label(
                    if self.compiler.is_compiling() {
                        Color32::from_rgb(242, 201, 76)
                    } else {
                        Color32::from_rgb(152, 195, 121)
                    },
                    &self.status_message,
                );

                ui.separator();

                ui.label(format!("Lines: {}", self.document.line_count()));
                ui.label(format!("Words: {}", self.document.word_count()));
                ui.label(format!("Chars: {}", self.document.char_count()));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let api_label = match (self.api_port, self.shared_api_state.is_some()) {
                        (Some(port), true) => format!("REST API: :{port} (Stateful)"),
                        (Some(port), false) => format!("REST API: :{port}"),
                        (None, _) => "REST API: Off".to_string(),
                    };
                    ui.colored_label(
                        Color32::from_rgb(97, 175, 239),
                        api_label,
                    );
                    ui.separator();
                    ui.label(format!("Zoom: {:.0}%", self.canvas.zoom * 100.0));
                    ui.separator();
                    let page_total = match self.view_mode {
                        ViewMode::NativeVector => self.typeset_pages.len().max(1),
                        ViewMode::PdfRaster => self.canvas.total_pages.max(1),
                    };
                    ui.label(format!("Page: {}/{}", self.canvas.current_page + 1, page_total));
                });
            });
        });

        // 3. Collapsible Bottom Drawer (Diagnostics & Terminal Logs)
        if self.show_drawer {
            TopBottomPanel::bottom("collapsible_drawer")
                .resizable(true)
                .default_height(180.0)
                .show(ctx, |ui| {
                    let action = self.drawer.show(
                        ui,
                        &self.document.content,
                        &self.document.file_name(),
                        self.engine,
                        &self.diagnostics,
                        &self.raw_stdout,
                        &self.raw_stderr,
                        self.api_port,
                        self.shared_api_state.is_some(),
                        self.latest_pdf_bytes.as_deref(),
                    );
                    match action {
                        DrawerAction::JumpToLine(line) => {
                            self.editor_panel.jump_to_line(line);
                        }
                        DrawerAction::ExportErrorReport => {
                            self.export_error_log();
                        }
                        DrawerAction::ExportAuditReport => {
                            self.export_audit_report();
                        }
                        DrawerAction::PromptInstallDependency(dep) => {
                            self.prompt_dependency_consent(dep);
                        }
                        DrawerAction::None => {}
                    }
                });
        }

        // 4. Collapsible Left Sidebar (Outline, Packages, Snippets)
        if self.show_sidebar {
            let side_frame = egui::Frame::side_top_panel(&ctx.style())
                .inner_margin(egui::Margin::symmetric(8, 4));
            egui::SidePanel::left("left_sidebar")
                .resizable(true)
                .default_width(240.0)
                .width_range(160.0..=450.0)
                .frame(side_frame)
                .show(ctx, |ui| {
                    let action = self.sidebar.show(
                        ui,
                        &self.document.outline_cache,
                        &self.document.package_cache,
                        self.document.word_count(),
                        self.document.char_count(),
                        self.document.line_count(),
                        self.last_duration_ms,
                        &mut self.project_manager,
                        self.document.path.as_deref(),
                    );

                    match action {
                        SidebarAction::JumpToLine(line) => {
                            self.editor_panel.jump_to_line(line);
                        }
                        SidebarAction::InsertPackage(pkg, missing_dep) => {
                            self.document.content =
                                PackageManager::insert_package(&self.document.content, &pkg);
                            self.document.is_dirty = true;
                            self.document.refresh_caches();
                            self.drawer.log_user_event(
                                "Package Imported",
                                format!(
                                    "Injected \\usepackage{{{pkg}}} into '{}'",
                                    self.document.file_name()
                                ),
                                Some(2),
                            );
                            if self.auto_compile {
                                self.trigger_compilation();
                            } else {
                                self.recompute_native_layout();
                            }
                            if let Some(dep) = missing_dep {
                                self.prompt_dependency_consent(dep);
                            }
                        }
                        SidebarAction::InsertSnippet(snippet) => {
                            self.insert_snippet_into_editor(ctx, &snippet);
                        }
                        SidebarAction::PromptInstallDependency(dep) => {
                            self.prompt_dependency_consent(dep);
                        }
                        SidebarAction::Project(project_act) => {
                            self.handle_project_action(ctx, project_act);
                        }
                        SidebarAction::None => {}
                    }
                });
        }

        // 5. Central Split Area: Code Editor (Left) & Live Document Viewer (Right)
        let central_frame = egui::Frame::central_panel(&ctx.style())
            .inner_margin(egui::Margin::symmetric(8, 4));
        egui::CentralPanel::default().frame(central_frame).show(ctx, |ui| {
            let available_width = ui.available_width();
            let editor_width = (available_width * 0.48).max(300.0);

            ui.horizontal_top(|ui| {
                // Left pane: Code Editor
                ui.allocate_ui_with_layout(
                    vec2(editor_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        let active_doc_name = self.document.file_name();
                        let active_doc_stem = Path::new(&active_doc_name)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&active_doc_name);
                        let is_compiled_via_master = !self.document.content.contains("\\documentclass")
                            && self.project_manager.master_document.as_ref().is_some_and(|m| {
                                self.document.path.as_ref().is_some_and(|c| c != m)
                            });
                        let editor_diagnostics: std::borrow::Cow<'_, [Diagnostic]> =
                            if is_compiled_via_master {
                                std::borrow::Cow::Owned(
                                    self.diagnostics
                                        .iter()
                                        .filter(|d| {
                                            d.file.as_deref().is_some_and(|f| {
                                                let f_name = Path::new(f)
                                                    .file_name()
                                                    .and_then(|s| s.to_str())
                                                    .unwrap_or(f);
                                                let f_stem = Path::new(f_name)
                                                    .file_stem()
                                                    .and_then(|s| s.to_str())
                                                    .unwrap_or(f_name);
                                                f_name == active_doc_name || f_stem == active_doc_stem
                                            })
                                        })
                                        .cloned()
                                        .collect(),
                                )
                            } else {
                                std::borrow::Cow::Borrowed(&self.diagnostics)
                            };

                        crate::log_startup("update: editor_panel.show starting");
                        let text_changed = self.editor_panel.show_with_theme(
                            ui,
                            &mut self.document.content,
                            &editor_diagnostics,
                            self.current_theme.dark_mode,
                            Some(&self.current_theme),
                        );
                        crate::log_startup("update: editor_panel.show done");

                        if let Some((act, det, line)) =
                            self.editor_panel.pending_audit_event.take()
                        {
                            self.drawer.log_user_event(act, det, line);
                            self.user_edit_audit_deadline = None;
                        } else if text_changed {
                            let active_line = self
                                .editor_panel
                                .last_cursor_range
                                .map(|(s, _)| {
                                    self.document
                                        .content
                                        .chars()
                                        .take(s)
                                        .filter(|&c| c == '\n')
                                        .count()
                                        + 1
                                });
                            self.user_edit_audit_deadline = Some((
                                std::time::Instant::now() + Duration::from_millis(1500),
                                active_line,
                            ));
                        }

                        if let Some((deadline, edit_line)) = self.user_edit_audit_deadline {
                            if std::time::Instant::now() >= deadline {
                                self.drawer.log_user_event(
                                    "Document Edited",
                                    format!(
                                        "Modified '{}' ({} lines, {} words)",
                                        self.document.file_name(),
                                        self.document.line_count(),
                                        self.document.word_count()
                                    ),
                                    edit_line,
                                );
                                self.user_edit_audit_deadline = None;
                            } else {
                                ctx.request_repaint_after(Duration::from_millis(250));
                            }
                        }

                        if text_changed {
                            self.document.is_dirty = true;
                            self.document.refresh_caches();
                            if self.auto_compile {
                                self.trigger_compilation();
                            } else {
                                self.recompute_native_layout();
                            }
                        }

                        if (self.editor_panel.font_size - self.config.font_size).abs() > 0.01 {
                            self.config.font_size = self.editor_panel.font_size;
                            self.font_size_save_deadline =
                                Some(std::time::Instant::now() + Duration::from_millis(500));
                        }
                        if let Some(deadline) = self.font_size_save_deadline {
                            if std::time::Instant::now() >= deadline {
                                let _ = self.config.save();
                                self.font_size_save_deadline = None;
                            } else {
                                ctx.request_repaint_after(Duration::from_millis(100));
                            }
                        }
                    },
                );

                ui.separator();

                // Right pane: Live Document Viewer
                let has_errors = self
                    .diagnostics
                    .iter()
                    .any(|d| d.severity == crate::compiler::DiagnosticSeverity::Error);

                let invert_colors = self.canvas.invert_colors;
                ui.allocate_ui_with_layout(
                    vec2(ui.available_width(), ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        crate::log_startup("update: ViewerPanel::show starting");
                        let action = ViewerPanel::show(
                            ui,
                            &mut self.canvas,
                            &self.renderer,
                            &self.typeset_pages,
                            self.latest_pdf_bytes.as_deref(),
                            &mut self.view_mode,
                            invert_colors,
                            has_errors,
                            self.document.line_count(),
                        );
                        crate::log_startup("update: ViewerPanel::show done");
                        if let ViewerAction::JumpToLine(line) = action {
                            self.editor_panel.jump_to_line(line);
                        }
                    },
                );
            });
        });

        // 6. External Dependency Installation Consent Modal
        self.render_dependency_prompt(ctx);

        // 7. Overleaf-style Working Directory & Project Files Modal
        let modal_action = self
            .project_manager
            .show_modal(ctx, self.document.path.as_deref());
        self.handle_project_action(ctx, modal_action);

        // 8. Application Settings & Preferences Modal
        let settings_action = self.settings_modal.show(ctx, &self.available_themes);
        if let SettingsAction::Apply(new_config) = settings_action {
            let prev_engine = self.engine;
            self.config = new_config;
            self.engine = self.config.default_engine;
            self.view_mode = self.config.view_mode;
            self.editor_panel.font_size = self.config.font_size;
            crate::i18n::set_locale(&self.config.language);
            if let Some(t) = self
                .available_themes
                .iter()
                .find(|t| t.name == self.config.theme_name)
                .cloned()
            {
                self.current_theme = t;
            }
            self.canvas.set_color_mode(self.config.pdf_color_mode);
            self.canvas.set_base_render_width(self.config.pdf_render_width);
            self.canvas.fit_to_width = self.config.fit_to_width;
            self.auto_compile = self.config.auto_compile;
            self.status_message = "Preferences updated & saved".to_string();
            self.drawer.log_user_event(
                "Settings Updated",
                format!(
                    "Updated preferences (theme='{}', engine={}, auto_compile={})",
                    self.config.theme_name,
                    self.engine.display_name(),
                    self.auto_compile
                ),
                None,
            );
            if self.engine != prev_engine {
                self.canvas.invalidate();
                self.trigger_compilation();
            }
        }

        // 9. Unsaved Changes Confirmation Modal
        self.render_unsaved_changes_modal(ctx);

        // Keep stateful REST API snapshot synchronized with the live GUI editor
        self.sync_shared_api_state();

        // Request rapid repaints while tasks are compiling or installing
        if self.compiler.is_compiling() || self.dependency_prompt.is_installing {
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        static FIRST_FRAME_END: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
        if FIRST_FRAME_END.swap(false, std::sync::atomic::Ordering::Relaxed) {
            crate::log_startup("First frame update() completed!");
        }
    }
}
