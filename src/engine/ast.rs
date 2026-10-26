use egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperSize {
    #[default]
    A4,
    Letter,
}

impl PaperSize {
    /// Width and height in points (72 pt = 1 inch).
    pub fn dimensions_pt(&self) -> (f32, f32) {
        match self {
            Self::A4 => (595.28, 841.89),   // 210 x 297 mm
            Self::Letter => (612.0, 792.0), // 8.5 x 11 in
        }
    }
}

#[derive(Debug, Clone)]
pub struct DocumentMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub paper_size: PaperSize,
    pub base_font_size: f32,
    /// in points (54 = 0.75in, 72 = 1in)
    pub margin_pt: f32,
    pub packages: Vec<String>,
}

impl Default for DocumentMetadata {
    fn default() -> Self {
        Self {
            title: None,
            author: None,
            date: None,
            paper_size: PaperSize::A4,
            base_font_size: 11.0,
            margin_pt: 54.0,
            packages: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum InlineSpan {
    Text(String),
    Bold(String),
    /// `\textit` and `\emph`
    Italic(String),
    /// `\texttt`
    Code(String),
    Math(String),
    Colored(Color32, String),
    Url(String),
    Ref(String),
    Cite(String),
}

impl InlineSpan {
    pub fn text(&self) -> &str {
        match self {
            Self::Text(t)
            | Self::Bold(t)
            | Self::Italic(t)
            | Self::Code(t)
            | Self::Math(t)
            | Self::Colored(_, t)
            | Self::Url(t)
            | Self::Ref(t)
            | Self::Cite(t) => t.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone)]
pub struct TableRow {
    pub cells: Vec<Vec<InlineSpan>>,
    pub is_header: bool,
}

/// `line` fields are 0/1-based source lines as produced by the parser, used for SyncTeX-style jumps.
#[derive(Debug, Clone)]
pub enum DocumentBlock {
    TitleBlock {
        line: usize,
    },
    Abstract {
        spans: Vec<InlineSpan>,
        line: usize,
    },
    Section {
        /// 1 = section, 2 = subsection, 3 = subsubsection, 4 = paragraph
        level: usize,
        /// e.g. "1.2"
        number: String,
        title: String,
        line: usize,
    },
    Paragraph {
        spans: Vec<InlineSpan>,
        is_noindent: bool,
        line: usize,
    },
    EquationBlock {
        formula: String,
        number: Option<usize>,
        label: Option<String>,
        line: usize,
    },
    AlignBlock {
        equations: Vec<String>,
        start_number: usize,
        line: usize,
    },
    List {
        /// enumerate vs itemize
        ordered: bool,
        items: Vec<Vec<InlineSpan>>,
        line: usize,
    },
    Table {
        caption: Option<String>,
        label: Option<String>,
        headers: Vec<String>,
        rows: Vec<TableRow>,
        has_booktabs: bool,
        alignments: Vec<ColumnAlign>,
        line: usize,
    },
    Figure {
        caption: Option<String>,
        label: Option<String>,
        image_path: Option<String>,
        number: usize,
        line: usize,
    },
    /// minted / listings / verbatim
    CodeBlock {
        language: String,
        code: String,
        line: usize,
    },
    HorizontalRule {
        line: usize,
    },
    CenteredBlock {
        spans: Vec<InlineSpan>,
        line: usize,
    },
    QuoteBlock {
        spans: Vec<InlineSpan>,
        line: usize,
    },
    TableOfContents {
        line: usize,
    },
    /// `\newpage` / `\clearpage`
    PageBreak {
        line: usize,
    },
    VerticalSpace {
        pt: f32,
        line: usize,
    },
}

impl DocumentBlock {
    pub fn line(&self) -> usize {
        match self {
            Self::TitleBlock { line }
            | Self::Abstract { line, .. }
            | Self::Section { line, .. }
            | Self::Paragraph { line, .. }
            | Self::EquationBlock { line, .. }
            | Self::AlignBlock { line, .. }
            | Self::List { line, .. }
            | Self::Table { line, .. }
            | Self::Figure { line, .. }
            | Self::CodeBlock { line, .. }
            | Self::HorizontalRule { line }
            | Self::CenteredBlock { line, .. }
            | Self::QuoteBlock { line, .. }
            | Self::TableOfContents { line }
            | Self::PageBreak { line }
            | Self::VerticalSpace { line, .. } => *line,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LatexAst {
    pub metadata: DocumentMetadata,
    pub blocks: Vec<DocumentBlock>,
}
