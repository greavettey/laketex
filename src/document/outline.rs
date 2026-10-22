//! # Document Outline & Structural Navigation
//!
//! Parses LaTeX source into a hierarchical table of contents ([`OutlineItem`]) containing
//! sections, subsections, figures, tables, math blocks, and labels with their line locations.
//!
//! Powers the sidebar outline tree and click-to-jump SyncTeX navigation into the code editor.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Structural category of an outline entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutlineItemKind {
    /// Document part (`\part{...}`)
    Part,
    /// Book or report chapter (`\chapter{...}`)
    Chapter,
    /// Major section (`\section{...}`)
    Section,
    /// Second-level section (`\subsection{...}`)
    Subsection,
    /// Third-level section (`\subsubsection{...}`)
    Subsubsection,
    /// Paragraph header (`\paragraph{...}`)
    Paragraph,
    /// Math equation block (`\begin{equation}`, `\begin{align}`)
    Equation,
    /// Floated figure environment (`\begin{figure}`)
    Figure,
    /// Floated table environment (`\begin{table}`)
    Table,
    /// Cross-reference anchor label (`\label{...}`)
    Label,
    /// Literature citation (`\cite{...}`)
    Citation,
}

impl OutlineItemKind {
    /// Indentation depth level for sidebar rendering
    pub fn depth(&self) -> usize {
        match self {
            Self::Part => 0,
            Self::Chapter => 1,
            Self::Section => 2,
            Self::Subsection => 3,
            Self::Subsubsection => 4,
            Self::Paragraph => 5,
            Self::Equation | Self::Figure | Self::Table => 3,
            Self::Label | Self::Citation => 4,
        }
    }

    /// Short unicode glyph or textual icon representing the item type
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Part => "[Part]",
            Self::Chapter => "[Ch]",
            Self::Section => "§",
            Self::Subsection => "§§",
            Self::Subsubsection => "¶",
            Self::Paragraph => "p.",
            Self::Equation => "∑",
            Self::Figure => "[Fig]",
            Self::Table => "[Tab]",
            Self::Label => "[lbl]",
            Self::Citation => "[cite]",
        }
    }
}

/// A parsed outline navigation node in the document hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineItem {
    /// Kind of outline element
    pub kind: OutlineItemKind,
    /// Extracted title or label description
    pub title: String,
    /// 1-based source line number where this element begins
    pub line: usize,
    /// Visual indentation hierarchy level
    pub level: usize,
}

/// Extractor for document structural outlines.
pub struct OutlineParser;

impl OutlineParser {
    /// Extracts structured outline items with line numbers from LaTeX source.
    pub fn parse(source: &str) -> Vec<OutlineItem> {
        let mut items = Vec::new();

        for (line_idx, line_raw) in source.lines().enumerate() {
            let line_number = line_idx + 1;
            let trimmed = line_raw.trim();

            if trimmed.starts_with('%') {
                continue;
            }

            if let Some(title) = Self::extract_arg(trimmed, "\\part") {
                items.push(OutlineItem {
                    kind: OutlineItemKind::Part,
                    title,
                    line: line_number,
                    level: 0,
                });
            } else if let Some(title) = Self::extract_arg(trimmed, "\\chapter") {
                items.push(OutlineItem {
                    kind: OutlineItemKind::Chapter,
                    title,
                    line: line_number,
                    level: 1,
                });
            } else if let Some(title) = Self::extract_arg(trimmed, "\\section") {
                items.push(OutlineItem {
                    kind: OutlineItemKind::Section,
                    title,
                    line: line_number,
                    level: 2,
                });
            } else if let Some(title) = Self::extract_arg(trimmed, "\\subsection") {
                items.push(OutlineItem {
                    kind: OutlineItemKind::Subsection,
                    title,
                    line: line_number,
                    level: 3,
                });
            } else if let Some(title) = Self::extract_arg(trimmed, "\\subsubsection") {
                items.push(OutlineItem {
                    kind: OutlineItemKind::Subsubsection,
                    title,
                    line: line_number,
                    level: 4,
                });
            } else if let Some(title) = Self::extract_arg(trimmed, "\\paragraph") {
                items.push(OutlineItem {
                    kind: OutlineItemKind::Paragraph,
                    title,
                    line: line_number,
                    level: 5,
                });
            } else if trimmed.contains("\\begin{equation}") || trimmed.contains("\\begin{align}") {
                items.push(OutlineItem {
                    kind: OutlineItemKind::Equation,
                    title: "Equation Block".to_string(),
                    line: line_number,
                    level: 3,
                });
            } else if trimmed.contains("\\begin{figure}") {
                items.push(OutlineItem {
                    kind: OutlineItemKind::Figure,
                    title: "Figure".to_string(),
                    line: line_number,
                    level: 3,
                });
            } else if trimmed.contains("\\begin{table}") {
                items.push(OutlineItem {
                    kind: OutlineItemKind::Table,
                    title: "Table".to_string(),
                    line: line_number,
                    level: 3,
                });
            } else if let Some(label) = Self::extract_arg(trimmed, "\\label") {
                items.push(OutlineItem {
                    kind: OutlineItemKind::Label,
                    title: format!("label: {label}"),
                    line: line_number,
                    level: 4,
                });
            }
        }

        items
    }

    fn extract_arg(line: &str, command: &str) -> Option<String> {
        if let Some(pos) = line.find(command) {
            let rest = &line[pos + command.len()..];
            let rest = if let Some(stripped) = rest.strip_prefix('*') {
                stripped
            } else {
                rest
            };

            let rest = rest.trim_start();
            if rest.starts_with('{') {
                if let Some(end_brace) = rest.find('}') {
                    let content = &rest[1..end_brace];
                    return Some(content.trim().to_string());
                }
            }
        }
        None
    }
}
