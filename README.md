# 🌊 Laketex

> **Lightweight, portable LaTeX editor, live viewer, and typesetting studio in 100% Rust.**  
> Zero-installation, with an embedded compressed Tectonic engine for publication-grade XeTeX output, a pure-Rust sub-millisecond native preview engine, dual-mode live viewing, bidirectional SyncTeX navigation, a visual parity benchmark harness, and a headless REST API.

---

## 🎯 Native Engine vs. Tectonic: The Parity Vision

Laketex solves the historic trade-off in LaTeX editing between **instantaneous interactive previewing** and **publication-grade TeX typesetting**:

1. **Embedded Tectonic Engine (Production Reference)**:
   - The official statically-linked Tectonic (XeTeX) engine is **compressed and embedded directly inside the Laketex binary** (`assets/tectonic.exe.gz`).
   - Zero external engine downloads are required. On first compile, Laketex unpacks Tectonic from its own `.rdata` section in ~350 milliseconds.
   - Acts as the out-of-the-box default typesetting engine, delivering 100% genuine TeX layout, math kerning, and CTAN package support.
2. **Pure-Rust Native Engine (Instant Offline / Draft Typesetting)**:
   - A built-in, fault-tolerant LaTeX parser, Unicode mathematical typesetter, and `printpdf` vector generator written in 100% safe Rust.
   - Compiles documents in **under 10 milliseconds** directly in-process without spawning external compiler processes, writing temporary disk files, or requiring internet connectivity.
3. **The Parity Goal**:
   - The ongoing mission of the Native engine is to achieve **typographical and structural parity** with Tectonic on everyday academic STEM documents (articles, homework, papers, lab reports).
   - The built-in benchmark harness (`laketex compare` and `tests/visual_parity_tests.rs`) compares native vector outputs directly against Tectonic PDF rasterizations, computing Pixel Match Percentage, Mean Squared Error (MSE), and Peak Signal-to-Noise Ratio (PSNR).

---

## ⚠️ Program Shortcomings & Architectural Boundaries

While Laketex provides a modern, fast, and self-contained LaTeX workflow, users should understand the fundamental architectural differences between an AST-based parser and a full TeX virtual machine:

### 1. Turing-Complete Macro Expansion vs. AST Parsing
- $\LaTeX$ is not a static declarative markup language like Markdown or HTML—it is an **imperative, Turing-complete macro expansion system running on Donald Knuth's TeX engine**.
- Packages frequently redefine category codes (`\catcode`), define dynamic looping macros (`\loop`), and execute dimensional arithmetic (`\dimexpr`).
- The **Native engine** uses a structured, fault-tolerant Abstract Syntax Tree (AST) parser. It excels at recognizing standard environments and macros, but **cannot execute arbitrary TeX macro definitions or catcode alterations**. Complex custom macros require the embedded Tectonic backend.

### 2. Math Micro-Typography (Knuth's Appendix G)
- The TeX engine formats mathematics according to the 22 rigorous rules of Knuth's *Appendix G*, relying on precise font metric bounding boxes (`.tfm` / OpenType `MATH` tables) to dynamically calculate fraction vinculum baselines, delimiter stretch steps, radical hook baselines, and italic slant corrections.
- The **Native engine** formats formulas with balanced curly-brace nesting, multi-row matrix grids, Unicode superscripts/subscripts, and Greek operators. While clean and legible for live drafts, it uses glyph approximations rather than full TeX font kerning tables.

### 3. Global Paragraph Breaking (Knuth-Plass)
- TeX evaluates badness, penalties, and glue elasticity across an *entire paragraph simultaneously* to optimize typographic grayness (Knuth-Plass algorithm).
- The **Native engine** paginator employs greedy line-wrapping with standard word-boundary hyphenation. Paragraph margins are consistent, but do not match TeX's global line-breaking demerits.

