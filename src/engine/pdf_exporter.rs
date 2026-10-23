//! # Native PDF Document Exporter
//!
//! Generates standalone, publication-quality PDF documents directly from [`LatexAst`] using
//! `printpdf`. Produces clean vector typography with academic margins, serif headings,
//! equation rows, and booktabs tabular rules without needing any external TeX installation.

use crate::engine::ast::*;
use crate::engine::math::MathRenderer;
use printpdf::{GeneratePdfOptions, PdfDocument, PdfSaveOptions};
use std::collections::BTreeMap;

/// High-performance pure-Rust PDF exporter.
pub struct NativePdfExporter;

impl NativePdfExporter {
    /// XML/HTML entity escaping for reliable PDFium / printpdf rendering
    pub fn escape_xml(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    /// Converts a [`LatexAst`] into formatted HTML with embedded CSS styling for print rendering.
    pub fn ast_to_html(ast: &LatexAst) -> String {
        let mut html = String::new();
        html.push_str("<!DOCTYPE html><html><head><meta charset=\"UTF-8\"><style>");
        html.push_str(
            "body { font-family: 'Times New Roman', serif; font-size: 11pt; line-height: 1.32; color: #111; margin: 20mm; }
             h1.title { font-size: 17.5pt; text-align: center; margin: 0 0 10pt 0; font-weight: bold; letter-spacing: -0.2px; }
             p.author { text-align: center; font-size: 11pt; margin: 3pt 0; color: #111; }
             p.date { text-align: center; font-size: 10pt; margin: 3pt 0 14pt 0; color: #333; }
             hr.divider { border: 0; border-top: 0.8pt solid #111; margin: 12pt auto; width: 100%; }
             div.abstract { margin: 12pt 32pt; font-size: 9pt; line-height: 1.3; text-align: justify; }
             div.abstract strong { font-size: 9.5pt; font-weight: bold; }
             h2.section { font-size: 14pt; margin-top: 16pt; margin-bottom: 5pt; font-weight: bold; }
             h3.subsection { font-size: 12pt; margin-top: 11pt; margin-bottom: 3pt; font-weight: bold; }
             h4.subsubsection { font-size: 11pt; margin-top: 8pt; margin-bottom: 2pt; font-weight: bold; }
             h5.paragraph { font-size: 10.5pt; margin-top: 6pt; margin-bottom: 2pt; font-weight: bold; }
             p { text-align: justify; margin: 0; text-indent: 1.5em; line-height: 1.32; }
             h1 + p, h2 + p, h3 + p, h4 + p, h5 + p, div.abstract + p, div.abstract + div + p, div.toc + p, p.noindent, blockquote p { text-indent: 0; }
             blockquote { margin: 10pt 24pt; font-size: 10pt; font-style: italic; }
             div.centered { text-align: center; margin: 8pt 0; }
             div.toc { margin: 16pt 0; }
             div.toc-item { margin: 3pt 0; font-size: 10.5pt; }
             div.toc-l1 { font-weight: bold; margin-top: 6pt; }
             div.toc-l2 { padding-left: 16pt; font-weight: normal; }
             div.toc-l3 { padding-left: 32pt; font-size: 9.5pt; }
             table.eq-row { width: 100%; margin: 8pt 0; border-collapse: collapse; }
             table.eq-row td.eq-side { width: 50pt; text-align: right; border: none; padding: 2pt 0; font-size: 11pt; color: #222; font-style: normal; }
             table.eq-row td.eq-math { text-align: center; border: none; padding: 2pt 4pt; font-style: italic; font-size: 11pt; }
             table.math-matrix-outer { display: inline-table; vertical-align: middle; border-collapse: collapse; margin: 2pt 4pt; border-spacing: 0; }
             table.math-matrix-inner { border-collapse: collapse; border-spacing: 0; margin: 0; }
             table.math-matrix-inner td { padding: 2pt 6pt; text-align: center; vertical-align: middle; font-style: italic; white-space: nowrap; border: none; font-size: 10.5pt; }
             td.mat-body { padding: 0; vertical-align: middle; border: none; }
             td.mat-bracket-l, td.mat-bracket-r { vertical-align: stretch; width: 4pt; padding: 0; border: none; }
             td.mat-b-l { border-left: 1.5pt solid #111; border-top: 1.5pt solid #111; border-bottom: 1.5pt solid #111; }
             td.mat-b-r { border-right: 1.5pt solid #111; border-top: 1.5pt solid #111; border-bottom: 1.5pt solid #111; }
             td.mat-p-l { border-left: 1.5pt solid #111; border-radius: 4pt 0 0 4pt; }
             td.mat-p-r { border-right: 1.5pt solid #111; border-radius: 0 4pt 4pt 0; }
             td.mat-v-l { border-left: 1.2pt solid #111; }
             td.mat-v-r { border-right: 1.2pt solid #111; }
             td.mat-V-l { border-left: 3pt double #111; }
             td.mat-V-r { border-right: 3pt double #111; }
             td.mat-B-l { border-left: 1.5pt solid #111; border-radius: 3pt 0 0 3pt; }
             td.mat-B-r { border-right: 1.5pt solid #111; border-radius: 0 3pt 3pt 0; }
             span.fraction { display: inline-block; vertical-align: middle; text-align: center; padding: 0 2pt; font-size: 0.88em; line-height: 1.15; }
             span.fraction > span.num { display: block; border-bottom: 0.8pt solid #111; padding: 0 2pt 1.5pt 2pt; text-align: center; }
             span.fraction > span.den { display: block; padding: 1.5pt 2pt 0 2pt; text-align: center; }
             span.sqrt { display: inline-block; vertical-align: middle; margin: 0 1pt; }
             span.sqrt > span.radical { font-size: 1.15em; vertical-align: middle; margin-right: -1pt; }
             span.sqrt > span.radicand { display: inline-block; border-top: 0.8pt solid #111; padding: 0 2pt; vertical-align: middle; }
             strong.math-bold { font-family: 'Times New Roman', serif; font-weight: bold; font-style: normal; }
             span.math-roman { font-family: 'Times New Roman', serif; font-style: normal; }
             em.math-italic { font-family: 'Times New Roman', serif; font-style: italic; }
             sup { font-size: 0.72em; vertical-align: 0.4em; line-height: 0; }
             sub { font-size: 0.72em; vertical-align: -0.3em; line-height: 0; }
             table.latex-table { margin: 12pt auto; border-top: 1.5pt solid #111; border-bottom: 1.5pt solid #111; border-collapse: collapse; font-size: 10.5pt; }
             table.latex-table th { border: none; border-bottom: 0.75pt solid #111; padding: 4pt 10pt; font-weight: bold; }
             table.latex-table td { border: none; padding: 3pt 10pt; }
             table.booktabs { border-top: 2pt solid #111; border-bottom: 2pt solid #111; margin: 12pt auto; }
             table.booktabs th { border-bottom: 0.8pt solid #111; }
             ul, ol { margin: 4pt 0 4pt 20pt; padding-left: 0; }
             li { margin: 2pt 0; }
             span.math { font-style: italic; font-family: 'Times New Roman', serif; }
             span.code { font-family: 'Courier New', monospace; font-size: 9.5pt; background-color: #f6f7f9; padding: 1px 4px; border-radius: 2px; }
             div.code-container { margin: 12pt 0; border: 1px solid #d0d7de; border-radius: 4px; overflow: hidden; background-color: #f6f8fa; }
             div.code-lang { background-color: #eaeef2; color: #57606a; font-family: sans-serif; font-size: 8.5pt; font-weight: bold; padding: 3px 10px; text-transform: uppercase; border-bottom: 1px solid #d0d7de; }
             pre.code-block { margin: 0; padding: 8pt 12pt; font-family: 'Courier New', monospace; font-size: 9.5pt; line-height: 1.4; color: #24292f; white-space: pre-wrap; word-wrap: break-word; }
             pre.code-block .k, pre.code-block .kd, pre.code-block .kn { color: #008000; font-weight: bold; }
             pre.code-block .nf, pre.code-block .fm { color: #0000FF; }
             pre.code-block .nc, pre.code-block .kt { color: #0000FF; font-weight: bold; }
             pre.code-block .s, pre.code-block .s1, pre.code-block .s2, pre.code-block .sa, pre.code-block .si { color: #BA2121; }
             pre.code-block .c, pre.code-block .c1, pre.code-block .cm { color: #408080; font-style: italic; }
             pre.code-block .m, pre.code-block .mi, pre.code-block .mf { color: #008800; }
             pre.code-block .o { color: #666666; }
             pre.code-block .nb { color: #008000; }
             div.figure-container { margin: 14pt auto; text-align: center; }
             div.figure-box { margin: 0 auto 6pt auto; width: 85%; height: 80px; border: 1.5px dashed #b0b8c4; border-radius: 4px; background-color: #f8fafc; display: flex; align-items: center; justify-content: center; font-family: sans-serif; font-size: 10pt; color: #64748b; }
             p.caption { text-align: center; font-size: 9.5pt; margin-top: 4pt; color: #334155; }",
        );
        html.push_str("</style></head><body>");

        if ast.blocks.is_empty() {
            html.push_str("<p>&#160;</p>");
        }

        for block in &ast.blocks {
            match block {
                DocumentBlock::TitleBlock { .. } => {
                    if let Some(title) = &ast.metadata.title {
                        html.push_str(&format!("<h1 class=\"title\">{}</h1>", Self::escape_xml(title)));
                    }
                    if let Some(author) = &ast.metadata.author {
                        html.push_str(&format!("<p class=\"author\">{}</p>", Self::escape_xml(author)));
                    }
                    if let Some(date) = &ast.metadata.date {
                        html.push_str(&format!("<p class=\"date\">{}</p>", Self::escape_xml(date)));
                    }
                }
                DocumentBlock::Abstract { spans, .. } => {
                    html.push_str("<div class=\"abstract\"><strong>Abstract</strong> — ");
                    html.push_str(&Self::spans_to_html(spans));
                    html.push_str("</div>");
                }
                DocumentBlock::Section { level, number, title, .. } => {
                    let (tag, cls) = match level {
                        1 => ("h2", "section"),
                        2 => ("h3", "subsection"),
                        3 => ("h4", "subsubsection"),
                        _ => ("h5", "paragraph"),
                    };
                    let header_text = if number.is_empty() {
                        title.clone()
                    } else {
                        format!("{number} {title}")
                    };
                    html.push_str(&format!("<{tag} class=\"{cls}\">{}</{tag}>", Self::escape_xml(&header_text)));
                }
                DocumentBlock::Paragraph { spans, is_noindent, .. } => {
                    let cls = if *is_noindent { " class=\"noindent\"" } else { "" };
                    html.push_str(&format!("<p{cls}>{}</p>", Self::spans_to_html(spans)));
                }
                DocumentBlock::EquationBlock { formula, number, .. } => {
                    let html_formatted = MathRenderer::formula_to_html(formula);
                    let num_str = number.map(|n| format!("({n})")).unwrap_or_default();
                    html.push_str("<table class=\"eq-row\"><tr>");
                    html.push_str("<td class=\"eq-side\">&#160;</td>");
                    html.push_str(&format!("<td class=\"eq-math\">{html_formatted}</td>"));
                    html.push_str(&format!("<td class=\"eq-side\">{num_str}</td>"));
                    html.push_str("</tr></table>");
                }
                DocumentBlock::AlignBlock { equations, start_number, .. } => {
                    for (idx, eq) in equations.iter().enumerate() {
                        let html_formatted = MathRenderer::formula_to_html(eq);
                        let num_str = format!("({})", start_number + idx);
                        html.push_str("<table class=\"eq-row\"><tr>");
                        html.push_str("<td class=\"eq-side\">&#160;</td>");
                        html.push_str(&format!("<td class=\"eq-math\">{html_formatted}</td>"));
                        html.push_str(&format!("<td class=\"eq-side\">{num_str}</td>"));
                        html.push_str("</tr></table>");
                    }
                }
                DocumentBlock::List { ordered, items, .. } => {
                    let tag = if *ordered { "ol" } else { "ul" };
                    html.push_str(&format!("<{tag}>"));
                    for item_spans in items {
                        html.push_str("<li>");
                        html.push_str(&Self::spans_to_html(item_spans));
                        html.push_str("</li>");
                    }
                    html.push_str(&format!("</{tag}>"));
                }
                DocumentBlock::Table { caption, headers, rows, has_booktabs, alignments, .. } => {
                    let table_class = if *has_booktabs { "latex-table booktabs" } else { "latex-table" };
                    html.push_str(&format!("<table class=\"{table_class}\"><thead><tr>"));
                    for (col_idx, h) in headers.iter().enumerate() {
                        let align_style = match alignments.get(col_idx) {
                            Some(ColumnAlign::Center) => "text-align:center;",
                            Some(ColumnAlign::Right) => "text-align:right;",
                            _ => "text-align:left;",
                        };
                        html.push_str(&format!("<th style=\"{align_style}\">{}</th>", Self::escape_xml(h)));
                    }
                    html.push_str("</tr></thead><tbody>");
                    for row in rows {
                        html.push_str("<tr>");
                        for (col_idx, cell) in row.cells.iter().enumerate() {
                            let align_style = match alignments.get(col_idx) {
                                Some(ColumnAlign::Center) => "text-align:center;",
                                Some(ColumnAlign::Right) => "text-align:right;",
                                _ => "text-align:left;",
                            };
                            html.push_str(&format!("<td style=\"{align_style}\">{}</td>", Self::spans_to_html(cell)));
                        }
                        html.push_str("</tr>");
                    }
                    html.push_str("</tbody></table>");
                    if let Some(cap) = caption {
                        html.push_str(&format!("<p style=\"text-align:center;font-size:9.5pt;margin-top:4pt;\"><em>{}</em></p>", Self::escape_xml(cap)));
                    }
                }
                DocumentBlock::Figure { caption, number, image_path, .. } => {
                    let label = image_path.as_deref().unwrap_or("Graphic Object");
                    html.push_str("<div class=\"figure-container\">");
                    html.push_str(&format!("<div class=\"figure-box\">🖼 [{}]</div>", Self::escape_xml(label)));
                    if let Some(cap) = caption {
                        html.push_str(&format!("<p class=\"caption\"><em>Figure {number}: {}</em></p>", Self::escape_xml(cap)));
                    }
                    html.push_str("</div>");
                }
                DocumentBlock::CodeBlock { language, code, .. } => {
                    let highlighted = crate::engine::syntax::SyntaxHighlighter::highlight_html(code, language);
                    html.push_str("<div class=\"code-container\">");
                    if !language.is_empty() {
                        html.push_str(&format!("<div class=\"code-lang\">{}</div>", Self::escape_xml(language)));
                    }
                    html.push_str(&format!("<pre class=\"code-block\"><code>{highlighted}</code></pre>"));
                    html.push_str("</div>");
                }
                DocumentBlock::CenteredBlock { spans, .. } => {
                    html.push_str(&format!("<div class=\"centered\">{}</div>", Self::spans_to_html(spans)));
                }
                DocumentBlock::QuoteBlock { spans, .. } => {
                    html.push_str(&format!("<blockquote><p>{}</p></blockquote>", Self::spans_to_html(spans)));
                }
                DocumentBlock::TableOfContents { .. } => {
                    html.push_str("<div class=\"toc\"><h2 class=\"section\">Contents</h2>");
                    for b in &ast.blocks {
                        if let DocumentBlock::Section { level, number, title, .. } = b {
                            let cls = match level {
                                1 => "toc-item toc-l1",
                                2 => "toc-item toc-l2",
                                _ => "toc-item toc-l3",
                            };
                            let title_escaped = Self::escape_xml(title);
                            html.push_str(&format!("<div class=\"{cls}\"><span>{number} {title_escaped}</span></div>"));
                        }
                    }
                    html.push_str("</div>");
                }
                DocumentBlock::PageBreak { .. } => {
                    html.push_str("<div style=\"page-break-after:always;\"></div>");
                }
                DocumentBlock::VerticalSpace { pt, .. } => {
                    html.push_str(&format!("<div style=\"height:{pt}pt;\"></div>"));
                }
                DocumentBlock::HorizontalRule { .. } => {
                    html.push_str("<hr class=\"divider\">");
                }
            }
        }

        html.push_str("</body></html>");
        html
    }

    fn spans_to_html(spans: &[InlineSpan]) -> String {
        let mut out = String::new();
        for span in spans {
            match span {
                InlineSpan::Text(t) => out.push_str(&Self::escape_xml(t)),
                InlineSpan::Bold(t) => out.push_str(&format!("<strong>{}</strong>", Self::escape_xml(t))),
                InlineSpan::Italic(t) => out.push_str(&format!("<em>{}</em>", Self::escape_xml(t))),
                InlineSpan::Code(t) => out.push_str(&format!("<span class=\"code\">{}</span>", Self::escape_xml(t))),
                InlineSpan::Math(m) => {
                    let html_math = MathRenderer::formula_to_html(m);
                    out.push_str(&format!("<span class=\"math\">{html_math}</span>"));
                }
                InlineSpan::Colored(c, t) => {
                    out.push_str(&format!("<span style=\"color:rgb({},{},{})\">{}</span>", c.r(), c.g(), c.b(), Self::escape_xml(t)));
                }
                InlineSpan::Url(u) => out.push_str(&format!("<a href=\"{}\">{}</a>", Self::escape_xml(u), Self::escape_xml(u))),
                InlineSpan::Ref(r) | InlineSpan::Cite(r) => out.push_str(&format!("<span>{}</span>", Self::escape_xml(r))),
            }
        }
        out
    }

    /// Renders the document AST directly into valid PDF bytes.
    pub fn export_to_pdf_bytes(ast: &LatexAst) -> Result<Vec<u8>, String> {
        let html = Self::ast_to_html(ast);
        let (page_w_mm, page_h_mm) = match ast.metadata.paper_size {
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::Letter => (215.9, 279.4),
        };
        let margin_mm = ((ast.metadata.margin_pt / 72.0) * 25.4).max(8.0);
        let options = GeneratePdfOptions {
            page_width: Some(page_w_mm),
            page_height: Some(page_h_mm),
            margin_top: Some(margin_mm),
            margin_bottom: Some(margin_mm),
            margin_left: Some(margin_mm),
            margin_right: Some(margin_mm),
            ..Default::default()
        };

        let mut warnings = Vec::new();
        let images = BTreeMap::new();
        let fonts = BTreeMap::new();

        let doc = PdfDocument::from_html(&html, &images, &fonts, &options, &mut warnings)
            .map_err(|e| format!("Native PDF generation error: {e}"))?;

        let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
        Ok(bytes)
    }
}
