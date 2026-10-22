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

pub mod drawer;
pub mod editor_panel;
pub mod sidebar;
pub mod theme;
pub mod toolbar;
pub mod viewer_panel;

use crate::compiler::engine::EngineKind;
use crate::compiler::{CompilerWorker, Diagnostic};
use crate::document::packages::PackageManager;
use crate::document::Document;
use crate::engine::layout::{DocumentLayout, TypesetPage};
use crate::engine::parser::NativeLatexParser;
use crate::ui::drawer::{Drawer, DrawerAction};
use crate::ui::editor_panel::EditorPanel;
use crate::ui::sidebar::{Sidebar, SidebarAction};
use crate::ui::theme::ThemeConfig;
use crate::ui::toolbar::Toolbar;
use crate::ui::viewer_panel::{ViewMode, ViewerAction, ViewerPanel};
use crate::viewer::canvas::PdfCanvas;
use crate::viewer::PdfRenderer;
use eframe::egui::{self, vec2, Color32, Key, Modifiers, TopBottomPanel};
use std::path::PathBuf;
use std::time::Duration;

pub struct LakeTexApp {
    pub document: Document,
    pub renderer: PdfRenderer,
    pub canvas: PdfCanvas,
    pub compiler: CompilerWorker,
    pub engine: EngineKind,
    pub view_mode: ViewMode,
    pub auto_compile: bool,
    pub current_theme: ThemeConfig,
    pub available_themes: Vec<ThemeConfig>,

    pub editor_panel: EditorPanel,
    pub sidebar: Sidebar,
    pub drawer: Drawer,

    pub show_sidebar: bool,
    pub show_drawer: bool,

    pub typeset_pages: Vec<TypesetPage>,
    pub latest_pdf_bytes: Option<Vec<u8>>,
    pub diagnostics: Vec<Diagnostic>,
    pub raw_stdout: String,
    pub raw_stderr: String,
    pub last_duration_ms: Option<u128>,

    pub api_port: u16,
    pub status_message: String,
}

impl LakeTexApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        initial_file: Option<PathBuf>,
        api_port: u16,
    ) -> Self {
        // Configure math and symbol fallback fonts
        ThemeConfig::configure_fonts(&cc.egui_ctx);

        let document = if let Some(path) = initial_file {
            Document::load_from_file(&path).unwrap_or_default()
        } else {
            Document::default()
        };

        let renderer = PdfRenderer::new();
        let canvas = PdfCanvas::new();
        let compiler = CompilerWorker::new();
        let engine = EngineKind::Native; // Default to Native Pure-Rust Engine
        let view_mode = ViewMode::NativeVector;

        // Compute initial native layout
        let ast = NativeLatexParser::parse(&document.content);
        let typeset_pages = DocumentLayout::layout_document(&ast);
        let latest_pdf_bytes = crate::engine::pdf_exporter::NativePdfExporter::export_to_pdf_bytes(&ast).ok();

        // Submit initial compilation
        compiler.submit(document.content.clone(), engine);

        let available_themes = ThemeConfig::discover_themes();
        let current_theme = available_themes
            .iter()
            .find(|t| t.name == "Dracula (JetBrains)")
            .cloned()
            .unwrap_or_else(ThemeConfig::dracula_jetbrains);

        Self {
            document,
            renderer,
            canvas,
            compiler,
            engine,
            view_mode,
            auto_compile: true,
            current_theme,
            available_themes,
            editor_panel: EditorPanel::new(),
            sidebar: Sidebar::new(),
            drawer: Drawer::new(),
            show_sidebar: true,
            show_drawer: false,
            typeset_pages,
            latest_pdf_bytes,
            diagnostics: Vec::new(),
            raw_stdout: String::new(),
            raw_stderr: String::new(),
            last_duration_ms: None,
            api_port,
            status_message: "Ready".to_string(),
        }
    }

    pub fn recompute_native_layout(&mut self) {
        let ast = NativeLatexParser::parse(&self.document.content);
        self.typeset_pages = DocumentLayout::layout_document(&ast);
    }

    pub fn trigger_compilation(&mut self) {
        self.recompute_native_layout();

        if self.engine == EngineKind::Native {
            // Instant in-process compilation (< 5 ms)
            let start = std::time::Instant::now();
            let ast = NativeLatexParser::parse(&self.document.content);
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
            self.compiler
                .submit(self.document.content.clone(), self.engine);
            self.status_message = "Compiling...".to_string();
        }
    }

    pub fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::B)) {
            self.trigger_compilation();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::S)) {
            self.save_document();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::O)) {
            self.open_file_dialog();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::N)) {
            self.new_document();
        }
    }

    pub fn save_document(&mut self) {
        if let Some(path) = self.document.path.clone() {
            if let Err(e) = self.document.save_to_file(&path) {
                self.status_message = format!("Save error: {e}");
            } else {
                self.status_message = format!("Saved {}", path.display());
            }
        } else {
            self.save_document_as();
        }
    }

    pub fn save_document_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("LaTeX Document", &["tex"])
            .save_file()
        {
            if let Err(e) = self.document.save_to_file(&path) {
                self.status_message = format!("Save error: {e}");
            } else {
                self.status_message = format!("Saved as {}", path.display());
            }
        }
    }

    pub fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("LaTeX Document", &["tex"])
            .pick_file()
        {
            match Document::load_from_file(&path) {
                Ok(doc) => {
                    self.document = doc;
                    self.canvas.reset_cache();
                    let ast = NativeLatexParser::parse(&self.document.content);
                    self.latest_pdf_bytes =
                        crate::engine::pdf_exporter::NativePdfExporter::export_to_pdf_bytes(&ast)
                            .ok();
                    self.status_message = format!("Loaded {}", path.display());
                    self.trigger_compilation();
                }
                Err(e) => {
                    self.status_message = format!("Open error: {e}");
                }
            }
        }
    }

    pub fn new_document(&mut self) {
        self.document = Document::default();
        self.canvas.reset_cache();
        let ast = NativeLatexParser::parse(&self.document.content);
        self.latest_pdf_bytes =
            crate::engine::pdf_exporter::NativePdfExporter::export_to_pdf_bytes(&ast).ok();
        self.status_message = "New document created".to_string();
        self.trigger_compilation();
    }

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
}

