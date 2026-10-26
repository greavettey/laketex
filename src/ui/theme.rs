//! UI theme definitions and palette loader.

use egui::{Color32, CornerRadius, Margin, Stroke, Style, Visuals};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemeConfig {
    pub name: String,
    #[serde(default = "default_dark")]
    pub dark_mode: bool,
    #[serde(default)]
    pub colors: HashMap<String, String>,
}

fn default_dark() -> bool {
    true
}

/// Type alias for backward compatibility with previous UI code
pub type AppTheme = ThemeConfig;

impl Default for ThemeConfig {
    fn default() -> Self {
        Self::dracula_jetbrains()
    }
}

impl ThemeConfig {
    /// Returns the human-readable name of the theme
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Creates a new custom theme with an empty color map
    pub fn new(name: impl Into<String>, dark_mode: bool) -> Self {
        Self {
            name: name.into(),
            dark_mode,
            colors: HashMap::new(),
        }
    }

    /// Sets a color value in the theme by key name
    pub fn with_color(mut self, key: impl Into<String>, hex_value: impl Into<String>) -> Self {
        self.colors.insert(key.into(), hex_value.into());
        self
    }

    /// Built-in JetBrains-styled Dracula theme.
    ///
    /// Implements the authentic Dracula IDE palette featuring `#282a36` editor background,
    /// `#21222c` tool window panels, `#44475a` selection, `#f8f8f2` foreground text,
    /// `#6272a4` comments/muted text, `#bd93f9` purple accents, and `#ff5555` error indicators.
    pub fn dracula_jetbrains() -> Self {
        let mut colors = HashMap::new();
        colors.insert("background".into(), "#282a36".into());
        colors.insert("panel_background".into(), "#21222c".into());
        colors.insert("editor_background".into(), "#282a36".into());
        colors.insert("faint_bg".into(), "#282a36".into());
        colors.insert("text".into(), "#f8f8f2".into());
        colors.insert("text_muted".into(), "#6272a4".into());
        colors.insert("selection".into(), "#44475a".into());
        colors.insert("selection_stroke".into(), "#bd93f9".into());
        colors.insert("accent".into(), "#bd93f9".into());
        colors.insert("border".into(), "#383a59".into());
        colors.insert("button_bg".into(), "#32344a".into());
        colors.insert("button_hover".into(), "#44475a".into());
        colors.insert("button_active".into(), "#6272a4".into());
        colors.insert("error".into(), "#ff5555".into());
        colors.insert("warning".into(), "#ffb86c".into());
        colors.insert("success".into(), "#50fa7b".into());
        colors.insert("info".into(), "#8be9fd".into());

        // Syntax highlighting tokens
        colors.insert("syntax_command".into(), "#8be9fd".into());          // Cyan
        colors.insert("syntax_argument".into(), "#50fa7b".into());         // Dracula green
        colors.insert("syntax_package".into(), "#ffb86c".into());          // Dracula orange/gold
        colors.insert("syntax_tag_property".into(), "#ff79c6".into());     // Dracula pink
        colors.insert("syntax_property_number".into(), "#bd93f9".into());  // Dracula purple
        colors.insert("syntax_property_boolean".into(), "#bd93f9".into()); // Dracula purple
        colors.insert("syntax_property_value".into(), "#f1fa8c".into());   // Dracula yellow
        colors.insert("syntax_math".into(), "#f1fa8c".into());             // Yellow
        colors.insert("syntax_math_block".into(), "#ffb86c".into());       // Orange
        colors.insert("syntax_brace".into(), "#ff79c6".into());            // Pink
        colors.insert("syntax_bracket".into(), "#8be9fd".into());          // Cyan
        colors.insert("syntax_special".into(), "#ff5555".into());          // Red
        colors.insert("syntax_comment".into(), "#6272a4".into());          // Dracula blue-gray
        colors.insert("syntax_text".into(), "#f8f8f2".into());             // Foreground

        // Bracket tree depth colors
        colors.insert("bracket_level_0".into(), "#bd93f9".into());         // Purple
        colors.insert("bracket_level_1".into(), "#8be9fd".into());         // Cyan
        colors.insert("bracket_level_2".into(), "#50fa7b".into());         // Green
        colors.insert("bracket_level_3".into(), "#ffb86c".into());         // Orange

        Self {
            name: "Dracula (JetBrains)".to_string(),
            dark_mode: true,
            colors,
        }
    }

