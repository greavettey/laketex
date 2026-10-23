//! # Settings Modal & Configuration Dialog
//!
//! Provides a centralized graphical settings menu for Laketex:
//! - Visual theme selection (moved from toolbar into settings)
//! - Default PDF color modes (Normal Daylight, Academic Dark, Sepia Eye-Care)
//! - Default PDF rasterization resolution (presets & custom width slider)
//! - User configurable keyboard shortcut rebinds with interactive key-capture
//! - Compiler defaults, auto-compilation toggle, and editor font size
//! - Persistent disk synchronization to `laketex_config.json` alongside the executable

use crate::compiler::engine::EngineKind;
use crate::ui::config::{format_shortcut, AppConfig};
use crate::ui::theme::ThemeConfig;
use crate::ui::viewer_panel::ViewMode;
use crate::viewer::PdfColorMode;
use egui::{vec2, Color32, Key, RichText, Ui};

/// Active category tab within the settings modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    /// Interface language, color themes, and font sizing.
    Appearance,
    /// PDF color inversion modes and rendering raster resolution.
    PdfViewer,
    /// Interactive key rebinding for application shortcuts.
    Shortcuts,
    /// Default engine selection, embedded packaging status, and auto-compilation.
    Compiler,
}

/// Identifies an individual rebindable keyboard shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutTarget {
    /// Compile document action.
    Compile,
    /// Save current file action.
    Save,
    /// Open file dialog action.
    Open,
    /// Create new document action.
    NewDoc,
    /// Toggle project manager files drawer action.
    ProjectFiles,
    /// Find search drawer action.
    Find,
    /// Find and replace drawer action.
    Replace,
    /// Open settings preferences modal action.
    Settings,
}

/// Action emitted when closing or applying settings in the dialog.
pub enum SettingsAction {
    /// No changes applied; modal remains open or cancelled.
    None,
    /// New configuration submitted and persisted to disk.
    Apply(AppConfig),
}

/// The settings dialog state manager.
pub struct SettingsModal {
    /// Whether the settings preferences dialog window is currently visible.
    pub is_open: bool,
    /// Currently active navigation tab in the preferences modal.
    pub active_tab: SettingsTab,
    /// Staged configuration changes being edited within the dialog.
    pub draft_config: AppConfig,
    /// Target shortcut currently awaiting a keypress sequence from the user.
    pub capturing_target: Option<ShortcutTarget>,
    /// Temporary feedback or confirmation message displayed in the dialog footer.
    pub status_message: Option<String>,
}

impl Default for SettingsModal {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsModal {
    /// Constructs a new, closed [`SettingsModal`] initialized with default configuration values.
    pub fn new() -> Self {
        Self {
            is_open: false,
            active_tab: SettingsTab::Appearance,
            draft_config: AppConfig::default(),
            capturing_target: None,
            status_message: None,
        }
    }

    /// Opens the settings modal and initializes the working draft with the active configuration.
    pub fn open(&mut self, current_config: &AppConfig) {
        self.draft_config = current_config.clone();
        self.capturing_target = None;
        self.status_message = None;
        self.is_open = true;
    }

    /// Toggles the settings modal open or closed.
    pub fn toggle(&mut self, current_config: &AppConfig) {
        if self.is_open {
            self.is_open = false;
            self.capturing_target = None;
        } else {
            self.open(current_config);
        }
    }

    /// Returns true if the modal is currently awaiting a keypress for shortcut rebinding.
    pub fn is_capturing(&self) -> bool {
        self.is_open && self.capturing_target.is_some()
    }

