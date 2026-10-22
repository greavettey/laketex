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
            "body { font-family: 'Times New Roman', serif; font-size: 11pt; line-height: 1.5; color: #111; margin: 20mm; }
             h1.title { font-size: 19pt; text-align: center; margin-bottom: 6px; font-weight: bold; letter-spacing: -0.2px; }
             p.author { text-align: center; font-size: 11.5pt; margin: 4px 0; color: #222; }
             p.date { text-align: center; font-size: 10pt; margin: 4px 0 16px 0; color: #555; }
             hr.divider { border: 0; border-top: 1px solid #ddd; margin: 16px auto; width: 85%; }
             div.abstract { margin: 16px 36px; font-size: 9.8pt; line-height: 1.45; text-align: justify; }
             div.abstract strong { font-size: 10.5pt; font-family: sans-serif; }
             h2.section { font-size: 13.5pt; margin-top: 22px; margin-bottom: 8px; font-weight: bold; }
             h3.subsection { font-size: 11.5pt; margin-top: 16px; margin-bottom: 6px; font-weight: bold; }
             p { text-align: justify; margin: 7px 0; text-indent: 1.6em; }
             p.noindent { text-indent: 0; }
             table.eq-row { width: 100%; margin: 10px 0; border-collapse: collapse; }
             table.eq-row td.eq-math { text-align: center; border: none; padding: 6px 4px; font-style: italic; font-size: 12pt; white-space: normal; }
             table.eq-row td.eq-num { width: 45px; text-align: right; border: none; padding: 4px; font-style: normal; font-size: 11pt; color: #444; }
             table.math-matrix { display: inline-table; vertical-align: middle; border-collapse: collapse; margin: 2px 4px; border-spacing: 0; }
             table.math-matrix td.m-cell { padding: 3px 8px; text-align: center; vertical-align: middle; font-style: italic; white-space: nowrap; border: none; }
             table.pmatrix { border-left: 1.5pt solid #111; border-right: 1.5pt solid #111; border-radius: 6pt; padding: 1pt 3pt; }
             table.bmatrix { border-left: 2pt solid #111; border-right: 2pt solid #111; padding: 1pt 4pt; }
             table.Bmatrix { border-left: 1.5pt solid #111; border-right: 1.5pt solid #111; border-radius: 4pt; padding: 1pt 3pt; }
             table.vmatrix { border-left: 1.2pt solid #111; border-right: 1.2pt solid #111; padding: 0 4pt; }
             table.Vmatrix { border-left: 3pt double #111; border-right: 3pt double #111; padding: 0 4pt; }
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
             table.latex-table { margin: 16px auto; border-top: 1.5pt solid #111; border-bottom: 1.5pt solid #111; border-collapse: collapse; }
             table.latex-table th { border: none; border-bottom: 0.75pt solid #111; padding: 6px 14px; text-align: left; font-weight: bold; }
             table.latex-table td { border: none; padding: 5px 14px; text-align: left; }
             ul, ol { margin: 6px 0 6px 24px; padding-left: 0; }
             li { margin: 3px 0; }
             span.math { font-style: italic; font-family: 'Times New Roman', serif; }
             span.code { font-family: 'Courier New', monospace; font-size: 9.5pt; background-color: #f6f7f9; padding: 1px 4px; border-radius: 2px; }
             div.code-container { margin: 14px 0; border: 1px solid #d0d7de; border-radius: 5px; overflow: hidden; background-color: #f6f8fa; }
             div.code-lang { background-color: #eaeef2; color: #57606a; font-family: sans-serif; font-size: 8.5pt; font-weight: bold; padding: 3px 10px; text-transform: uppercase; border-bottom: 1px solid #d0d7de; }
             pre.code-block { margin: 0; padding: 10px 14px; font-family: 'Courier New', monospace; font-size: 9.5pt; line-height: 1.45; color: #24292f; white-space: pre-wrap; word-wrap: break-word; }
             div.figure-container { margin: 18px auto; text-align: center; }
             div.figure-box { margin: 0 auto 6px auto; width: 85%; height: 90px; border: 1.5px dashed #b0b8c4; border-radius: 4px; background-color: #f8fafc; display: flex; align-items: center; justify-content: center; font-family: sans-serif; font-size: 10pt; color: #64748b; }
             p.caption { text-align: center; font-size: 9.5pt; margin-top: 4px; color: #334155; }
             table.booktabs { border-top: 2pt solid #111; border-bottom: 2pt solid #111; margin: 14px auto; }
             table.booktabs th { border-bottom: 0.8pt solid #111; }",
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
                    html.push_str("<hr class=\"divider\">");
                }
                DocumentBlock::Abstract { spans, .. } => {
                    html.push_str("<div class=\"abstract\"><strong>Abstract</strong> — ");
                    html.push_str(&Self::spans_to_html(spans));
                    html.push_str("</div>");
                }
                DocumentBlock::Section { level, number, title, .. } => {
                    let tag = if *level == 1 { "h2" } else { "h3" };
                    let cls = if *level == 1 { "section" } else { "subsection" };
                    let header_text = if number.is_empty() {
                        title.clone()
                    } else {
                        format!("{number} {title}")
                    };
                    html.push_str(&format!("<{tag} class=\"{cls}\">{}</{tag}>", Self::escape_xml(&header_text)));
                }
                DocumentBlock::Paragraph { spans, .. } => {
                    html.push_str("<p>");
                    html.push_str(&Self::spans_to_html(spans));
                    html.push_str("</p>");
                }
                DocumentBlock::EquationBlock { formula, number, .. } => {
                    let html_formatted = MathRenderer::formula_to_html(formula);
                    let num_str = number.map(|n| format!("({n})")).unwrap_or_default();
                    html.push_str("<table class=\"eq-row\"><tr>");
                    html.push_str(&format!("<td class=\"eq-math\">{html_formatted}</td>"));
                    html.push_str(&format!("<td class=\"eq-num\">{num_str}</td>"));
                    html.push_str("</tr></table>");
                }
                DocumentBlock::AlignBlock { equations, start_number, .. } => {
                    for (idx, eq) in equations.iter().enumerate() {
                        let html_formatted = MathRenderer::formula_to_html(eq);
                        let num_str = format!("({})", start_number + idx);
                        html.push_str("<table class=\"eq-row\"><tr>");
                        html.push_str(&format!("<td class=\"eq-math\">{html_formatted}</td>"));
                        html.push_str(&format!("<td class=\"eq-num\">{num_str}</td>"));
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
                DocumentBlock::Table { caption, headers, rows, has_booktabs, .. } => {
                    let table_class = if *has_booktabs { "latex-table booktabs" } else { "latex-table" };
                    html.push_str(&format!("<table class=\"{table_class}\"><thead><tr>"));
                    for h in headers {
                        html.push_str(&format!("<th>{}</th>", Self::escape_xml(h)));
                    }
                    html.push_str("</tr></thead><tbody>");
                    for row in rows {
                        html.push_str("<tr>");
                        for cell in &row.cells {
                            html.push_str("<td>");
                            html.push_str(&Self::spans_to_html(cell));
                            html.push_str("</td>");
                        }
                        html.push_str("</tr>");
                    }
                    html.push_str("</tbody></table>");
                    if let Some(cap) = caption {
                        html.push_str(&format!("<p style=\"text-align:center;font-size:9.5pt;margin-top:4px;\"><em>{}</em></p>", Self::escape_xml(cap)));
                    }
                }
                DocumentBlock::Figure { caption, number, image_path, .. } => {
                    let label = image_path.as_deref().unwrap_or("Graphic Object");
                    html.push_str("<div class=\"figure-container\">");
                    html.push_str(&format!("<div class=\"figure-box\">📷 [{}]</div>", Self::escape_xml(label)));
                    if let Some(cap) = caption {
                        html.push_str(&format!("<p class=\"caption\"><em>Figure {number}: {}</em></p>", Self::escape_xml(cap)));
                    }
                    html.push_str("</div>");
                }
                DocumentBlock::CodeBlock { language, code, .. } => {
                    let escaped = Self::escape_xml(code);
                    html.push_str("<div class=\"code-container\">");
                    if !language.is_empty() {
                        html.push_str(&format!("<div class=\"code-lang\">{}</div>", Self::escape_xml(language)));
                    }
                    html.push_str(&format!("<pre class=\"code-block\"><code>{escaped}</code></pre>"));
                    html.push_str("</div>");
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
