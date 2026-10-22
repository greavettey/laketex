# Laketex User Guide & Operations Manual

Laketex is a lightweight, zero-installation, information-dense $\LaTeX$ editor and live viewer in Rust.

---

## 1. Getting Started & Portability

Laketex is packaged as a **single standalone executable** (`laketex.exe` or `laketex`). It does not require any installer, registry modifications, or external TeX Live / MiKTeX installations.

### Running Laketex
To launch the graphical editor:
```bash
./laketex
```
To open an existing file directly:
```bash
./laketex my_paper.tex
```

---

## 2. Interface Overview

```
+-------------------------------------------------------------------------------------------------+
| [Laketex]  [New] [Open] [Save] | Engine: [Native (Pure Rust) v] | [⚡ Recompile] | Auto: [x]    |
+---------------------+-----------------------------------+---------------------------------------+
|  OUTLINE / PACKAGES |  LATEX SOURCE CODE                |  LIVE PREVIEW (Page 1/2)  Zoom: 100%  |
|                     |                                   |  [Native Live View] [PDF Vector View] |
| § 1. Introduction   |  1 | \documentclass{article}      |                                       |
| § 2. Formulations   |  2 | \title{Quantum Field Note}   |          Quantum Field Note           |
|   ƒ Eq. (1)         |  3 | \begin{document}             |          Research Group               |
| § 3. Benchmarks     |  4 | \maketitle                   |                                       |
|   ≡ Table 1         |  5 | \section{Introduction}       |  1. Introduction                      |
|                     |  6 | Scientific models demand...  |  Scientific models demand precision.  |
|                     |  7 |                              |                                       |
|                     |  8 | \begin{equation}             |             E = ℏω             (1)    |
|                     |  9 | E = \hbar \omega             |                                       |
|                     | 10 | \end{equation}               |                                       |
|                     | 11 |                              |                                       |
+---------------------+-----------------------------------+---------------------------------------+
|  DIAGNOSTICS & LOGS (0 Errors, 0 Warnings)                                     REST API: :3030  |
+-------------------------------------------------------------------------------------------------+
```

### Key Interface Sections:
1. **Top Toolbar**:
   - **File Operations**: `New Document`, `Open File...`, `Save Document`, `Save As...`.
   - **Engine Selector Dropdown**: Toggle between `Native (Pure Rust)`, `Tectonic`, `pdfLaTeX`, `XeLaTeX`, or `LuaLaTeX`.
   - **Recompile Button (`⚡`)**: Forces an immediate full compilation run.
   - **Auto-Compile Toggle**: Toggles live debounced compilation as you type (default: ON, 180 ms debounce).
   - **Theme Selector Dropdown**: Choose between built-in themes (such as JetBrains Dracula) and custom JSON/YAML themes discovered on disk.
   - **Panel Visibility Buttons**: Toggle the left sidebar and bottom diagnostic drawer on or off.

2. **Code Editor Panel (Center Left)**:
   - **Syntax Highlighting**: Real-time syntax coloring for commands, math mode, comments, environments, and brackets.
   - **Line Number Gutter**: Displays line numbers with red error dots (`●`) indicating syntax issues.
   - **Font Sizing**: Click `A+` or `A-` in the editor header to quickly adjust editor font size.

3. **Live Viewer Panel (Center Right)**:
   - **Dual Viewing Modes**:
     - **Native Live View**: 100% pure-Rust instant vector layout. Renders AST vectors directly on canvas in < 1 ms.
     - **PDF Vector View**: Full rasterized view rendered by embedded PDFium.
   - **Page Navigation**: Click `◀ Prev` and `Next ▶` buttons to navigate multi-page documents.
   - **Dark Mode Inversion**: Automatically adjusts document contrast to academic dark mode or crisp daylight white.

4. **Left Sidebar (Outline & Packages)**:
   - **Outline Tab**: Displays sections (`§`), subsections, equations (`ƒ`), tables (`≡`), and labels.
   - **Packages Tab**: Shows detected `\usepackage` declarations and lets you search and insert CTAN packages.
   - **Math Tab**: One-click insertion of Greek letters, operators, fractions, matrices, and symbols.
   - **Info Tab**: Document word count, character count, line count, and last compilation duration.

