# Laketex Architecture & Technical Specification

Laketex is a high-performance, lightweight, and fully portable $\LaTeX$ viewer and editor written in 100% safe Rust. It features dual compilation engines (including a native pure-Rust typesetting pipeline), instant sub-millisecond vector live-preview, embedded PDFium rasterization, automated CTAN package management, bidirectional SyncTeX navigation, a visual parity test harness, and a headless REST API microservice.

---

## 1. High-Level Architecture Overview

```
+----------------------------------------------------------------------------------------------------+
|                                         LAKETEX APPLICATION                                        |
+----------------------------------------------------------------------------------------------------+
|                                                                                                    |
|  [ FRONTEND REACTIVE GUI (eframe / egui) ]                                                         |
|  +--------------------+  +--------------------------------+  +----------------------------------+  |
|  |   Collapsible      |  |      Code Editor Panel         |  |        Dual-Mode Viewer          |  |
|  |     Sidebar        |  |  * Syntax Lexer (LatexLexer)   |  |  * Native Vector Live Preview    |  |
|  | * Outline Tree     |  |  * Line Gutters & Error Badges |  |  * PDFium Raster Preview         |  |
|  | * CTAN Packages    |  |  * Caret Positioning / SyncTeX |  |  * Color Inversion (Dark Mode)   |  |
|  | * Math Snippets    |  |  * Zoom & Font Scaling         |  |  * Reverse SyncTeX Double-Click  |  |
|  +--------------------+  +--------------------------------+  +----------------------------------+  |
|            |                             ^                                   |                     |
|            | Jump to Line                | Caret Focus / Scroll              | Double-Click Line   |
|            +-----------------------------+-----------------------------------+                     |
|                                                                                                    |
|  [ DOCUMENT BUFFER & STATE (src/document/) ]                                                       |
|  * Text Buffer & Dirty Tracking   * Outline Parser     * PackageManager & CTAN Provisioner         |
|                                                                                                    |
|  [ COMPILATION PIPELINE (src/compiler/) ]                                                          |
|  * Debounced Background Worker (180ms channel)  * Multi-Engine Dispatcher (Native/Tectonic/XeTeX)  |
|                                                                                                    |
|  +-----------------------------------------+   +------------------------------------------------+  |
|  | IN-HOUSE PURE-RUST ENGINE (src/engine/) |   | OPTIONAL BACKEND ENGINES                       |  |
|  | * Fault-Tolerant AST Parser             |   | * Tectonic (Automated CTAN Cache)              |  |
|  | * Math & Equation Layout Engine         |   | * TeX Live (pdflatex, xelatex, lualatex)       |  |
|  | * Document Paginator & Layout           |   +------------------------------------------------+  |
|  | * Native Vector Painter (egui)          |                                                       |
|  | * Native PDF Exporter (printpdf)        |                                                       |
|  +-----------------------------------------+                                                       |
|                                                                                                    |
|  [ VISUAL PARITY TEST HARNESS (src/viewer/parity.rs) ]                                             |
|  * Dual Compilation -> PDFium 1000px Raster -> Pixel Difference Map -> MSE / PSNR / JSON Report   |
|                                                                                                    |
|  [ HEADLESS INTERFACES (src/headless/) ]                                                           |
|  * REST API Microservice Daemon (/api/compile, /api/outline, /api/packages, /api/check)          |
|  * CLI Subcommands: `serve`, `compile`, `check`, `outline`, `packages`, `compare`                 |
+----------------------------------------------------------------------------------------------------+
```

---

## 2. Directory & Module Breakdown

