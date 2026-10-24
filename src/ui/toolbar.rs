//! # Main Application Toolbar
//!
//! Provides the primary top navigation and action bar for LakeTex, housing:
//! - File operations (New, Open, Save)
//! - Compilation trigger and live compile duration / error badge indicators
//! - Compilation engine switcher (Pure-Rust Native, Tectonic, pdfLaTeX, XeLaTeX, LuaLaTeX)
//! - Document preview zoom, width-fitting, and dark mode toggles
//! - Dynamic key-value theme selection (including Dracula and custom themes)
//! - Sidebar and console drawer toggles

use crate::compiler::engine::EngineKind;
use crate::document::{BuiltinDoc, BuiltinDocKind};
use crate::t;
use crate::viewer::PdfColorMode;
use egui::{vec2, Color32, RichText, Ui};

/// Actions triggered by toolbar buttons during an application frame.
#[derive(Debug, Default, Clone, Copy)]
pub struct ToolbarAction {
    /// True if the user pressed the Compile button or shortcut
    pub compile_requested: bool,
    /// True if the user clicked New Document
    pub new_requested: bool,
    /// True if the user clicked Open File
    pub open_requested: bool,
    /// True if the user clicked Save File
    pub save_requested: bool,
    /// True if the user clicked Save File As
    pub save_as_requested: bool,
    /// True if the user clicked Settings or pressed Ctrl+,
    pub settings_requested: bool,
    /// User requested to load a built-in documentation manual into the editor
    pub load_builtin_doc: Option<BuiltinDocKind>,
    /// User requested to inspect errors in console drawer
    pub inspect_errors: bool,
}

/// The main application toolbar widget.
pub struct Toolbar;

