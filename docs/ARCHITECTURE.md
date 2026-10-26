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
| [`src/main.rs`](../src/main.rs) | 101 LOC | CLI entry point, argument dispatcher, and Windows GUI launcher |
| [`src/lib.rs`](../src/lib.rs) | 69 LOC | Library root exposing compiler, document, engine, headless, and viewer modules |
| [`src/error.rs`](../src/error.rs) | 48 LOC | Unified strongly-typed error hierarchy (`LakeTexError`) |
| **`src/engine/`** | **5,700 LOC** | **In-house pure-Rust typesetting, AST, layout, render debugger, and PDF engine** |
| `src/engine.rs` | 32 LOC | Engine subsystem root, re-exports, and public interfaces |
| `src/engine/ast.rs` | 297 LOC | Strongly-typed AST for LaTeX documents with line tracking and paper sizes |
| `src/engine/parser.rs` | 1,603 LOC | Fault-tolerant recursive descent parser designed for incomplete draft code |
| `src/engine/math.rs` | 951 LOC | Nested fraction parser, matrix formatter, super/subscripts, AMS symbols |
| `src/engine/layout.rs` | 615 LOC | Document paginator, A4/Letter margins, equation numbering, element bounding boxes |
| `src/engine/native_renderer.rs` | 563 LOC | Instant vector canvas renderer and double-click hit tester |
| `src/engine/pdf_exporter.rs` | 297 LOC | Vector PDF generator using `printpdf` with authentic academic styling |
| `src/engine/render_debugger.rs` | 846 LOC | Typographic & matrix render flaw auditor with automated fix suggestions |
| `src/engine/syntax.rs` | 496 LOC | Syntax highlighting token rules, style definitions, and HTML generator |
| **`src/ui/`** | **8,662 LOC** | **Reactive GUI shell, editor panel, project manager, drawer, and theme engine** |
| `src/ui.rs` | 1,773 LOC | Application shell, event loop, stateful API bridge, shortcut handlers, close modal |
| `src/ui/editor_panel.rs` | 1,123 LOC | Syntax-highlighted code editor, bracket tree jump badge, line gutter, find/replace |
| `src/ui/project_manager.rs` | 2,096 LOC | Overleaf-style project files tree, file operations, master document pinning |
| `src/ui/drawer.rs` | 854 LOC | Collapsible drawer with diagnostics, render audit, logs, and in-memory audit log |
| `src/ui/settings.rs` | 765 LOC | Preferences modal, shortcut remapping, engine and font configuration |
| `src/ui/theme.rs` | 750 LOC | Custom theme loader (JSON/YAML), JetBrains Dracula, Synthwave Neon, and semantic syntax colors |
| `src/ui/config.rs` | 372 LOC | Application configuration file serialization and persistence |
| `src/ui/sidebar.rs` | 330 LOC | Document outline tree, package inspector, math snippet inserter |
| `src/ui/logo.rs` | 290 LOC | Bundled logo graphics, brand text layout, and app icon generators |
| `src/ui/toolbar.rs` | 234 LOC | Top toolbar, cancel compile button, duration badges, engine switcher |
| `src/ui/viewer_panel.rs` | 190 LOC | Dual-mode viewer selector, page navigator, and reverse SyncTeX dispatcher |
| **`src/compiler/`** | **2,357 LOC** | **Engine orchestration, worker threads, CTAN provisioning, process cancellation** |
| `src/compiler.rs` | 22 LOC | Compiler module root, engine abstraction, and worker re-exports |
| `src/compiler/engine.rs` | 1,109 LOC | Multi-engine runner (`Native`, `Tectonic`, `PdfLatex`, `XeLatex`, `LuaLatex`) |
| `src/compiler/error_report.rs` | 427 LOC | Contextual error diagnostic report generator (Markdown/JSON/log) |
| `src/compiler/provisioner.rs` | 337 LOC | Package dependency resolution, Python/Conda discovery, CTAN metadata |
| `src/compiler/worker.rs` | 254 LOC | Background worker thread with debounced compile channel and cancel token |
| `src/compiler/diagnostic.rs` | 208 LOC | Regex-based LaTeX/TeX log diagnostic parser (errors, warnings, underfull/overfull) |
| **`src/document/`** | **2,179 LOC** | **Document buffer, bracket tree lexing, package dependency detection** |
| `src/document.rs` | 95 LOC | In-memory document model with line/word count caches and file I/O |
| `src/document/packages.rs` | 1,189 LOC | Automated `\usepackage` detector and non-destructive preamble injector |
| `src/document/lexer.rs` | 1,350 LOC | Tokenizer with tag property & package lexing, bracket tree parser, and syntax highlighter |
| `src/document/outline.rs` | 182 LOC | Section, subsection, equation, table, and label structural tree extractor |
| `src/document/builtin_docs.rs` | 82 LOC | Bundled offline LaTeX documentation manuals catalog |
| **`src/viewer/`** | **772 LOC** | **PDF canvas, PDFium integration, and visual parity comparator** |
| `src/viewer.rs` | 179 LOC | Embedded PDFium wrapper, rasterizer, and dynamic library binder |
| `src/viewer/canvas.rs` | 324 LOC | High-DPI raster canvas with zoom, invert colors, and click coordinate mapping |
| `src/viewer/parity.rs` | 191 LOC | Visual Parity Harness: dual-engine comparator, MSE/PSNR calculation, diff PNG generator |
| `src/viewer/color_mode.rs` | 78 LOC | Color grading modes (Daylight White, Academic Dark, Sepia Parchment) |
| **`src/headless/`** | **2,674 LOC** | **CLI subcommands, stateful/stateless HTTP REST daemon, agent specification** |
| `src/headless.rs` | 10 LOC | Headless automation root, CLI entry points, and REST microservice re-exports |
| `src/headless/server.rs` | 2,053 LOC | Stateful/stateless REST API server daemon, security checks, agent JSON spec |
| `src/headless/cli.rs` | 611 LOC | Clap command parser with safe printing helpers (`BrokenPipe` resilience) |
| **`src/i18n.rs`** | **253 LOC** | **Internationalization catalog, translation macros, and locale loader** |
| **`tests/`** | **1,457 LOC** | **Automated unit and integration test suites** |
| `tests/document_tests.rs` | 1,423 LOC | Lexer, diagnostic parser, outline, packages, stateful API, and engine tests |
| `tests/visual_parity_tests.rs` | 34 LOC | Automated visual parity benchmark between Tectonic and Native engines |
| **Total** | **~24,200 LOC** | **Complete standalone LaTeX studio and compiler pipeline** |

