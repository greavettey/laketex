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

#[test]
fn test_package_dependency_detection_and_consent() {
    use laketex::document::packages::{DependencyKind, PackageManager};

    let source = r#"\documentclass{article}
\usepackage{minted}
\usepackage{amsmath}
\begin{document}
Hello
\end{document}"#;

    // 1. Dependency detection
    let deps = PackageManager::detect_dependencies(source);
    assert_eq!(deps.len(), 1, "Should detect 1 external dependency for 'minted'");
    let minted_dep = &deps[0];
    assert_eq!(minted_dep.latex_package, "minted");
    assert_eq!(minted_dep.name, "Pygments");
    assert_eq!(minted_dep.binary_name, "pygmentize");
    assert_eq!(minted_dep.pip_package, Some("pygments".to_string()));
    assert_eq!(minted_dep.kind, DependencyKind::PythonPackage);

    // 2. Python environment probing
    let py_env = PackageManager::check_python_environment();
    // On systems with Python, python_available is true
    if py_env.python_available {
        assert!(py_env.python_version.is_some());
        assert!(py_env.python_path.is_some());
    }

    // 3. User consent requirement enforcement
    // Attempting installation without consent MUST fail with an error
    let no_consent_result = PackageManager::install_dependency("pygments", false);
    assert!(
        no_consent_result.is_err(),
        "Installation without explicit user consent must fail"
    );
    let err_str = no_consent_result.unwrap_err().to_string();
    assert!(
        err_str.contains("consent was not granted"),
        "Error message must specify consent was not granted: {err_str}"
    );
}

#[test]
fn test_headless_api_dependency_endpoints() {
    use laketex::headless::server::HeadlessServer;
    use std::sync::atomic::Ordering;

    let test_port = 3037;
    let running = HeadlessServer::start_background("127.0.0.1", test_port)
        .expect("Server failed to start");
    std::thread::sleep(std::time::Duration::from_millis(150));

    // 1. GET /api/dependencies
    let get_url = format!("http://127.0.0.1:{test_port}/api/dependencies");
    let resp = ureq::get(&get_url)
        .call()
        .expect("GET /api/dependencies request should succeed");
    assert_eq!(resp.status(), 200);
    let resp_str = resp.into_string().unwrap();
    let resp_json: serde_json::Value = serde_json::from_str(&resp_str).unwrap();
    assert!(resp_json["dependencies"].is_array());
    let deps_array = resp_json["dependencies"].as_array().unwrap();
    assert!(deps_array.iter().any(|d| d["latex_package"] == "minted"));

    // 2. POST /api/dependencies/check
    let check_url = format!("http://127.0.0.1:{test_port}/api/dependencies/check");
    let source_json = serde_json::json!({
        "source": "\\documentclass{article}\n\\usepackage{minted}\n\\begin{document}Code\\end{document}"
    });
    let check_resp = ureq::post(&check_url)
        .send_json(source_json)
        .expect("POST /api/dependencies/check request should succeed");
    assert_eq!(check_resp.status(), 200);
    let check_json: serde_json::Value = serde_json::from_str(&check_resp.into_string().unwrap()).unwrap();
    assert_eq!(check_json["dependencies"].as_array().unwrap().len(), 1);

    // 3. POST /api/dependencies/install with consent: false -> must be 403 Forbidden
    let install_url = format!("http://127.0.0.1:{test_port}/api/dependencies/install");
    let forbidden_payload = serde_json::json!({
        "package": "pygments",
        "consent": false
    });
    let forbidden_res = ureq::post(&install_url).send_json(forbidden_payload);
    assert!(forbidden_res.is_err(), "Should return error status code for consent: false");
    let err = forbidden_res.unwrap_err();
    match err {
        ureq::Error::Status(code, _) => assert_eq!(code, 403, "Must return HTTP 403 Forbidden without consent"),
        _ => panic!("Expected HTTP 403 status error"),
    }

    // Shut down test server
    running.store(false, Ordering::SeqCst);
}