5. **Bottom Collapsible Drawer**:
   - **Diagnostics**: Displays parsed compiler warnings, errors, line numbers, and file offsets.
   - **Terminal Output**: Raw `stdout` and `stderr` logs for low-level debugging.
   - **Error Export**: `📋 Copy Report` and `⬇ Export Error Log...` buttons.

---

## 3. Navigation & SyncTeX

Laketex includes full forward and reverse synchronization between the source code and the preview:

### Forward Navigation (Outline -> Editor)
- Click on any section heading, equation, or table in the **Outline sidebar**.
- The editor immediately scrolls to that exact line and positions the text cursor and keyboard focus directly at the start of that section.

### Reverse Navigation (Preview -> Editor)
- Double-click on any paragraph, equation, heading, or table directly in either the **Native Live View** or **PDF Vector View**.
- Laketex maps the click coordinate to the corresponding document element and jumps your editor cursor straight to that line in your `.tex` source code.

---

## 4. Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + S` | Save current document to disk |
| `Ctrl + Shift + S` | Save document as a new file |
| `Ctrl + O` | Open file dialog |
| `Ctrl + N` | Create a new document |
| `Ctrl + B` | Force recompile document (`⚡`) |
| `Ctrl + \` | Toggle left sidebar visibility |
| `Ctrl + J` | Toggle bottom diagnostic drawer |
| `Ctrl + =` / `Ctrl + +` | Zoom in document preview |
| `Ctrl + -` | Zoom out document preview |
| `Ctrl + 0` | Reset preview zoom to 100% |

---

## 5. Command-Line Interface (CLI)

Laketex provides CLI subcommands for build automation and scripting:

### 1. Compile LaTeX directly to PDF:
```bash
# Compile using Native Pure-Rust engine (default, < 70 ms)
laketex compile document.tex -o document.pdf

# Compile using Tectonic engine and export error report if compilation fails
laketex compile document.tex -o document.pdf --engine tectonic --export-log error_report.md
```

### 2. Lint and check a document for errors:
```bash
# Human-readable diagnostic output with surrounding code snippets and intent
laketex check document.tex

# Export full contextual error report to Markdown or JSON
laketex check document.tex --export-log error_report.md
laketex check document.tex --format json
```

### 3. Extract document outline as JSON:
```bash
laketex outline document.tex
```

### 4. Inspect imported packages:
```bash
laketex packages document.tex
```

### 5. Compare Tectonic vs Native Engine Visual Parity:
```bash
laketex compare document.tex --output-dir ./parity_report
```
Generates high-resolution PNG renders (`tectonic_render.png`, `native_render.png`), a pixel difference map (`diff_comparison.png`), and computes exact MSE, PSNR, and parity percentage in `visual_parity_report.json`.

### 6. Deep Render Audit for Math & Matrix Commands:
```bash
# Scan document and print human-readable audit with line numbers and explanations
laketex debug document.tex

# Output machine-readable JSON for headless agents or CI consumption
laketex debug document.tex --format json