    /// Built-in Synthwave Neon theme featuring deep dark backgrounds and bright, glowing syntax colors.
    pub fn synthwave_neon() -> Self {
        let mut colors = HashMap::new();
        // UI backgrounds (deep dark space and twilight obsidian)
        colors.insert("background".into(), "#13111c".into());
        colors.insert("panel_background".into(), "#181524".into());
        colors.insert("editor_background".into(), "#0f0d17".into());
        colors.insert("faint_bg".into(), "#1d192b".into());
        colors.insert("text".into(), "#f8f8fc".into());
        colors.insert("text_muted".into(), "#797596".into());
        colors.insert("selection".into(), "#382a54".into());
        colors.insert("selection_stroke".into(), "#ff2a85".into());
        colors.insert("accent".into(), "#ff2a85".into());
        colors.insert("border".into(), "#2b2540".into());
        colors.insert("button_bg".into(), "#231e34".into());
        colors.insert("button_hover".into(), "#322b4a".into());
        colors.insert("button_active".into(), "#463d66".into());
        colors.insert("error".into(), "#ff3860".into());
        colors.insert("warning".into(), "#ffdd57".into());
        colors.insert("success".into(), "#00ff9f".into());
        colors.insert("info".into(), "#00f0ff".into());

        // Bright pretty syntax highlighting colors
        colors.insert("syntax_command".into(), "#00f0ff".into());          // Radiant electric cyan
        colors.insert("syntax_argument".into(), "#00ff9f".into());         // Bright spring mint
        colors.insert("syntax_package".into(), "#ffe600".into());          // Radiant canary yellow
        colors.insert("syntax_tag_property".into(), "#ff2a85".into());     // Hot glowing pink
        colors.insert("syntax_property_number".into(), "#ff9f43".into());  // Bright neon tangerine
        colors.insert("syntax_property_boolean".into(), "#b967ff".into()); // Bright electric violet
        colors.insert("syntax_property_value".into(), "#2ee6ca".into());   // Bright neon aqua
        colors.insert("syntax_math".into(), "#ffe600".into());             // Bright canary yellow
        colors.insert("syntax_math_block".into(), "#ff9f43".into());       // Bright neon tangerine
        colors.insert("syntax_brace".into(), "#ff2a85".into());            // Pretty hot pink
        colors.insert("syntax_bracket".into(), "#b967ff".into());          // Bright electric violet
        colors.insert("syntax_special".into(), "#ff5e7e".into());          // Pretty neon coral
        colors.insert("syntax_comment".into(), "#797596".into());          // Pastel twilight lavender
        colors.insert("syntax_text".into(), "#f8f8fc".into());             // Crisp white

        // Bracket tree depth colors
        colors.insert("bracket_level_0".into(), "#ff2a85".into());     // Level 0: Pretty hot pink
        colors.insert("bracket_level_1".into(), "#00f0ff".into());     // Level 1: Electric cyan
        colors.insert("bracket_level_2".into(), "#ffe600".into());     // Level 2: Canary yellow
        colors.insert("bracket_level_3".into(), "#00ff9f".into());     // Level 3: Spring mint

        Self {
            name: "Synthwave Neon".to_string(),
            dark_mode: true,
            colors,
        }
    }

    /// Built-in Academic Dark theme optimized for long editing sessions.
    pub fn academic_dark() -> Self {
        let mut colors = HashMap::new();
        colors.insert("background".into(), "#181a20".into());
        colors.insert("panel_background".into(), "#14161b".into());
        colors.insert("editor_background".into(), "#0e0f12".into());
        colors.insert("faint_bg".into(), "#1c1f26".into());
        colors.insert("text".into(), "#dce1eb".into());
        colors.insert("text_muted".into(), "#8892b0".into());
        colors.insert("selection".into(), "#2d558c".into());
        colors.insert("selection_stroke".into(), "#78b4ff".into());
        colors.insert("accent".into(), "#61afef".into());
        colors.insert("border".into(), "#2d323e".into());
        colors.insert("button_bg".into(), "#20242e".into());
        colors.insert("button_hover".into(), "#2d3444".into());
        colors.insert("button_active".into(), "#374158".into());
        colors.insert("error".into(), "#eb5757".into());
        colors.insert("warning".into(), "#f2c94c".into());
        colors.insert("success".into(), "#27ae60".into());

        // Syntax highlighting tokens
        colors.insert("syntax_command".into(), "#61afef".into());          // Soft blue
        colors.insert("syntax_argument".into(), "#98c379".into());         // Sage green
        colors.insert("syntax_package".into(), "#e5c07b".into());          // Warm gold
        colors.insert("syntax_tag_property".into(), "#e06c75".into());     // Coral red
        colors.insert("syntax_property_number".into(), "#d19a66".into());  // Warm amber
        colors.insert("syntax_property_boolean".into(), "#c678dd".into()); // Soft violet
        colors.insert("syntax_property_value".into(), "#56b6c2".into());   // Muted cyan
        colors.insert("syntax_math".into(), "#e5c07b".into());             // Warm gold
        colors.insert("syntax_math_block".into(), "#d19a66".into());       // Warm amber
        colors.insert("syntax_brace".into(), "#c678dd".into());            // Soft violet
        colors.insert("syntax_bracket".into(), "#56b6c2".into());          // Muted cyan
        colors.insert("syntax_special".into(), "#e06c75".into());          // Coral red
        colors.insert("syntax_comment".into(), "#5c6370".into());          // Muted slate
        colors.insert("syntax_text".into(), "#dce1eb".into());             // Off-white

        Self {
            name: "Academic Dark".to_string(),
            dark_mode: true,
            colors,
        }
    }

