//! # Native LaTeX Parser
//!
//! Recursive descent parser converting LaTeX source text into a strongly-typed [`LatexAst`].
//! Parses document preambles, sectioning commands, math environments (`equation`, `align`),
//! tabular layouts, lists (`itemize`, `enumerate`), abstracts, and formatted inline spans.

use crate::engine::ast::*;

/// Parser that converts raw LaTeX markup into an in-memory [`LatexAst`].
pub struct NativeLatexParser;

impl NativeLatexParser {
    /// Parses an entire LaTeX document string into [`LatexAst`].
    pub fn parse(source: &str) -> LatexAst {
        let mut ast = LatexAst::default();
        let lines: Vec<&str> = source.lines().collect();
        let mut i = 0;

        let mut section_counter = 0;
        let mut subsection_counter = 0;
        let mut subsubsection_counter = 0;
        let mut equation_counter = 0;
        let mut table_counter = 0;
        let mut figure_counter = 0;

        while i < lines.len() {
            let start_line = i + 1;
            let line = lines[i].trim();

            if line.starts_with('%') {
                i += 1;
                continue;
            }

            if line.starts_with("\\documentclass") {
                if line.contains("letterpaper") || line.contains("letter") {
                    ast.metadata.paper_size = PaperSize::Letter;
                }
                if line.contains("12pt") {
                    ast.metadata.base_font_size = 12.0;
                } else if line.contains("10pt") {
                    ast.metadata.base_font_size = 10.0;
                } else if line.contains("11pt") {
                    ast.metadata.base_font_size = 11.0;
                }
                i += 1;
                continue;
            }

            if line.contains("\\geometry{") {
                if let Some(opts) = Self::extract_brace_content(line, "\\geometry") {
                    let opts_lower = opts.to_lowercase();
                    if opts_lower.contains("letterpaper") || opts_lower.contains("letter") {
                        ast.metadata.paper_size = PaperSize::Letter;
                    } else if opts_lower.contains("a4paper") {
                        ast.metadata.paper_size = PaperSize::A4;
                    }
                    if opts_lower.contains("margin=1in") || opts_lower.contains("margin=72pt") {
                        ast.metadata.margin_pt = 72.0;
                    } else if opts_lower.contains("margin=0.75in") {
                        ast.metadata.margin_pt = 54.0;
                    } else if opts_lower.contains("margin=0.5in") {
                        ast.metadata.margin_pt = 36.0;
                    } else if opts_lower.contains("margin=20mm") {
                        ast.metadata.margin_pt = 56.7;
                    } else if opts_lower.contains("margin=25mm") {
                        ast.metadata.margin_pt = 70.8;
                    } else if opts_lower.contains("margin=15mm") {
                        ast.metadata.margin_pt = 42.5;
                    }
                }
                i += 1;
                continue;
            }

            if line.starts_with("\\PassOptionsToPackage")
                || line.starts_with("\\hypersetup")
                || line.starts_with("\\graphicspath")
                || line.starts_with("\\definecolor")
                || line.starts_with("\\pagestyle")
                || line.starts_with("\\thispagestyle")
            {
                i += 1;
                continue;
            }

            if line.contains("\\title{") {
                if let Some(t) = Self::extract_brace_content(line, "\\title") {
                    ast.metadata.title = Some(Self::clean_inline_str(&t));
                }
                i += 1;
                continue;
            }

            if line.contains("\\author{") {
                if let Some(a) = Self::extract_brace_content(line, "\\author") {
                    ast.metadata.author = Some(Self::clean_inline_str(&a));
                }
                i += 1;
                continue;
            }

            if line.contains("\\date{") {
                if let Some(d) = Self::extract_brace_content(line, "\\date") {
                    ast.metadata.date = Some(Self::clean_inline_str(&d));
                }
                i += 1;
                continue;
            }

            if line.contains("\\usepackage") {
                if let Some(pkg_str) = Self::extract_brace_content(line, "\\usepackage") {
                    for pkg in pkg_str.split(',') {
                        let trimmed = pkg.trim();
                        if !trimmed.is_empty() {
                            ast.metadata.packages.push(trimmed.to_string());
                        }
                    }
                }
                i += 1;
                continue;
            }

            if line.contains("\\maketitle") {
                ast.blocks.push(DocumentBlock::TitleBlock { line: start_line });
                i += 1;
                continue;
            }

            if line.contains("\\begin{abstract}") {
                let mut abstract_text = String::new();
                i += 1;
                while i < lines.len() && !lines[i].contains("\\end{abstract}") {
                    abstract_text.push_str(lines[i]);
                    abstract_text.push('\n');
                    i += 1;
                }
                let spans = Self::parse_inlines(&abstract_text);
                ast.blocks.push(DocumentBlock::Abstract { spans, line: start_line });
                if i < lines.len() {
                    i += 1;
                }
                continue;
            }

            if line.contains("\\section{") || line.contains("\\section*{") {
                let is_starred = line.contains("\\section*");
                if let Some(title) = Self::extract_brace_content(line, "\\section") {
                    let num = if is_starred {
                        String::new()
                    } else {
                        section_counter += 1;
                        subsection_counter = 0;
                        subsubsection_counter = 0;
                        format!("{section_counter}.")
                    };
                    ast.blocks.push(DocumentBlock::Section {
                        level: 1,
                        number: num,
                        title: Self::clean_inline_str(&title),
                        line: start_line,
                    });
                }
                i += 1;
                continue;
            }

            if line.contains("\\subsection{") || line.contains("\\subsection*{") {
                let is_starred = line.contains("\\subsection*");
                if let Some(title) = Self::extract_brace_content(line, "\\subsection") {
                    let num = if is_starred {
                        String::new()
                    } else {
                        subsection_counter += 1;
                        subsubsection_counter = 0;
                        format!("{section_counter}.{subsection_counter}")
                    };
                    ast.blocks.push(DocumentBlock::Section {
                        level: 2,
                        number: num,
                        title: Self::clean_inline_str(&title),
                        line: start_line,
                    });
                }
                i += 1;
                continue;
            }

            if line.contains("\\subsubsection{") {
                if let Some(title) = Self::extract_brace_content(line, "\\subsubsection") {
                    subsubsection_counter += 1;
                    let num = format!("{section_counter}.{subsection_counter}.{subsubsection_counter}");
                    ast.blocks.push(DocumentBlock::Section {
                        level: 3,
                        number: num,
                        title: Self::clean_inline_str(&title),
                        line: start_line,
                    });
                }
                i += 1;
                continue;
            }

            if line.contains("\\begin{equation")
                || line.starts_with("\\[")
                || (line.starts_with("$$") && (!line[2..].contains("$$") || line.ends_with("$$")))
                || line.starts_with("\\begin{pmatrix}")
                || line.starts_with("\\begin{bmatrix}")
                || line.starts_with("\\begin{Bmatrix}")
                || line.starts_with("\\begin{vmatrix}")
                || line.starts_with("\\begin{matrix}")
            {
                let is_starred = line.contains('*') || line.starts_with("\\[") || line.starts_with("$$");
                let end_marker = if line.starts_with("\\[") {
                    "\\]"
                } else if line.starts_with("$$") {
                    "$$"
                } else if line.starts_with("\\begin{pmatrix}") {
                    "\\end{pmatrix}"
                } else if line.starts_with("\\begin{bmatrix}") {
                    "\\end{bmatrix}"
                } else if line.starts_with("\\begin{Bmatrix}") {
                    "\\end{Bmatrix}"
                } else if line.starts_with("\\begin{vmatrix}") {
                    "\\end{vmatrix}"
                } else if line.starts_with("\\begin{matrix}") {
                    "\\end{matrix}"
                } else {
                    "\\end{equation"
                };

                let mut formula = String::new();
                let mut label = None;

                // Handle single-line display math e.g. \[ E = mc^2 \] or $$ E = mc^2 $$
                if (line.starts_with("\\[") && line.ends_with("\\]") && line.len() >= 4)
                    || (line.starts_with("$$") && line[2..].contains("$$") && line.len() >= 4)
                {
                    let inner = if line.starts_with("\\[") {
                        &line[2..line.len() - 2]
                    } else {
                        &line[2..line.len() - 2]
                    };
                    formula = inner.trim().to_string();
                } else {
                    if line.starts_with("\\begin{") && !line.starts_with("\\begin{equation") {
                        formula.push_str(line);
                        formula.push(' ');
                    } else if line.starts_with("\\[") && line.len() > 2 {
                        formula.push_str(&line[2..]);
                        formula.push(' ');
                    } else if line.starts_with("$$") && line.len() > 2 {
                        formula.push_str(&line[2..]);
                        formula.push(' ');
                    }

                    i += 1;
                    while i < lines.len() && !lines[i].contains(end_marker) {
                        let eq_line = lines[i].trim();
                        if eq_line.contains("\\label{") {
                            label = Self::extract_brace_content(eq_line, "\\label");
                        } else {
                            formula.push_str(eq_line);
                            formula.push(' ');
                        }
                        i += 1;
                    }

                    // Include end line if it contains closing matrix tags
                    if i < lines.len() && end_marker.starts_with("\\end{") {
                        formula.push_str(end_marker);
                    }
                }

                let num = if is_starred {
                    None
                } else {
                    equation_counter += 1;
                    Some(equation_counter)
                };
                ast.blocks.push(DocumentBlock::EquationBlock {
                    formula: formula.trim().to_string(),
                    number: num,
                    label,
                    line: start_line,
                });
                if i < lines.len() {
                    i += 1;
                }
                continue;
            }

            if line.contains("\\begin{align}") || line.contains("\\begin{align*}") {
                let mut eq_lines = Vec::new();
                i += 1;
                while i < lines.len() && !lines[i].contains("\\end{align") {
                    let l = lines[i].trim().trim_end_matches('\\').trim();
                    if !l.is_empty() && !l.starts_with('%') {
                        eq_lines.push(l.to_string());
                    }
                    i += 1;
                }
                equation_counter += 1;
                ast.blocks.push(DocumentBlock::AlignBlock {
                    equations: eq_lines,
                    start_number: equation_counter,
                    line: start_line,
                });
                if i < lines.len() {
                    i += 1;
                }
                continue;
            }

            if line.contains("\\begin{itemize}") || line.contains("\\begin{enumerate}") {
                let is_ordered = line.contains("\\begin{enumerate}");
                let end_marker = if is_ordered { "\\end{enumerate}" } else { "\\end{itemize}" };
                let mut items = Vec::new();
                let mut current_item = String::new();
                i += 1;

                while i < lines.len() && !lines[i].contains(end_marker) {
                    let item_line = lines[i].trim();
                    if let Some(pos) = item_line.find("\\item") {
                        if !current_item.trim().is_empty() {
                            items.push(Self::parse_inlines(current_item.trim()));
                            current_item.clear();
                        }
                        current_item.push_str(&item_line[pos + 5..]);
                    } else {
                        current_item.push(' ');
                        current_item.push_str(item_line);
                    }
                    i += 1;
                }
                if !current_item.trim().is_empty() {
                    items.push(Self::parse_inlines(current_item.trim()));
                }
                ast.blocks.push(DocumentBlock::List {
                    ordered: is_ordered,
                    items,
                    line: start_line,
                });
                if i < lines.len() {
                    i += 1;
                }
                continue;
            }

            if line.contains("\\begin{minted}")
                || line.contains("\\begin{lstlisting}")
                || line.contains("\\begin{verbatim}")
            {
                let (end_marker, lang) = if line.contains("\\begin{minted}") {
                    let lang = Self::extract_brace_content(line, "\\begin{minted}")
                        .unwrap_or_else(|| "code".to_string());
                    ("\\end{minted}", lang)
                } else if line.contains("\\begin{lstlisting}") {
                    let lang = if line.contains("language=") {
                        line.split("language=")
                            .nth(1)
                            .and_then(|s| s.split(|c: char| c == ',' || c == ']' || c == '}').next())
                            .unwrap_or("code")
                            .trim()
                            .to_string()
                    } else {
                        "code".to_string()
                    };
                    ("\\end{lstlisting}", lang)
                } else {
                    ("\\end{verbatim}", "verbatim".to_string())
                };

                let mut code_lines = Vec::new();
                i += 1;
                while i < lines.len() && !lines[i].contains(end_marker) {
                    code_lines.push(lines[i]);
                    i += 1;
                }
                if i < lines.len() {
                    i += 1;
                }

                ast.blocks.push(DocumentBlock::CodeBlock {
                    language: lang,
                    code: code_lines.join("\n"),
                    line: start_line,
                });
                continue;
            }

            if line.contains("\\begin{figure}") {
                let mut caption = None;
                let mut label = None;
                let mut image_path = None;
                i += 1;
                while i < lines.len() && !lines[i].contains("\\end{figure}") {
                    let fig_line = lines[i].trim();
                    if fig_line.contains("\\caption{") {
                        caption = Self::extract_brace_content(fig_line, "\\caption");
                    } else if fig_line.contains("\\label{") {
                        label = Self::extract_brace_content(fig_line, "\\label");
                    } else if fig_line.contains("\\includegraphics") {
                        image_path = Self::extract_brace_content(fig_line, "\\includegraphics");
                    }
                    i += 1;
                }
                if i < lines.len() {
                    i += 1;
                }
                figure_counter += 1;
                ast.blocks.push(DocumentBlock::Figure {
                    caption,
                    label,
                    image_path,
                    number: figure_counter,
                    line: start_line,
                });
                continue;
            }

            if line.contains("\\begin{table}") || line.contains("\\begin{tabular}") {
                let mut caption = None;
                let mut label = None;
                let mut headers = Vec::new();
                let mut rows = Vec::new();
                let mut has_booktabs = false;

                while i < lines.len() {
                    let t_line = lines[i].trim();
                    if t_line.contains("\\toprule")
                        || t_line.contains("\\midrule")
                        || t_line.contains("\\bottomrule")
                        || t_line.contains("\\cmidrule")
                    {
                        has_booktabs = true;
                    }

                    if t_line.contains("\\caption{") {
                        caption = Self::extract_brace_content(t_line, "\\caption");
                    } else if t_line.contains("\\label{") {
                        label = Self::extract_brace_content(t_line, "\\label");
                    } else if t_line.contains('&') {
                        let cells_str: Vec<&str> = t_line
                            .trim_end_matches('\\')
                            .split('&')
                            .collect();

                        let is_header = headers.is_empty() && (t_line.contains("\\textbf") || rows.is_empty());

                        if is_header && headers.is_empty() {
                            headers = cells_str
                                .iter()
                                .map(|s| Self::clean_inline_str(&Self::clean_cell_content(s)))
                                .collect();
                        } else {
                            let cell_spans: Vec<Vec<InlineSpan>> = cells_str
                                .iter()
                                .map(|s| Self::parse_inlines(&Self::clean_cell_content(s)))
                                .collect();
                            rows.push(TableRow {
                                cells: cell_spans,
                                is_header,
                            });
                        }
                    }

                    if t_line.contains("\\end{table}") || (t_line.contains("\\end{tabular}") && !lines.get(i + 1).map(|l| l.contains("\\end{table}")).unwrap_or(false)) {
                        i += 1;
                        break;
                    }
                    i += 1;
                }

                table_counter += 1;
                ast.blocks.push(DocumentBlock::Table {
                    caption: caption.or_else(|| Some(format!("Table {table_counter}"))),
                    label,
                    headers,
                    rows,
                    has_booktabs,
                    line: start_line,
                });
                continue;
            }

            if !line.is_empty()
                && (!line.starts_with('\\')
                || (line.starts_with('\\') && !line.starts_with("\\begin") && !line.starts_with("\\end") && !line.starts_with("\\documentclass")))
            {
                let mut para_text = line.to_string();
                i += 1;
                while i < lines.len() {
                    let next_line = lines[i].trim();
                    if next_line.is_empty() || next_line.starts_with('\\') {
                        break;
                    }
                    para_text.push(' ');
                    para_text.push_str(next_line);
                    i += 1;
                }
                let spans = Self::parse_inlines(&para_text);
                if !spans.is_empty() {
                    ast.blocks.push(DocumentBlock::Paragraph { spans, line: start_line });
                }
                continue;
            }

            i += 1;
        }

        ast
    }

