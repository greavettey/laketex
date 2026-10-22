//! # Mathematical Formula Typesetting Engine
//!
//! Transforms LaTeX mathematical formulas into high-fidelity Unicode typographic layouts.
//!
//! Handles:
//! - Recursive balanced-brace fraction formatting (`\frac{num}{den}`)
//! - Multi-row bracketed matrix formatting (`pmatrix`, `bmatrix`, `vmatrix`)
//! - Unicode superscripts and subscripts
//! - Greek letters, mathematical operators, and relational symbols
//! - Equation centering and numbering

use egui::{Color32, FontId, Ui};

/// Pure-Rust mathematical typesetter for formulas and matrices.
pub struct MathRenderer;

impl MathRenderer {
    /// Extracts balanced curly braces `{...}` from an iterator
    pub fn extract_balanced_braces<I: Iterator<Item = char>>(
        chars: &mut std::iter::Peekable<I>,
    ) -> String {
        if chars.next() != Some('{') {
            return String::new();
        }
        let mut depth = 1;
        let mut content = String::new();
        while let Some(c) = chars.next() {
            if c == '{' {
                depth += 1;
                content.push(c);
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                content.push(c);
            } else {
                content.push(c);
            }
        }
        content
    }

    /// Recursively formats LaTeX fractions \frac{num}{den} into readable typographic representation
    pub fn format_fractions(text: &str) -> String {
        let mut result = String::new();
        let mut chars = text.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '\\' {
                let mut cmd = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc.is_alphabetic() {
                        cmd.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                if cmd == "frac" || cmd == "dfrac" || cmd == "tfrac" {
                    while chars.peek() == Some(&' ') {
                        chars.next();
                    }
                    if chars.peek() == Some(&'{') {
                        let num = Self::extract_balanced_braces(&mut chars);
                        while chars.peek() == Some(&' ') {
                            chars.next();
                        }
                        let den = if chars.peek() == Some(&'{') {
                            Self::extract_balanced_braces(&mut chars)
                        } else {
                            String::new()
                        };
                        let fmt_num = Self::format_fractions(&num);
                        let fmt_den = Self::format_fractions(&den);
                        result.push_str(&format!("({fmt_num} / {fmt_den})"));
                        continue;
                    }
                }

                result.push('\\');
                result.push_str(&cmd);
                continue;
            }
            result.push(c);
        }

        result
    }

    /// Parses the first matrix environment in `text`, returning (start, end, env_name, rows_of_cells)
    pub fn parse_matrix_environment(
        text: &str,
    ) -> Option<(usize, usize, String, Vec<Vec<String>>)> {
        for env in [
            "pmatrix",
            "bmatrix",
            "Bmatrix",
            "vmatrix",
            "Vmatrix",
            "matrix",
            "smallmatrix",
            "array",
        ] {
            let begin_tag = format!("\\begin{{{env}}}");
            let end_tag = format!("\\end{{{env}}}");

            if let Some(start_idx) = text.find(&begin_tag) {
                if let Some(end_rel) = text[start_idx..].find(&end_tag) {
                    let end_idx = start_idx + end_rel + end_tag.len();
                    let mut inner = &text[start_idx + begin_tag.len()..start_idx + end_rel];

                    // For array environment, strip column specification if present e.g. {cc} or {l|r}
                    let inner_trimmed = inner.trim_start();
                    if env == "array" && inner_trimmed.starts_with('{') {
                        if let Some(brace_end) = inner_trimmed.find('}') {
                            inner = &inner_trimmed[brace_end + 1..];
                        }
                    }

                    let raw_rows: Vec<&str> = inner
                        .split("\\\\")
                        .map(|r| r.trim())
                        .filter(|r| !r.is_empty())
                        .collect();

                    let mut rows = Vec::new();
                    for r_str in raw_rows {
                        let cells: Vec<String> = r_str
                            .split('&')
                            .map(|c| c.trim().to_string())
                            .collect();
                        rows.push(cells);
                    }

                    return Some((start_idx, end_idx, env.to_string(), rows));
                }
            }
        }
        None
    }