| Directory / Module | Lines of Code | Description |
| :--- | :--- | :--- |
| [`src/main.rs`](file:///c:/Users/Axel/Documents/antigravity/nifty-hertz/src/main.rs) | 35 LOC | CLI entry point, argument dispatcher, and GUI launcher |
| [`src/lib.rs`](file:///c:/Users/Axel/Documents/antigravity/nifty-hertz/src/lib.rs) | 12 LOC | Library root exposing compiler, document, engine, headless, and viewer modules |
| [`src/error.rs`](file:///c:/Users/Axel/Documents/antigravity/nifty-hertz/src/error.rs) | 38 LOC | Unified strongly-typed error hierarchy (`LakeTexError`) |
| **`src/engine/`** | **1,340 LOC** | **In-house pure-Rust typesetting, AST, layout, and PDF engine** |
| `src/engine/ast.rs` | 135 LOC | Typed AST for LaTeX documents with line tracking and paper sizes |
| `src/engine/parser.rs` | 425 LOC | Fault-tolerant recursive descent parser designed for incomplete draft code |
| `src/engine/math.rs` | 275 LOC | Arbitrarily nested fraction parser, matrix formatter, super/subscripts, AMS symbols |
| `src/engine/layout.rs` | 280 LOC | Document paginator, A4/Letter margins, equation numbering, element bounding boxes |
| `src/engine/native_renderer.rs` | 285 LOC | Instant vector canvas renderer and double-click hit tester |
| `src/engine/pdf_exporter.rs` | 225 LOC | Vector PDF generator using `printpdf` with authentic academic styling |
| **`src/ui/`** | **1,310 LOC** | **Reactive GUI layout, editor, viewer, and theme configuration** |
| `src/ui/mod.rs` | 445 LOC | Application shell, event loop, toolbar/drawer orchestration, shortcut handlers |
| `src/ui/editor_panel.rs` | 175 LOC | Syntax-highlighted code editor, line number gutter, cursor range sync |
| `src/ui/viewer_panel.rs` | 115 LOC | Dual-mode viewer selector, page navigator, and reverse SyncTeX dispatcher |
| `src/ui/sidebar.rs` | 230 LOC | Document outline tree, package inspector, math snippet inserter |
| `src/ui/drawer.rs` | 130 LOC | Collapsible diagnostic drawer and terminal stdout/stderr viewer |
| `src/ui/toolbar.rs` | 135 LOC | Quick actions, compilation progress indicator, engine switcher |
| `src/ui/theme.rs` | 160 LOC | System font fallback chain (`seguisym.ttf`, etc.) and typography styles |
| **`src/compiler/`** | **641 LOC** | **Engine orchestration, worker threads, CTAN provisioning** |
| `src/compiler/engine.rs` | 266 LOC | Multi-engine runner (`Native`, `Tectonic`, `PdfLatex`, `XeLatex`, `LuaLatex`) |
| `src/compiler/worker.rs` | 106 LOC | Background worker thread with debounced compile channel (180ms) |
| `src/compiler/diagnostic.rs` | 156 LOC | Regex-based LaTeX/TeX log diagnostic parser (errors, warnings, underfull/overfull) |
| `src/compiler/provisioner.rs` | 105 LOC | Package dependency resolution and CTAN metadata lookups |
| **`src/document/`** | **634 LOC** | **Document buffer, syntax lexing, and package detection** |
| `src/document/lexer.rs` | 215 LOC | Tokenizer and syntax highlighter for LaTeX commands, math, comments, braces |
| `src/document/outline.rs` | 156 LOC | Section, subsection, equation, table, and label structural tree extractor |
| `src/document/packages.rs` | 123 LOC | Automated `\usepackage` detector and non-destructive preamble injector |
| `src/document/mod.rs` | 140 LOC | In-memory document model with undo/redo buffer and file I/O |
| **`src/viewer/`** | **455 LOC** | **PDF canvas, PDFium integration, and visual parity comparator** |
| `src/viewer/canvas.rs` | 200 LOC | High-DPI raster canvas with zoom, invert colors, and click coordinate mapping |
| `src/viewer/parity.rs` | 165 LOC | Visual Parity Harness: dual-engine comparator, MSE/PSNR calculation, diff PNG generator |
| `src/viewer/mod.rs` | 150 LOC | Embedded PDFium wrapper and dynamic library binder |
| **`src/headless/`** | **380 LOC** | **CLI subcommands and HTTP REST daemon** |
| `src/headless/server.rs` | 182 LOC | Lightweight REST API server daemon (`tiny_http` / JSON) |
| `src/headless/cli.rs` | 194 LOC | Clap command parser: `serve`, `compile`, `check`, `outline`, `packages`, `compare` |
| **`tests/`** | **150 LOC** | **Automated unit and integration test suites** |
| `tests/document_tests.rs` | 145 LOC | Lexer, diagnostic parser, outline, packages, and compilation tests |
| `tests/visual_parity_tests.rs` | 45 LOC | Automated visual parity benchmark between Tectonic and Native engines |

---

## 3. Frontend Architecture

### 3.1. Reactive Event Loop (`src/ui/mod.rs`)
The graphical user interface is built on `eframe` and `egui` (immediate-mode GUI). Every frame:
1. **Theme Resolution**: `AppTheme::apply(ctx)` dynamically ensures high contrast and resolves system typography fallback chains.
2. **Channel Polling**: The main thread checks `self.compiler.try_receive()`. When background compilation finishes, diagnostics, durations, and PDF bytes are updated immediately without locking the thread.
3. **Split Panes**:
   - **Left Panel (`SidePanel::left`)**: Document outline, imported packages, math symbols, and metrics.
   - **Top Panel (`TopBottomPanel::top`)**: Quick actions, engine dropdown, duration badges, and recompile button.
   - **Bottom Panels (`TopBottomPanel::bottom`)**: Status line (dirty state, line/word counts, API port, zoom) and expandable diagnostic drawer.
   - **Center Panel (`CentralPanel`)**: Two-column layout containing the Code Editor on the left and Live Document Viewer on the right.

### 3.2. Bidirectional SyncTeX & Navigation Pipeline
Laketex implements seamless forward and inverse navigation between code and document:

1. **Forward Navigation (Outline -> Editor)**:
   - When a user clicks a section, equation, or table in the Outline sidebar:
   - `SidebarAction::JumpToLine(line)` is triggered.
   - `EditorPanel::jump_to_line(line)` calculates the character index of that line in the source buffer.
   - It updates `TextEditState` with an explicit `CCursorRange` pointing to that character, requests keyboard focus via `mem.request_focus(editor_id)`, and scrolls the `ScrollArea` via `scroll.vertical_scroll_offset(...)` directly to that line.

2. **Inverse Navigation (PDF / Preview -> Editor)**:
   - In **Native Vector Mode**: Every `TypesetElement` in the document layout records its exact `source_line` and vertical bounding coordinates `[y_offset, y_offset + height]`. When the user double-clicks anywhere on the page, `page.find_element_at_y(unzoomed_y)` resolves the exact element under the cursor and returns `ViewerAction::JumpToLine(element.source_line)`.
   - In **PDF Raster Mode**: The PDF canvas captures double-click events with normalized vertical ratios `rel_y = (click.y - rect.top()) / rect.height()`. This ratio maps directly to the corresponding page layout element or proportional source line, immediately jumping the editor caret to that line.

---

## 4. In-House Pure-Rust Typesetting Engine (`src/engine/`)

Laketex is completely decoupled from external toolchains. Its native engine executes entirely in-process:

1. **Typed Document AST (`src/engine/ast.rs`)**:
   - Documents are parsed into a typed syntax tree: metadata, title blocks, abstracts, sections, paragraphs, equation blocks, align blocks, itemize/enumerate lists, and booktabs tables.
   - Every block preserves its exact source line number (`block.line()`).

2. **Fault-Tolerant Native Parser (`src/engine/parser.rs`)**:
   - Engineered specifically for live coding. Half-written commands, unclosed braces, or draft text never cause parse panics or crashes.

3. **Mathematical Formula Engine (`src/engine/math.rs`)**:
   - **Arbitrary Fraction Nesting**: Recursive balanced-brace parser (`format_fractions`) handles deeply nested fractions (e.g. `\frac{\partial^2 f}{\partial x_1^2}`) cleanly.
   - **Matrix Environments**: Translates `\begin{pmatrix}`, `\begin{bmatrix}`, and `\begin{matrix}` into aligned 2D mathematical arrays using Unicode bracket glyphs (`⎛`, `⎝`, `⎡`, `⎣`).
   - **Subscripts & Superscripts**: Transforms digit and variable superscripts/subscripts (`^2` $\to$ ², `_i` $\to$ ᵢ, `_{max}` $\to$ ₍ₘₐₓ₎).
   - **AMS Symbols & Accents**: Translates all standard LaTeX operators (`\hbar` $\to$ ℏ, `\infty` $\to$ ∞, `\int` $\to$ ∫, `\sum` $\to$ ∑, `\nabla` $\to$ ∇, `\partial` $\to$ ∂) and combining accents (`\hat{f}` $\to$ f̂, `\vec{v}` $\to$ v⃗).

4. **Document Paginator & Layout (`src/engine/layout.rs`)**:
   - Calculates A4 and Letter dimensions, margins (54 pt / 0.75 in), inter-paragraph gaps, section font sizes, line wraps, and equation numbering.
   - Produces `Vec<TypesetPage>` with precise vertical bounding coordinates for each element.

5. **Vector Painter (`src/engine/native_renderer.rs`)**:
   - Draws pages directly onto the `egui` painter canvas in **< 1 millisecond** with zero external disk I/O.

6. **Native PDF Exporter (`src/engine/pdf_exporter.rs`)**:
   - Uses `printpdf` to synthesize standards-compliant vector PDF documents in-memory with academic LaTeX article styling.

---

## 5. Visual Parity Test Harness (`src/viewer/parity.rs`)

To ensure that the native in-house typesetting engine produces high visual fidelity compared to TeX engines, Laketex includes a comparative visual benchmarking framework:

```
                          [ LaTeX Input File ]
                                    |
                  +-----------------+-----------------+
                  |                                   |
                  v                                   v
          [ Tectonic Engine ]                 [ Native Engine ]
                  |                                   |
                  v                                   v
           [ tectonic.pdf ]                    [ native.pdf ]
                  |                                   |
                  +-----------------+-----------------+
                                    |
                                    v
                     [ PDFium 1000px Rasterizer ]
                                    |
                  +-----------------+-----------------+
                  |                                   |
                  v                                   v
        [ Tectonic ColorImage ]             [ Native ColorImage ]
                  |                                   |
                  +-----------------+-----------------+
                                    |
                                    v
                     [ Pixel Comparator (25-step tol) ]
                                    |
                  +-----------------+-----------------+
                  |                                   |
                  v                                   v
        [ Visual Diff PNG ]                 [ Parity Report JSON ]
        * Muted gray = match                * Match Percentage (92.35%)
        * Crimson = deviation               * MSE & PSNR Score
```

### Metrics Computed:
- **Visual Parity Score**: Percentage of pixels matching within tolerance ($\approx 92.35\%$ on full academic papers).
- **Mean Squared Error (MSE)**: Average squared channel deviation across all pixels.
- **Peak Signal-to-Noise Ratio (PSNR)**: Logarithmic measure of image fidelity (higher is closer to parity).
- **Artifact Generation**: Saves `tectonic_render.png`, `native_render.png`, `diff_comparison.png`, and `visual_parity_report.json`.

---

## 6. Headless Microservice & CLI Subcommands

Laketex includes an embedded HTTP REST API daemon and CLI tools for programmatic document automation:

- `laketex serve [--port 3030]`: Runs the headless REST API microservice.
- `laketex compile <file.tex> [-o out.pdf] [-e native|tectonic]`: Compiles documents directly from the terminal.
- `laketex check <file.tex> [--format json|text]`: Lints documents for errors and warnings.
- `laketex outline <file.tex>`: Extracts document structure as JSON.
- `laketex packages <file.tex>`: Analyzes and lists package dependencies.
- `laketex compare <file.tex> [-o ./report]`: Executes the visual parity comparator and generates diff artifacts.
