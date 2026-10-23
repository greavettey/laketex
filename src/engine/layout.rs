//! # Document Layout & Page Geometry Engine
//!
//! Converts high-level [`LatexAst`] structures into physical paginated layouts ([`TypesetPage`]),
//! calculating vertical coordinate offsets, line breaking, margin bounds, and mapping
//! screen coordinates back to source line numbers for forward and backward SyncTeX.

#![allow(dead_code)]

use crate::engine::ast::*;

/// Typographic element kind within a laid out page.
#[derive(Debug, Clone)]
pub enum ElementKind {
    /// Document title
    Title {
        /// Text string
        text: String,
        /// Font size in points
        font_size: f32,
    },
    /// Author byline
    Author {
        /// Text string
        text: String,
        /// Font size in points
        font_size: f32,
    },
    /// Document date
    Date {
        /// Text string
        text: String,
        /// Font size in points
        font_size: f32,
    },
    /// Abstract summary box
    Abstract {
        /// Label title (e.g. "Abstract")
        title: String,
        /// Content spans
        spans: Vec<InlineSpan>,
    },
    /// Section or subsection heading
    Heading {
        /// Heading depth level
        level: usize,
        /// Section number string (e.g. "2.1")
        number: String,
        /// Title text
        title: String,
        /// Font size in points
        font_size: f32,
    },
    /// Paragraph body text
    Paragraph(Vec<InlineSpan>),
    /// Standalone mathematical formula
    Equation {
        /// Cleaned formula string
        formula: String,
        /// Optional equation number
        number: Option<usize>,
    },
    /// Bullet or numbered list
    List {
        /// True if ordered
        ordered: bool,
        /// List items
        items: Vec<Vec<InlineSpan>>,
    },
    /// Formatted data table
    Table {
        /// Table caption
        caption: Option<String>,
        /// Column headers
        headers: Vec<String>,
        /// Data rows
        rows: Vec<TableRow>,
        /// Whether booktabs rules are applied
        has_booktabs: bool,
    },
    /// Floated figure with caption and visual frame
    Figure {
        /// Figure caption
        caption: Option<String>,
        /// Cross-reference label
        label: Option<String>,
        /// Optional graphic image path
        image_path: Option<String>,
        /// Figure sequence index
        number: usize,
    },
    /// Syntax-highlighted code block (minted / listings / verbatim)
    CodeBlock {
        /// Programming language identifier
        language: String,
        /// Verbatim source code
        code: String,
    },
    /// Horizontal dividing rule
    Divider,
    /// Vertical whitespace spacing
    Space(f32),
}

/// A laid-out element positioned at an exact vertical coordinate on a page.
#[derive(Debug, Clone)]
pub struct TypesetElement {
    /// Element content and style
    pub kind: ElementKind,
    /// Vertical offset from the top printable margin in typographic points
    pub y_offset: f32,
    /// Height in typographic points
    pub height: f32,
    /// Associated source code line number for SyncTeX navigation
    pub source_line: usize,
}

impl TypesetElement {
    /// Constructs a new laid-out element
    pub fn new(kind: ElementKind, y_offset: f32, height: f32, source_line: usize) -> Self {
        Self {
            kind,
            y_offset,
            height,
            source_line,
        }
    }
}

/// A complete physical typeset page with dimensions, margins, and placed elements.
#[derive(Debug, Clone)]
pub struct TypesetPage {
    /// 1-based page index
    pub page_number: usize,
    /// Total pages in the document
    pub total_pages: usize,
    /// Width in typographic points
    pub width: f32,
    /// Height in typographic points
    pub height: f32,
    /// Page margin width in points
    pub margin: f32,
    /// Ordered elements laid out on this page
    pub elements: Vec<TypesetElement>,
}

impl TypesetPage {
    /// Finds the typeset element closest to the given vertical coordinate for SyncTeX navigation
    pub fn find_element_at_y(&self, y: f32) -> Option<&TypesetElement> {
        self.elements
            .iter()
            .find(|el| y >= el.y_offset && y <= el.y_offset + el.height)
            .or_else(|| {
                self.elements.iter().min_by(|a, b| {
                    let dist_a = (a.y_offset - y).abs();
                    let dist_b = (b.y_offset - y).abs();
                    dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
                })
            })
    }
}

/// Layout planner for converting an AST into physical pages.
pub struct DocumentLayout;

