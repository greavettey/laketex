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
use crate::t;
use crate::ui::config::{format_shortcut, AppConfig};
use crate::ui::theme::ThemeConfig;
use crate::ui::viewer_panel::ViewMode;
use crate::viewer::PdfColorMode;
use egui::{vec2, Color32, CornerRadius, Frame, Key, Margin, RichText, Ui};

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
    /// Language code that was active before opening the modal, restored if cancelled.
    pub original_language: String,
    /// Cached list of available `(code, display_name)` locales.
    pub available_locales: Vec<(String, String)>,
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
        let draft = AppConfig::default();
        let original_language = draft.language.clone();
        Self {
            is_open: false,
            active_tab: SettingsTab::Appearance,
            draft_config: draft,
            original_language,
            available_locales: crate::i18n::available_locales(),
            capturing_target: None,
            status_message: None,
        }
    }

    /// Opens the settings modal and initializes the working draft with the active configuration.
    pub fn open(&mut self, current_config: &AppConfig) {
        self.draft_config = current_config.clone();
        self.original_language = current_config.language.clone();
        self.available_locales = crate::i18n::available_locales();
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
        let mut is_open = self.is_open;

        egui::Window::new("⚙ Settings")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_size(vec2(760.0, 520.0))
            .min_size(vec2(660.0, 440.0))
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .frame(
                Frame::window(&ctx.style())
                    .inner_margin(Margin::same(14))
                    .corner_radius(CornerRadius::same(6)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 6.0);

                let body_height = (ui.available_height() - 44.0).max(340.0);

                ui.horizontal(|ui| {
                    // Left navigation sidebar
                    ui.vertical(|ui| {
                        ui.set_width(160.0);
                        ui.set_min_width(160.0);
                        ui.spacing_mut().item_spacing = vec2(0.0, 4.0);

                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            crate::ui::logo::render_brand_label(ui, 14.0, ui.visuals().strong_text_color());
                        });
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("PREFERENCES")
                                .size(11.0)
                                .strong()
                                .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(4.0);

                        if nav_item(ui, "🎨", "Appearance", self.active_tab == SettingsTab::Appearance) {
                            self.active_tab = SettingsTab::Appearance;
                        }
                        if nav_item(ui, "📄", "PDF & Viewer", self.active_tab == SettingsTab::PdfViewer) {
                            self.active_tab = SettingsTab::PdfViewer;
                        }
                        if nav_item(ui, "⌨", "Shortcuts", self.active_tab == SettingsTab::Shortcuts) {
                            self.active_tab = SettingsTab::Shortcuts;
                        }
                        if nav_item(ui, "⚡", "Compiler", self.active_tab == SettingsTab::Compiler) {
                            self.active_tab = SettingsTab::Compiler;
                        }
                    });

                    ui.separator();

                    // Right content pane
                    let right_width = ui.available_width().max(460.0);
                    ui.vertical(|ui| {
                        ui.set_width(right_width);
                        ui.set_min_width(right_width);

                        egui::ScrollArea::vertical()
                            .id_salt("settings_tab_content_scroll")
                            .max_height(body_height)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.set_width(right_width);
                                ui.add_space(2.0);
                                let (title, desc) = match self.active_tab {
                                    SettingsTab::Appearance => (
                                        "Appearance & Theme",
                                        "Configure display language, visual themes, and editor typography.",
                                    ),
                                    SettingsTab::PdfViewer => (
                                        "PDF & Viewer",
                                        "Set rendering resolution, color inversion grading, and preview modes.",
                                    ),
                                    SettingsTab::Shortcuts => (
                                        "Keyboard Shortcuts",
                                        "Customize hotkeys for rapid document compilation and editing.",
                                    ),
                                    SettingsTab::Compiler => (
                                        "Compiler & Workflow",
                                        "Manage default typesetting engine and live compilation automation.",
                                    ),
                                };

                                ui.heading(title);
                                ui.label(
                                    RichText::new(desc)
                                        .small()
                                        .color(ui.visuals().weak_text_color()),
                                );
                                ui.add_space(6.0);
                                ui.separator();
                                ui.add_space(8.0);

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
                    });
                });

                ui.separator();
                ui.add_space(4.0);

                // Modal Footer
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 0.0);

                    if ui
                        .button("↺ Reset All")
                        .on_hover_text("Reset all settings to default values")
                        .clicked()
                    {
                        self.draft_config = AppConfig::default();
                        crate::i18n::set_locale(&self.draft_config.language);
                        self.capturing_target = None;
                        self.status_message =
                            Some("Reset all settings to default values".to_string());
                    }

                    if let Some(msg) = &self.status_message {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(msg)
                                .small()
                                .color(Color32::from_rgb(152, 195, 121)),
                        );
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let save_btn = egui::Button::new(
                            RichText::new("✓ Save & Apply").strong(),
                        )
                        .min_size(vec2(110.0, 26.0))
                        .fill(ui.visuals().selection.bg_fill);

                        if ui.add(save_btn).clicked() {
                            request_save = true;
                        }

                        if ui
                            .add(egui::Button::new("Cancel").min_size(vec2(75.0, 26.0)))
                            .clicked()
                        {
                            request_close = true;
                        }
                    });
                });
            });

        if !is_open {
            request_close = true;
        }

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
            crate::i18n::set_locale(&self.original_language);
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
        setting_card(
            ui,
            "Interface Language",
            "Application display language (translations from locales/*.json)",
            |ui| {
                let current_display = self
                    .available_locales
                    .iter()
                    .find(|(code, _)| code == &self.draft_config.language)
                    .map(|(_, name)| name.clone())
                    .unwrap_or_else(|| self.draft_config.language.clone());

                egui::ComboBox::from_id_salt("settings_language_select")
                    .selected_text(current_display)
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for (code, name) in &self.available_locales {
                            let is_selected = &self.draft_config.language == code;
                            if ui.selectable_label(is_selected, name).clicked() {
                                self.draft_config.language = code.clone();
                                crate::i18n::set_locale(code);
                            }
                        }
                    });
            },
        );

        ui.add_space(6.0);

        setting_card(
            ui,
            "Color Theme",
            "UI and syntax highlighting theme (custom themes from themes/)",
            |ui| {
                egui::ComboBox::from_id_salt("settings_theme_select")
                    .selected_text(&self.draft_config.theme_name)
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for theme in available_themes {
                            let is_selected = self.draft_config.theme_name == theme.name;
                            if ui.selectable_label(is_selected, theme.name()).clicked() {
                                self.draft_config.theme_name = theme.name.clone();
                            }
                        }
                    });
            },
        );

        ui.add_space(6.0);

        setting_card(
            ui,
            "Editor Font Size",
            "Base typography size for LaTeX source code editor",
            |ui| {
                ui.add(
                    egui::Slider::new(&mut self.draft_config.font_size, 10.0..=24.0)
                        .suffix(" pt")
                        .step_by(0.5),
                );
            },
        );
    }

    fn show_pdf_viewer_tab(&mut self, ui: &mut Ui) {
        setting_card_vertical(
            ui,
            "Default PDF Color Mode",
            "Color grading applied to compiled PDF pages",
            |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                    for mode in PdfColorMode::all() {
                        let selected = self.draft_config.pdf_color_mode == mode;
                        let icon = match mode {
                            PdfColorMode::Normal => "☀",
                            PdfColorMode::AcademicDark => "🌙",
                            PdfColorMode::Sepia => "📜",
                        };
                        let label = format!("{icon} {}", mode.localized_name());
                        if segmented_button(ui, &label, selected) {
                            self.draft_config.pdf_color_mode = mode;
                        }
                    }
                });

                ui.add_space(4.0);
                let mode = self.draft_config.pdf_color_mode;
                let icon = match mode {
                    PdfColorMode::Normal => "☀",
                    PdfColorMode::AcademicDark => "🌙",
                    PdfColorMode::Sepia => "📜",
                };
                let active_desc = format!("{icon} {} — {}", mode.localized_name(), mode.localized_description());
                ui.label(
                    RichText::new(active_desc)
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            },
        );

        ui.add_space(6.0);

        setting_card_vertical(
            ui,
            "PDF Rasterization Resolution",
            "Target pixel width for PDFium rendering. Higher values produce sharper math formulas.",
            |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                    let presets = [
                        ("Compact (800px)", 800),
                        ("Standard (1200px)", 1200),
                        ("Retina (1600px)", 1600),
                        ("Ultra HD (2000px)", 2000),
                    ];
                    for (label, val) in presets {
                        let is_active = self.draft_config.pdf_render_width == val;
                        if segmented_button(ui, label, is_active) {
                            self.draft_config.pdf_render_width = val;
                        }
                    }
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Custom width:")
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.draft_config.pdf_render_width, 600..=3000)
                            .suffix(" px")
                            .step_by(50.0),
                    );
                });
            },
        );

        ui.add_space(6.0);

        setting_card(
            ui,
            "Default Preview Mode",
            "Initial viewer rendering pipeline on document open",
            |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
                    let is_pdf = self.draft_config.view_mode == ViewMode::PdfRaster;
                    let is_native = self.draft_config.view_mode == ViewMode::NativeVector;
                    if segmented_button(ui, &t!("viewer.pdf_view"), is_pdf) {
                        self.draft_config.view_mode = ViewMode::PdfRaster;
                    }
                    if segmented_button(ui, &t!("viewer.native_view"), is_native) {
                        self.draft_config.view_mode = ViewMode::NativeVector;
                    }
                });
            },
        );

        ui.add_space(6.0);

        setting_card(
            ui,
            "Auto-Fit Preview Width",
            "Automatically scale document width to fit preview pane when opened",
            |ui| {
                ui.checkbox(&mut self.draft_config.fit_to_width, "");
            },
        );
    }

    fn show_shortcuts_tab(&mut self, ui: &mut Ui) {
        let shortcuts_table = [
            (
                ShortcutTarget::Compile,
                "Recompile Document",
                "Triggers background or native typesetting compilation",
                self.draft_config.shortcuts.compile.clone(),
                "Ctrl+B",
            ),
            (
                ShortcutTarget::Save,
                "Save Document",
                "Saves current editor content to disk",
                self.draft_config.shortcuts.save.clone(),
                "Ctrl+S",
            ),
            (
                ShortcutTarget::Open,
                "Open File",
                "Prompts file picker dialog to open .tex document",
                self.draft_config.shortcuts.open.clone(),
                "Ctrl+O",
            ),
            (
                ShortcutTarget::NewDoc,
                "New Document",
                "Creates a blank default LaTeX template",
                self.draft_config.shortcuts.new_doc.clone(),
                "Ctrl+N",
            ),
            (
                ShortcutTarget::ProjectFiles,
                "Project Files",
                "Opens Overleaf-style working directory manager",
                self.draft_config.shortcuts.project_files.clone(),
                "Ctrl+P",
            ),
            (
                ShortcutTarget::Find,
                "Find in Editor",
                "Opens search bar with match counts and navigation",
                self.draft_config.shortcuts.find.clone(),
                "Ctrl+F",
            ),
            (
                ShortcutTarget::Replace,
                "Replace in Editor",
                "Opens search and replace panel",
                self.draft_config.shortcuts.replace.clone(),
                "Ctrl+H",
            ),
            (
                ShortcutTarget::Settings,
                "Open Settings",
                "Toggles this preferences modal",
                self.draft_config.shortcuts.settings.clone(),
                "Ctrl+,",
            ),
        ];

        for (target, name, desc, current_val, default_val) in shortcuts_table {
            setting_card(ui, name, desc, |ui| {
                let is_modified = current_val != default_val;
                let is_capturing = self.capturing_target == Some(target);

                if ui
                    .add_enabled(is_modified, egui::Button::new("↺").small())
                    .on_hover_text(format!("Reset to default ({default_val})"))
                    .clicked()
                {
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

                let btn_text = if is_capturing {
                    RichText::new("⌛ Press keys...")
                        .color(Color32::from_rgb(242, 201, 76))
                        .strong()
                } else {
                    RichText::new(&current_val).strong()
                };

                let mut key_btn = egui::Button::new(btn_text).min_size(vec2(85.0, 24.0));
                if is_capturing {
                    key_btn = key_btn.fill(ui.visuals().selection.bg_fill);
                }

                if ui.add(key_btn).on_hover_text("Click to rebind hotkey (Esc to cancel)").clicked() {
                    if is_capturing {
                        self.capturing_target = None;
                    } else {
                        self.capturing_target = Some(target);
                    }
                }
            });

            ui.add_space(4.0);
        }
    }

    fn show_compiler_tab(&mut self, ui: &mut Ui) {
        setting_card(
            ui,
            "Default Typesetting Engine",
            "Initial compiler backend initialized on startup",
            |ui| {
                egui::ComboBox::from_id_salt("settings_engine_select")
                    .selected_text(self.draft_config.default_engine.display_name())
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        for engine in [
                            EngineKind::Native,
                            EngineKind::Tectonic,
                            EngineKind::PdfLatex,
                            EngineKind::XeLatex,
                            EngineKind::LuaLatex,
                        ] {
                            ui.selectable_value(
                                &mut self.draft_config.default_engine,
                                engine,
                                engine.display_name(),
                            );
                        }
                    });
            },
        );

        ui.add_space(6.0);

        setting_card(
            ui,
            "Tectonic XeTeX Engine",
            "Tectonic is compressed inside laketex.exe (zero external downloads needed)",
            |ui| {
                let is_ready = crate::compiler::provisioner::Provisioner::is_tectonic_ready();
                if is_ready {
                    ui.label(
                        RichText::new("✔ Ready (Embedded)")
                            .color(Color32::from_rgb(152, 195, 121))
                            .strong(),
                    );
                } else {
                    ui.label(
                        RichText::new("📦 Unpacks on first compile")
                            .color(ui.visuals().selection.stroke.color)
                            .strong(),
                    );
                }
            },
        );

        ui.add_space(6.0);

        setting_card(
            ui,
            "Live Keystroke Compilation",
            "Trigger automatic background compilation on document edits",
            |ui| {
                ui.checkbox(&mut self.draft_config.auto_compile, "");
            },
        );

        ui.add_space(6.0);

        setting_card(
            ui,
            "Allow Shell-Escape (-shell-escape)",
            "Permit external TeX engines to execute external shell commands (e.g. Pygments for minted)",
            |ui| {
                ui.checkbox(&mut self.draft_config.allow_shell_escape, "");
            },
        );

        ui.add_space(6.0);

        setting_card(
            ui,
            "Auto-Save Subdocuments on Background Compile",
            "Automatically write unsaved child files to disk during background master compilation",
            |ui| {
                ui.checkbox(&mut self.draft_config.auto_save_on_compile, "");
            },
        );
    }
}

