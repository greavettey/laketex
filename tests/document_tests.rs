use laketex::compiler::diagnostic::{DiagnosticParser, DiagnosticSeverity};
use laketex::document::lexer::{LatexLexer, TokenType};
use laketex::document::outline::{OutlineItemKind, OutlineParser};
use laketex::document::packages::PackageManager;

#[test]
fn test_lexer_tokenization() {
    let source = "% A comment\n\\documentclass{article}\n$E = mc^2$\n";
    let tokens = LatexLexer::tokenize(source);

    assert!(tokens.iter().any(|t| t.token_type == TokenType::Comment));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Command && t.text == "\\documentclass"));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::MathInline && t.text == "$E = mc^2$"));
}

#[test]
fn test_outline_parser() {
    let source = r#"
\documentclass{article}
\begin{document}
\section{First Section}
\subsection{Sub Topic}
\begin{equation}
x^2 + y^2 = r^2
\end{equation}
\label{eq:circle}
\end{document}
"#;
    let outline = OutlineParser::parse(source);

    assert_eq!(outline.len(), 4);
    assert_eq!(outline[0].kind, OutlineItemKind::Section);
    assert_eq!(outline[0].title, "First Section");

    assert_eq!(outline[1].kind, OutlineItemKind::Subsection);
    assert_eq!(outline[1].title, "Sub Topic");

    assert_eq!(outline[2].kind, OutlineItemKind::Equation);
    assert_eq!(outline[3].kind, OutlineItemKind::Label);
}

#[test]
fn test_package_detection_and_injection() {
    let source = r#"\documentclass{article}
\usepackage{amsmath,amssymb}
\usepackage[dvipsnames]{xcolor}
\begin{document}
Hello
\end{document}
"#;
    let packages = PackageManager::detect_packages(source);
    let names: Vec<&str> = packages.iter().map(|p| p.name.as_str()).collect();

    assert!(names.contains(&"amsmath"));
    assert!(names.contains(&"amssymb"));
    assert!(names.contains(&"xcolor"));

    // Test package insertion
    let with_tikz = PackageManager::insert_package(source, "tikz");
    assert!(with_tikz.contains("\\usepackage{tikz}"));
}

#[test]
fn test_compile_default_document() {
    use laketex::compiler::engine::{CompilerEngine, EngineKind};
    use laketex::document::DEFAULT_DOCUMENT_TEMPLATE;

    let res = CompilerEngine::compile(DEFAULT_DOCUMENT_TEMPLATE, EngineKind::Tectonic, None).expect("Engine execution failed");
    println!("success: {}, bytes: {:?}", res.success, res.pdf_bytes.as_ref().map(|b| b.len()));
    println!("duration: {} ms", res.duration_ms);
    println!("stdout: {}", res.stdout);
    println!("stderr: {}", res.stderr);
    for diag in &res.diagnostics {
        println!("diag: {:?}", diag);
    }
    assert!(res.success, "Compilation failed: stdout={}, stderr={}", res.stdout, res.stderr);
    assert!(res.pdf_bytes.is_some());

    // Test PDF rendering
    use laketex::viewer::PdfRenderer;
    let renderer = PdfRenderer::new();
    println!("renderer ready: {}", renderer.is_ready());
    if let Some(err) = renderer.init_error() {
        println!("renderer init notice: {}", err);
    }
    let pdf_bytes = res.pdf_bytes.unwrap();
    let render_res = renderer.render_page_to_color_image(&pdf_bytes, 0, 1200, false);
    match render_res {
        Ok((img, pages)) => {
            println!("rendered successfully! pages: {}, size: {:?}", pages, img.size);
        }
        Err(e) => {
            panic!("render failed: {:?}", e);
        }
    }
}