### 4. CTAN Package Streaming Cache
- While the **Tectonic executable is 100% embedded in Laketex** with zero engine downloads, TeX packages (`.sty`, `.cls`, font metric definitions) are streamed on demand from CTAN via HTTPS when a document imports them for the first time.
- Once downloaded, Tectonic caches packages locally in `%LOCALAPPDATA%\Tectonic\cache`. In complete offline environments without a pre-existing cache, compile with the **Native engine** for 100% offline, zero-network compilation.

---

## 📋 Feature Parity & Support Matrix: What Works and What Doesn't

| LaTeX Feature / Package | Pure-Rust Native Engine | Embedded Tectonic Engine | Notes / Fallback Behavior |
| :--- | :---: | :---: | :--- |
| **Document Structure** (`\title`, `\section`, `\subsection`) | ✅ Supported | ✅ Supported | Native maps sections to clean hierarchy and outline tree. |
| **Text Inlines** (`\textbf`, `\textit`, `\emph`, `\texttt`) | ✅ Supported | ✅ Supported | Full typographical style mapping in both vector canvas and PDF. |
| **Escaped Symbols** (`\%`, `\$`, `\&`, `\#`, `\_`) | ✅ Supported | ✅ Supported | Correctly escaped without macro leaks. |
| **Equations** (`equation`, `\[...\]`, `$...$`) | ✅ Supported | ✅ Supported | Centered equation numbering and balanced parsing. |
| **Nested Fractions** (`\frac{a}{b}`) | ✅ Supported | ✅ Supported | Native recursively nests fractions with horizontal vinculum rules. |
| **Matrices** (`pmatrix`, `bmatrix`, `vmatrix`, `matrix`) | ✅ Supported | ✅ Supported | Multi-row grids with dynamic bracket and parenthesis fences. |
| **Lists** (`itemize`, `enumerate`, `description`) | ✅ Supported | ✅ Supported | Bullet indents, arabic/roman numbering, custom item labels. |
| **Tables & Tabular** (`tabular`, `\hline`, `\toprule`) | ✅ Supported | ✅ Supported | Supports `l`, `c`, `r` alignments, multi-row, and multi-column cells. |
| **Code Highlighting (`minted`, `lstlisting`)** | ✅ Supported | ✅ Supported | Native delegates to Pygments when installed; pure-Rust lexer fallback. |
| **Missing Image Handling** (`\includegraphics`) | ✅ Supported | ✅ Supported | Overleaf-style placeholder draft box inserted if file is missing. |
| **Cross-References** (`\label`, `\ref`, `\cite`) | ✅ Supported | ✅ Supported | In-memory reference resolution and numeric citation numbering. |
| **Vector Drawings (`tikz`, `pgfplots`, `circuitikz`)** | ⚠️ Suppressed | ✅ Supported | Native isolates & hides raw directives to prevent text leaks; use Tectonic. |
| **Custom Macro Expansion (`\def`, `\newcommand`)** | ⚠️ Basic Only | ✅ Supported | Native parses simple parameter substitution; use Tectonic for loops. |
| **Advanced Micro-Typing (`microtype`)** | ❌ Not Supported | ✅ Supported | Optical margin expansion requires TeX virtual machine. |
| **Custom TrueType Fonts (`fontspec`)** | ❌ Not Supported | ✅ Supported | Native uses embedded academic Unicode fallbacks. |
| **Offline Independence** | ✅ 100% Offline | ⚠️ Requires Cache | Native needs 0 internet; Tectonic caches packages from CTAN. |

---

## 📊 Market Comparison