/// Renders a single navigation item in the preferences sidebar.
fn nav_item(ui: &mut Ui, icon: &str, title: &str, is_selected: bool) -> bool {
    let text = RichText::new(format!("{icon}  {title}")).size(13.0);
    let text = if is_selected { text.strong() } else { text };

    let mut btn = egui::Button::new(text).min_size(vec2(ui.available_width(), 32.0));

    if is_selected {
        btn = btn.fill(ui.visuals().selection.bg_fill);
    }

    ui.add(btn).clicked()
}

/// Renders a segmented option button that highlights when active.
fn segmented_button(ui: &mut Ui, label: &str, is_selected: bool) -> bool {
    let text = if is_selected {
        RichText::new(label).strong()
    } else {
        RichText::new(label).color(ui.visuals().weak_text_color())
    };
    let mut btn = egui::Button::new(text)
        .min_size(vec2(0.0, 26.0))
        .corner_radius(CornerRadius::same(4));
    if is_selected {
        btn = btn.fill(ui.visuals().selection.bg_fill);
    }
    ui.add(btn).clicked()
}

/// Renders a clean card row with a title, description, and right-aligned control.
fn setting_card<R>(
    ui: &mut Ui,
    title: &str,
    desc: &str,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    Frame::group(ui.style())
        .inner_margin(Margin::symmetric(14, 10))
        .corner_radius(CornerRadius::same(6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let control_width = 200.0;
                let label_width = (ui.available_width() - control_width - 16.0).max(120.0);
                ui.allocate_ui_with_layout(
                    vec2(label_width, 0.0),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.label(RichText::new(title).strong().size(13.0));
                        if !desc.is_empty() {
                            ui.label(
                                RichText::new(desc)
                                    .size(11.0)
                                    .color(ui.visuals().weak_text_color()),
                            );
                        }
                    },
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    add_contents(ui)
                })
                .inner
            })
            .inner
        })
        .inner
}

/// Renders a card with a header and a vertical content layout below.
fn setting_card_vertical<R>(
    ui: &mut Ui,
    title: &str,
    desc: &str,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    Frame::group(ui.style())
        .inner_margin(Margin::symmetric(14, 10))
        .corner_radius(CornerRadius::same(6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical(|ui| {
                ui.label(RichText::new(title).strong().size(13.0));
                if !desc.is_empty() {
                    ui.label(
                        RichText::new(desc)
                            .size(11.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                }
                ui.add_space(8.0);
                add_contents(ui)
            })
            .inner
        })
        .inner
}