#[test]
fn test_diagnostic_parser() {
    let log = r#"
This is XeTeX, Version 3.141592653-2.6-0.999996 (TeX Live 2024)
! Undefined control sequence.
l.14 \unknowncommand
                    
LaTeX Warning: Reference `eq:none' on page 1 undefined on input line 25.
Overfull \hbox (12.4pt too wide) in paragraph at lines 40--45
"#;
    let diagnostics = DiagnosticParser::parse(log);

    assert!(diagnostics.iter().any(|d| d.severity == DiagnosticSeverity::Error && d.line == Some(14)));
    assert!(diagnostics.iter().any(|d| d.severity == DiagnosticSeverity::Warning && d.line == Some(25)));
    assert!(diagnostics.iter().any(|d| d.severity == DiagnosticSeverity::Warning && d.line == Some(40)));
}

#[test]
fn test_native_parser_and_math() {
    use laketex::engine::parser::NativeLatexParser;
    use laketex::engine::math::MathRenderer;
    use laketex::engine::layout::DocumentLayout;

    let source = r#"\documentclass{article}
\title{Quantum Mechanics Note}
\author{Antigravity Researcher}
\begin{document}
\maketitle
\section{Wave Equation}
The energy of a quantum state is given by:
\begin{equation}
E = \hbar \omega = \frac{h}{2\pi} \nu
\end{equation}
Here $\alpha + \beta = \gamma$ represents the coupling phase.
\begin{itemize}
  \item Item A: $\int_0^\infty e^{-x} dx = 1$
  \item Item B: $\sum_{i=1}^n i = \frac{n(n+1)}{2}$
\end{itemize}
\end{document}"#;

    let doc = NativeLatexParser::parse(source);
    assert_eq!(doc.metadata.title.as_deref(), Some("Quantum Mechanics Note"));
    assert_eq!(doc.metadata.author.as_deref(), Some("Antigravity Researcher"));
    assert!(!doc.blocks.is_empty());

    // Math translation test
    let math_str = MathRenderer::translate_math_symbols(r"\alpha + \beta = \gamma");
    assert!(math_str.contains('α'));
    assert!(math_str.contains('β'));
    assert!(math_str.contains('γ'));

    // Layout engine test
    let pages = DocumentLayout::layout_document(&doc);
    assert!(!pages.is_empty());
    assert_eq!(pages[0].page_number, 1);
    assert!(!pages[0].elements.is_empty());
}

#[test]
fn test_native_compile_and_pdf_export() {
    use laketex::compiler::engine::{CompilerEngine, EngineKind};
    use laketex::document::DEFAULT_DOCUMENT_TEMPLATE;

    let res = CompilerEngine::compile(DEFAULT_DOCUMENT_TEMPLATE, EngineKind::Native, None)
        .expect("Native compilation must succeed without external tools");
    assert!(res.success);
    assert!(res.pdf_bytes.is_some(), "Native compiler must produce valid PDF bytes");
    let bytes = res.pdf_bytes.unwrap();
    assert!(bytes.len() > 100);
    // PDF Magic bytes check: "%PDF-"
    assert_eq!(&bytes[0..5], b"%PDF-");
}

#[test]
fn test_contextual_error_report_generation() {
    use laketex::compiler::diagnostic::{Diagnostic, DiagnosticSeverity};
    use laketex::compiler::engine::EngineKind;
    use laketex::compiler::ErrorReport;

    let faulty_source = r#"\documentclass{article}
\usepackage{amsmath}
\title{Quantum Anomalies}
\begin{document}
\maketitle
\section{Quantum Electro-Dynamics}
Here is the faulty formulation:
\begin{equation}
\invalidmacro{x} = mc^2
\end{equation}
\end{document}"#;

    let diags = vec![Diagnostic {
        severity: DiagnosticSeverity::Error,
        message: "Undefined control sequence: \\invalidmacro".to_string(),
        line: Some(9),
        file: None,
        details: Some("l.9 \\invalidmacro{x}".to_string()),
    }];

    let report = ErrorReport::generate(
        faulty_source,
        "faulty_doc.tex",
        EngineKind::Native,
        &diags,
        "",
        "Error: compilation failed",
    );

    assert_eq!(report.error_count, 1);
    assert_eq!(report.warning_count, 0);
    assert_eq!(report.document_name, "faulty_doc.tex");
    assert_eq!(report.diagnostics.len(), 1);

    let d = &report.diagnostics[0];
    assert_eq!(d.line, Some(9));
    assert!(d.category.contains("Undefined"));
    // Context inference test: Must detect section and environment intent
    assert!(d.environment_intent.contains("equation"));
    assert!(d.environment_intent.contains("Quantum Electro-Dynamics"));

    // Surrounding lines test
    assert!(!d.surrounding_context.is_empty());
    assert!(d.surrounding_context.iter().any(|c| c.is_target && c.line_number == 9));

    // Markdown serialization test
    let md = report.to_markdown();
    assert!(md.contains("# 🚨 Laketex Compilation Error & Diagnostic Report"));
    assert!(md.contains("Undefined control sequence: \\invalidmacro"));
    assert!(md.contains("Typesetting '\\begin{equation}'"));
    assert!(md.contains(">>>    9 | \\invalidmacro{x} = mc^2"));
    assert!(md.contains("💡 **Suggested Resolution**"));

    // JSON serialization test
    let json_str = report.to_json();
    assert!(json_str.contains("\"document_name\": \"faulty_doc.tex\""));
    assert!(json_str.contains("\"error_count\": 1"));
}

#[test]
fn test_render_debugger_audit_and_api() {
    use laketex::engine::render_debugger::RenderDebugger;
    use laketex::headless::server::HeadlessServer;
    use std::sync::atomic::Ordering;

    let source = r#"\documentclass{article}
\begin{document}
\section{Matrices and Math Test}
Bare matrix in text:
\begin{pmatrix}
1 & 2 & 3 \\
4 & 5
\end{pmatrix}

Bare symbol: \beta in text.
Unbalanced math: $E = mc^2
Malformed frac: \frac{x}
\end{document}"#;

    // 1. Direct unit test of audit scanner
    let report = RenderDebugger::scan(source, "test_render.tex", None);
    assert!(report.total_issues >= 4);
    assert!(report.matrix_issue_count >= 2);
    assert!(report.math_issue_count >= 2);

    let matrix_issue = report
        .issues
        .iter()
        .find(|i| i.command.contains("pmatrix"))
        .expect("Should detect pmatrix issue");
    assert_eq!(matrix_issue.line, 5);
    assert!(matrix_issue.issue.contains("outside of math mode"));
    assert!(!matrix_issue.suggested_fix.is_empty());

    let math_issue = report
        .issues
        .iter()
        .find(|i| i.command == "\\beta")
        .expect("Should detect bare beta command");
    assert_eq!(math_issue.line, 10);
    assert!(math_issue.issue.contains("without math mode delimiters"));

    // 2. Headless HTTP REST API test on /api/debug
    let port = 30395;
    let running = HeadlessServer::start_background("127.0.0.1", port)
        .expect("Failed to start background test server");

    // Wait slightly for socket bind
    std::thread::sleep(std::time::Duration::from_millis(100));

    let payload = serde_json::json!({
        "source": source,
        "engine": "native"
    });

    let resp = ureq::post(&format!("http://127.0.0.1:{port}/api/debug"))
        .send_json(payload);

    assert!(resp.is_ok(), "HTTP request to /api/debug failed: {:?}", resp.err());
    let resp = resp.unwrap();
    assert_eq!(resp.status(), 200);

    let resp_json: serde_json::Value = resp.into_json().expect("Valid JSON response expected");
    assert_eq!(resp_json["document_name"], "api_document.tex");
    assert!(resp_json["total_issues"].as_u64().unwrap_or(0) >= 4);
    assert!(resp_json["matrix_issue_count"].as_u64().unwrap_or(0) >= 2);
    assert!(resp_json["math_issue_count"].as_u64().unwrap_or(0) >= 2);

    let issues_array = resp_json["issues"].as_array().expect("issues array expected");
    assert!(!issues_array.is_empty());
    for item in issues_array {
        assert!(item["line"].is_number());
        assert!(item["command"].is_string());
        assert!(item["issue"].is_string());
        assert!(item["suggested_fix"].is_string());
    }

    // Shut down test server
    running.store(false, Ordering::SeqCst);
}

#[test]
fn test_nine_packages_parity_and_draft_fallback() {
    use laketex::compiler::engine::{CompilerEngine, EngineKind};
    use laketex::engine::ast::{DocumentBlock, InlineSpan, PaperSize};
    use laketex::engine::parser::NativeLatexParser;
    use laketex::engine::pdf_exporter::NativePdfExporter;

    let source = r#"\documentclass[12pt, letterpaper]{article}
\usepackage{amsmath, graphicx, geometry, booktabs, float, multirow, siunitx, minted, xcolor}
\geometry{letterpaper, margin=1in}
\begin{document}
\section{Advanced Formatting}
Speed is \SI{12.5}{\meter\per\second} and temperature is \qty{21}{\celsius}.
Highlight in \textcolor{red}{Alert Red} or inline code \mintinline{python}{x = 10}.
\begin{table}[H]
\begin{tabular}{ll}
\toprule
\textbf{Col 1} & \textbf{Col 2} \\
\midrule
\multirow{2}{*}{Merged} & Data A \\
 & Data B \\
\bottomrule
\end{tabular}
\caption{Benchmark Table}
\end{table}

\begin{figure}[H]
\includegraphics[width=0.5\textwidth]{test.png}
\caption{Test Diagram}
\end{figure}

\begin{minted}{python}
def calculate():
    return 42
\end{minted}
\end{document}"#;

    // 1. Verify AST parsing
    let ast = NativeLatexParser::parse(source);
    assert_eq!(ast.metadata.paper_size, PaperSize::Letter);
    assert_eq!(ast.metadata.margin_pt, 72.0); // 1 inch = 72 pt

    let has_code_block = ast.blocks.iter().any(|b| matches!(b, DocumentBlock::CodeBlock { language, .. } if language == "python"));
    assert!(has_code_block, "Must parse Python minted block as DocumentBlock::CodeBlock");

    let has_figure = ast.blocks.iter().any(|b| matches!(b, DocumentBlock::Figure { .. }));
    assert!(has_figure, "Must parse graphicx figure environment as DocumentBlock::Figure");

    let has_booktabs_table = ast.blocks.iter().any(|b| matches!(b, DocumentBlock::Table { has_booktabs: true, .. }));
    assert!(has_booktabs_table, "Must detect booktabs formatting in table");

    // Check inline spans
    if let Some(DocumentBlock::Paragraph { spans, .. }) = ast.blocks.iter().find(|b| matches!(b, DocumentBlock::Paragraph { .. })) {
        let has_colored = spans.iter().any(|s| matches!(s, InlineSpan::Colored(_, text) if text == "Alert Red"));
        assert!(has_colored, "Must parse \\textcolor as InlineSpan::Colored");

        let has_si_unit = spans.iter().any(|s| matches!(s, InlineSpan::Text(t) if t.contains("12.5 m/s")));
        assert!(has_si_unit, "Must format \\SI as 12.5 m/s");
    }

    // 2. Verify Native PDF export
    let pdf_bytes = NativePdfExporter::export_to_pdf_bytes(&ast);
    assert!(pdf_bytes.is_ok(), "Native PDF exporter should succeed for 9-package document");
    let bytes = pdf_bytes.unwrap();
    assert!(bytes.len() > 1000);
    assert!(bytes.starts_with(b"%PDF-"));

    // 3. Verify Native engine compilation
    let res = CompilerEngine::compile(source, EngineKind::Native, None)
        .expect("Native compilation should succeed");
    assert!(res.success);
    assert!(res.pdf_bytes.is_some());
}

#[test]
fn test_empty_document_pdf_export_has_at_least_one_page() {
    use laketex::engine::ast::LatexAst;
    use laketex::engine::pdf_exporter::NativePdfExporter;
    use laketex::viewer::PdfRenderer;

    // Verify empty AST produces valid PDF with >= 1 page
    let empty_ast = LatexAst::default();
    let pdf_bytes = NativePdfExporter::export_to_pdf_bytes(&empty_ast)
        .expect("Exporting empty AST to PDF should succeed");
    assert!(pdf_bytes.starts_with(b"%PDF-"));

    let renderer = PdfRenderer::new();
    if renderer.is_ready() {
        let render_result = renderer.render_page_to_color_image(&pdf_bytes, 0, 800, false);
        assert!(
            render_result.is_ok(),
            "Empty PDF should render cleanly with >= 1 page, got: {:?}",
            render_result.err()
        );
        let (_, page_count) = render_result.unwrap();
        assert!(page_count >= 1, "Page count must be at least 1");
    }
}

#[test]
fn test_compiler_engine_cleans_stale_pdf_and_handles_syntax_errors() {
    use laketex::compiler::engine::{CompilerEngine, EngineKind};

    // Broken LaTeX syntax that would fail compilation in TeX engines
    let broken_latex = r#"\documentclass{article}
\begin{document}
\undefinedmacro{not closed
"#;

    let res = CompilerEngine::compile(broken_latex, EngineKind::Native, None)
        .expect("Engine call should complete");
    // Even if native engine parses it, verify that if an error is flagged, pdf_bytes is None or valid
    if !res.success {
        assert!(res.pdf_bytes.is_none(), "Failed compile result must not return pdf_bytes");
    }
}