    /// Built-in Slate Minimal theme with deep low-contrast tones.
    pub fn slate_minimal() -> Self {
        let mut colors = HashMap::new();
        colors.insert("background".into(), "#121418".into());
        colors.insert("panel_background".into(), "#0f1114".into());
        colors.insert("editor_background".into(), "#0a0b0e".into());
        colors.insert("faint_bg".into(), "#16191e".into());
        colors.insert("text".into(), "#d7dce6".into());
        colors.insert("text_muted".into(), "#7a8499".into());
        colors.insert("selection".into(), "#253347".into());
        colors.insert("accent".into(), "#64b5f6".into());
        colors.insert("border".into(), "#222630".into());
        colors.insert("button_bg".into(), "#1a1e26".into());
        colors.insert("button_hover".into(), "#242a36".into());
        colors.insert("button_active".into(), "#303948".into());

        // Syntax highlighting tokens
        colors.insert("syntax_command".into(), "#64b5f6".into());          // Slate sky blue
        colors.insert("syntax_argument".into(), "#81c784".into());         // Slate soft green
        colors.insert("syntax_package".into(), "#ffd54f".into());          // Subdued amber
        colors.insert("syntax_tag_property".into(), "#e57373".into());     // Subdued terracotta
        colors.insert("syntax_property_number".into(), "#ffb74d".into());  // Warm peach
        colors.insert("syntax_property_boolean".into(), "#ba68c8".into()); // Muted lavender
        colors.insert("syntax_property_value".into(), "#4dd0e1".into());   // Light cyan
        colors.insert("syntax_math".into(), "#ffd54f".into());
        colors.insert("syntax_math_block".into(), "#ffb74d".into());
        colors.insert("syntax_brace".into(), "#ba68c8".into());
        colors.insert("syntax_bracket".into(), "#4dd0e1".into());
        colors.insert("syntax_special".into(), "#e57373".into());
        colors.insert("syntax_comment".into(), "#7a8499".into());
        colors.insert("syntax_text".into(), "#d7dce6".into());

        Self {
            name: "Slate Minimal".to_string(),
            dark_mode: true,
            colors,
        }
    }

    /// Built-in Arctic Nord Frost theme.
    pub fn nord_frost() -> Self {
        let mut colors = HashMap::new();
        colors.insert("background".into(), "#2e3440".into());
        colors.insert("panel_background".into(), "#242933".into());
        colors.insert("editor_background".into(), "#212630".into());
        colors.insert("faint_bg".into(), "#3b4252".into());
        colors.insert("text".into(), "#eceff4".into());
        colors.insert("text_muted".into(), "#d8dee9".into());
        colors.insert("selection".into(), "#434c5e".into());
        colors.insert("selection_stroke".into(), "#88c0d0".into());
        colors.insert("accent".into(), "#88c0d0".into());
        colors.insert("border".into(), "#4c566a".into());
        colors.insert("button_bg".into(), "#3b4252".into());
        colors.insert("button_hover".into(), "#434c5e".into());
        colors.insert("button_active".into(), "#4c566a".into());

        // Syntax highlighting tokens (authentic Nord 7-15 palette)
        colors.insert("syntax_command".into(), "#88c0d0".into());          // Frost ice blue (Nord 8)
        colors.insert("syntax_argument".into(), "#a3be8c".into());         // Aurora green (Nord 14)
        colors.insert("syntax_package".into(), "#ebcb8b".into());          // Aurora yellow (Nord 13)
        colors.insert("syntax_tag_property".into(), "#bf616a".into());     // Aurora red (Nord 11)
        colors.insert("syntax_property_number".into(), "#d08770".into());  // Aurora orange (Nord 12)
        colors.insert("syntax_property_boolean".into(), "#b48ead".into()); // Aurora purple (Nord 15)
        colors.insert("syntax_property_value".into(), "#8fbcbb".into());   // Frost teal (Nord 7)
        colors.insert("syntax_math".into(), "#ebcb8b".into());
        colors.insert("syntax_math_block".into(), "#d08770".into());
        colors.insert("syntax_brace".into(), "#b48ead".into());
        colors.insert("syntax_bracket".into(), "#81a1c1".into());          // Frost blue (Nord 9)
        colors.insert("syntax_special".into(), "#bf616a".into());
        colors.insert("syntax_comment".into(), "#616e88".into());
        colors.insert("syntax_text".into(), "#eceff4".into());

        Self {
            name: "Nord Frost".to_string(),
            dark_mode: true,
            colors,
        }
    }