impl Toolbar {
    /// Renders the toolbar UI horizontally across the top of the window.
    pub fn show(
        ui: &mut Ui,
        is_compiling: bool,
        last_duration_ms: Option<u128>,
        error_count: usize,
        engine: &mut EngineKind,
        auto_compile: &mut bool,
        show_sidebar: &mut bool,
        show_drawer: &mut bool,
        color_mode: &mut PdfColorMode,
        fit_width: &mut bool,
        zoom: &mut f32,
    ) -> ToolbarAction {
        let mut action = ToolbarAction::default();

        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), crate::ui::PANEL_HEADER_HEIGHT),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_height(crate::ui::PANEL_HEADER_HEIGHT);
                ui.spacing_mut().item_spacing = vec2(5.0, 0.0);

                // Brand
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    crate::ui::logo::render_brand_label(ui, 14.5, Color32::from_rgb(97, 175, 239));
                });

                ui.separator();

                // File controls
                if ui.button(t!("toolbar.new_file")).clicked() {
                    action.new_requested = true;
                }
                if ui.button(t!("toolbar.open_file")).clicked() {
                    action.open_requested = true;
                }
                if ui.button(t!("toolbar.save_file")).clicked() {
                    action.save_requested = true;
                }

                // Built-in LaTeX Documentation Menu
                ui.menu_button(t!("toolbar.docs"), |ui| {
                    ui.set_min_width(240.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new("LaTeX Documentation Library").strong(),
                        )
                        .selectable(false),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new("Compilable .tex manuals bundled offline in the binary")
                                .small()
                                .color(Color32::from_gray(160)),
                        )
                        .selectable(false),
                    );
                    ui.separator();
                    for doc in BuiltinDoc::all() {
                        let text = format!("📄 {}", doc.title);
                        if ui
                            .button(text)
                            .on_hover_text(format!("{}\n\nFile: {}", doc.description, doc.filename))
                            .clicked()
                        {
                            action.load_builtin_doc = Some(doc.kind);
                            ui.close_menu();
                        }
                    }
                });

                ui.separator();

                // Compile button
                let compile_btn_text = if is_compiling {
                    RichText::new(t!("toolbar.compiling")).color(Color32::from_rgb(242, 201, 76))
                } else {
                    RichText::new(format!("{} (Ctrl+B)", t!("toolbar.compile")))
                        .strong()
                        .color(Color32::WHITE)
                };

                let compile_btn = egui::Button::new(compile_btn_text).fill(if is_compiling {
                    Color32::from_rgb(90, 75, 20)
                } else {
                    Color32::from_rgb(34, 110, 60)
                });

                if ui.add_enabled(!is_compiling, compile_btn).clicked() {
                    action.compile_requested = true;
                }

                ui.checkbox(auto_compile, "Auto");

                // Status chip
                if is_compiling {
                    ui.add(
                        egui::Label::new(
                            RichText::new("⌛ Working").color(Color32::from_rgb(242, 201, 76)),
                        )
                        .selectable(false),
                    );
                } else if error_count > 0 {
                    let err_btn = ui.add(
                        egui::Button::new(
                            RichText::new(format!("✗ {error_count} Errors"))
                                .color(Color32::from_rgb(235, 87, 87))
                                .strong(),
                        )
                        .frame(false),
                    ).on_hover_text("Click to inspect errors in Console Drawer");
                    if err_btn.clicked() {
                        *show_drawer = true;
                        action.inspect_errors = true;
                    }
                } else if let Some(ms) = last_duration_ms {
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("✓ {ms}ms")).color(Color32::from_rgb(97, 175, 239)),
                        )
                        .selectable(false),
                    );
                }

                ui.separator();

                // Engine selector
                egui::ComboBox::from_id_salt("engine_select")
                    .selected_text(engine.display_name())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(engine, EngineKind::Native, EngineKind::Native.display_name());
                        ui.selectable_value(engine, EngineKind::Tectonic, EngineKind::Tectonic.display_name());
                        ui.selectable_value(engine, EngineKind::PdfLatex, EngineKind::PdfLatex.display_name());
                        ui.selectable_value(engine, EngineKind::XeLatex, EngineKind::XeLatex.display_name());
                        ui.selectable_value(engine, EngineKind::LuaLatex, EngineKind::LuaLatex.display_name());
                    });

                ui.separator();

                // Viewer controls
                if ui.button(" - ").clicked() {
                    *zoom = (*zoom - 0.15).max(0.25);
                    *fit_width = false;
                }
                ui.add(egui::Label::new(format!("{:.0}%", *zoom * 100.0)).selectable(false));
                if ui.button(" + ").clicked() {
                    *zoom = (*zoom + 0.15).min(4.0);
                    *fit_width = false;
                }
                if ui.selectable_label(*fit_width, t!("toolbar.fit_width")).clicked() {
                    *fit_width = !*fit_width;
                }

                let color_btn_text = match *color_mode {
                    PdfColorMode::Normal => t!("viewer.color_day"),
                    PdfColorMode::AcademicDark => t!("viewer.color_dark"),
                    PdfColorMode::Sepia => t!("viewer.color_sepia"),
                };
                if ui
                    .button(color_btn_text)
                    .on_hover_text("Toggle PDF Color Mode (Daylight -> Academic Dark -> Sepia)")
                    .clicked()
                {
                    *color_mode = match *color_mode {
                        PdfColorMode::Normal => PdfColorMode::AcademicDark,
                        PdfColorMode::AcademicDark => PdfColorMode::Sepia,
                        PdfColorMode::Sepia => PdfColorMode::Normal,
                    };
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(format!("{} (Ctrl+,)", t!("toolbar.settings")))
                        .on_hover_text("Open Settings & Preferences (Theme, Shortcuts, PDF Defaults)")
                        .clicked()
                    {
                        action.settings_requested = true;
                    }

                    if ui.selectable_label(*show_drawer, "Console").clicked() {
                        *show_drawer = !*show_drawer;
                    }

                    if ui.selectable_label(*show_sidebar, "Sidebar").clicked() {
                        *show_sidebar = !*show_sidebar;
                    }
                });
            },
        );

        action
    }
}