impl DocumentLayout {
    /// Computes full document layout, flowing text and equations across paginated boundaries.
    pub fn layout_document(ast: &LatexAst) -> Vec<TypesetPage> {
        let (page_width, page_height) = ast.metadata.paper_size.dimensions_pt();
        let margin = ast.metadata.margin_pt;
        let printable_height = page_height - (margin * 2.0);

        let mut pages = Vec::new();
        let mut current_page_elements = Vec::new();
        let mut current_y = 0.0;

        let base_font_size = ast.metadata.base_font_size;

        for block in &ast.blocks {
            let line = block.line();
            match block {
                DocumentBlock::TitleBlock { .. } => {
                    let mut title_height = 0.0;
                    if let Some(title) = &ast.metadata.title {
                        current_page_elements.push(TypesetElement::new(
                            ElementKind::Title {
                                text: title.clone(),
                                font_size: base_font_size + 9.0,
                            },
                            current_y + title_height,
                            36.0,
                            line,
                        ));
                        title_height += 36.0;
                    }
                    if let Some(author) = &ast.metadata.author {
                        current_page_elements.push(TypesetElement::new(
                            ElementKind::Author {
                                text: author.clone(),
                                font_size: base_font_size + 1.0,
                            },
                            current_y + title_height,
                            20.0,
                            line,
                        ));
                        title_height += 20.0;
                    }
                    if let Some(date) = &ast.metadata.date {
                        current_page_elements.push(TypesetElement::new(
                            ElementKind::Date {
                                text: date.clone(),
                                font_size: base_font_size - 1.0,
                            },
                            current_y + title_height,
                            18.0,
                            line,
                        ));
                        title_height += 18.0;
                    }
                    current_page_elements.push(TypesetElement::new(
                        ElementKind::Space(16.0),
                        current_y + title_height,
                        16.0,
                        line,
                    ));
                    title_height += 16.0;

                    current_y += title_height;
                }

                DocumentBlock::Abstract { spans, line } => {
                    let est_height = 60.0;
                    if current_y + est_height > printable_height && !current_page_elements.is_empty() {
                        let page_num = pages.len() + 1;
                        pages.push(TypesetPage {
                            page_number: page_num,
                            total_pages: 0,
                            width: page_width,
                            height: page_height,
                            margin,
                            elements: std::mem::take(&mut current_page_elements),
                        });
                        current_y = 0.0;
                    }
                    current_page_elements.push(TypesetElement::new(
                        ElementKind::Abstract {
                            title: "Abstract".to_string(),
                            spans: spans.clone(),
                        },
                        current_y,
                        est_height,
                        *line,
                    ));
                    current_y += est_height + 14.0;
                }

                DocumentBlock::Section {
                    level,
                    number,
                    title,
                    line,
                } => {
                    let (font_size, heading_height) = match level {
                        1 => (base_font_size + 5.0, 32.0),
                        2 => (base_font_size + 2.5, 26.0),
                        3 => (base_font_size + 1.0, 22.0),
                        _ => (base_font_size, 18.0),
                    };

                    if current_y + heading_height + 40.0 > printable_height
                        && !current_page_elements.is_empty()
                    {
                        let page_num = pages.len() + 1;
                        pages.push(TypesetPage {
                            page_number: page_num,
                            total_pages: 0,
                            width: page_width,
                            height: page_height,
                            margin,
                            elements: std::mem::take(&mut current_page_elements),
                        });
                        current_y = 0.0;
                    }

                    current_page_elements.push(TypesetElement::new(
                        ElementKind::Heading {
                            level: *level,
                            number: number.clone(),
                            title: title.clone(),
                            font_size,
                        },
                        current_y,
                        heading_height,
                        *line,
                    ));
                    current_y += heading_height + 6.0;
                }

                DocumentBlock::Paragraph { spans, line, .. } => {
                    let total_chars: usize = spans
                        .iter()
                        .map(|s| match s {
                            InlineSpan::Text(t)
                            | InlineSpan::Bold(t)
                            | InlineSpan::Italic(t)
                            | InlineSpan::Code(t)
                            | InlineSpan::Math(t)
                            | InlineSpan::Colored(_, t)
                            | InlineSpan::Url(t)
                            | InlineSpan::Ref(t)
                            | InlineSpan::Cite(t) => t.len(),
                        })
                        .sum();

                    let est_lines = (total_chars / 75).max(1);
                    let line_height = base_font_size * 1.25;
                    let para_height = (est_lines as f32) * line_height + 4.0;

                    if current_y + para_height > printable_height
                        && !current_page_elements.is_empty()
                    {
                        let page_num = pages.len() + 1;
                        pages.push(TypesetPage {
                            page_number: page_num,
                            total_pages: 0,
                            width: page_width,
                            height: page_height,
                            margin,
                            elements: std::mem::take(&mut current_page_elements),
                        });
                        current_y = 0.0;
                    }

                    current_page_elements.push(TypesetElement::new(
                        ElementKind::Paragraph(spans.clone()),
                        current_y,
                        para_height,
                        *line,
                    ));
                    current_y += para_height + 2.0;
                }

                DocumentBlock::EquationBlock {
                    formula,
                    number,
                    line,
                    ..
                } => {
                    let line_count = formula.lines().count().max(1);
                    let matrix_row_count = if formula.contains("\\begin{") {
                        formula.matches("\\\\").count() + 1
                    } else {
                        1
                    };
                    let effective_rows = line_count.max(matrix_row_count);
                    let eq_height = if effective_rows > 1 {
                        (effective_rows as f32) * 22.0 + 16.0
                    } else {
                        34.0
                    };
                    if current_y + eq_height > printable_height && !current_page_elements.is_empty() {
                        let page_num = pages.len() + 1;
                        pages.push(TypesetPage {
                            page_number: page_num,
                            total_pages: 0,
                            width: page_width,
                            height: page_height,
                            margin,
                            elements: std::mem::take(&mut current_page_elements),
                        });
                        current_y = 0.0;
                    }

                    current_page_elements.push(TypesetElement::new(
                        ElementKind::Equation {
                            formula: formula.clone(),
                            number: *number,
                        },
                        current_y,
                        eq_height,
                        *line,
                    ));
                    current_y += eq_height + 10.0;
                }

                DocumentBlock::AlignBlock {
                    equations,
                    start_number,
                    line,
                } => {
                    for (idx, eq) in equations.iter().enumerate() {
                        let eq_height = 30.0;
                        if current_y + eq_height > printable_height
                            && !current_page_elements.is_empty()
                        {
                            let page_num = pages.len() + 1;
                            pages.push(TypesetPage {
                                page_number: page_num,
                                total_pages: 0,
                                width: page_width,
                                height: page_height,
                                margin,
                                elements: std::mem::take(&mut current_page_elements),
                            });
                            current_y = 0.0;
                        }

                        current_page_elements.push(TypesetElement::new(
                            ElementKind::Equation {
                                formula: eq.clone(),
                                number: Some(start_number + idx),
                            },
                            current_y,
                            eq_height,
                            *line,
                        ));
                        current_y += eq_height + 6.0;
                    }
                    current_y += 8.0;
                }

                DocumentBlock::List {
                    ordered,
                    items,
                    line,
                } => {
                    let item_height = (base_font_size * 1.35 + 4.0) * (items.len() as f32);
                    if current_y + item_height > printable_height
                        && !current_page_elements.is_empty()
                    {
                        let page_num = pages.len() + 1;
                        pages.push(TypesetPage {
                            page_number: page_num,
                            total_pages: 0,
                            width: page_width,
                            height: page_height,
                            margin,
                            elements: std::mem::take(&mut current_page_elements),
                        });
                        current_y = 0.0;
                    }

                    current_page_elements.push(TypesetElement::new(
                        ElementKind::List {
                            ordered: *ordered,
                            items: items.clone(),
                        },
                        current_y,
                        item_height,
                        *line,
                    ));
                    current_y += item_height + 8.0;
                }

                DocumentBlock::Table {
                    caption,
                    headers,
                    rows,
                    has_booktabs,
                    line,
                    ..
                } => {
                    let row_count = rows.len() + 1;
                    let table_height = (row_count as f32) * 22.0 + 32.0;

                    if current_y + table_height > printable_height
                        && !current_page_elements.is_empty()
                    {
                        let page_num = pages.len() + 1;
                        pages.push(TypesetPage {
                            page_number: page_num,
                            total_pages: 0,
                            width: page_width,
                            height: page_height,
                            margin,
                            elements: std::mem::take(&mut current_page_elements),
                        });
                        current_y = 0.0;
                    }

                    current_page_elements.push(TypesetElement::new(
                        ElementKind::Table {
                            caption: caption.clone(),
                            headers: headers.clone(),
                            rows: rows.clone(),
                            has_booktabs: *has_booktabs,
                        },
                        current_y,
                        table_height,
                        *line,
                    ));
                    current_y += table_height + 12.0;
                }

                DocumentBlock::Figure {
                    caption,
                    label,
                    image_path,
                    number,
                    line,
                } => {
                    let fig_height = 100.0;
                    if current_y + fig_height > printable_height
                        && !current_page_elements.is_empty()
                    {
                        let page_num = pages.len() + 1;
                        pages.push(TypesetPage {
                            page_number: page_num,
                            total_pages: 0,
                            width: page_width,
                            height: page_height,
                            margin,
                            elements: std::mem::take(&mut current_page_elements),
                        });
                        current_y = 0.0;
                    }

                    current_page_elements.push(TypesetElement::new(
                        ElementKind::Figure {
                            caption: caption.clone(),
                            label: label.clone(),
                            image_path: image_path.clone(),
                            number: *number,
                        },
                        current_y,
                        fig_height,
                        *line,
                    ));
                    current_y += fig_height + 12.0;
                }

                DocumentBlock::CodeBlock {
                    language,
                    code,
                    line,
                } => {
                    let line_count = code.lines().count().max(1);
                    let code_height = (line_count as f32) * 16.0 + 28.0;

                    if current_y + code_height > printable_height
                        && !current_page_elements.is_empty()
                    {
                        let page_num = pages.len() + 1;
                        pages.push(TypesetPage {
                            page_number: page_num,
                            total_pages: 0,
                            width: page_width,
                            height: page_height,
                            margin,
                            elements: std::mem::take(&mut current_page_elements),
                        });
                        current_y = 0.0;
                    }

                    current_page_elements.push(TypesetElement::new(
                        ElementKind::CodeBlock {
                            language: language.clone(),
                            code: code.clone(),
                        },
                        current_y,
                        code_height,
                        *line,
                    ));
                    current_y += code_height + 10.0;
                }

                DocumentBlock::HorizontalRule { line } => {
                    current_page_elements.push(TypesetElement::new(
                        ElementKind::Divider,
                        current_y,
                        8.0,
                        *line,
                    ));
                    current_y += 12.0;
                }

                DocumentBlock::CenteredBlock { spans, line } => {
                    let total_chars: usize = spans.iter().map(|s| s.text().len()).sum();
                    let est_lines = (total_chars / 60).max(1);
                    let line_height = base_font_size * 1.25;
                    let height = (est_lines as f32) * line_height + 4.0;
                    current_page_elements.push(TypesetElement::new(
                        ElementKind::Paragraph(spans.clone()),
                        current_y,
                        height,
                        *line,
                    ));
                    current_y += height + 8.0;
                }

                DocumentBlock::QuoteBlock { spans, line } => {
                    let total_chars: usize = spans.iter().map(|s| s.text().len()).sum();
                    let est_lines = (total_chars / 65).max(1);
                    let line_height = base_font_size * 1.25;
                    let height = (est_lines as f32) * line_height + 6.0;
                    current_page_elements.push(TypesetElement::new(
                        ElementKind::Paragraph(spans.clone()),
                        current_y,
                        height,
                        *line,
                    ));
                    current_y += height + 10.0;
                }

                DocumentBlock::TableOfContents { line } => {
                    current_page_elements.push(TypesetElement::new(
                        ElementKind::Heading {
                            level: 1,
                            number: String::new(),
                            title: "Contents".to_string(),
                            font_size: 14.0,
                        },
                        current_y,
                        22.0,
                        *line,
                    ));
                    current_y += 28.0;
                }

                DocumentBlock::PageBreak { line: _ } => {
                    if !current_page_elements.is_empty() {
                        let page_num = pages.len() + 1;
                        pages.push(TypesetPage {
                            page_number: page_num,
                            total_pages: 0,
                            width: page_width,
                            height: page_height,
                            margin,
                            elements: std::mem::take(&mut current_page_elements),
                        });
                        current_y = 0.0;
                    }
                }

                DocumentBlock::VerticalSpace { pt, line: _ } => {
                    current_y += pt;
                }
            }
        }

        if !current_page_elements.is_empty() || pages.is_empty() {
            let page_num = pages.len() + 1;
            pages.push(TypesetPage {
                page_number: page_num,
                total_pages: 0,
                width: page_width,
                height: page_height,
                margin,
                elements: current_page_elements,
            });
        }

        let total_pages = pages.len();
        for page in &mut pages {
            page.total_pages = total_pages;
        }

        pages
    }
}