    /// Built-in Paper White theme for high-contrast daylight editing.
    pub fn paper_white() -> Self {
        let mut colors = HashMap::new();
        colors.insert("background".into(), "#fafafc".into());
        colors.insert("panel_background".into(), "#f4f5f8".into());
        colors.insert("editor_background".into(), "#ffffff".into());
        colors.insert("faint_bg".into(), "#eef0f4".into());
        colors.insert("text".into(), "#232830".into());
        colors.insert("text_muted".into(), "#636d7e".into());
        colors.insert("selection".into(), "#b4d7ff".into());
        colors.insert("selection_stroke".into(), "#3264c8".into());
        colors.insert("accent".into(), "#2b6cb0".into());
        colors.insert("border".into(), "#d0d5dd".into());
        colors.insert("button_bg".into(), "#e8ecf2".into());
        colors.insert("button_hover".into(), "#dce2ec".into());
        colors.insert("button_active".into(), "#cbd4e2".into());

        // Syntax highlighting tokens
        colors.insert("syntax_command".into(), "#105cb0".into());          // Editorial royal blue
        colors.insert("syntax_argument".into(), "#1b7a32".into());         // Deep forest green
        colors.insert("syntax_package".into(), "#b46a00".into());          // Warm amber ochre
        colors.insert("syntax_tag_property".into(), "#a62438".into());     // Burgundy crimson
        colors.insert("syntax_property_number".into(), "#c45500".into());  // Warm terracotta
        colors.insert("syntax_property_boolean".into(), "#7525a6".into()); // Deep royal purple
        colors.insert("syntax_property_value".into(), "#007a87".into());   // Deep petroleum teal
        colors.insert("syntax_math".into(), "#b46a00".into());
        colors.insert("syntax_math_block".into(), "#c45500".into());
        colors.insert("syntax_brace".into(), "#7525a6".into());
        colors.insert("syntax_bracket".into(), "#007a87".into());
        colors.insert("syntax_special".into(), "#a62438".into());
        colors.insert("syntax_comment".into(), "#636d7e".into());
        colors.insert("syntax_text".into(), "#232830".into());

        Self {
            name: "Paper White".to_string(),
            dark_mode: false,
            colors,
        }
    }

    /// Returns the list of standard built-in themes
    pub fn built_in_themes() -> Vec<Self> {
        vec![
            Self::dracula_jetbrains(),
            Self::synthwave_neon(),
            Self::academic_dark(),
            Self::slate_minimal(),
            Self::nord_frost(),
            Self::paper_white(),
        ]
    }

    /// Returns the syntax highlight color for a specific token type in this theme.
    /// Falls back to default dark or light token color if not explicitly defined in `colors`.
    pub fn token_color(&self, token_type: crate::document::lexer::TokenType) -> Color32 {
        use crate::document::lexer::TokenType;
        let keys: &[&str] = match token_type {
            TokenType::Command => &["syntax_command", "command"],
            TokenType::Argument => &["syntax_argument", "argument"],
            TokenType::PackageName => &["syntax_package", "syntax_package_name", "syntax_import"],
            TokenType::TagProperty => &["syntax_tag_property", "syntax_property", "syntax_attribute"],
            TokenType::PropertyValueNumber => &["syntax_property_number", "syntax_number"],
            TokenType::PropertyValueBoolean => &["syntax_property_boolean", "syntax_boolean"],
            TokenType::PropertyValue => &["syntax_property_value", "syntax_value"],
            TokenType::MathInline => &["syntax_math", "math"],
            TokenType::MathBlock => &["syntax_math_block", "math_block"],
            TokenType::Brace => &["syntax_brace", "brace"],
            TokenType::Bracket => &["syntax_bracket", "bracket"],
            TokenType::SpecialChar => &["syntax_special", "special"],
            TokenType::Comment => &["syntax_comment", "comment"],
            TokenType::PlainText => &["syntax_text", "text"],
        };
        if let Some(c) = self.get_color(keys) {
            return c;
        }
        if self.dark_mode {
            crate::document::lexer::LatexLexer::token_color_dark(token_type)
        } else {
            crate::document::lexer::LatexLexer::token_color_light(token_type)
        }
    }