| Metric / Capability | TeXstudio | Overleaf | VS Code + LaTeX Workshop | **Laketex** |
| :--- | :--- | :--- | :--- | :--- |
| **Executable Size** | ~180 MB | Web SaaS | ~520 MB | **~33.4 MB (All-in-One Binary)** |
| **RAM Footprint** | ~180 MB | Cloud-dependent | ~700 MB | **~38 MB** |
| **Startup Time** | 3.2 s | Network latency | 4.5 s | **< 0.08 s (Instant)** |
| **Installation Requirement**| 5–8 GB TeX Live | Browser / Account | VS Code + TeX Live | **Zero (100% Standalone Portable)** |
| **Embedded TeX Engine** | None (External) | Cloud Servers | None (External) | **Yes (Embedded Compressed Tectonic)** |
| **Pure-Rust Offline Engine** | None | None | None | **Yes (`src/engine/`, < 10 ms)** |
| **Live Recompile Latency** | Manual / Save | Cloud Round-Trip | On Save | **Sub-10ms (Native) / ~350ms (Tectonic)** |
| **Auto-Package Retrieval** | MiKTeX dialogs | Cloud-managed | Distro-dependent | **Automated CTAN Cache Streaming** |
| **Visual Parity Test Harness** | None | None | None | **Built-in (`laketex compare`)** |
| **Headless REST API** | None | Paid Enterprise | None | **Built-in (`laketex serve`)** |
| **Interactive Preferences** | Complex Qt tabs | Account settings | `settings.json` | **GUI Modal + `laketex_config.json`** |

---

## 🚀 Quick Start

### 1. Launching the GUI Studio
```bash
# Launch with academic starter template
laketex.exe

# Or open an existing file directly
laketex.exe paper.tex

# Launch with headless background REST API server enabled on port 3030
laketex.exe --with-api --api-port 3030
```

### 2. Command-Line Interface (Headless & Automation)
```bash
# Compile LaTeX to PDF directly from terminal (defaults to Native pure-Rust)
laketex.exe compile document.tex -o output.pdf

# Compile using the embedded Tectonic XeTeX engine
laketex.exe compile document.tex -e tectonic -o output.pdf

# Visually compare Tectonic vs Native rendering and generate MSE/PSNR diff map
laketex.exe compare document.tex --output-dir ./parity_report

# Run intelligent render debugger for math flaws, unclosed fences, and matrix bugs
laketex.exe debug document.tex --format json

# Extract hierarchical document outline as JSON
laketex.exe outline document.tex

# Inspect package dependencies and check external CLI tools (e.g., Pygments)
laketex.exe packages document.tex

# Start headless HTTP REST API daemon for CI/CD or agentic workflows
laketex.exe serve --port 3030
```

### 3. Running Verification & Parity Test Suites
```powershell
# Run the complete unit, document, and embedded engine test suite
cargo test

# Run the automated visual parity benchmark test
cargo test --test visual_parity_tests -- --nocapture

# Verify zero missing Rustdoc warnings across all public types and methods
cargo doc --no-deps
```

---

## ⚙️ Configuration & Customization

Laketex saves user preferences to a simple, human-readable JSON configuration file alongside the executable (`laketex_config.json`):

```json
{
  "theme_name": "Monokai Pro",
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

### Custom Themes
Laketex discovers `.yaml` and `.json` theme definitions placed in a `./themes/` folder alongside the executable. Built-in themes include **Monokai Pro**, **Dracula (JetBrains)**, **Academic Dark**, **Slate Minimal**, **Nord Frost**, and **Paper White**.

---

## 🏛️ Documentation & Architecture

- 🏛️ **[System Architecture & Subsystems](docs/ARCHITECTURE.md)**: Deep dive into the reactive GUI state machine, parser, math translator, paginator, SyncTeX pipeline, and visual comparator.
- 🔌 **[REST API Reference Manual](docs/API_REFERENCE.md)**: Complete JSON API documentation with endpoint schemas, request/response formats, and `curl` examples.
- 📖 **[User Guide & Operations Manual](docs/USER_GUIDE.md)**: Detailed keyboard shortcuts, working directory project manager, find/replace drawer, and CLI options.
- 📚 **Generated Rustdoc**: Complete API documentation is generated via `cargo doc --no-deps --open`.