---

## 3. Frontend Architecture

### 3.1. Reactive Event Loop (`src/ui.rs`)
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
- `laketex api-spec`: Emits the full machine-readable AI agent JSON specification.
- `laketex docs [--list | --print | --export <dir>]`: Lists, prints, or exports bundled LaTeX documentation manuals.

---

## 7. Stateful GUI REST Bridge & Lock-Free Command Queue (`SharedAppState`)

When launched with `laketex --with-api --with-state`, Laketex bridges the asynchronous HTTP REST server thread pool with the immediate-mode desktop GUI render thread (`eframe`/`egui`) via `SharedAppState`:

```
[ External AI Agent / REST Client ]
                |
                v  (HTTP JSON Request)
+-------------------------------------------------------+
|  HEADLESS SERVER WORKER THREADS (src/headless/server) |
|  * CSRF / Loopback Security Validator                 |
|  * Reads non-blocking LiveEditorSnapshot              |
|  * Enqueues StatefulUiCommand                         |
|  * Calls ctx.request_repaint()                        |
+-------------------------------------------------------+
                |
                | Arc<SharedAppState>
                v
+-------------------------------------------------------+
|  LOCK-FREE COMMAND BRIDGE                             |
|  * Arc<Mutex<VecDeque<StatefulUiCommand>>>            |
|  * Arc<Mutex<LiveEditorSnapshot>>                     |
|  * egui::Context handle                               |
+-------------------------------------------------------+
                |
                | Drained each frame in update()
                v
+-------------------------------------------------------+
|  DESKTOP GUI RENDER THREAD (src/ui.rs)                |
|  * process_stateful_api_commands()                    |
|    - SetDocumentContent / Surgical Edits              |
|    - Navigate (caret line, preview page, drawer tabs) |
|    - SetEngine / TriggerCompilation                   |
|  * sync_shared_api_state() flushes snapshot back     |
+-------------------------------------------------------+
```

### Key Architectural Invariants:
1. **Thread Decoupling**: The HTTP worker never acquires locks across frame boundaries. Read endpoints (`GET /api/document`, `GET /health`) access an atomic snapshot without halting GPU rendering.
2. **Deterministic Command Sequencing**: Mutations (`POST /api/document`, `POST /api/navigate`) push typed `StatefulUiCommand` enum variants into a FIFO queue and notify `egui::Context::request_repaint()`.
3. **Surgical Document Patching**: `DocumentEditOperation` allows clients to apply precise `line_range` or `search_replace` edits without full document serialization overhead.
4. **Zero-Drift Snapshot Synchronization**: At the end of every GUI frame, `LakeTexApp::sync_shared_api_state()` synchronizes active document contents, dirty status, caret position, and diagnostics back to the snapshot, ensuring clients always observe consistent state.

---

## 8. Bracket Tree Parsing Pipeline (`document::lexer::BracketTree`)

To power balanced syntax highlighting and the interactive `{…}` header jump badge, Laketex implements a dedicated, fault-tolerant bracket tree lexical analyzer in `src/document/lexer.rs`:

```
Raw Source Code ───> Lexical Scan ───> Bracket Interval Stack ───> BracketTree Hierarchy
                                             |
                         +-------------------+-------------------+
                         |                                       |
                         v                                       v
             Depth-Aware Syntax Highlighting        Paired Delimiter Jump Badge
             (Color-coded nested bracket pairs)     (Caret context -> Matching Line)
```

### Architectural Details:
1. **Comment & Escape Isolation**: Delimiter scanning ignores escaped characters (`\{`, `\}`, `\[`, `\]`) and skips TeX comment spans (`% ... \n`), avoiding false-positive bracket pairing.
2. **Half-Open Interval Representation**: Delimiters are indexed as half-open byte intervals `(start_byte, end_byte)` paired with integer nesting depth.
3. **Resilient Error Recovery**: Malformed, unmatched, or half-open LaTeX math brackets (`\[ ...` without `\]`) do not discard prior valid intervals; the tree closes unmatched spans gracefully at document boundaries.
4. **Enclosing Pair Resolution**: When the editor caret moves, `BracketTree::find_enclosing_pair(byte_offset)` performs a fast binary search over active intervals to identify the tightest enclosing pair, calculating the exact target line for the `{…}` jump badge in $O(\log N)$ time.