    /// Converts a matrix environment into semantic HTML table format
    pub fn matrix_to_html(env: &str, rows: &[Vec<String>]) -> String {
        let mut html = format!("<table class=\"math-matrix {env}\"><tbody>");
        for row in rows {
            html.push_str("<tr>");
            for cell in row {
                let cell_html = Self::formula_to_html(cell);
                html.push_str(&format!("<td class=\"m-cell\">{cell_html}</td>"));
            }
            html.push_str("</tr>");
        }
        html.push_str("</tbody></table>");
        html
    }

    /// Converts fractions into publication-grade HTML vertical fractions
    pub fn format_fractions_html(text: &str) -> String {
        let mut result = String::new();
        let mut chars = text.chars().peekable();
        let mut inside_tag = false;

        while let Some(c) = chars.next() {
            if c == '<' {
                inside_tag = true;
                result.push(c);
                continue;
            }
            if c == '>' {
                inside_tag = false;
                result.push(c);
                continue;
            }
            if inside_tag {
                result.push(c);
                continue;
            }

            if c == '\\' {
                let mut cmd = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc.is_alphabetic() {
                        cmd.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                if cmd == "frac" || cmd == "dfrac" || cmd == "tfrac" {
                    while chars.peek() == Some(&' ') {
                        chars.next();
                    }
                    if chars.peek() == Some(&'{') {
                        let num = Self::extract_balanced_braces(&mut chars);
                        while chars.peek() == Some(&' ') {
                            chars.next();
                        }
                        let den = if chars.peek() == Some(&'{') {
                            Self::extract_balanced_braces(&mut chars)
                        } else {
                            String::new()
                        };
                        let fmt_num = Self::formula_to_html(&num);
                        let fmt_den = Self::formula_to_html(&den);
                        result.push_str(&format!(
                            "<span class=\"fraction\"><span class=\"num\">{fmt_num}</span><span class=\"den\">{fmt_den}</span></span>"
                        ));
                        continue;
                    }
                }

                result.push('\\');
                result.push_str(&cmd);
                continue;
            }
            result.push(c);
        }

        result
    }

    /// Converts square roots into HTML
    pub fn format_sqrt_html(text: &str) -> String {
        let mut result = String::new();
        let mut chars = text.chars().peekable();
        let mut inside_tag = false;

        while let Some(c) = chars.next() {
            if c == '<' {
                inside_tag = true;
                result.push(c);
                continue;
            }
            if c == '>' {
                inside_tag = false;
                result.push(c);
                continue;
            }
            if inside_tag {
                result.push(c);
                continue;
            }

            if c == '\\' {
                let mut cmd = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc.is_alphabetic() {
                        cmd.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                if cmd == "sqrt" {
                    while chars.peek() == Some(&' ') {
                        chars.next();
                    }
                    let root_degree = if chars.peek() == Some(&'[') {
                        chars.next();
                        let mut deg = String::new();
                        while let Some(dc) = chars.next() {
                            if dc == ']' {
                                break;
                            }
                            deg.push(dc);
                        }
                        Some(deg)
                    } else {
                        None
                    };

                    while chars.peek() == Some(&' ') {
                        chars.next();
                    }
                    if chars.peek() == Some(&'{') {
                        let radicand = Self::extract_balanced_braces(&mut chars);
                        let fmt_radicand = Self::formula_to_html(&radicand);
                        if let Some(deg) = root_degree {
                            let fmt_deg = Self::formula_to_html(&deg);
                            result.push_str(&format!(
                                "<span class=\"sqrt\"><sup>{fmt_deg}</sup><span class=\"radical\">√</span><span class=\"radicand\">{fmt_radicand}</span></span>"
                            ));
                        } else {
                            result.push_str(&format!(
                                "<span class=\"sqrt\"><span class=\"radical\">√</span><span class=\"radicand\">{fmt_radicand}</span></span>"
                            ));
                        }
                        continue;
                    }
                }

                result.push('\\');
                result.push_str(&cmd);
                continue;
            }
            result.push(c);
        }

        result
    }

    /// Converts superscripts and subscripts to HTML `<sup>` and `<sub>`
    pub fn format_scripts_html(text: &str) -> String {
        let mut result = String::new();
        let mut chars = text.chars().peekable();
        let mut inside_tag = false;

        while let Some(c) = chars.next() {
            if c == '<' {
                inside_tag = true;
                result.push(c);
                continue;
            }
            if c == '>' {
                inside_tag = false;
                result.push(c);
                continue;
            }
            if inside_tag {
                result.push(c);
                continue;
            }

            if c == '^' {
                let sup_content = if chars.peek() == Some(&'{') {
                    Self::extract_balanced_braces(&mut chars)
                } else if let Some(&sc) = chars.peek() {
                    if sc.is_alphanumeric() || sc == '+' || sc == '-' {
                        chars.next();
                        sc.to_string()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                if !sup_content.is_empty() {
                    let formatted = Self::formula_to_html(&sup_content);
                    result.push_str(&format!("<sup>{formatted}</sup>"));
                    continue;
                }
            }

            if c == '_' {
                let sub_content = if chars.peek() == Some(&'{') {
                    Self::extract_balanced_braces(&mut chars)
                } else if let Some(&sc) = chars.peek() {
                    if sc.is_alphanumeric() || sc == '+' || sc == '-' {
                        chars.next();
                        sc.to_string()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                if !sub_content.is_empty() {
                    let formatted = Self::formula_to_html(&sub_content);
                    result.push_str(&format!("<sub>{formatted}</sub>"));
                    continue;
                }
            }

            result.push(c);
        }

        result
    }

    /// Converts a LaTeX math formula into semantic, publication-grade HTML
    pub fn formula_to_html(formula: &str) -> String {
        // Pre-escape mathematical relational operators to valid XML/HTML entities
        let mut text = formula
            .replace('<', "&lt;")
            .replace('>', "&gt;");

        // 1. Convert any matrix environments to HTML tables
        while let Some((start, end, env, rows)) = Self::parse_matrix_environment(&text) {
            let matrix_html = Self::matrix_to_html(&env, &rows);
            text.replace_range(start..end, &matrix_html);
        }

        // 2. Convert fractions \frac{num}{den} to HTML vertical fraction spans
        text = Self::format_fractions_html(&text);

        // 3. Convert square roots \sqrt{radicand}
        text = Self::format_sqrt_html(&text);

        // 4. Convert font style macros
        for prefix in ["\\mathbf{", "\\bm{", "\\boldsymbol{"] {
            while let Some(pos) = text.find(prefix) {
                if let Some(end) = text[pos + prefix.len()..].find('}') {
                    let inner = &text[pos + prefix.len()..pos + prefix.len() + end];
                    let formatted_inner = Self::formula_to_html(inner);
                    let rep = format!("<strong class=\"math-bold\">{formatted_inner}</strong>");
                    text.replace_range(pos..pos + prefix.len() + 1 + end, &rep);
                } else {
                    break;
                }
            }
        }

        for prefix in ["\\mathrm{", "\\text{", "\\textnormal{"] {
            while let Some(pos) = text.find(prefix) {
                if let Some(end) = text[pos + prefix.len()..].find('}') {
                    let inner = &text[pos + prefix.len()..pos + prefix.len() + end];
                    let rep = format!("<span class=\"math-roman\">{inner}</span>");
                    text.replace_range(pos..pos + prefix.len() + 1 + end, &rep);
                } else {
                    break;
                }
            }
        }

        for prefix in ["\\mathit{"] {
            while let Some(pos) = text.find(prefix) {
                if let Some(end) = text[pos + prefix.len()..].find('}') {
                    let inner = &text[pos + prefix.len()..pos + prefix.len() + end];
                    let formatted_inner = Self::formula_to_html(inner);
                    let rep = format!("<em class=\"math-italic\">{formatted_inner}</em>");
                    text.replace_range(pos..pos + prefix.len() + 1 + end, &rep);
                } else {
                    break;
                }
            }
        }

        // 5. Convert superscripts and subscripts to <sup> and <sub>
        text = Self::format_scripts_html(&text);

        // 6. Mathematical symbol replacements
        let replacements = [
            // Greek lowercase
            ("\\alpha", "α"), ("\\beta", "β"), ("\\gamma", "γ"), ("\\delta", "δ"),
            ("\\epsilon", "ε"), ("\\varepsilon", "ε"), ("\\zeta", "ζ"), ("\\eta", "η"),
            ("\\theta", "θ"), ("\\vartheta", "ϑ"), ("\\iota", "ι"), ("\\kappa", "κ"),
            ("\\lambda", "λ"), ("\\mu", "μ"), ("\\nu", "ν"), ("\\xi", "ξ"),
            ("\\pi", "π"), ("\\varpi", "ϖ"), ("\\rho", "ρ"), ("\\varrho", "ϱ"),
            ("\\sigma", "σ"), ("\\varsigma", "ς"), ("\\tau", "τ"), ("\\upsilon", "υ"),
            ("\\phi", "ϕ"), ("\\varphi", "φ"), ("\\chi", "χ"), ("\\psi", "ψ"), ("\\omega", "ω"),

            // Greek uppercase
            ("\\Gamma", "Γ"), ("\\Delta", "Δ"), ("\\Theta", "Θ"), ("\\Lambda", "Λ"),
            ("\\Xi", "Ξ"), ("\\Pi", "Π"), ("\\Sigma", "Σ"), ("\\Upsilon", "Υ"),
            ("\\Phi", "Φ"), ("\\Psi", "Ψ"), ("\\Omega", "Ω"),

            // Constants & Sets
            ("\\hbar", "ℏ"), ("\\infty", "∞"), ("\\partial", "∂"), ("\\nabla", "∇"),
            ("\\mathbb{R}", "ℝ"), ("\\mathbb{C}", "ℂ"), ("\\mathbb{N}", "ℕ"),
            ("\\mathbb{Z}", "ℤ"), ("\\mathbb{Q}", "ℚ"),

            // Operators
            ("\\int", "∫"), ("\\iint", "∬"), ("\\iiint", "∭"), ("\\oint", "∮"),
            ("\\sum", "∑"), ("\\prod", "∏"), ("\\coprod", "∐"),

            // Dots
            ("\\cdots", "⋯"), ("\\vdots", "⋮"), ("\\ddots", "⋱"), ("\\ldots", "…"), ("\\dots", "…"),

            // Relations
            ("\\leq", "≤"), ("\\le", "≤"), ("\\geq", "≥"), ("\\ge", "≥"),
            ("\\neq", "≠"), ("\\approx", "≈"), ("\\sim", "∼"), ("\\equiv", "≡"),
            ("\\ll", "≪"), ("\\gg", "≫"), ("\\propto", "∝"),

            // Set theory & logic
            ("\\in", "∈"), ("\\notin", "∉"), ("\\subset", "⊂"), ("\\supset", "⊃"),
            ("\\subseteq", "⊆"), ("\\supseteq", "⊇"), ("\\cap", "∩"), ("\\cup", "∪"),
            ("\\forall", "∀"), ("\\exists", "∃"), ("\\nexists", "∄"), ("\\emptyset", "∅"),

            // Arrows
            ("\\rightarrow", "→"), ("\\leftarrow", "←"), ("\\Rightarrow", "⇒"),
            ("\\Leftarrow", "⇐"), ("\\leftrightarrow", "↔"), ("\\Leftrightarrow", "⇔"),
            ("\\to", "→"), ("\\mapsto", "↦"), ("\\implies", "⟹"), ("\\iff", "⟺"),

            // Binary operators
            ("\\pm", "±"), ("\\mp", "∓"), ("\\times", "×"), ("\\div", "÷"), ("\\cdot", "·"),
            ("\\circ", "∘"), ("\\bullet", "•"), ("\\star", "⋆"), ("\\ast", "∗"),

            // Functions
            ("\\sin", "sin"), ("\\cos", "cos"), ("\\tan", "tan"), ("\\cot", "cot"),
            ("\\sec", "sec"), ("\\csc", "csc"), ("\\arcsin", "arcsin"), ("\\arccos", "arccos"),
            ("\\arctan", "arctan"), ("\\sinh", "sinh"), ("\\cosh", "cosh"), ("\\tanh", "tanh"),
            ("\\ln", "ln"), ("\\log", "log"), ("\\exp", "exp"), ("\\det", "det"),
            ("\\dim", "dim"), ("\\ker", "ker"), ("\\deg", "deg"), ("\\gcd", "gcd"),
            ("\\lim", "lim"), ("\\max", "max"), ("\\min", "min"), ("\\sup", "sup"),
            ("\\inf", "inf"), ("\\arg", "arg"), ("\\hom", "hom"),

            // Delimiters
            ("\\left(", "("), ("\\right)", ")"),
            ("\\left[", "["), ("\\right]", "]"),
            ("\\left\\{", "{"), ("\\right\\}", "}"),
            ("\\left|", "|"), ("\\right|", "|"),
            ("\\left.", ""), ("\\right.", ""),

            // Spacing
            ("\\quad", "&#8195;"), ("\\qquad", "&#8195;&#8195;"), ("\\,", "&#160;"), ("\\;", "&#160;&#160;"), ("\\!", ""),
        ];

        for (from, to) in replacements {
            text = text.replace(from, to);
        }

        // Combining Accents
        let accents = [
            ("\\hat{", "\u{0302}"),
            ("\\vec{", "\u{20D7}"),
            ("\\bar{", "\u{0304}"),
            ("\\dot{", "\u{0307}"),
            ("\\ddot{", "\u{0308}"),
            ("\\tilde{", "\u{0303}"),
        ];
        for (prefix, mark) in accents {
            while let Some(pos) = text.find(prefix) {
                if let Some(end) = text[pos + prefix.len()..].find('}') {
                    let base = &text[pos + prefix.len()..pos + prefix.len() + end];
                    let combined = format!("{base}{mark}");
                    text.replace_range(pos..pos + prefix.len() + 1 + end, &combined);
                } else {
                    break;
                }
            }
        }

        text
    }

    /// Converts LaTeX matrix environments (pmatrix, bmatrix, matrix) into 2D formatted text representations
    pub fn format_matrices(text: &str) -> String {
        let mut out = text.to_string();

        while let Some((start_idx, end_idx, env, rows)) = Self::parse_matrix_environment(&out) {
            let mut formatted_rows = Vec::new();

            for r_cells in rows {
                let cells: Vec<String> = r_cells
                    .iter()
                    .map(|c| Self::translate_math_symbols(c.trim()))
                    .collect();
                let row_content = cells.join("   ");

                let (left, right) = match env.as_str() {
                    "pmatrix" => ("( ", " )"),
                    "bmatrix" => ("[ ", " ]"),
                    "Bmatrix" => ("{ ", " }"),
                    "Vmatrix" => ("‖ ", " ‖"),
                    "vmatrix" => ("| ", " |"),
                    "smallmatrix" => ("[", "]"),
                    _ => ("| ", " |"),
                };

                formatted_rows.push(format!("{left}{row_content}{right}"));
            }

            let matrix_str = formatted_rows.join("\n");
            out.replace_range(start_idx..end_idx, &matrix_str);
        }

        out
    }

    /// Converts subscripts and superscripts to Unicode
    pub fn format_scripts(text: &str) -> String {
        let mut result = String::new();
        let mut chars = text.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '^' {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    while let Some(sc) = chars.next() {
                        if sc == '}' {
                            break;
                        }
                        result.push(Self::to_superscript(sc));
                    }
                } else if let Some(&sc) = chars.peek() {
                    chars.next();
                    result.push(Self::to_superscript(sc));
                }
                continue;
            }

            if c == '_' {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    while let Some(sc) = chars.next() {
                        if sc == '}' {
                            break;
                        }
                        result.push(Self::to_subscript(sc));
                    }
                } else if let Some(&sc) = chars.peek() {
                    chars.next();
                    result.push(Self::to_subscript(sc));
                }
                continue;
            }

            result.push(c);
        }

        result
    }

    fn to_superscript(c: char) -> char {
        match c {
            '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴',
            '5' => '⁵', '6' => '⁶', '7' => '⁷', '8' => '⁸', '9' => '⁹',
            '+' => '⁺', '-' => '⁻', '=' => '⁼', '(' => '⁽', ')' => '⁾',
            'n' => 'ⁿ', 'i' => 'ⁱ', 'x' => 'ˣ', 'y' => 'ʸ', 't' => 'ᵗ',
            _ => c,
        }
    }

    fn to_subscript(c: char) -> char {
        match c {
            '0' => '₀', '1' => '₁', '2' => '₂', '3' => '₃', '4' => '₄',
            '5' => '₅', '6' => '₆', '7' => '₇', '8' => '₈', '9' => '₉',
            '+' => '₊', '-' => '₋', '=' => '₌', '(' => '₍', ')' => '₎',
            'a' => 'ₐ', 'e' => 'ₑ', 'h' => 'ₕ', 'i' => 'ᵢ', 'j' => 'ⱼ',
            'k' => 'ₖ', 'l' => 'ₗ', 'm' => 'ₘ', 'n' => 'ₙ', 'o' => 'ₒ',
            'p' => 'ₚ', 'r' => 'ᵣ', 's' => 'ₛ', 't' => 'ₜ', 'u' => 'ᵤ',
            'v' => 'ᵥ', 'x' => 'ₓ',
            _ => c,
        }
    }

    /// Replaces standard LaTeX math symbols and Greek macros with their proper Unicode characters.
    pub fn translate_math_symbols(formula: &str) -> String {
        // Step 1: Format nested fractions
        let frac_handled = Self::format_fractions(formula);

        // Step 2: Format matrices
        let matrix_handled = Self::format_matrices(&frac_handled);

        let mut text = matrix_handled;

        let replacements = [
            // Greek lowercase
            ("\\alpha", "α"), ("\\beta", "β"), ("\\gamma", "γ"), ("\\delta", "δ"),
            ("\\epsilon", "ε"), ("\\varepsilon", "ε"), ("\\zeta", "ζ"), ("\\eta", "η"),
            ("\\theta", "θ"), ("\\vartheta", "ϑ"), ("\\iota", "ι"), ("\\kappa", "κ"),
            ("\\lambda", "λ"), ("\\mu", "μ"), ("\\nu", "ν"), ("\\xi", "ξ"),
            ("\\pi", "π"), ("\\varpi", "ϖ"), ("\\rho", "ρ"), ("\\varrho", "ϱ"),
            ("\\sigma", "σ"), ("\\varsigma", "ς"), ("\\tau", "τ"), ("\\upsilon", "υ"),
            ("\\phi", "ϕ"), ("\\varphi", "φ"), ("\\chi", "χ"), ("\\psi", "ψ"), ("\\omega", "ω"),

            // Greek uppercase
            ("\\Gamma", "Γ"), ("\\Delta", "Δ"), ("\\Theta", "Θ"), ("\\Lambda", "Λ"),
            ("\\Xi", "Ξ"), ("\\Pi", "Π"), ("\\Sigma", "Σ"), ("\\Upsilon", "Υ"),
            ("\\Phi", "Φ"), ("\\Psi", "Ψ"), ("\\Omega", "Ω"),

            // Math Constants & Special sets
            ("\\hbar", "ℏ"), ("\\infty", "∞"), ("\\partial", "∂"), ("\\nabla", "∇"),
            ("\\mathbb{R}", "ℝ"), ("\\mathbb{C}", "ℂ"), ("\\mathbb{N}", "ℕ"),
            ("\\mathbb{Z}", "ℤ"), ("\\mathbb{Q}", "ℚ"),

            // Operators & Calculus
            ("\\int", "∫"), ("\\iint", "∬"), ("\\iiint", "∭"), ("\\oint", "∮"),
            ("\\sum", "∑"), ("\\prod", "∏"), ("\\coprod", "∐"),
            ("\\sqrt", "√"),

            // Ellipses
            ("\\cdots", "⋯"), ("\\vdots", "⋮"), ("\\ddots", "⋱"), ("\\ldots", "…"),

            // Relations
            ("\\leq", "≤"), ("\\le", "≤"), ("\\geq", "≥"), ("\\ge", "≥"),
            ("\\neq", "≠"), ("\\approx", "≈"), ("\\sim", "∼"), ("\\equiv", "≡"),
            ("\\ll", "≪"), ("\\gg", "≫"), ("\\propto", "∝"),

            // Set theory & logic
            ("\\in", "∈"), ("\\notin", "∉"), ("\\subset", "⊂"), ("\\supset", "⊃"),
            ("\\subseteq", "⊆"), ("\\supseteq", "⊇"), ("\\cap", "∩"), ("\\cup", "∪"),
            ("\\forall", "∀"), ("\\exists", "∃"), ("\\nexists", "∄"), ("\\emptyset", "∅"),

            // Arrows
            ("\\rightarrow", "→"), ("\\leftarrow", "←"), ("\\Rightarrow", "⇒"),
            ("\\Leftarrow", "⇐"), ("\\leftrightarrow", "↔"), ("\\Leftrightarrow", "⇔"),
            ("\\to", "→"), ("\\mapsto", "↦"), ("\\implies", "⟹"), ("\\iff", "⟺"),

            // Binary operators
            ("\\pm", "±"), ("\\mp", "∓"), ("\\times", "×"), ("\\div", "÷"), ("\\cdot", "·"),
            ("\\circ", "∘"), ("\\bullet", "•"), ("\\star", "⋆"), ("\\ast", "∗"),

            // Common mathematical functions
            ("\\sin", "sin"), ("\\cos", "cos"), ("\\tan", "tan"), ("\\cot", "cot"),
            ("\\sec", "sec"), ("\\csc", "csc"), ("\\arcsin", "arcsin"), ("\\arccos", "arccos"),
            ("\\arctan", "arctan"), ("\\sinh", "sinh"), ("\\cosh", "cosh"), ("\\tanh", "tanh"),
            ("\\ln", "ln"), ("\\log", "log"), ("\\exp", "exp"), ("\\det", "det"),
            ("\\dim", "dim"), ("\\ker", "ker"), ("\\deg", "deg"), ("\\gcd", "gcd"),
            ("\\lim", "lim"), ("\\max", "max"), ("\\min", "min"), ("\\sup", "sup"),
            ("\\inf", "inf"), ("\\arg", "arg"), ("\\hom", "hom"),

            // Delimiters
            ("\\left(", "("), ("\\right)", ")"),
            ("\\left[", "["), ("\\right]", "]"),
            ("\\left\\{", "{"), ("\\right\\}", "}"),
            ("\\left|", "|"), ("\\right|", "|"),
            ("\\left.", ""), ("\\right.", ""),

            // Spacing
            ("\\quad", "   "), ("\\qquad", "      "), ("\\,", " "), ("\\;", "  "), ("\\!", ""),
        ];

        for (from, to) in replacements {
            text = text.replace(from, to);
        }

        // Combining Accents
        let accents = [
            ("\\hat{", "\u{0302}"),
            ("\\vec{", "\u{20D7}"),
            ("\\bar{", "\u{0304}"),
            ("\\dot{", "\u{0307}"),
            ("\\ddot{", "\u{0308}"),
            ("\\tilde{", "\u{0303}"),
        ];
        for (prefix, mark) in accents {
            while let Some(pos) = text.find(prefix) {
                if let Some(end) = text[pos + prefix.len()..].find('}') {
                    let base = &text[pos + prefix.len()..pos + prefix.len() + end];
                    let combined = format!("{base}{mark}");
                    text.replace_range(pos..pos + prefix.len() + 1 + end, &combined);
                } else {
                    break;
                }
            }
        }

        // Clean up font style wrappers: \mathbf{...}, \mathrm{...}, \mathit{...}, \text{...}
        for prefix in ["\\mathbf{", "\\mathrm{", "\\mathit{", "\\bm{", "\\text{"] {
            while let Some(pos) = text.find(prefix) {
                if let Some(end) = text[pos + prefix.len()..].find('}') {
                    let inner = text[pos + prefix.len()..pos + prefix.len() + end].to_string();
                    text.replace_range(pos..pos + prefix.len() + 1 + end, &inner);
                } else {
                    break;
                }
            }
        }

        // Step 3: Format superscripts and subscripts
        Self::format_scripts(&text)
    }

    /// Renders a display equation centered with optional equation number
    pub fn render_equation_block(
        ui: &mut Ui,
        formula: &str,
        number: Option<usize>,
        is_dark: bool,
    ) {
        let text_color = if is_dark {
            Color32::from_rgb(230, 235, 245)
        } else {
            Color32::from_rgb(25, 30, 40)
        };

        ui.horizontal(|ui| {
            ui.add_space(28.0);

            if let Some((start, end, env, rows)) = Self::parse_matrix_environment(formula) {
                let prefix = formula[..start].trim();
                let suffix = formula[end..].trim();

                if !prefix.is_empty() {
                    let clean_prefix = Self::translate_math_symbols(prefix);
                    ui.label(
                        egui::RichText::new(clean_prefix)
                            .font(FontId::proportional(14.0))
                            .color(text_color),
                    );
                }

                // Render matrix in egui with proper left/right brackets and aligned cells
                let (bracket_left, bracket_right) = match env.as_str() {
                    "pmatrix" => ("(", ")"),
                    "bmatrix" => ("[", "]"),
                    "Bmatrix" => ("{", "}"),
                    "vmatrix" => ("|", "|"),
                    "Vmatrix" => ("‖", "‖"),
                    _ => ("[", "]"),
                };

                let row_count = rows.len();

                // Left delimiter
                ui.label(
                    egui::RichText::new(bracket_left)
                        .font(FontId::proportional(if row_count > 1 { 22.0 } else { 15.0 }))
                        .color(text_color),
                );

                egui::Grid::new(format!("egui_matrix_{}_{:p}", env, formula))
                    .spacing(egui::vec2(16.0, 6.0))
                    .show(ui, |ui| {
                        for row in &rows {
                            for cell in row {
                                ui.vertical_centered(|ui| {
                                    let clean_cell = Self::translate_math_symbols(cell);
                                    ui.label(
                                        egui::RichText::new(clean_cell)
                                            .font(FontId::proportional(13.0))
                                            .italics()
                                            .color(text_color),
                                    );
                                });
                            }
                            ui.end_row();
                        }
                    });

                // Right delimiter
                ui.label(
                    egui::RichText::new(bracket_right)
                        .font(FontId::proportional(if row_count > 1 { 22.0 } else { 15.0 }))
                        .color(text_color),
                );

                if !suffix.is_empty() {
                    let clean_suffix = Self::translate_math_symbols(suffix);
                    ui.label(
                        egui::RichText::new(clean_suffix)
                            .font(FontId::proportional(14.0))
                            .color(text_color),
                    );
                }
            } else {
                let clean_formula = Self::translate_math_symbols(formula);
                ui.label(
                    egui::RichText::new(&clean_formula)
                        .font(FontId::proportional(14.5))
                        .color(text_color),
                );
            }

            if let Some(num) = number {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new(format!("({num})"))
                            .font(FontId::proportional(12.5))
                            .color(Color32::from_gray(140)),
                    );
                });
            }
        });
    }
}