    /// Renders the settings dialog window.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        available_themes: &[ThemeConfig],
    ) -> SettingsAction {
        if !self.is_open {
            return SettingsAction::None;
        }

        // Handle interactive key-capture for shortcut rebinding
        self.handle_key_capture(ctx);

        let mut action = SettingsAction::None;
        let mut request_close = false;
        let mut request_save = false;

        egui::Window::new("⚙ Laketex Settings & Preferences")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .fixed_size(vec2(680.0, 480.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⚙").size(24.0));
                    ui.vertical(|ui| {
                        ui.heading("Application Preferences");
                        ui.label(
                            RichText::new("Configurations are saved to laketex_config.json alongside laketex.exe")
                                .small()
                                .color(Color32::from_gray(160)),
                        );
                    });
                });

                ui.separator();

                // Navigation Tabs
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.active_tab,
                        SettingsTab::Appearance,
                        "🎨 Appearance & Theme",
                    );
                    ui.selectable_value(
                        &mut self.active_tab,
                        SettingsTab::PdfViewer,
                        "📄 PDF & Viewer",
                    );
                    ui.selectable_value(
                        &mut self.active_tab,
                        SettingsTab::Shortcuts,
                        "⌨ Keyboard Shortcuts",
                    );
                    ui.selectable_value(
                        &mut self.active_tab,
                        SettingsTab::Compiler,
                        "⚡ Compiler & Workflow",
                    );
                });

                ui.separator();

                // Tab Content Area
                egui::ScrollArea::vertical()
                    .max_height(330.0)
                    .show(ui, |ui| {
                        match self.active_tab {
                            SettingsTab::Appearance => {
                                self.show_appearance_tab(ui, available_themes);
                            }
                            SettingsTab::PdfViewer => {
                                self.show_pdf_viewer_tab(ui);
                            }
                            SettingsTab::Shortcuts => {
                                self.show_shortcuts_tab(ui);
                            }
                            SettingsTab::Compiler => {
                                self.show_compiler_tab(ui);
                            }
                        }
                    });

                ui.separator();

                // Status Message Feedback
                if let Some(msg) = &self.status_message {
                    ui.colored_label(Color32::from_rgb(152, 195, 121), msg);
                }

                // Modal Footer Actions
                ui.horizontal(|ui| {
                    if ui.button("↺ Reset All to Defaults").clicked() {
                        self.draft_config = AppConfig::default();
                        self.capturing_target = None;
                        self.status_message = Some("Reset all settings to default values".to_string());
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(
                                RichText::new("✓ Save & Apply")
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .clicked()
                        {
                            request_save = true;
                        }

                        if ui.button("Cancel").clicked() {
                            request_close = true;
                        }
                    });
                });
            });

        if request_save {
            match self.draft_config.save() {
                Ok(saved_path) => {
                    self.status_message = Some(format!("Saved to {}", saved_path.display()));
                    action = SettingsAction::Apply(self.draft_config.clone());
                    self.is_open = false;
                }
                Err(e) => {
                    self.status_message = Some(format!("Failed to save config: {e}"));
                }
            }
        } else if request_close {
            self.is_open = false;
            self.capturing_target = None;
        }

        action
    }

    fn handle_key_capture(&mut self, ctx: &egui::Context) {
        let Some(target) = self.capturing_target else {
            return;
        };

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.capturing_target = None;
            return;
        }

        // List of bindable keys
        let bindable_keys = [
            Key::A, Key::B, Key::C, Key::D, Key::E, Key::F, Key::G, Key::H, Key::I,
            Key::J, Key::K, Key::L, Key::M, Key::N, Key::O, Key::P, Key::Q, Key::R,
            Key::S, Key::T, Key::U, Key::V, Key::W, Key::X, Key::Y, Key::Z,
            Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6,
            Key::Num7, Key::Num8, Key::Num9,
            Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6, Key::F7, Key::F8,
            Key::F9, Key::F10, Key::F11, Key::F12,
            Key::Comma, Key::Period, Key::Semicolon, Key::Colon,
            Key::Enter, Key::Space, Key::Tab, Key::Backspace, Key::Delete,
        ];

        ctx.input(|i| {
            for key in bindable_keys {
                if i.key_pressed(key) {
                    let shortcut = format_shortcut(i.modifiers, key);
                    match target {
                        ShortcutTarget::Compile => self.draft_config.shortcuts.compile = shortcut,
                        ShortcutTarget::Save => self.draft_config.shortcuts.save = shortcut,
                        ShortcutTarget::Open => self.draft_config.shortcuts.open = shortcut,
                        ShortcutTarget::NewDoc => self.draft_config.shortcuts.new_doc = shortcut,
                        ShortcutTarget::ProjectFiles => self.draft_config.shortcuts.project_files = shortcut,
                        ShortcutTarget::Find => self.draft_config.shortcuts.find = shortcut,
                        ShortcutTarget::Replace => self.draft_config.shortcuts.replace = shortcut,
                        ShortcutTarget::Settings => self.draft_config.shortcuts.settings = shortcut,
                    }
                    self.capturing_target = None;
                    break;
                }
            }
        });
    }

    fn show_appearance_tab(&mut self, ui: &mut Ui, available_themes: &[ThemeConfig]) {
        ui.label(RichText::new("Interface Language").strong());
        ui.label(
            RichText::new("Select application display language. Translations are loaded from locales/*.json.")
                .small()
                .color(Color32::from_gray(160)),
        );
        ui.add_space(4.0);

        let available_langs = crate::i18n::available_locales();
        let current_display = available_langs
            .iter()
            .find(|(code, _)| code == &self.draft_config.language)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| self.draft_config.language.clone());

        egui::ComboBox::from_id_salt("settings_language_select")
            .selected_text(current_display)
            .width(260.0)
            .show_ui(ui, |ui| {
                for (code, name) in available_langs {
                    let is_selected = self.draft_config.language == code;
                    if ui.selectable_label(is_selected, &name).clicked() {
                        self.draft_config.language = code.clone();
                        crate::i18n::set_locale(&code);
                    }
                }
            });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label(RichText::new("Theme & Visual Style").strong());
        ui.label(
            RichText::new("Select a default color theme. Custom themes placed in the themes/ directory appear here.")
                .small()
                .color(Color32::from_gray(160)),
        );
        ui.add_space(4.0);

        egui::ComboBox::from_id_salt("settings_theme_select")
            .selected_text(&self.draft_config.theme_name)
            .width(260.0)
            .show_ui(ui, |ui| {
                for theme in available_themes {
                    let is_selected = self.draft_config.theme_name == theme.name;
                    if ui.selectable_label(is_selected, theme.name()).clicked() {
                        self.draft_config.theme_name = theme.name.clone();
                    }
                }
            });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label(RichText::new("Editor Typography").strong());
        ui.horizontal(|ui| {
            ui.label("Editor Font Size:");
            ui.add(
                egui::Slider::new(&mut self.draft_config.font_size, 10.0..=24.0)
                    .suffix(" pt")
                    .step_by(0.5),
            );
        });
    }

    fn show_pdf_viewer_tab(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Default PDF Render Color Mode").strong());
        ui.label(
            RichText::new("Configure the color grading applied to compiled PDF pages.")
                .small()
                .color(Color32::from_gray(160)),
        );
        ui.add_space(4.0);

        for mode in PdfColorMode::all() {
            let selected = self.draft_config.pdf_color_mode == mode;
            let text = match mode {
                PdfColorMode::Normal => "☀ Daylight — High contrast white paper with black text",
                PdfColorMode::AcademicDark => "🌙 Academic Dark — Inverted RGB with low blue light fatigue",
                PdfColorMode::Sepia => "📜 Sepia Eye-Care — Soothing warm parchment with dark sepia ink",
            };
            if ui.selectable_label(selected, text).clicked() {
                self.draft_config.pdf_color_mode = mode;
            }
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label(RichText::new("Default PDF Rasterization Resolution").strong());
        ui.label(
            RichText::new("Controls target pixel width for PDFium rendering. Higher values give razor-sharp formulas.")
                .small()
                .color(Color32::from_gray(160)),
        );
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui.button("Compact (800px)").clicked() {
                self.draft_config.pdf_render_width = 800;
            }
            if ui.button("Standard (1200px)").clicked() {
                self.draft_config.pdf_render_width = 1200;
            }
            if ui.button("Retina (1600px)").clicked() {
                self.draft_config.pdf_render_width = 1600;
            }
            if ui.button("Ultra HD (2000px)").clicked() {
                self.draft_config.pdf_render_width = 2000;
            }
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Resolution Base Width:");
            ui.add(
                egui::Slider::new(&mut self.draft_config.pdf_render_width, 600..=3000)
                    .suffix(" px")
                    .step_by(50.0),
            );
        });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label(RichText::new("Default Preview Mode & Sizing").strong());
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.draft_config.view_mode,
                ViewMode::NativeVector,
                "Pure-Rust Native Live View",
            );
            ui.selectable_value(
                &mut self.draft_config.view_mode,
                ViewMode::PdfRaster,
                "PDF Vector View",
            );
        });

        ui.checkbox(
            &mut self.draft_config.fit_to_width,
            "Automatically fit document width to preview pane",
        );
    }

    fn show_shortcuts_tab(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Configurable Keyboard Shortcuts").strong());
        ui.label(
            RichText::new("Click any shortcut button and press your desired key combination to rebind it.")
                .small()
                .color(Color32::from_gray(160)),
        );
        ui.add_space(6.0);

        let shortcuts_table = [
            (
                ShortcutTarget::Compile,
                "Recompile Document",
                "Triggers background or native typesetting compilation",
                &self.draft_config.shortcuts.compile.clone(),
                "Ctrl+B",
            ),
            (
                ShortcutTarget::Save,
                "Save Document",
                "Saves current editor content to disk",
                &self.draft_config.shortcuts.save.clone(),
                "Ctrl+S",
            ),
            (
                ShortcutTarget::Open,
                "Open File",
                "Prompts file picker dialog to open .tex document",
                &self.draft_config.shortcuts.open.clone(),
                "Ctrl+O",
            ),
            (
                ShortcutTarget::NewDoc,
                "New Document",
                "Creates a blank default LaTeX template",
                &self.draft_config.shortcuts.new_doc.clone(),
                "Ctrl+N",
            ),
            (
                ShortcutTarget::ProjectFiles,
                "Project Files Manager",
                "Opens Overleaf-style working directory manager",
                &self.draft_config.shortcuts.project_files.clone(),
                "Ctrl+P",
            ),
            (
                ShortcutTarget::Find,
                "Find in Editor",
                "Opens search bar with match counts and navigation",
                &self.draft_config.shortcuts.find.clone(),
                "Ctrl+F",
            ),
            (
                ShortcutTarget::Replace,
                "Replace in Editor",
                "Opens search and replace panel",
                &self.draft_config.shortcuts.replace.clone(),
                "Ctrl+H",
            ),
            (
                ShortcutTarget::Settings,
                "Open Settings",
                "Toggles this preferences modal",
                &self.draft_config.shortcuts.settings.clone(),
                "Ctrl+,",
            ),
        ];

        egui::Grid::new("shortcuts_grid")
            .num_columns(4)
            .spacing([12.0, 8.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(RichText::new("Action").strong());
                ui.label(RichText::new("Description").strong());
                ui.label(RichText::new("Shortcut").strong());
                ui.label(RichText::new("Reset").strong());
                ui.end_row();

                for (target, name, desc, current_val, default_val) in shortcuts_table {
                    ui.label(RichText::new(name).strong());
                    ui.label(RichText::new(desc).small().color(Color32::from_gray(160)));

                    let is_capturing = self.capturing_target == Some(target);
                    let btn_text = if is_capturing {
                        RichText::new("⌛ Press keys...").color(Color32::from_rgb(242, 201, 76)).strong()
                    } else {
                        RichText::new(current_val).color(Color32::from_rgb(97, 175, 239)).strong()
                    };

                    if ui.button(btn_text).clicked() {
                        if is_capturing {
                            self.capturing_target = None;
                        } else {
                            self.capturing_target = Some(target);
                        }
                    }

                    if ui.button("Default").clicked() {
                        let def_str = default_val.to_string();
                        match target {
                            ShortcutTarget::Compile => self.draft_config.shortcuts.compile = def_str,
                            ShortcutTarget::Save => self.draft_config.shortcuts.save = def_str,
                            ShortcutTarget::Open => self.draft_config.shortcuts.open = def_str,
                            ShortcutTarget::NewDoc => self.draft_config.shortcuts.new_doc = def_str,
                            ShortcutTarget::ProjectFiles => self.draft_config.shortcuts.project_files = def_str,
                            ShortcutTarget::Find => self.draft_config.shortcuts.find = def_str,
                            ShortcutTarget::Replace => self.draft_config.shortcuts.replace = def_str,
                            ShortcutTarget::Settings => self.draft_config.shortcuts.settings = def_str,
                        }
                    }

                    ui.end_row();
                }
            });
    }

    fn show_compiler_tab(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Default Typesetting Engine").strong());
        ui.label(
            RichText::new("Choose which LaTeX engine Laketex initializes with on launch.")
                .small()
                .color(Color32::from_gray(160)),
        );
        ui.add_space(4.0);

        egui::ComboBox::from_id_salt("settings_engine_select")
            .selected_text(self.draft_config.default_engine.display_name())
            .width(260.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.draft_config.default_engine,
                    EngineKind::Native,
                    EngineKind::Native.display_name(),
                );
                ui.selectable_value(
                    &mut self.draft_config.default_engine,
                    EngineKind::Tectonic,
                    EngineKind::Tectonic.display_name(),
                );
                ui.selectable_value(
                    &mut self.draft_config.default_engine,
                    EngineKind::PdfLatex,
                    EngineKind::PdfLatex.display_name(),
                );
                ui.selectable_value(
                    &mut self.draft_config.default_engine,
                    EngineKind::XeLatex,
                    EngineKind::XeLatex.display_name(),
                );
                ui.selectable_value(
                    &mut self.draft_config.default_engine,
                    EngineKind::LuaLatex,
                    EngineKind::LuaLatex.display_name(),
                );
            });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label(RichText::new("Engine Status & Packaging").strong());
        let is_ready = crate::compiler::provisioner::Provisioner::is_tectonic_ready();
        ui.horizontal(|ui| {
            ui.label("Tectonic XeTeX:");
            if is_ready {
                ui.label(RichText::new("✔ Ready (Embedded)").color(Color32::from_rgb(152, 195, 121)).strong());
            } else {
                ui.label(RichText::new("📦 Embedded (Unpacks automatically on first compile)").color(Color32::from_rgb(97, 175, 239)));
            }
        });
        ui.label(
            RichText::new("Tectonic is compressed and embedded inside laketex.exe. Zero external downloads are required to run the engine.")
                .small()
                .color(Color32::from_gray(160)),
        );

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label(RichText::new("Compilation Automation").strong());
        ui.checkbox(
            &mut self.draft_config.auto_compile,
            "Enable live compilation on keystroke edit by default",
        );
    }
}
