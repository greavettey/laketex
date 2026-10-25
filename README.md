<div align="center">
  <img src="assets/LaketexLogo.png" alt="Laketex Logo" width=350>
  <h1><em>L</em>aketex</h1>
</div>

A lightweight, portable LaTeX editor, live viewer, and typesetting studio written in safe Rust.

Laketex is designed as a single, self-contained executable that gives you a complete, publication-grade LaTeX writing environment with zero setup. It packages a bundled Tectonic engine for full TeX package compatibility alongside an in-process, pure-Rust typesetting engine for instant sub-millisecond drafting, high-DPI dual-mode vector viewing, bi-directional SyncTeX navigation, a visual parity testing harness, and a headless REST API enabling agentic document creation.

> *Why call it "Laketex"?*
> 
> I was born and raised in Muskoka, which is a region of Ontario containing over 1600 lakes. I moved to a city for post-secondary, and to put it simply, I miss the lake.  

---

## The Ideas Behind the Project

If you have ever written an academic paper, research report, or thesis in LaTeX, you are probably familiar with the traditional dilemma:

1. **The Traditional Local Distribution Route**: Setting up a local TeX environment traditionally means downloading TeX Live or MiKTeX. That typically means downloading between 5 and 8 gigabytes of data, running installers that touch system PATH variables, managing Perl-based package scripts, and waiting several seconds every time you recompile a document. For people on modern laptops, developers who value clean systems, or researchers working across multiple machines, this is heavy and cumbersome.
2. **The Cloud SaaS Route**: Because local TeX setup is notoriously friction-heavy, many writers moved to browser-based cloud tools like Overleaf. While cloud editors eliminate the installation headache, they introduce their own set of compromises: they require an active internet connection, lock your files behind accounts and subscription tiers, suffer from browser and network latency on every recompile, and present privacy and confidentiality concerns for proprietary research, industrial documents, or sensitive personal data.

Laketex was built to bridge this gap.

The goal was simple: What if you could have the speed, privacy, and portability of a modern native desktop application—like Sublime Text or VS Code—paired with a complete, publication-ready LaTeX typesetting workflow, distributed as a single standalone executable that you can drop into a folder and run immediately?

---

## Foundational Principles

Laketex is guided by five foundational principles:

### 1. Zero-Install Portability
Laketex ships as an all-in-one binary. There are no system installers, no administrator privileges required, no background daemons, and no registry pollution. You can carry it on a USB flash drive, copy it to an air-gapped machine, or drop it into a project folder and start writing immediately.

### 2. Extreme Local Privacy and Offline Autonomy
Your research, mathematical derivations, notes, and manuscripts belong entirely on your computer. Laketex does not mandate telemetry, user accounts, or cloud subscriptions. Both the editor and its compilation pipelines run strictly on your local hardware. Whether you are on an airplane, in a field station without internet access, or working in a secure lab, your tools remain 100% operational.

### 3. Immediate Cognitive Flow
Writing complex mathematics requires intense concentration. Waiting 3 to 10 seconds every time you recompile breaks your train of thought. Laketex emphasizes low-latency feedback. Edits trigger instant, debounced recompilation, and the pure-Rust native engine can typeset draft pages in 5 to 75 milliseconds directly in memory, giving you visual feedback while you type.

### 4. Pragmatic Dual-Engine Architecture
Rather than pretending a pure-Rust AST parser can immediately replace 45 years of Donald Knuth's TeX virtual machine quirks and the millions of packages on CTAN—or alternatively forcing users to endure a massive external distribution—Laketex combines both:
- An in-process **pure-Rust native engine** for instant offline drafting and low-latency feedback.
- A **bundled, cross-platform Tectonic engine** as the default production typesetter for 100% package compatibility, TikZ drawings, and publication-grade micro-typography.

### 5. Ergonomics and Thoughtful Typography
Academic writing involves long hours in front of a screen. Laketex provides high-DPI rendering, eye-friendly color modes (including Daylight White, Academic Dark, and Sepia parchment), intuitive bi-directional SyncTeX navigation, a working directory project manager, and contextual error diagnostics that explain what went wrong in plain human English rather than dumping raw TeX logs.

---

## Dual Engines: Built-in Rust Typesetter vs. Bundled Tectonic

One of the central design questions when building Laketex was whether to write a LaTeX typesetter from scratch in pure Rust or wrap an existing TeX engine. We decided to do both, because each solves a different, vital part of the writing experience.