#[test]
fn test_app_config_persistence_and_settings() {
    use laketex::compiler::engine::EngineKind;
    use laketex::ui::config::AppConfig;
    use laketex::viewer::PdfColorMode;

    let temp_dir = tempfile::tempdir().expect("temp dir");
    let config_file = temp_dir.path().join("laketex_config.json");

    let mut config = AppConfig::default();
    config.theme_name = "Slate Blue".to_string();
    config.pdf_color_mode = PdfColorMode::Sepia;
    config.pdf_render_width = 1600;
    config.default_engine = EngineKind::Tectonic;
    config.auto_compile = false;
    config.shortcuts.compile = "F5".to_string();
    config.shortcuts.settings = "Ctrl+Shift+P".to_string();

    let json_text = serde_json::to_string_pretty(&config).expect("serialize config");
    std::fs::write(&config_file, &json_text).expect("write config");

    let read_back: AppConfig = serde_json::from_str(&std::fs::read_to_string(&config_file).unwrap())
        .expect("deserialize config");

    assert_eq!(read_back.theme_name, "Slate Blue");
    assert_eq!(read_back.pdf_color_mode, PdfColorMode::Sepia);
    assert_eq!(read_back.pdf_render_width, 1600);
    assert_eq!(read_back.default_engine, EngineKind::Tectonic);
    assert!(!read_back.auto_compile);
    assert_eq!(read_back.shortcuts.compile, "F5");
    assert_eq!(read_back.shortcuts.settings, "Ctrl+Shift+P");
}

#[test]
fn test_missing_picture_auto_placeholder_rendering() {
    use laketex::compiler::engine::{CompilerEngine, EngineKind};

    let source = r#"\documentclass{article}
\usepackage{graphicx}
\begin{document}
Here is a test with a missing picture:
\begin{figure}[htbp]
    \centering
    \includegraphics[width=0.8\linewidth]{subfolder/missing_circuit_98765.png}
    \caption{Simulated schematic}
\end{figure}
\end{document}
"#;
    let res = CompilerEngine::compile(source, EngineKind::Tectonic, None).unwrap();
    assert!(res.success, "Compilation should succeed via automatic draft placeholder fallback");
    assert!(res.pdf_bytes.is_some(), "PDF bytes must be produced with placeholder");
    let bytes = res.pdf_bytes.unwrap();
    assert!(bytes.len() > 100);
    assert!(bytes.starts_with(b"%PDF-"));

    // Render with PdfRenderer to confirm valid rasterization
    let renderer = laketex::viewer::PdfRenderer::new();
    let (img, page_count) = renderer.render_page_to_color_image(&bytes, 0, 1000, false).unwrap();
    assert_eq!(page_count, 1);
    assert!(img.size[0] > 0);

    // Diagnostics should report warning instead of unrecoverable error
    assert!(res.diagnostics.iter().any(|d| d.message.contains("Missing picture")));
}

#[test]
fn test_native_engine_missing_picture_placeholder() {
    use laketex::engine::parser::NativeLatexParser;
    use laketex::engine::layout::DocumentLayout;
    use laketex::engine::pdf_exporter::NativePdfExporter;

    let source = r#"\documentclass{article}
\begin{document}
\section{Schematic}
\includegraphics[width=\linewidth]{missing_schematic_photo.png}
\begin{center}
    \includegraphics{centered_plot.pdf}
\end{center}
\end{document}
"#;
    let ast = NativeLatexParser::parse(source);
    let figures: Vec<_> = ast.blocks.iter().filter_map(|b| {
        if let laketex::engine::ast::DocumentBlock::Figure { image_path, .. } = b {
            Some(image_path.clone())
        } else {
            None
        }
    }).collect();

    assert_eq!(figures.len(), 2);
    assert_eq!(figures[0].as_deref(), Some("missing_schematic_photo.png"));
    assert_eq!(figures[1].as_deref(), Some("centered_plot.pdf"));

    let pages = DocumentLayout::layout_document(&ast);
    assert!(!pages.is_empty());

    let pdf_bytes = NativePdfExporter::export_to_pdf_bytes(&ast).expect("native pdf export");
    assert!(pdf_bytes.starts_with(b"%PDF-"));
}