impl eframe::App for LakeTexApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply active theme
        self.current_theme.apply(ctx);

        // Check for completed background compilation results
        if let Some(output) = self.compiler.try_receive() {
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

        // Auto-generate Native PDF fallback if switching to PDF Vector View without cached PDF bytes
        if self.view_mode == ViewMode::PdfRaster && self.latest_pdf_bytes.is_none() {
            let ast = NativeLatexParser::parse(&self.document.content);
            if let Ok(bytes) =
                crate::engine::pdf_exporter::NativePdfExporter::export_to_pdf_bytes(&ast)
            {
                self.latest_pdf_bytes = Some(bytes);
            }
        }

        // Handle global keyboard shortcuts
        self.handle_shortcuts(ctx);

        // 1. Top Toolbar
        TopBottomPanel::top("top_toolbar").show(ctx, |ui| {
            let error_count = self
                .diagnostics
                .iter()
                .filter(|d| d.severity == crate::compiler::DiagnosticSeverity::Error)
                .count();

            let action = Toolbar::show(
                ui,
                self.compiler.is_compiling(),
                self.last_duration_ms,
                error_count,
                &mut self.engine,
                &mut self.auto_compile,
                &mut self.show_sidebar,
                &mut self.show_drawer,
                &mut self.canvas.invert_colors,
                &mut self.canvas.fit_to_width,
                &mut self.canvas.zoom,
                &mut self.current_theme,
                &self.available_themes,
            );

            if action.compile_requested {
                self.trigger_compilation();
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
        });

        // 2. Bottom Status Bar
        TopBottomPanel::bottom("bottom_status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 0.0);

                let dirty_marker = if self.document.is_dirty { "● " } else { "" };
                ui.label(format!("{}{}", dirty_marker, self.document.file_name()));

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
                    ui.colored_label(
                        Color32::from_rgb(97, 175, 239),
                        format!("REST API: :{}", self.api_port),
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
                        DrawerAction::None => {}
                    }
                });
        }

        // 4. Collapsible Left Sidebar (Outline, Packages, Snippets)
        if self.show_sidebar {
            egui::SidePanel::left("left_sidebar")
                .resizable(true)
                .default_width(240.0)
                .width_range(160.0..=450.0)
                .show(ctx, |ui| {
                    let action = self.sidebar.show(
                        ui,
                        &self.document.outline_cache,
                        &self.document.package_cache,
                        self.document.word_count(),
                        self.document.char_count(),
                        self.document.line_count(),
                        self.last_duration_ms,
                    );

                    match action {
                        SidebarAction::JumpToLine(line) => {
                            self.editor_panel.jump_to_line(line);
                        }
                        SidebarAction::InsertPackage(pkg) => {
                            self.document.content =
                                PackageManager::insert_package(&self.document.content, &pkg);
                            self.document.refresh_caches();
                            self.recompute_native_layout();
                            if self.auto_compile {
                                self.trigger_compilation();
                            }
                        }
                        SidebarAction::InsertSnippet(snippet) => {
                            self.document.content.push_str(&snippet);
                            self.document.refresh_caches();
                            self.recompute_native_layout();
                            if self.auto_compile {
                                self.trigger_compilation();
                            }
                        }
                        SidebarAction::None => {}
                    }
                });
        }

        // 5. Central Split Area: Code Editor (Left) & Live Document Viewer (Right)
        egui::CentralPanel::default().show(ctx, |ui| {
            let available_width = ui.available_width();
            let editor_width = (available_width * 0.48).max(300.0);

            ui.horizontal_top(|ui| {
                // Left pane: Code Editor
                ui.allocate_ui_with_layout(
                    vec2(editor_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        let text_changed = self.editor_panel.show(
                            ui,
                            &mut self.document.content,
                            &self.diagnostics,
                            self.current_theme.dark_mode,
                        );

                        if text_changed {
                            self.document.is_dirty = true;
                            self.document.refresh_caches();
                            self.recompute_native_layout();
                            if self.auto_compile {
                                self.trigger_compilation();
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
                        if let ViewerAction::JumpToLine(line) = action {
                            self.editor_panel.jump_to_line(line);
                        }
                    },
                );
            });
        });

        // Request rapid repaints while tasks are compiling or active
        if self.compiler.is_compiling() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}