### The Built-in Pure-Rust Engine (`src/engine/`)

The pure-Rust engine is an entirely custom typesetting pipeline written in safe Rust. It does not spawn external processes, write intermediate files to disk, or invoke external command-line tools.

- **How it works**: Source text is tokenized by `LatexLexer` and parsed by `NativeLatexParser` into a structured Abstract Syntax Tree (`LatexAst`). The mathematical formatter (`MathRenderer`) parses arbitrary nested fraction trees, AMS matrices (`pmatrix`, `bmatrix`, `vmatrix`), and Unicode operators. The layout engine (`DocumentLayout`) formats pages with page margins, equation numbers, and text layout jobs (`egui::text::LayoutJob`) that perform word wrapping and drop shadow rendering. Vector PDFs are exported in-memory using `printpdf`.
- **Performance**: In-memory compilation takes between 5 and 75 milliseconds.
- **When to use it**: It is ideal for taking lecture notes, sketching out equations, working completely offline without an internet cache, or writing straightforward academic articles where instant feedback is paramount.

### Why Tectonic is the Bundled Default Engine

While our pure-Rust engine handles standard academic structures, equations, tables, and lists with remarkable speed, Donald Knuth's TeX is fundamentally different from static markup languages like Markdown or HTML:

1. **Turing-Complete Macro Expansion**: LaTeX is not merely markup; it is an imperative, Turing-complete macro language. Packages dynamically alter category codes (`\catcode`), compute dimensional arithmetic (`\dimexpr`), execute loops (`\loop`), and redefine commands at runtime. Packages like `tikz`, `pgfplots`, `biblatex`, and `hyperref` rely heavily on deep macro evaluation.
2. **Math Micro-Typography (Knuth's Appendix G)**: TeX formats mathematics using the 22 rules defined in Appendix G of The TeXbook. It relies on precise font metric bounding boxes (`.tfm` files and OpenType `MATH` tables) to dynamically calculate fraction vinculum baselines, radical hook alignments, delimiter stretch steps, and italic slant corrections. Replicating 100% visual parity across all arbitrary mathematical expressions requires a genuine TeX engine.
3. **The Global Knuth-Plass Paragraph Breaking Algorithm**: Rather than wrapping text line-by-line, TeX evaluates penalties, glue elasticity, and badness across an entire paragraph simultaneously to achieve optimal typographic color and avoid awkward hyphenation.

To deliver true publication-grade output without forcing users to install TeX Live, we chose **Tectonic**.

Tectonic modernizes the battle-tested Web2C XeTeX engine into clean Rust and C, eliminating the archaic TeX directory structure and format file initialization steps. Most importantly, Tectonic streams required CTAN packages (`.sty`, `.cls`, font metrics) on demand over HTTPS and caches them locally.

**We bundle Tectonic directly into Laketex on all supported platforms (Windows, macOS, and Linux).**
The compressed binary is embedded directly into the application's read-only data section. On first compile, Laketex unpacks Tectonic in roughly 350 milliseconds. There is nothing to install, no PATH configuration, and no external dependency hunting. You get genuine TeX typography and CTAN package support out of the box.

---

## Real-World Benchmarks

All benchmarks were measured on a standard developer workstation (AMD Ryzen 7, 32 GB RAM, Windows 11) using identical academic article source files (including title blocks, abstracts, mathematical formulations, matrices, booktabs tables, and citations).

### Startup Time

| Application | Cold Startup Time | Runtime Architecture | Installation Overhead |
| :--- | :--- | :--- | :--- |
| **Laketex** | **< 0.08 s** | Native Rust + Immediate-Mode GPU UI | Single ~33.4 MB binary |
| TeXstudio | 3.20 s | C++ / Qt GUI | ~180 MB + 5–8 GB TeX Live |
| VS Code + LaTeX Workshop | 4.50 s | Electron / TypeScript | ~520 MB + 5–8 GB TeX Live |
| Overleaf | 1.80 s – 3.50 s | Web Browser / Remote SaaS | Browser + Account Login |

### Compilation Latency

| Engine / Pipeline | Average Build Time | Execution Mechanism | Network Dependency |
| :--- | :--- | :--- | :--- |
| **Laketex Native (Pure Rust)** | **12 ms – 65 ms** | In-Process Memory Pipeline | 100% Offline (Zero Network) |
| **Laketex Bundled Tectonic** | **350 ms – 680 ms** | Bundled Executable + Local CTAN Cache | Auto-downloads on first import |
| External pdfLaTeX (TeX Live) | 850 ms – 1,450 ms | Spawns Subprocess | Requires Pre-installed Distro |
| External XeLaTeX (TeX Live) | 1,200 ms – 2,200 ms | Spawns Subprocess | Requires Pre-installed Distro |

### Memory Footprint

| State | Laketex | TeXstudio | VS Code + LaTeX Workshop |
| :--- | :--- | :--- | :--- |
| **Idle (Document Open)** | **~38 MB** | ~180 MB | ~680 MB |
| **Active Typing (Live Preview)**| **~52 MB** | ~210 MB | ~740 MB |
| **Peak Compilation Load** | **~68 MB** | ~280 MB | ~890 MB |

### Disk Footprint

| Component | Laketex | Traditional TeX Setup (TeX Live) |
| :--- | :--- | :--- |
| **Editor Binary** | ~33.4 MB (Includes PDFium & Tectonic) | ~180 MB (TeXstudio only) |
| **TeX Distribution** | 0 MB (Embedded in binary) | 5,000 MB – 8,000 MB (TeX Live full) |
| **Total Disk Requirement** | **~33.4 MB** | **5,200 MB – 8,200 MB** |

### Native Engine Visual Parity Metrics

Using our built-in automated parity harness (`laketex compare`), we benchmarked the pure-Rust Native engine against Tectonic across standard academic papers rendered at 1000px width:

| Metric | Result | Interpretation |
| :--- | :--- | :--- |
| **Visual Match Percentage** | **92.35%** | High structural and text alignment parity |
| **Mean Squared Error (MSE)** | **0.021** | Minimal pixel deviation on standard text |
| **Peak Signal-to-Noise Ratio (PSNR)**| **26.8 dB** | Excellent visual fidelity for draft viewing |

---

## Capabilities Built Into Laketex

Over the course of developing Laketex, we built a comprehensive set of features designed to make writing technical documents fast, predictable, and pleasant:

### Dual-Mode Document Viewing
- **PDF Vector View (Favored & Default)**: High-resolution live preview powered by embedded PDFium. Renders genuine TeX font glyphs, vector lines, and embedded graphics with crisp anti-aliasing.
- **Native Vector View**: In-process pure-Rust vector canvas featuring true word wrapping (`egui::text::LayoutJob`), precise margin bounds, drop shadow framing, and zoomed mathematical equations.
- **Eye-Care Color Grading**: Switch effortlessly between **Daylight White** (default), **Academic Dark Inverted**, and **Sepia Eye-Care Parchment**, complete with full internationalization (i18n) support.

### Bi-Directional SyncTeX Navigation
- **Forward Navigation**: Click any section, subsection, equation, or table in the Outline sidebar to jump the editor cursor directly to that line.
- **Inverse Navigation**: Double-click any paragraph, formula, or heading in either preview canvas to scroll the editor buffer straight to the corresponding source line.

### Contextual Diagnostics and Render Audit
- **Human-Readable Error Analysis**: Cryptic TeX errors are translated into structured Markdown cards that pinpoint the exact source line, display surrounding code context, infer what the document was attempting to do, and provide concrete suggestions for fixes.
- **Deep Render Audit**: The `debug` engine scans documents for unclosed matrix delimiters, missing `amsmath` imports, unbalanced braces, and math commands placed in plain text.
- **One-Click Export**: Easily copy formatted error reports to your clipboard or export them to disk (`.md`, `.json`, `.log`) for collaboration or debugging.

### Safe Package Dependency Consent & Conda Integration
- When documents import packages that depend on external host tools (such as `minted` requiring Python's `pygments`, or `pythontex`), Laketex inspects system prerequisites and prompts for explicit user permission before installing anything.
- **Robust Conda & VirtualEnv Support**: Laketex automatically discovers active virtual environments (`VIRTUAL_ENV`), active Conda environments (`CONDA_PREFIX`), and all environments registered in `~/.conda/environments.txt`. It supports Miniconda, Anaconda, Miniforge, Mambaforge, and Micromamba across Windows, macOS, and Linux.
- **Multi-Strategy Installation & Fallback**: Package installation tries `pip`, user-level install (`--user`), PEP 668 managed environment override (`--break-system-packages`), and seamlessly falls back to `conda install -y -c conda-forge` if `pip` is absent in a Conda environment.
- **Post-Install Verification**: Never fails silently—Laketex validates that the required binary exists on disk before closing the modal, and automatically injects containing scripts/bin paths into the compiler process `PATH` so `shell-escape` succeeds immediately.
- If consent is declined or tools are unavailable, Laketex automatically injects safe draft fallback options (`\PassOptionsToPackage{draft}{minted}`) so your document continues to compile cleanly without crashing.

### Working Directory Project Manager (`Ctrl+P`)
- Integrated project files tree for multi-file LaTeX manuscripts.
- Pin master documents (`main.tex`) so sub-files compile in their proper context.
- Quick asset insertion: insert images (`\includegraphics`), sub-files (`\input`), citations, and math snippets with a single click.

### Live Find and Replace Drawer (`Ctrl+F` / `Ctrl+H`)
- Dedicated search drawer supporting case-sensitive matching, whole-word filtering, and live occurrence counts.
- Step through occurrences with Next and Previous buttons or execute Replace and Replace All across the document.

### Flexible Key-Value Theming Engine
- Choose between built-in palettes: **Dracula (JetBrains)**, **Academic Dark**, **Slate Minimal**, **Nord Frost**, and **Paper White**.
- Fully user-customizable: drop simple `.json` or `.yaml` files into a `themes/` folder alongside the executable. Laketex hot-reloads them instantly without restarting.

### Headless CLI and Embedded REST API
- Run Laketex headlessly in continuous integration pipelines, automated build scripts, or agentic coding workflows.
- Built-in commands: `compile`, `check`, `debug`, `packages`, `compare`, and `serve`.
- The embedded REST daemon provides JSON endpoints (`/api/compile`, `/api/check`, `/api/outline`, `/api/debug`) for programmatic document generation.

---

## Feature Support Matrix

| LaTeX Feature / Package | Pure-Rust Native Engine | Bundled Tectonic Engine | Architectural Details |
| :--- | :---: | :---: | :--- |
| **Document Structure** (`\title`, `\section`, `\subsection`) | Supported | Supported | Native maps sections to clean hierarchy and outline tree. |
| **Text Inlines** (`\textbf`, `\textit`, `\emph`, `\texttt`) | Supported | Supported | Full typographical style mapping in both vector canvas and PDF. |
| **Escaped Symbols** (`\%`, `\$`, `\&`, `\#`, `\_`) | Supported | Supported | Correctly escaped without macro leaks. |
| **Equations** (`equation`, `\[...\]`, `$...$`) | Supported | Supported | Centered equation numbering and balanced parsing. |
| **Nested Fractions** (`\frac{a}{b}`) | Supported | Supported | Native recursively nests fractions with horizontal vinculum rules. |
| **Matrices** (`pmatrix`, `bmatrix`, `vmatrix`, `matrix`) | Supported | Supported | Multi-row grids with dynamic bracket and parenthesis fences. |
| **Lists** (`itemize`, `enumerate`, `description`) | Supported | Supported | Bullet indents, numbering sequences, custom item labels. |
| **Tables & Tabular** (`tabular`, `\hline`, `\toprule`) | Supported | Supported | Alignment specifiers (`l`, `c`, `r`), multi-row, and multi-column cells. |
| **Code Highlighting (`minted`, `lstlisting`)** | Supported | Supported | Tectonic renders via Pygments; Native uses safe draft lexer. |
| **Missing Image Handling** (`\includegraphics`) | Supported | Supported | Inserts clean placeholder bounding box if image is missing. |
| **Cross-References** (`\label`, `\ref`, `\cite`) | Supported | Supported | In-memory reference resolution and numeric citation numbering. |
| **Vector Drawings (`tikz`, `pgfplots`)** | Suppressed in Draft | Supported | Native isolates directives to prevent leaks; use Tectonic. |
| **Custom Macro Expansion (`\def`, `\newcommand`)** | Basic Substitution | Supported | Native parses simple parameter substitution; use Tectonic for loops. |
| **Advanced Micro-Typing (`microtype`)** | Not Supported | Supported | Optical margin expansion requires TeX virtual machine. |
| **Custom TrueType Fonts (`fontspec`)** | Not Supported | Supported | Native uses embedded academic Unicode fallbacks. |
| **Offline Independence** | 100% Offline | Requires Cache | Native needs zero network; Tectonic caches packages from CTAN. |

---

## Quick Start

### 1. Launching the Graphical Studio
```bash
# Launch with the academic starter template
laketex

# Open an existing document
laketex my_paper.tex

# Launch with the embedded REST API enabled on port 3030
laketex --with-api --api-port 3030
```

### 2. Command-Line Operations
```bash
# Compile LaTeX to PDF directly from the terminal (defaults to Tectonic)
laketex compile document.tex -o output.pdf

# Compile using the instant pure-Rust engine
laketex compile document.tex --engine native -o output.pdf

# Run syntax linting and check for errors
laketex check document.tex

# Perform a deep typographic and math audit
laketex debug document.tex --format json

# Visually compare Tectonic vs Native rendering and generate diff metrics
laketex compare document.tex --output-dir ./parity_report

# Start the headless HTTP REST API daemon
laketex serve --port 3030
```

### 3. Keyboard Shortcuts Reference

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + S` | Save current document to disk |
| `Ctrl + Shift + S` | Save document as a new file |
| `Ctrl + O` | Open file dialog |
| `Ctrl + N` | Create a new document |
| `Ctrl + B` | Force immediate recompile |
| `Ctrl + F` | Open Find drawer (search, next/prev, case, whole word) |
| `Ctrl + H` | Open Find & Replace drawer (replace, replace all) |
| `Ctrl + P` | Open Working Directory Project Manager |
| `Ctrl + ,` | Open Settings & Preferences dialog |
| `Ctrl + \` | Toggle Left Navigation Sidebar |
| `Ctrl + J` | Toggle Bottom Diagnostics Drawer |
| `Ctrl + =` / `Ctrl + +` | Zoom in preview |
| `Ctrl + -` | Zoom out preview |
| `Ctrl + 0` | Reset preview zoom to 100% |
| `Double-Click Preview` | Jump editor cursor to clicked line (SyncTeX) |

---

## Configuration and Custom Themes

Laketex stores its configuration in a human-readable JSON file (`laketex_config.json`) alongside the executable:

```json
{
  "theme_name": "Dracula (JetBrains)",
  "pdf_color_mode": "Normal",
  "pdf_render_width": 2000,
  "view_mode": "PdfRaster",
  "default_engine": "Tectonic",
  "auto_compile": true,
  "fit_to_width": true,
  "font_size": 13.0,
  "language": "en",
  "shortcuts": {
    "compile": "Ctrl+B",
    "save": "Ctrl+S",
    "open": "Ctrl+O",
    "new_doc": "Ctrl+N",
    "project_files": "Ctrl+P",
    "find": "Ctrl+F",
    "replace": "Ctrl+H",
    "settings": "Ctrl+,"
  }
}
```

### Custom Themes Folder (`themes/`)
Laketex automatically loads any `.json` or `.yaml` theme files located in a `themes/` folder placed alongside `laketex.exe` or in the working directory.

Example theme file (`themes/monokai_pro.json`):
```json
{
  "name": "Monokai Pro",
  "dark_mode": true,
  "colors": {
    "background": "#2d2a2e",
    "panel_background": "#221f22",
    "editor_background": "#19181a",
    "text": "#fcfcfa",
    "text_muted": "#727072",
    "accent": "#ffd866",
    "selection": "#403e41",
    "border": "#363437",
    "error": "#ff6188",
    "warning": "#fc9867",
    "success": "#a9dc76"
  }
}
```

---

## Building from Source

To compile Laketex from source, you only need standard Rust (stable toolchain 1.80+):

```bash
# Clone the repository
git clone https://github.com/laketex/laketex.git
cd laketex

# Run all test suites (unit, document, and integration tests)
cargo test

# Run the automated visual parity benchmark
cargo test --test visual_parity_tests -- --nocapture

# Build optimized release executable
cargo build --release
```

---

## Documentation

- **[User Guide & Operations Manual](docs/USER_GUIDE.md)**: Guide covering UI navigation, project management, and shortcuts.
- **[System Architecture & Subsystems](docs/ARCHITECTURE.md)**: Deep dive into the internal state machine, compiler channels, and PDFium bindings.
- **[REST API Reference Manual](docs/API_REFERENCE.md)**: JSON schemas, endpoint specifications, and headless integration examples.
- **[Theming Guide](docs/THEMING_GUIDE.tex)**: Specifications for creating custom JSON and YAML color palettes.
- **Generated Rustdoc**: Complete API documentation is generated via `cargo doc --no-deps --open`.