    /// Returns the bracket tree depth color for nested brackets in this theme.
    pub fn bracket_depth_color(&self, depth: usize, is_matched: bool) -> Color32 {
        if !is_matched {
            return self
                .get_color(&["error", "syntax_error"])
                .unwrap_or(if self.dark_mode {
                    Color32::from_rgb(244, 71, 71)
                } else {
                    Color32::from_rgb(210, 30, 30)
                });
        }
        let key = format!("bracket_level_{}", depth % 4);
        if let Some(c) = self.get_color(&[&key]) {
            return c;
        }
        crate::document::lexer::LatexLexer::bracket_depth_color(depth, is_matched, self.dark_mode)
    }

    /// Looks up a color by one or more alias keys, returning the parsed [`Color32`].
    pub fn get_color(&self, keys: &[&str]) -> Option<Color32> {
        for key in keys {
            if let Some(val) = self.colors.get(*key) {
                if let Some(color) = parse_hex_color(val) {
                    return Some(color);
                }
            }
            // Case-insensitive fallback
            let key_lower = key.to_ascii_lowercase();
            for (k, v) in &self.colors {
                if k.to_ascii_lowercase() == key_lower {
                    if let Some(color) = parse_hex_color(v) {
                        return Some(color);
                    }
                }
            }
        }
        None
    }

    /// Registers fallback symbol/math fonts in egui so unicode chars don't render as tofu.
    pub fn configure_fonts(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();

        // On Windows, load native system symbol and emoji fonts
        if cfg!(windows) {
            let candidates = [
                ("segoe_symbol", "C:\\Windows\\Fonts\\seguisym.ttf"),
                ("segoe_ui", "C:\\Windows\\Fonts\\segoeui.ttf"),
                ("segoe_emoji", "C:\\Windows\\Fonts\\seguiemj.ttf"),
            ];

            for (font_name, font_path) in candidates {
                if let Ok(font_bytes) = std::fs::read(font_path) {
                    fonts.font_data.insert(
                        font_name.to_owned(),
                        std::sync::Arc::new(egui::FontData::from_owned(font_bytes)),
                    );

                    fonts
                        .families
                        .entry(egui::FontFamily::Proportional)
                        .or_default()
                        .push(font_name.to_owned());

                    fonts
                        .families
                        .entry(egui::FontFamily::Monospace)
                        .or_default()
                        .push(font_name.to_owned());
                }
            }
        }

        ctx.set_fonts(fonts);
    }