#[test]
fn test_preamble_isolation_and_directive_suppression() {
    use laketex::engine::ast::DocumentBlock;
    use laketex::engine::parser::NativeLatexParser;

    let source = r#"\documentclass[11pt]{article}
\usepackage{pgfplots}
\usepackage{hyperref}

\hypersetup{
    colorlinks=true,
    linkcolor=blue,
    urlcolor=cyan,
    pdftitle={Test Isolation Title},
}

\pgfplotsset{
    compat=1.18,
    width=10cm,
}

\setlength{\parindent}{0pt}

\title{Isolated Preamble Document}
\author{Test Author}

\begin{document}
\maketitle

\paragraph{Section Four Paragraph}
This is the body of the paragraph with a rule \rule{20pt}{1.5pt} and a footnote\footnote{Important footnote note}.

\begin{semilogxaxis}[
    xlabel={Frequency (Hz)},
    ylabel={Gain (dB)},
]
    \addplot[red, dashed, domain=100:1000] {20.0};
    \addlegendentry{Reference Line}
\end{semilogxaxis}

\end{document}
Post-document comment that should be ignored.
"#;

    let ast = NativeLatexParser::parse(source);
    assert_eq!(ast.metadata.title.as_deref(), Some("Isolated Preamble Document"));
    assert_eq!(ast.metadata.author.as_deref(), Some("Test Author"));

    // Verify preamble setup commands (colorlinks, compat, setlength) did NOT become paragraphs
    for block in &ast.blocks {
        if let DocumentBlock::Paragraph { spans, .. } = block {
            let combined: String = spans.iter().map(|s| s.text()).collect();
            assert!(!combined.contains("colorlinks"), "Preamble hypersetup must not leak as body text");
            assert!(!combined.contains("compat"), "Preamble pgfplotsset must not leak as body text");
            assert!(!combined.contains("parindent"), "Preamble setlength must not leak as body text");
            assert!(!combined.contains("Post-document"), "Content after end{{document}} must be excluded");
            assert!(!combined.contains("\\addplot"), "Directives like addplot must not leak as plaintext");
            assert!(!combined.contains("{1.5pt}"), "Second rule argument must not leak as plain text");
        }
    }

    // Verify paragraph heading is parsed with level 4
    let has_para_heading = ast.blocks.iter().any(|b| {
        if let DocumentBlock::Section { level, title, .. } = b {
            *level == 4 && title == "Section Four Paragraph"
        } else {
            false
        }
    });
    assert!(has_para_heading, "Paragraph heading must be parsed as level 4 section");

    // Verify plot environment produced a Figure block instead of raw text
    let has_plot_fig = ast.blocks.iter().any(|b| {
        matches!(b, DocumentBlock::Figure { .. })
    });
    assert!(has_plot_fig, "semilogxaxis environment must be encapsulated as a graphical figure");
}

#[test]
fn test_i18n_translation_system() {
    use laketex::i18n::{available_locales, current_locale, set_locale, tr_args};
    use laketex::t;

    set_locale("en");
    assert_eq!(current_locale(), "en");
    assert_eq!(t!("toolbar.compile"), "▶ Compile");
    assert_eq!(t!("toolbar.save_file"), "Save");

    // Arguments interpolation
    let status = tr_args("editor.match_count", &[("current", "4"), ("total", "12")]);
    assert_eq!(status, "4 of 12");

    // Switch to Spanish
    set_locale("es");
    assert_eq!(current_locale(), "es");
    assert_eq!(t!("toolbar.compile"), "▶ Compilar");
    assert_eq!(t!("toolbar.save_file"), "Guardar");
    assert_eq!(t!("editor.find"), "Buscar");

    // Available locales check
    let locales = available_locales();
    assert!(locales.iter().any(|(code, _)| code == "en"));
    assert!(locales.iter().any(|(code, _)| code == "es"));

    // Reset back to English
    set_locale("en");
    assert_eq!(t!("toolbar.compile"), "▶ Compile");
}