    pub fn extract_brace_content(line: &str, command: &str) -> Option<String> {
        if let Some(pos) = line.find(command) {
            let rest = &line[pos + command.len()..];
            let mut rest = rest.trim_start_matches('*').trim_start();
            if rest.starts_with('[') {
                if let Some(bracket_end) = rest.find(']') {
                    rest = rest[bracket_end + 1..].trim_start();
                }
            }
            if rest.starts_with('{') {
                if let Some(end) = rest.find('}') {
                    return Some(rest[1..end].trim().to_string());
                }
            }
        }
        None
    }

    pub fn parse_inlines(input: &str) -> Vec<InlineSpan> {
        let mut spans = Vec::new();
        let mut chars = input.chars().peekable();
        let mut buffer = String::new();

        while let Some(c) = chars.next() {
            if c == '\\' {
                let mut cmd = String::new();
                while let Some(&next_c) = chars.peek() {
                    if next_c.is_alphabetic() {
                        cmd.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                if matches!(cmd.as_str(), "textbf" | "textit" | "emph" | "texttt") {
                    if !buffer.is_empty() {
                        spans.push(InlineSpan::Text(buffer.clone()));
                        buffer.clear();
                    }
                    let inner = Self::extract_braced_arg(&mut chars);
                    match cmd.as_str() {
                        "textbf" => spans.push(InlineSpan::Bold(inner)),
                        "textit" | "emph" => spans.push(InlineSpan::Italic(inner)),
                        "texttt" => spans.push(InlineSpan::Code(inner)),
                        _ => spans.push(InlineSpan::Text(inner)),
                    }
                    continue;
                } else if cmd == "textcolor" {
                    if !buffer.is_empty() {
                        spans.push(InlineSpan::Text(buffer.clone()));
                        buffer.clear();
                    }
                    let _opt = Self::skip_optional_bracket(&mut chars);
                    let color_name = Self::extract_braced_arg(&mut chars);
                    let text = Self::extract_braced_arg(&mut chars);
                    let color = Self::parse_color_name(&color_name);
                    spans.push(InlineSpan::Colored(color, text));
                    continue;
                } else if cmd == "colorbox" {
                    if !buffer.is_empty() {
                        spans.push(InlineSpan::Text(buffer.clone()));
                        buffer.clear();
                    }
                    let color_name = Self::extract_braced_arg(&mut chars);
                    let text = Self::extract_braced_arg(&mut chars);
                    let color = Self::parse_color_name(&color_name);
                    spans.push(InlineSpan::Colored(color, text));
                    continue;
                } else if cmd == "mintinline" {
                    if !buffer.is_empty() {
                        spans.push(InlineSpan::Text(buffer.clone()));
                        buffer.clear();
                    }
                    let _opt = Self::skip_optional_bracket(&mut chars);
                    let _lang = Self::extract_braced_arg(&mut chars);
                    while chars.peek() == Some(&' ') {
                        chars.next();
                    }
                    let code = if chars.peek() == Some(&'{') {
                        Self::extract_braced_arg(&mut chars)
                    } else if let Some(delim) = chars.next() {
                        let mut code_buf = String::new();
                        while let Some(c) = chars.next() {
                            if c == delim {
                                break;
                            }
                            code_buf.push(c);
                        }
                        code_buf
                    } else {
                        String::new()
                    };
                    spans.push(InlineSpan::Code(code));
                    continue;
                } else if cmd == "lstinline" {
                    if !buffer.is_empty() {
                        spans.push(InlineSpan::Text(buffer.clone()));
                        buffer.clear();
                    }
                    let _opt = Self::skip_optional_bracket(&mut chars);
                    let code = Self::extract_braced_arg(&mut chars);
                    spans.push(InlineSpan::Code(code));
                    continue;
                } else if cmd == "SI" || cmd == "qty" {
                    if !buffer.is_empty() {
                        spans.push(InlineSpan::Text(buffer.clone()));
                        buffer.clear();
                    }
                    let _opt = Self::skip_optional_bracket(&mut chars);
                    let val = Self::extract_braced_arg(&mut chars);
                    let unit = Self::extract_braced_arg(&mut chars);
                    let val_fmt = Self::format_siunitx_number(&val);
                    let unit_fmt = Self::format_siunitx_unit(&unit);
                    spans.push(InlineSpan::Text(format!("{val_fmt} {unit_fmt}")));
                    continue;
                } else if cmd == "si" || cmd == "unit" {
                    if !buffer.is_empty() {
                        spans.push(InlineSpan::Text(buffer.clone()));
                        buffer.clear();
                    }
                    let _opt = Self::skip_optional_bracket(&mut chars);
                    let unit = Self::extract_braced_arg(&mut chars);
                    let unit_fmt = Self::format_siunitx_unit(&unit);
                    spans.push(InlineSpan::Text(unit_fmt));
                    continue;
                } else if cmd == "num" {
                    if !buffer.is_empty() {
                        spans.push(InlineSpan::Text(buffer.clone()));
                        buffer.clear();
                    }
                    let _opt = Self::skip_optional_bracket(&mut chars);
                    let num = Self::extract_braced_arg(&mut chars);
                    let num_fmt = Self::format_siunitx_number(&num);
                    spans.push(InlineSpan::Text(num_fmt));
                    continue;
                } else if cmd == "ang" {
                    if !buffer.is_empty() {
                        spans.push(InlineSpan::Text(buffer.clone()));
                        buffer.clear();
                    }
                    let _opt = Self::skip_optional_bracket(&mut chars);
                    let angle = Self::extract_braced_arg(&mut chars);
                    spans.push(InlineSpan::Text(format!("{angle}°")));
                    continue;
                } else if cmd == "ref" {
                    let target = Self::extract_braced_arg(&mut chars);
                    spans.push(InlineSpan::Ref(target));
                    continue;
                } else if cmd == "cite" {
                    let target = Self::extract_braced_arg(&mut chars);
                    spans.push(InlineSpan::Cite(target));
                    continue;
                } else if cmd == "url" {
                    let u = Self::extract_braced_arg(&mut chars);
                    spans.push(InlineSpan::Url(u));
                    continue;
                } else if cmd == "href" {
                    let u = Self::extract_braced_arg(&mut chars);
                    let t = Self::extract_braced_arg(&mut chars);
                    spans.push(InlineSpan::Url(if t.is_empty() { u } else { t }));
                    continue;
                } else if matches!(cmd.as_str(), "rule" | "vspace" | "hspace" | "label" | "index") {
                    let _ = Self::skip_optional_bracket(&mut chars);
                    let _ = Self::extract_braced_arg(&mut chars);
                    continue;
                } else if cmd == "LaTeX" {
                    buffer.push_str("LaTeX");
                    continue;
                } else if cmd == "TeX" {
                    buffer.push_str("TeX");
                    continue;
                } else {
                    buffer.push('\\');
                    buffer.push_str(&cmd);
                    continue;
                }
            }

            if c == '$' {
                if !buffer.is_empty() {
                    spans.push(InlineSpan::Text(buffer.clone()));
                    buffer.clear();
                }
                let mut math_content = String::new();
                while let Some(m_c) = chars.next() {
                    if m_c == '$' {
                        break;
                    }
                    math_content.push(m_c);
                }
                spans.push(InlineSpan::Math(math_content));
                continue;
            }

            buffer.push(c);
        }

        if !buffer.is_empty() {
            spans.push(InlineSpan::Text(buffer));
        }

        spans
    }

    pub fn clean_inline_str(s: &str) -> String {
        s.replace("\\textbf{", "")
            .replace("\\textit{", "")
            .replace("\\emph{", "")
            .replace("\\texttt{", "")
            .replace("\\LaTeX", "LaTeX")
            .replace("\\TeX", "TeX")
            .replace('}', "")
            .replace('{', "")
            .replace('\\', "")
            .trim()
            .to_string()
    }

    pub fn extract_braced_arg(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
        while chars.peek() == Some(&' ') {
            chars.next();
        }
        if chars.peek() != Some(&'{') {
            return String::new();
        }
        chars.next(); // consume '{'
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

    pub fn skip_optional_bracket(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<String> {
        while chars.peek() == Some(&' ') {
            chars.next();
        }
        if chars.peek() != Some(&'[') {
            return None;
        }
        chars.next(); // consume '['
        let mut opt = String::new();
        while let Some(c) = chars.next() {
            if c == ']' {
                break;
            }
            opt.push(c);
        }
        Some(opt)
    }

    pub fn clean_cell_content(cell: &str) -> String {
        let mut s = cell.trim().to_string();
        if let Some(pos) = s.find("\\multirow") {
            if let Some(open) = s[pos..].rfind('{') {
                if let Some(close) = s[pos + open..].find('}') {
                    let inner = s[pos + open + 1..pos + open + close].to_string();
                    s = inner;
                }
            }
        }
        if let Some(pos) = s.find("\\multicolumn") {
            if let Some(open) = s[pos..].rfind('{') {
                if let Some(close) = s[pos + open..].find('}') {
                    let inner = s[pos + open + 1..pos + open + close].to_string();
                    s = inner;
                }
            }
        }
        s
    }

    pub fn format_siunitx_unit(unit_raw: &str) -> String {
        let mut s = unit_raw.to_string();
        let replacements = [
            ("\\per\\second\\squared", "/s²"),
            ("\\per\\second", "/s"),
            ("\\per\\meter", "/m"),
            ("\\per\\hour", "/h"),
            ("\\meter\\per\\second", "m/s"),
            ("\\kilo\\gram", "kg"),
            ("\\meter\\squared", "m²"),
            ("\\meter\\cubed", "m³"),
            ("\\degreeCelsius", "°C"),
            ("\\degreeC", "°C"),
            ("\\degree", "°"),
            ("\\celsius", "°C"),
            ("\\percent", "%"),
            ("\\meter", "m"),
            ("\\second", "s"),
            ("\\gram", "g"),
            ("\\kilo", "k"),
            ("\\mega", "M"),
            ("\\giga", "G"),
            ("\\micro", "µ"),
            ("\\milli", "m"),
            ("\\centi", "c"),
            ("\\nano", "n"),
            ("\\newton", "N"),
            ("\\joule", "J"),
            ("\\watt", "W"),
            ("\\pascal", "Pa"),
            ("\\hertz", "Hz"),
            ("\\volt", "V"),
            ("\\ohm", "Ω"),
            ("\\liter", "L"),
            ("\\squared", "²"),
            ("\\cubed", "³"),
            ("\\per", "/"),
        ];
        for (from, to) in replacements {
            s = s.replace(from, to);
        }
        s.replace('{', "").replace('}', "").replace('\\', "").trim().to_string()
    }

    pub fn format_siunitx_number(num_raw: &str) -> String {
        let trimmed = num_raw.trim().replace('{', "").replace('}', "");
        if let Some(pos) = trimmed.find(|c: char| c == 'e' || c == 'E') {
            let base = &trimmed[..pos];
            let exp = &trimmed[pos + 1..];
            format!("{base} × 10^{exp}")
        } else {
            trimmed
        }
    }

    pub fn parse_color_name(name: &str) -> egui::Color32 {
        match name.trim().to_lowercase().as_str() {
            "red" => egui::Color32::from_rgb(220, 50, 50),
            "green" => egui::Color32::from_rgb(46, 160, 67),
            "blue" => egui::Color32::from_rgb(33, 110, 240),
            "cyan" => egui::Color32::from_rgb(0, 180, 216),
            "magenta" => egui::Color32::from_rgb(214, 51, 132),
            "yellow" => egui::Color32::from_rgb(230, 180, 0),
            "orange" => egui::Color32::from_rgb(240, 120, 20),
            "purple" => egui::Color32::from_rgb(138, 43, 226),
            "pink" => egui::Color32::from_rgb(255, 105, 180),
            "teal" => egui::Color32::from_rgb(32, 178, 170),
            "violet" => egui::Color32::from_rgb(148, 0, 211),
            "gray" | "grey" => egui::Color32::from_rgb(128, 128, 128),
            "darkgray" => egui::Color32::from_rgb(80, 80, 80),
            "lightgray" => egui::Color32::from_rgb(200, 200, 200),
            "black" => egui::Color32::from_rgb(15, 15, 15),
            "white" => egui::Color32::from_rgb(250, 250, 250),
            "brown" => egui::Color32::from_rgb(139, 69, 19),
            "lime" => egui::Color32::from_rgb(50, 205, 50),
            "olive" => egui::Color32::from_rgb(128, 128, 0),
            _ => egui::Color32::from_rgb(20, 20, 20),
        }
    }
}