# Export audit report directly to Markdown or JSON file
laketex debug document.tex --output render_audit.md
```
Scans LaTeX source code and the generated PDF preview to pinpoint commands that fail to render correctly. It specifically inspects:
- **Matrix Environments**: Detects matrices outside math mode, missing `amsmath` imports, mismatched column counts across rows, unclosed matrix delimiters, and width overflows.
- **Math Notation**: Detects math commands (`\alpha`, `\sum`, `\int`, `_`, `^`, etc.) used in plain text without `$...$`, unpaired inline dollar delimiters, malformed `\frac{num}{den}` without denominators, unbalanced `{ }` braces, and unsupported macros.
- **PDF Preview Integrity**: Validates PDF header magic bytes, size thresholds, and export failures.

### 7. Run the Headless REST API server:
```bash
laketex serve --port 3030 --host 127.0.0.1
```

---

## 6. Contextual Error Reporting & Log Export

When LaTeX code encounters a syntax error or missing macro, raw TeX logs can often be cryptic or difficult to debug. Laketex automatically extracts the surrounding context, infers the document's intent, and provides one-click export tools.

### What the Contextual Report Includes:
1. **Execution Metadata**: Document filename, timestamp, Laketex version, platform/OS, active typesetting engine, line and word counts.
2. **"What Went Wrong"**: The exact compiler error message, error classification (e.g. *Undefined Command*, *Missing Package*, *Mismatched Environment*, *Math Mode Syntax*), and severity badge.
3. **"What The File Was Doing"**: Automatically inferred semantic context (e.g., `Typesetting '\begin{equation}' environment inside section 'Wave Mechanics'` or `Declaring document metadata in preamble`).
4. **Surrounding Code Context**: 3 lines before and after the failure, with the offending line marked by `>>>` and highlighted in red.
5. **Tailored Suggested Fix**: Actionable guidance on missing packages (e.g., `\usepackage{amsmath}`), unclosed environments, or syntax typos.
6. **Active Packages & Raw Console**: List of all loaded packages and collapsible `stderr`/`stdout` streams.

### Exporting Reports from the GUI:
- Open the bottom **Diagnostics & Logs Drawer** (`Ctrl + J`).
- Click **`📋 Copy Report`**: Immediately places the complete, beautifully formatted Markdown report on your system clipboard so you can paste it directly into GitHub issues, chat, or documentation.
- Click **`⬇ Export Error Log...`**: Opens a save dialog to export as a formatted `.md` (Markdown), `.json`, or `.log` file on disk.

### Render Audit Tab (Diagnosing Matrices & Math Notation):
- Click the **`🔍 Render Audit`** tab in the bottom drawer.
- Shows real-time analysis of commands that cause visual degradation or rendering issues in either the Native or Tectonic engines.
- **Card-Based Inspection**: Each issue displays a badge (`🔴 ERROR`, `🟡 WARN`, `🟣 DEGRADED`), the offending LaTeX command, its category, and a clear statement of **why it isn't rendering correctly**.
- **Click-to-Jump Navigation**: Click any `Line X` button to jump the editor cursor directly to that line.
- **Report Sharing**: Use `📋 Copy Audit Report` or `⬇ Export Audit...` to share full Markdown or JSON reports.

---

## 7. Custom Key-Value Theming Engine & JetBrains Dracula

Laketex includes an extensible theming engine that controls all UI colors through simple key-value pairs (`#RRGGBB` or `#RGB` hex strings).

### Built-in JetBrains Dracula Theme
Laketex ships with an authentic **Dracula (JetBrains)** theme built-in, styled precisely after JetBrains's official Dracula IDE theme:
- Editor Background: `#282a36`
- Tool Window Panels / Sidebar: `#21222c`
- Selection: `#44475a`
- Caret & Accent: `#bd93f9` (Purple)
- Foreground Text: `#f8f8f2`
- Comments / Muted Text: `#6272a4`
- Status Indicators: Error `#ff5555`, Warning `#ffb86c`, Success `#50fa7b`, Info `#8be9fd`

Other built-in themes include `Academic Dark`, `Slate Minimal`, `Nord Frost`, and `Paper White`.

### Creating Custom Themes (JSON or YAML)
You can create custom themes simply by placing `.yaml`, `.yml`, or `.json` files:
1. In the directory **alongside `laketex.exe`**
2. In a `./themes/` folder

Laketex automatically discovers and loads all valid theme files on startup and adds them to the toolbar theme selector dropdown!

#### Example Custom Theme (`solarized_dark.yaml`):
```yaml
name: Solarized Dark
dark_mode: true
colors:
  background: "#002b36"
  panel_background: "#073642"
  editor_background: "#002b36"
  text: "#839496"
  text_muted: "#586e75"
  selection: "#073642"
  accent: "#268bd2"
  border: "#073642"
```

#### Example Custom Theme (`monokai.json`):
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
    "selection": "#403e41"
  }
}
```

#### Supported Color Keys:
- `background` / `window_fill` / `bg`
- `panel_background` / `panel_fill` / `surface`
- `editor_background` / `extreme_bg` / `code_bg`
- `text` / `foreground` / `fg`
- `text_muted` / `comment` / `muted`
- `selection` / `selection_bg`
- `selection_stroke`
- `accent` / `primary`
- `border` / `stroke`
- `button_bg`, `button_hover`, `button_active`
- `error`, `warning`, `success`, `info`

---

## 8. Built-in Rustdoc Documentation

All aspects of Laketex—both frontend UI modules and backend engines, parsers, and data structures—are fully documented with Rustdoc.

To view the complete interactive API documentation locally in your browser:
```bash
cargo doc --no-deps --open
```

The generated HTML documentation includes:
- Architecture walkthroughs for each subsystem
- Intra-doc cross-references between types
- Detailed method docstrings and usage examples
- No external network requests needed; completely portable and offline-accessible.