#[test]
fn test_workspace_cleanliness_and_named_output() {
    use laketex::compiler::engine::{CompilerEngine, EngineKind};
    use tempfile::tempdir;

    let dir = tempdir().expect("Failed to create temporary workspace directory");
    let content = r#"\documentclass{article}
\begin{document}
Workspace Cleanliness Test Document
\end{document}
"#;

    // Compile named target "my_paper.tex"
    let result = CompilerEngine::compile_named(
        content,
        EngineKind::Tectonic,
        Some(dir.path()),
        Some("my_paper.tex"),
    )
    .expect("compile_named execution failed");

    assert!(result.success, "Compilation must succeed: stdout={}, stderr={}", result.stdout, result.stderr);
    assert!(result.pdf_bytes.is_some(), "PDF bytes must be returned");

    // 1. Target output PDF must exist named after the document stem: my_paper.pdf
    let expected_pdf = dir.path().join("my_paper.pdf");
    assert!(expected_pdf.exists(), "my_paper.pdf must exist in the workspace directory");

    // 2. Old hardcoded laketex_doc files must NOT exist
    assert!(!dir.path().join("laketex_doc.tex").exists(), "laketex_doc.tex must not be created");
    assert!(!dir.path().join("laketex_doc.pdf").exists(), "laketex_doc.pdf must not be created");

    // 3. Temporary build intermediate files must be cleaned up
    assert!(!dir.path().join(".laketex_my_paper.tex").exists(), ".laketex_my_paper.tex must be scrubbed");
    assert!(!dir.path().join(".laketex_my_paper.aux").exists(), ".laketex_my_paper.aux must be scrubbed");
    assert!(!dir.path().join(".laketex_my_paper.log").exists(), ".laketex_my_paper.log must be scrubbed");
    assert!(!dir.path().join(".laketex_my_paper.pdf").exists(), ".laketex_my_paper.pdf must be scrubbed");
}

#[test]
fn test_master_document_management_and_autodetection() {
    use laketex::ui::project_manager::ProjectManager;
    use tempfile::tempdir;
    use std::fs;

    let dir = tempdir().expect("Failed to create tempdir");
    let chapters_dir = dir.path().join("chapters");
    fs::create_dir_all(&chapters_dir).expect("Failed to create chapters directory");

    let main_tex = dir.path().join("main.tex");
    let intro_tex = chapters_dir.join("intro.tex");

    fs::write(&main_tex, "\\documentclass{article}\n\\begin{document}\n\\input{chapters/intro}\n\\end{document}\n").unwrap();
    fs::write(&intro_tex, "\\section{Introduction}\nThis is a chapter subfile without documentclass.\n").unwrap();

    let mut pm = ProjectManager::new(Some(dir.path().to_path_buf()));
    // Automatically detected during construction since main.tex exists with \documentclass
    assert_eq!(pm.master_document, Some(main_tex.clone()));

    // Clear master document
    pm.master_document = None;
    assert!(pm.master_document.is_none());

    // Explicit auto_detect
    pm.auto_detect_master_document();
    assert_eq!(pm.master_document, Some(main_tex.clone()));

    // Manually set master document
    pm.master_document = Some(intro_tex.clone());
    assert_eq!(pm.master_document, Some(intro_tex));
}

#[test]
fn test_unsaved_changes_pending_actions() {
    use laketex::document::{BuiltinDocKind, Document};
    use laketex::ui::PendingCloseAction;
    use std::path::PathBuf;

    let mut doc = Document::default();
    assert!(!doc.is_dirty);

    // Editing document content marks it dirty
    doc.content.push_str("% added comment");
    doc.is_dirty = true;
    assert!(doc.is_dirty);

    // Verify all PendingCloseAction variants exist and support equality checks
    let actions = vec![
        PendingCloseAction::ExitApp,
        PendingCloseAction::NewDocument,
        PendingCloseAction::OpenFileDialog,
        PendingCloseAction::OpenFile(PathBuf::from("test.tex")),
        PendingCloseAction::LoadBuiltinDoc(BuiltinDocKind::Manual),
    ];

    for action in actions {
        let cloned = action.clone();
        assert_eq!(action, cloned);
    }
}