    /// Converts this theme configuration into concrete `egui::Visuals`
    pub fn to_visuals(&self) -> Visuals {
        let default_bg = if self.dark_mode {
            Color32::from_rgb(24, 26, 32)
        } else {
            Color32::from_rgb(250, 250, 252)
        };
        let default_panel = if self.dark_mode {
            Color32::from_rgb(20, 22, 27)
        } else {
            Color32::from_rgb(244, 245, 248)
        };
        let default_editor = if self.dark_mode {
            Color32::from_rgb(14, 15, 18)
        } else {
            Color32::WHITE
        };
        let default_faint = if self.dark_mode {
            Color32::from_rgb(28, 31, 38)
        } else {
            Color32::from_rgb(238, 240, 244)
        };
        let default_text = if self.dark_mode {
            Color32::from_rgb(220, 225, 235)
        } else {
            Color32::from_rgb(35, 40, 48)
        };
        let default_muted = if self.dark_mode {
            Color32::from_rgb(136, 146, 176)
        } else {
            Color32::from_rgb(99, 109, 126)
        };
        let default_selection = if self.dark_mode {
            Color32::from_rgb(45, 85, 140)
        } else {
            Color32::from_rgb(180, 215, 255)
        };
        let default_sel_stroke = if self.dark_mode {
            Color32::from_rgb(120, 180, 255)
        } else {
            Color32::from_rgb(50, 100, 200)
        };
        let default_accent = if self.dark_mode {
            Color32::from_rgb(97, 175, 239)
        } else {
            Color32::from_rgb(43, 108, 176)
        };
        let default_border = if self.dark_mode {
            Color32::from_rgb(45, 50, 62)
        } else {
            Color32::from_rgb(208, 213, 221)
        };
        let default_button = if self.dark_mode {
            Color32::from_rgb(32, 36, 46)
        } else {
            Color32::from_rgb(232, 236, 242)
        };
        let default_hover = if self.dark_mode {
            Color32::from_rgb(45, 52, 68)
        } else {
            Color32::from_rgb(220, 226, 236)
        };
        let default_active = if self.dark_mode {
            Color32::from_rgb(55, 65, 88)
        } else {
            Color32::from_rgb(203, 212, 226)
        };

        let bg = self
            .get_color(&["background", "window_fill", "bg"])
            .unwrap_or(default_bg);
        let panel_bg = self
            .get_color(&["panel_background", "panel_fill", "panel_bg", "surface"])
            .unwrap_or(default_panel);
        let editor_bg = self
            .get_color(&["editor_background", "extreme_bg", "code_bg"])
            .unwrap_or(default_editor);
        let faint_bg = self
            .get_color(&["faint_bg", "faint_background", "gutter"])
            .unwrap_or(default_faint);
        let text = self
            .get_color(&["text", "foreground", "fg", "text_color"])
            .unwrap_or(default_text);
        let text_muted = self
            .get_color(&["text_muted", "comment", "muted", "secondary_text"])
            .unwrap_or(default_muted);
        let selection = self
            .get_color(&["selection", "selection_bg", "select"])
            .unwrap_or(default_selection);
        let selection_stroke = self
            .get_color(&["selection_stroke", "selection_border"])
            .unwrap_or(default_sel_stroke);
        let accent = self
            .get_color(&["accent", "primary", "purple"])
            .unwrap_or(default_accent);
        let border = self
            .get_color(&["border", "stroke", "divider"])
            .unwrap_or(default_border);
        let button_bg = self
            .get_color(&["button_bg", "button_fill", "button_inactive"])
            .unwrap_or(default_button);
        let button_hover = self
            .get_color(&["button_hover", "hover"])
            .unwrap_or(default_hover);
        let button_active = self
            .get_color(&["button_active", "active"])
            .unwrap_or(default_active);

        let mut visuals = if self.dark_mode {
            Visuals::dark()
        } else {
            Visuals::light()
        };

        visuals.dark_mode = self.dark_mode;
        visuals.override_text_color = Some(text);
        visuals.window_fill = bg;
        visuals.panel_fill = panel_bg;
        visuals.faint_bg_color = faint_bg;
        visuals.extreme_bg_color = editor_bg;
        visuals.code_bg_color = editor_bg;

        visuals.selection = egui::style::Selection {
            bg_fill: selection,
            stroke: Stroke::new(1.0_f32, selection_stroke),
        };
        visuals.window_stroke = Stroke::new(1.0_f32, border);

        visuals.widgets.noninteractive = egui::style::WidgetVisuals {
            bg_fill: panel_bg,
            weak_bg_fill: panel_bg,
            bg_stroke: Stroke::new(1.0_f32, border),
            corner_radius: CornerRadius::same(3),
            fg_stroke: Stroke::new(1.0_f32, text_muted),
            expansion: 0.0,
        };
        visuals.widgets.inactive = egui::style::WidgetVisuals {
            bg_fill: button_bg,
            weak_bg_fill: panel_bg,
            bg_stroke: Stroke::new(1.0_f32, border),
            corner_radius: CornerRadius::same(3),
            fg_stroke: Stroke::new(1.0_f32, text),
            expansion: 0.0,
        };
        visuals.widgets.hovered = egui::style::WidgetVisuals {
            bg_fill: button_hover,
            weak_bg_fill: button_hover,
            bg_stroke: Stroke::new(1.0_f32, accent),
            corner_radius: CornerRadius::same(3),
            fg_stroke: Stroke::new(1.0_f32, Color32::WHITE),
            expansion: 0.5,
        };
        visuals.widgets.active = egui::style::WidgetVisuals {
            bg_fill: button_active,
            weak_bg_fill: button_active,
            bg_stroke: Stroke::new(1.0_f32, accent),
            corner_radius: CornerRadius::same(3),
            fg_stroke: Stroke::new(1.0_f32, Color32::WHITE),
            expansion: 0.0,
        };
        visuals.widgets.open = egui::style::WidgetVisuals {
            bg_fill: button_hover,
            weak_bg_fill: panel_bg,
            bg_stroke: Stroke::new(1.0_f32, accent),
            corner_radius: CornerRadius::same(3),
            fg_stroke: Stroke::new(1.0_f32, Color32::WHITE),
            expansion: 0.0,
        };

        visuals
    }

