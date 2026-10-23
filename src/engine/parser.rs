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
        let mut is_appendix = false;
        let has_doc_env = source.contains("\\begin{document}");
        let mut in_document = !has_doc_env;

        while i < lines.len() {
            let start_line = i + 1;
            let line = lines[i].trim();

            if line.starts_with('%') {
                i += 1;
                continue;
            }

            // Preamble metadata parsing
            if !in_document {
                if line.contains("\\begin{document}") {
                    in_document = true;
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

                if line.contains("\\title{") {
                    if let Some(t) = Self::extract_braced_content_multiline(&lines, &mut i, "\\title") {
                        ast.metadata.title = Some(Self::clean_inline_str(&t));
                    }
                    i += 1;
                    continue;
                }

                if line.contains("\\author{") {
                    if let Some(a) = Self::extract_braced_content_multiline(&lines, &mut i, "\\author") {
                        ast.metadata.author = Some(Self::clean_inline_str(&a));
                    }
                    i += 1;
                    continue;
                }

                if line.contains("\\date{") {
                    if let Some(d) = Self::extract_braced_content_multiline(&lines, &mut i, "\\date") {
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

                // If line contains preamble setup macros like \hypersetup{...}, consume across lines
                if line.starts_with('\\') {
                    let cmd = line[1..].split(|c: char| !c.is_alphabetic() && c != '*').next().unwrap_or("");
                    if matches!(
                        cmd,
                        "hypersetup"
                            | "pgfplotsset"
                            | "tikzset"
                            | "sisetup"
                            | "captionsetup"
                            | "definecolor"
                            | "PassOptionsToPackage"
                            | "graphicspath"
                            | "pagestyle"
                            | "thispagestyle"
                            | "newcommand"
                            | "renewcommand"
                            | "providecommand"
                            | "def"
                            | "let"
                            | "setlength"
                            | "setcounter"
                            | "usetikzlibrary"
                            | "usepgfplotslibrary"
                    ) {
                        Self::consume_multiline_braces(&lines, &mut i);
                        continue;
                    }
                }

                // Preamble non-command lines are silently ignored
                i += 1;
                continue;
            }

            // Once in document body: terminate at \end{document}
            if line.contains("\\end{document}") {
                break;
            }

            // In-document setup and config directives to consume silently
            if line.starts_with('\\') {
                let cmd = line[1..].split(|c: char| !c.is_alphabetic() && c != '*').next().unwrap_or("");
                if matches!(
                    cmd,
                    "setlength"
                        | "setcounter"
                        | "addtocounter"
                        | "stepcounter"
                        | "pagenumbering"
                        | "pagestyle"
                        | "thispagestyle"
                        | "hypersetup"
                        | "pgfplotsset"
                        | "tikzset"
                        | "sisetup"
                        | "captionsetup"
                        | "newcolumntype"
                        | "newcommand"
                        | "renewcommand"
                        | "providecommand"
                        | "def"
                        | "let"
                        | "bibliographystyle"
                        | "bibliography"
                        | "usetikzlibrary"
                        | "usepgfplotslibrary"
                        | "setmainfont"
                        | "input"
                        | "include"
                ) {
                    Self::consume_multiline_braces(&lines, &mut i);
                    continue;
                }
            }

            if line.contains("\\maketitle") {
                ast.blocks.push(DocumentBlock::TitleBlock { line: start_line });
                i += 1;
                continue;
            }

            if line.starts_with("\\tableofcontents") {
                ast.blocks.push(DocumentBlock::TableOfContents { line: start_line });
                i += 1;
                continue;
            }

            if line.starts_with("\\appendix") {
                is_appendix = true;
                section_counter = 0;
                subsection_counter = 0;
                subsubsection_counter = 0;
                i += 1;
                continue;
            }

            if line.starts_with("\\newpage") || line.starts_with("\\clearpage") || line.starts_with("\\pagebreak") {
                ast.blocks.push(DocumentBlock::PageBreak { line: start_line });
                i += 1;
                continue;
            }

            if line.starts_with("\\vspace") {
                let pt = Self::extract_brace_content(line, "\\vspace")
                    .and_then(|dim| Self::parse_dimension_pt(&dim))
                    .unwrap_or(12.0);
                ast.blocks.push(DocumentBlock::VerticalSpace { pt, line: start_line });
                i += 1;
                continue;
            }

            if line.starts_with("\\bigskip") {
                ast.blocks.push(DocumentBlock::VerticalSpace { pt: 12.0, line: start_line });
                i += 1;
                continue;
            }

            if line.starts_with("\\medskip") {
                ast.blocks.push(DocumentBlock::VerticalSpace { pt: 6.0, line: start_line });
                i += 1;
                continue;
            }

            if line.starts_with("\\smallskip") {
                ast.blocks.push(DocumentBlock::VerticalSpace { pt: 3.0, line: start_line });
                i += 1;
                continue;
            }

            if line.starts_with("\\hrule") {
                ast.blocks.push(DocumentBlock::HorizontalRule { line: start_line });
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

            if line.starts_with("\\begin{center}") {
                let mut center_text = String::new();
                let mut has_img = false;
                let mut img_path = None;
                i += 1;
                while i < lines.len() && !lines[i].contains("\\end{center}") {
                    let c_line = lines[i].trim();
                    if c_line.contains("\\includegraphics") {
                        has_img = true;
                        img_path = Self::extract_brace_content(c_line, "\\includegraphics");
                    }
                    if !c_line.is_empty() && !c_line.starts_with('%') {
                        center_text.push_str(c_line);
                        center_text.push(' ');
                    }
                    i += 1;
                }
                if has_img {
                    figure_counter += 1;
                    ast.blocks.push(DocumentBlock::Figure {
                        caption: None,
                        label: None,
                        image_path: img_path,
                        number: figure_counter,
                        line: start_line,
                    });
                } else {
                    let spans = Self::parse_inlines(center_text.trim());
                    ast.blocks.push(DocumentBlock::CenteredBlock { spans, line: start_line });
                }
                if i < lines.len() {
                    i += 1;
                }
                continue;
            }

            if line.starts_with("\\begin{quote}") || line.starts_with("\\begin{quotation}") {
                let end_marker = if line.starts_with("\\begin{quotation}") { "\\end{quotation}" } else { "\\end{quote}" };
                let mut quote_text = String::new();
                i += 1;
                while i < lines.len() && !lines[i].contains(end_marker) {
                    let q_line = lines[i].trim();
                    if !q_line.is_empty() && !q_line.starts_with('%') {
                        quote_text.push_str(q_line);
                        quote_text.push(' ');
                    }
                    i += 1;
                }
                let spans = Self::parse_inlines(quote_text.trim());
                ast.blocks.push(DocumentBlock::QuoteBlock { spans, line: start_line });
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
                        if is_appendix {
                            let letter = ((b'A' as usize + section_counter - 1) as u8) as char;
                            format!("{letter}.")
                        } else {
                            format!("{section_counter}.")
                        }
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
                        if is_appendix {
                            let letter = ((b'A' as usize + section_counter.max(1) - 1) as u8) as char;
                            format!("{letter}.{subsection_counter}")
                        } else {
                            format!("{section_counter}.{subsection_counter}")
                        }
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
                    let num = if is_appendix {
                        let letter = ((b'A' as usize + section_counter.max(1) - 1) as u8) as char;
                        format!("{letter}.{subsection_counter}.{subsubsection_counter}")
                    } else {
                        format!("{section_counter}.{subsection_counter}.{subsubsection_counter}")
                    };
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

            if line.contains("\\paragraph{") || line.contains("\\paragraph*{") {
                if let Some(title) = Self::extract_brace_content(line, "\\paragraph") {
                    ast.blocks.push(DocumentBlock::Section {
                        level: 4,
                        number: String::new(),
                        title: Self::clean_inline_str(&title),
                        line: start_line,
                    });
                }
                i += 1;
                continue;
            }

            if line.contains("\\subparagraph{") || line.contains("\\subparagraph*{") {
                if let Some(title) = Self::extract_brace_content(line, "\\subparagraph") {
                    ast.blocks.push(DocumentBlock::Section {
                        level: 5,
                        number: String::new(),
                        title: Self::clean_inline_str(&title),
                        line: start_line,
                    });
                }
                i += 1;
                continue;
            }

            if line.contains("\\begin{tikzpicture}")
                || line.contains("\\begin{axis}")
                || line.contains("\\begin{semilogxaxis}")
                || line.contains("\\begin{semilogyaxis}")
                || line.contains("\\begin{loglogaxis}")
                || line.contains("\\begin{circuitikz}")
            {
                let env_name = if line.contains("semilogxaxis") {
                    "semilogxaxis"
                } else if line.contains("semilogyaxis") {
                    "semilogyaxis"
                } else if line.contains("loglogaxis") {
                    "loglogaxis"
                } else if line.contains("axis") {
                    "axis"
                } else if line.contains("circuitikz") {
                    "circuitikz"
                } else {
                    "tikzpicture"
                };
                let end_marker = format!("\\end{{{env_name}}}");

                let mut plot_title = None;
                i += 1;
                while i < lines.len() && !lines[i].contains(&end_marker) {
                    let p_line = lines[i].trim();
                    if p_line.contains("title={") {
                        plot_title = Self::extract_brace_content(p_line, "title");
                    }
                    i += 1;
                }
                if i < lines.len() {
                    i += 1;
                }

                figure_counter += 1;
                let plot_label = plot_title.unwrap_or_else(|| format!("Vector Plot ({env_name})"));
                ast.blocks.push(DocumentBlock::Figure {
                    caption: Some(plot_label),
                    label: None,
                    image_path: Some(format!("Vector Diagram ({env_name})")),
                    number: figure_counter,
                    line: start_line,
                });
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

            if line.contains("\\begin{figure}") || line.contains("\\begin{figure*}") {
                let mut caption = None;
                let mut label = None;
                let mut image_path = None;
                i += 1;
                while i < lines.len() && !lines[i].contains("\\end{figure") {
                    let fig_line = lines[i].trim();
                    if fig_line.contains("\\caption{") {
                        caption = Self::extract_brace_content(fig_line, "\\caption");
                    } else if fig_line.contains("\\label{") {
                        label = Self::extract_brace_content(fig_line, "\\label");
                    } else if fig_line.contains("\\includegraphics") {
                        image_path = Self::extract_brace_content(fig_line, "\\includegraphics");
                    } else if fig_line.contains("\\begin{tikzpicture}")
                        || fig_line.contains("\\begin{semilogxaxis}")
                        || fig_line.contains("\\begin{axis}")
                        || fig_line.contains("\\begin{circuitikz}")
                    {
                        if image_path.is_none() {
                            image_path = Some("Vector Graphic / Plot".to_string());
                        }
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

            if line.contains("\\includegraphics") {
                let image_path = Self::extract_brace_content(line, "\\includegraphics");
                figure_counter += 1;
                ast.blocks.push(DocumentBlock::Figure {
                    caption: None,
                    label: None,
                    image_path,
                    number: figure_counter,
                    line: start_line,
                });
                i += 1;
                continue;
            }

            if line.contains("\\begin{table}") || line.contains("\\begin{tabular}") {
                let mut caption = None;
                let mut label = None;
                let mut headers = Vec::new();
                let mut rows = Vec::new();
                let mut has_booktabs = false;
                let mut alignments = Vec::new();

                while i < lines.len() {
                    let t_line = lines[i].trim();
                    if t_line.contains("\\begin{tabular") {
                        if let Some(spec) = Self::extract_brace_content(t_line, "\\begin{tabular") {
                            alignments = Self::parse_column_alignments(&spec);
                        }
                    }

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
                    alignments,
                    line: start_line,
                });
                continue;
            }

            let mut is_noindent = false;
            let mut cur_line = line;
            if cur_line.starts_with("\\noindent") {
                is_noindent = true;
                cur_line = cur_line["\\noindent".len()..].trim_start();
            }
            if cur_line.starts_with("\\centering") {
                cur_line = cur_line["\\centering".len()..].trim_start();
            }

            if !cur_line.is_empty() && !Self::is_block_command(cur_line) {
                let mut para_text = cur_line.to_string();
                i += 1;
                while i < lines.len() {
                    let next_line = lines[i].trim();
                    if next_line.is_empty() || Self::is_block_command(next_line) {
                        break;
                    }
                    para_text.push(' ');
                    para_text.push_str(next_line);
                    i += 1;
                }
                let spans = Self::parse_inlines(&para_text);
                if !spans.is_empty() {
                    ast.blocks.push(DocumentBlock::Paragraph {
                        spans,
                        is_noindent,
                        line: start_line,
                    });
                }
                continue;
            }

            i += 1;
        }

        ast
    }

    /// Extracts the contents inside curly braces `{...}` following a specified command name.
    /// Skips optional bracketed arguments `[...]` if present.
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

    /// Extracts content inside balanced curly braces `{...}` across multiple lines if needed.
    pub fn extract_braced_content_multiline(
        lines: &[&str],
        i: &mut usize,
        command: &str,
    ) -> Option<String> {
        let first_line = lines.get(*i)?;
        let start_pos = first_line.find(command)?;
        let rest = &first_line[start_pos + command.len()..];
        let mut rest_trimmed = rest.trim_start_matches('*').trim_start();
        if rest_trimmed.starts_with('[') {
            if let Some(bracket_end) = rest_trimmed.find(']') {
                rest_trimmed = rest_trimmed[bracket_end + 1..].trim_start();
            }
        }

        let mut depth = 0;
        let mut content = String::new();
        let mut found_open = false;

        for c in rest_trimmed.chars() {
            if c == '{' {
                if found_open {
                    content.push(c);
                }
                depth += 1;
                found_open = true;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    return Some(content);
                }
                content.push(c);
            } else if found_open {
                content.push(c);
            }
        }

        if !found_open {
            return None;
        }

        // Continue onto subsequent lines if depth > 0
        content.push(' ');
        while depth > 0 && *i + 1 < lines.len() {
            *i += 1;
            let next_line = lines[*i];
            for c in next_line.chars() {
                if c == '{' {
                    depth += 1;
                    content.push(c);
                } else if c == '}' {
                    depth -= 1;
                    if depth == 0 {
                        return Some(content);
                    }
                    content.push(c);
                } else {
                    content.push(c);
                }
            }
            content.push(' ');
        }

        Some(content)
    }

    /// Consumes a command and its balanced arguments `{...}` across one or more lines.
    pub fn consume_multiline_braces(lines: &[&str], i: &mut usize) {
        let first_line = match lines.get(*i) {
            Some(l) => *l,
            None => return,
        };

        let mut depth: usize = 0;
        let mut found_open = false;

        for c in first_line.chars() {
            if c == '{' {
                depth += 1;
                found_open = true;
            } else if c == '}' {
                depth = depth.saturating_sub(1);
            }
        }

        if !found_open || depth == 0 {
            *i += 1;
            return;
        }

        while depth > 0 && *i + 1 < lines.len() {
            *i += 1;
            for c in lines[*i].chars() {
                if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth = depth.saturating_sub(1);
                }
            }
        }
        *i += 1;
    }

    /// Parses an inline LaTeX text string into formatted [`InlineSpan`] tokens,
    /// supporting bold, italic, inline math (`$...$`), code, links, and symbols.
    pub fn parse_inlines(input: &str) -> Vec<InlineSpan> {
        let mut spans = Vec::new();
        let mut chars = input.chars().peekable();
        let mut buffer = String::new();

        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(&next_c) = chars.peek() {
                    match next_c {
                        '%' | '&' | '_' | '#' | '$' | '{' | '}' => {
                            chars.next();
                            buffer.push(next_c);
                            continue;
                        }
                        '\\' => {
                            chars.next();
                            let _ = Self::skip_optional_bracket(&mut chars);
                            buffer.push(' ');
                            continue;
                        }
                        ' ' | '\t' => {
                            chars.next();
                            buffer.push(' ');
                            continue;
                        }
                        ',' | ';' | '!' => {
                            chars.next();
                            buffer.push(' ');
                            continue;
                        }
                        '\'' | '`' | '^' | '"' | '~' | '=' | '.' => {
                            chars.next();
                            let target = if chars.peek() == Some(&'{') {
                                Self::extract_braced_arg(&mut chars)
                            } else {
                                chars.next().map(|ch| ch.to_string()).unwrap_or_default()
                            };
                            let accented = match (next_c, target.as_str()) {
                                ('\'', "a") => "á", ('\'', "e") => "é", ('\'', "i") => "í", ('\'', "o") => "ó", ('\'', "u") => "ú",
                                ('\'', "A") => "Á", ('\'', "E") => "É", ('\'', "I") => "Í", ('\'', "O") => "Ó", ('\'', "U") => "Ú",
                                ('`', "a") => "à", ('`', "e") => "è", ('`', "i") => "ì", ('`', "o") => "ò", ('`', "u") => "ù",
                                ('^', "a") => "â", ('^', "e") => "ê", ('^', "i") => "î", ('^', "o") => "ô", ('^', "u") => "û",
                                ('"', "a") => "ä", ('"', "e") => "ë", ('"', "i") => "ï", ('"', "o") => "ö", ('"', "u") => "ü",
                                ('"', "A") => "Ä", ('"', "O") => "Ö", ('"', "U") => "Ü",
                                ('~', "a") => "ã", ('~', "n") => "ñ", ('~', "o") => "õ",
                                ('~', "N") => "Ñ",
                                _ => &target,
                            };
                            buffer.push_str(accented);
                            continue;
                        }
                        _ => {}
                    }
                }

                let mut cmd = String::new();
                while let Some(&next_c) = chars.peek() {
                    if next_c.is_alphabetic() {
                        cmd.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                if matches!(cmd.as_str(), "textbf" | "textit" | "emph" | "texttt" | "textsf" | "textrm" | "textsc") {
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
                } else if cmd == "eqref" {
                    if !buffer.is_empty() {
                        spans.push(InlineSpan::Text(buffer.clone()));
                        buffer.clear();
                    }
                    let target = Self::extract_braced_arg(&mut chars);
                    spans.push(InlineSpan::Ref(format!("({target})")));
                    continue;
                } else if matches!(cmd.as_str(), "cite" | "citep" | "citet" | "nocite") {
                    if !buffer.is_empty() {
                        spans.push(InlineSpan::Text(buffer.clone()));
                        buffer.clear();
                    }
                    let _opt = Self::skip_optional_bracket(&mut chars);
                    let target = Self::extract_braced_arg(&mut chars);
                    spans.push(InlineSpan::Cite(format!("[{target}]")));
                    continue;
                } else if cmd == "ref" {
                    if !buffer.is_empty() {
                        spans.push(InlineSpan::Text(buffer.clone()));
                        buffer.clear();
                    }
                    let target = Self::extract_braced_arg(&mut chars);
                    spans.push(InlineSpan::Ref(target));
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
                } else if cmd == "rule" {
                    let _ = Self::skip_optional_bracket(&mut chars);
                    let _w = Self::extract_braced_arg(&mut chars);
                    let _h = Self::extract_braced_arg(&mut chars);
                    buffer.push_str(" — ");
                    continue;
                } else if matches!(
                    cmd.as_str(),
                    "vspace"
                        | "hspace"
                        | "label"
                        | "index"
                        | "setlength"
                        | "setcounter"
                        | "addtocounter"
                        | "stepcounter"
                        | "pagestyle"
                        | "thispagestyle"
                        | "pagenumbering"
                        | "setmainfont"
                        | "captionsetup"
                        | "hypersetup"
                        | "sisetup"
                        | "tikzset"
                        | "pgfplotsset"
                ) {
                    let _ = Self::skip_optional_bracket(&mut chars);
                    let _ = Self::extract_braced_arg(&mut chars);
                    let _ = Self::extract_braced_arg(&mut chars);
                    continue;
                } else if matches!(cmd.as_str(), "footnote" | "thanks") {
                    let _ = Self::skip_optional_bracket(&mut chars);
                    let note = Self::extract_braced_arg(&mut chars);
                    if !buffer.is_empty() {
                        spans.push(InlineSpan::Text(buffer.clone()));
                        buffer.clear();
                    }
                    spans.push(InlineSpan::Ref(format!(" [{note}]")));
                    continue;
                } else if cmd == "times" {
                    buffer.push('×');
                    continue;
                } else if cmd == "pm" || cmd == "textpm" {
                    buffer.push('±');
                    continue;
                } else if cmd == "mp" {
                    buffer.push('∓');
                    continue;
                } else if cmd == "leq" || cmd == "le" {
                    buffer.push('≤');
                    continue;
                } else if cmd == "geq" || cmd == "ge" {
                    buffer.push('≥');
                    continue;
                } else if cmd == "neq" || cmd == "ne" {
                    buffer.push('≠');
                    continue;
                } else if cmd == "approx" {
                    buffer.push('≈');
                    continue;
                } else if cmd == "equiv" {
                    buffer.push('≡');
                    continue;
                } else if cmd == "sim" {
                    buffer.push('∼');
                    continue;
                } else if cmd == "propto" {
                    buffer.push('∝');
                    continue;
                } else if cmd == "infty" {
                    buffer.push('∞');
                    continue;
                } else if cmd == "bullet" || cmd == "textbullet" {
                    buffer.push('•');
                    continue;
                } else if cmd == "degree" || cmd == "textdegree" {
                    buffer.push('°');
                    continue;
                } else if cmd == "textcelsius" {
                    buffer.push('℃');
                    continue;
                } else if cmd == "quad" {
                    buffer.push_str("\u{00A0}\u{00A0}");
                    continue;
                } else if cmd == "qquad" {
                    buffer.push_str("\u{00A0}\u{00A0}\u{00A0}\u{00A0}");
                    continue;
                } else if cmd == "newline" {
                    buffer.push('\n');
                    continue;
                } else if cmd == "LaTeX" {
                    buffer.push_str("LaTeX");
                    continue;
                } else if cmd == "TeX" {
                    buffer.push_str("TeX");
                    continue;
                } else if cmd == "copyright" {
                    buffer.push('©');
                    continue;
                } else if matches!(cmd.as_str(), "dots" | "ldots" | "cdots") {
                    buffer.push('…');
                    continue;
                } else if cmd == "dag" {
                    buffer.push('†');
                    continue;
                } else if cmd == "ddag" {
                    buffer.push('‡');
                    continue;
                } else if cmd == "S" {
                    buffer.push('§');
                    continue;
                } else if cmd == "P" {
                    buffer.push('¶');
                    continue;
                } else if cmd == "textbackslash" {
                    buffer.push('\\');
                    continue;
                } else if cmd == "textasciitilde" {
                    buffer.push('~');
                    continue;
                } else if cmd == "textasciicircum" {
                    buffer.push('^');
                    continue;
                } else if matches!(
                    cmd.as_str(),
                    "bfseries"
                        | "itshape"
                        | "slshape"
                        | "scshape"
                        | "rmfamily"
                        | "sffamily"
                        | "ttfamily"
                        | "normalfont"
                        | "normalsize"
                        | "small"
                        | "footnotesize"
                        | "scriptsize"
                        | "tiny"
                        | "large"
                        | "Large"
                        | "LARGE"
                        | "huge"
                        | "Huge"
                        | "centering"
                        | "raggedright"
                        | "raggedleft"
                        | "noindent"
                ) {
                    continue;
                } else {
                    // Check if followed by braced arguments e.g. \mycommand{content}
                    while chars.peek() == Some(&' ') {
                        chars.next();
                    }
                    if chars.peek() == Some(&'{') {
                        let inner = Self::extract_braced_arg(&mut chars);
                        buffer.push_str(&inner);
                    }
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

            if c == '~' {
                buffer.push('\u{00A0}');
                continue;
            }

            if c == '-' {
                if chars.peek() == Some(&'-') {
                    chars.next();
                    if chars.peek() == Some(&'-') {
                        chars.next();
                        buffer.push('—');
                    } else {
                        buffer.push('–');
                    }
                    continue;
                }
            }

            if c == '`' {
                if chars.peek() == Some(&'`') {
                    chars.next();
                    buffer.push('“');
                } else {
                    buffer.push('‘');
                }
                continue;
            }

            if c == '\'' {
                if chars.peek() == Some(&'\'') {
                    chars.next();
                    buffer.push('”');
                } else {
                    buffer.push('’');
                }
                continue;
            }

            buffer.push(c);
        }

        if !buffer.is_empty() {
            spans.push(InlineSpan::Text(buffer));
        }

        spans
    }

    /// Strips common inline LaTeX macro wrappers (`\textbf`, `\textit`, etc.) to produce clean plain text.
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

    /// Extracts the contents of a braced argument `{...}` from a character iterator, tracking nested braces.
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

    /// Parses and consumes an optional bracketed parameter `[...]` from a character iterator, if present.
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

    /// Cleans tabular cell content by resolving `\multirow` and `\multicolumn` wrapper commands.
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

    /// Translates `siunitx` macro unit tokens (e.g., `\kilo\gram`, `\meter\per\second\squared`) into formatted SI unit strings.
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

    /// Formats an `siunitx` scientific notation number string into readable typography (e.g., `1.23e5` $\to$ `1.23 × 10^5`).
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

    /// Resolves standard LaTeX/CSS color names (e.g., `"red"`, `"teal"`, `"gray"`) to [`egui::Color32`] RGB values.
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

    /// Parses a LaTeX dimension string into typographic points (pt)
    pub fn parse_dimension_pt(dim: &str) -> Option<f32> {
        let trimmed = dim.trim().to_lowercase();
        if let Some(val) = trimmed.strip_suffix("pt") {
            val.trim().parse::<f32>().ok()
        } else if let Some(val) = trimmed.strip_suffix("cm") {
            val.trim().parse::<f32>().map(|v| v * 28.3465).ok()
        } else if let Some(val) = trimmed.strip_suffix("mm") {
            val.trim().parse::<f32>().map(|v| v * 2.83465).ok()
        } else if let Some(val) = trimmed.strip_suffix("in") {
            val.trim().parse::<f32>().map(|v| v * 72.0).ok()
        } else if let Some(val) = trimmed.strip_suffix("em") {
            val.trim().parse::<f32>().map(|v| v * 10.0).ok()
        } else if let Some(val) = trimmed.strip_suffix("ex") {
            val.trim().parse::<f32>().map(|v| v * 5.0).ok()
        } else {
            trimmed.parse::<f32>().ok()
        }
    }

    /// Parses tabular column specifiers (e.g. `|l|c|r|`) into [`ColumnAlign`] values
    pub fn parse_column_alignments(spec: &str) -> Vec<ColumnAlign> {
        let mut aligns = Vec::new();
        let mut in_curly = false;
        for c in spec.chars() {
            if c == '{' {
                in_curly = true;
                continue;
            } else if c == '}' {
                in_curly = false;
                continue;
            }
            if in_curly {
                continue;
            }
            match c {
                'l' => aligns.push(ColumnAlign::Left),
                'c' => aligns.push(ColumnAlign::Center),
                'r' => aligns.push(ColumnAlign::Right),
                'p' | 'm' | 'b' => aligns.push(ColumnAlign::Left),
                _ => {}
            }
        }
        if aligns.is_empty() {
            vec![ColumnAlign::Left]
        } else {
            aligns
        }
    }

    /// Checks if a LaTeX line is a block-level structural directive that delimits paragraphs
    pub fn is_block_command(line: &str) -> bool {
        let t = line.trim();
        if t.is_empty() || t.starts_with('%') {
            return true;
        }
        if t.starts_with("\\[") || t.starts_with("$$") {
            return true;
        }
        if !t.starts_with('\\') {
            return false;
        }
        let cmd = t[1..].split(|c: char| !c.is_alphabetic() && c != '*').next().unwrap_or("");
        matches!(
            cmd,
            "section"
                | "subsection"
                | "subsubsection"
                | "paragraph"
                | "subparagraph"
                | "chapter"
                | "part"
                | "begin"
                | "end"
                | "documentclass"
                | "usepackage"
                | "title"
                | "author"
                | "date"
                | "maketitle"
                | "tableofcontents"
                | "appendix"
                | "newpage"
                | "clearpage"
                | "pagebreak"
                | "vspace"
                | "vspace*"
                | "bigskip"
                | "medskip"
                | "smallskip"
                | "hrule"
                | "rule"
                | "centering"
                | "noindent"
                | "PassOptionsToPackage"
                | "hypersetup"
                | "graphicspath"
                | "definecolor"
                | "pagestyle"
                | "thispagestyle"
                | "bibliography"
                | "bibliographystyle"
                | "setlength"
                | "setcounter"
                | "addtocounter"
                | "stepcounter"
                | "pagenumbering"
                | "pgfplotsset"
                | "tikzset"
                | "sisetup"
                | "captionsetup"
                | "newcommand"
                | "renewcommand"
                | "providecommand"
                | "def"
                | "let"
                | "usetikzlibrary"
                | "usepgfplotslibrary"
                | "input"
                | "include"
                | "addplot"
                | "addlegendentry"
                | "draw"
                | "node"
                | "path"
                | "coordinate"
        )
    }
}