    /// Applies this theme to the active egui context
    pub fn apply(&self, ctx: &egui::Context) {
        let visuals = self.to_visuals();
        let mut style = Style::default();
        style.visuals = visuals;
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(7.0, 4.0);
        style.spacing.window_margin = Margin::same(6);
        ctx.set_style(style);
    }

    /// Parses a theme from JSON text
    pub fn from_json_str(content: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(content)
    }

    /// Parses a theme from YAML text
    pub fn from_yaml_str(content: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(content)
    }

    /// Loads a theme from a disk file (`.json`, `.yaml`, or `.yml`)
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read theme file {}: {e}", path.display()))?;

        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        match extension.as_str() {
            "json" => Self::from_json_str(&content)
                .map_err(|e| format!("Invalid theme JSON in {}: {e}", path.display())),
            "yaml" | "yml" => Self::from_yaml_str(&content)
                .map_err(|e| format!("Invalid theme YAML in {}: {e}", path.display())),
            _ => {
                // Try JSON first, then YAML
                Self::from_json_str(&content)
                    .or_else(|_| Self::from_yaml_str(&content))
                    .map_err(|_| {
                        format!(
                            "Theme file {} is neither valid JSON nor YAML",
                            path.display()
                        )
                    })
            }
        }
    }

    /// Discovers all available themes by starting with built-ins and scanning disk directories.
    ///
    /// Scans the following candidate directories for `.json`, `.yaml`, and `.yml` theme files:
    /// 1. Directory alongside current executable (`std::env::current_exe()`)
    /// 2. `./themes/` folder in current working directory
    /// 3. Subdirectory `themes` alongside the executable
    /// 4. Current working directory `.`
    pub fn discover_themes() -> Vec<Self> {
        let mut themes = Self::built_in_themes();
        let mut candidate_dirs: Vec<PathBuf> = Vec::new();

        // 1. Dedicated themes directory alongside the executable
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                candidate_dirs.push(exe_dir.join("themes"));
            }
        }

        // 2. Dedicated ./themes directory in current working directory
        candidate_dirs.push(PathBuf::from("themes"));

        // 3. User configuration directory themes
        if let Some(bin_dir) = crate::compiler::CompilerEngine::laketex_bin_dir() {
            if let Some(parent) = bin_dir.parent() {
                let config_themes = parent.join("themes");
                if config_themes.is_dir() {
                    candidate_dirs.push(config_themes);
                }
            }
        }

        for dir in candidate_dirs {
            if !dir.is_dir() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    let ext = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();

                    if ext == "json" || ext == "yaml" || ext == "yml" {
                        if let Ok(theme) = Self::from_file(&path) {
                            // Check if a theme with this exact name already exists
                            if let Some(pos) = themes.iter().position(|t| t.name == theme.name) {
                                themes[pos] = theme; // override built-in or earlier definition
                            } else {
                                themes.push(theme);
                            }
                        }
                    }
                }
            }
        }

        // Also check current directory, but ONLY for files explicitly containing "theme" in the filename
        // to avoid expensive JSON deserialization attempts on unrelated files (like package.json, tsconfig.json)
        if let Ok(entries) = std::fs::read_dir(".") {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                if name.to_ascii_lowercase().contains("theme") {
                    let path = entry.path();
                    if path.is_file() {
                        let ext = path
                            .extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_ascii_lowercase();
                        if ext == "json" || ext == "yaml" || ext == "yml" {
                            if let Ok(theme) = Self::from_file(&path) {
                                if let Some(pos) = themes.iter().position(|t| t.name == theme.name) {
                                    themes[pos] = theme;
                                } else {
                                    themes.push(theme);
                                }
                            }
                        }
                    }
                }
            }
        }

        themes
    }
}

/// Parses a hex color string into [`Color32`].
///
/// Supports `#RGB`, `#RGBA`, `#RRGGBB`, and `#RRGGBBAA` formats, with or without leading `#`.
pub fn parse_hex_color(hex: &str) -> Option<Color32> {
    let clean = hex.trim().trim_start_matches('#');
    if !clean.is_ascii() {
        return None;
    }
    match clean.len() {
        3 => {
            let r = u8::from_str_radix(&clean[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&clean[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&clean[2..3], 16).ok()? * 17;
            Some(Color32::from_rgb(r, g, b))
        }
        4 => {
            let r = u8::from_str_radix(&clean[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&clean[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&clean[2..3], 16).ok()? * 17;
            let a = u8::from_str_radix(&clean[3..4], 16).ok()? * 17;
            Some(Color32::from_rgba_unmultiplied(r, g, b, a))
        }
        6 => {
            let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
            let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
            let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
            Some(Color32::from_rgb(r, g, b))
        }
        8 => {
            let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
            let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
            let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
            let a = u8::from_str_radix(&clean[6..8], 16).ok()?;
            Some(Color32::from_rgba_unmultiplied(r, g, b, a))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        assert_eq!(
            parse_hex_color("#282a36"),
            Some(Color32::from_rgb(0x28, 0x2a, 0x36))
        );
        assert_eq!(
            parse_hex_color("282a36"),
            Some(Color32::from_rgb(0x28, 0x2a, 0x36))
        );
        assert_eq!(
            parse_hex_color("#fff"),
            Some(Color32::from_rgb(255, 255, 255))
        );
        assert_eq!(
            parse_hex_color("#000"),
            Some(Color32::from_rgb(0, 0, 0))
        );
        assert_eq!(parse_hex_color("invalid"), None);
        assert_eq!(parse_hex_color("#ééé"), None);
        assert_eq!(
            parse_hex_color("#FF000080"),
            Some(Color32::from_rgba_unmultiplied(255, 0, 0, 128))
        );
    }

    #[test]
    fn test_dracula_jetbrains_theme() {
        let dracula = ThemeConfig::dracula_jetbrains();
        assert_eq!(dracula.name, "Dracula (JetBrains)");
        assert!(dracula.dark_mode);
        assert_eq!(
            dracula.get_color(&["background"]),
            Some(Color32::from_rgb(0x28, 0x2a, 0x36))
        );
        assert_eq!(
            dracula.get_color(&["panel_background"]),
            Some(Color32::from_rgb(0x21, 0x22, 0x2c))
        );
        assert_eq!(
            dracula.get_color(&["accent"]),
            Some(Color32::from_rgb(0xbd, 0x93, 0xf9))
        );
    }

    #[test]
    fn test_json_and_yaml_serialization() {
        let custom = ThemeConfig::new("Custom Solar", true)
            .with_color("background", "#002b36")
            .with_color("text", "#839496");

        let json_str = serde_json::to_string(&custom).unwrap();
        let from_json = ThemeConfig::from_json_str(&json_str).unwrap();
        assert_eq!(from_json.name, "Custom Solar");
        assert_eq!(
            from_json.get_color(&["background"]),
            Some(Color32::from_rgb(0x00, 0x2b, 0x36))
        );

        let yaml_str = serde_yaml::to_string(&custom).unwrap();
        let from_yaml = ThemeConfig::from_yaml_str(&yaml_str).unwrap();
        assert_eq!(from_yaml.name, "Custom Solar");
        assert_eq!(
            from_yaml.get_color(&["text"]),
            Some(Color32::from_rgb(0x83, 0x94, 0x96))
        );
    }

    #[test]
    fn test_synthwave_neon_theme() {
        let theme = ThemeConfig::synthwave_neon();
        assert_eq!(theme.name, "Synthwave Neon");
        assert!(theme.dark_mode);
        assert_eq!(theme.get_color(&["accent"]), Some(Color32::from_rgb(0xff, 0x2a, 0x85)));
        assert_eq!(theme.get_color(&["syntax_command"]), Some(Color32::from_rgb(0x00, 0xf0, 0xff)));
        assert_eq!(theme.get_color(&["syntax_argument"]), Some(Color32::from_rgb(0x00, 0xff, 0x9f)));
        assert_eq!(theme.get_color(&["syntax_brace"]), Some(Color32::from_rgb(0xff, 0x2a, 0x85)));
        assert_eq!(theme.get_color(&["editor_background"]), Some(Color32::from_rgb(0x0f, 0x0d, 0x17)));
    }

    #[test]
    fn test_discover_themes_loads_custom_themes() {
        let themes = ThemeConfig::discover_themes();
        let names: Vec<&str> = themes.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"Dracula (JetBrains)"));
        assert!(names.contains(&"Synthwave Neon"));
        assert!(names.contains(&"Academic Dark"));
        assert!(names.contains(&"Solarized Dark"));
        assert!(names.contains(&"Monokai Pro"));
    }
}
