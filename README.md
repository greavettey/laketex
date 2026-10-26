<div align="center">
  <img src="assets/LaketexLogo.png" alt="Laketex Logo" width=350>
  <h1><em>L</em>aketex</h1>
</div>

Laketex is a lightweight, portable LaTeX editor and live previewer written in Rust.

It is designed to run as a single executable without requiring a multi-gigabyte TeX Live or MiKTeX installation. It bundles the [Tectonic](https://tectonic-typesetting.github.io/) typesetting engine to handle standard LaTeX packages on demand, alongside a built-in pure-Rust typesetter for quick in-memory drafts.

#### *Why call it "Laketex"?*
> I was born and raised in Muskoka, which is a region of Ontario containing over 1600 lakes. I moved to a city for post-secondary, and to put it simply, I miss the lake.

#### *Statement on agentic AI*
> Initially, this project was scaffolded with agentic AI assistance. I used this as a crutch to help myself learn more of Rust. However, I now feel that the use of agentic AI does not further the goals of the project, and reduces what I enjoy most about coding. As a result of this I have begun stripping unnecessary features, rewriting previously agent-written files and utilities, and bringing the human element of the project back into focus. I don't regret the agentic AI use; it has become a standard practice in industry and can be useful in the hands of a human. Reading, verifying, and interogating the agent on its design decisions and framework choices has helped me broaden my understanding of Rust, and with my curation has created the foundations of a safe, performant project. Regardless of past use, I do now feel it's counterintuitive to this project, and will be stripping it back. 

---

## Features

- **Dual typesetting engines**:
  - **Bundled Tectonic**: Full LaTeX package compatibility, TikZ drawings, and BibTeX citations without a separate system distribution.
  - **Native Rust typesetter**: Fast in-process preview for basic documents, math formulas, matrices, and tables.
- **Live PDF preview**: Rendered via Google PDFium with dark mode color inversion and sepia reading options.
- **Bi-directional SyncTeX**: Click a heading or equation in the outline to jump to the code, or double-click anywhere in the PDF preview to jump back to that source line.
- **Project manager (`Ctrl+P`)**: Sidebar file tree for managing multi-file LaTeX projects, figures, and bibliographies.
- **Semantic theming**: Ships with Dracula, Synthwave Neon, Monokai Pro, Solarized Dark, Academic Dark, Slate Minimal, and Paper White. Custom themes can be added as `.json` or `.yaml` files in a `themes/` folder.
- **Headless CLI & REST API**: Build documents from scripts (`laketex compile document.tex -o output.pdf`) or run a local HTTP server (`laketex serve --port 3030`).

---

## Current Status & Known Issues

- **Startup time on Windows**: Cold launch currently takes longer than intended while the runtime and font fallbacks initialize. This is under active profiling and optimization.
- **Native engine limits**: The built-in Rust engine parses common LaTeX syntax (sections, fractions, matrices, lists, basic formatting), but does not support arbitrary CTAN macros, complex packages, or TikZ drawings. For full package support, use the bundled Tectonic engine.

---

## Quick Start

### Graphical Editor

```bash
# Open editor with default template
laketex

# Open an existing document
laketex paper.tex

# Run with local REST API enabled
laketex --with-api --api-port 3030
```

### Command Line Subcommands

```bash
# Compile to PDF using native draft engine
laketex compile document.tex -o output.pdf

# Compile using bundled Tectonic (full package support)
laketex compile document.tex --engine tectonic -o output.pdf

# Syntax check / lint a file
laketex check document.tex

# Start the headless HTTP server
laketex serve --port 3030

# Print JSON API specification
laketex api-spec
```

> **Windows PowerShell note**: Because `laketex.exe` is configured as a Windows GUI subsystem application to avoid flashing a console window when launched from the desktop, PowerShell detaches from it immediately by default. To wait for CLI commands in PowerShell:
> ```powershell
> .\laketex.exe compile document.tex --engine tectonic -o output.pdf | Out-Host
> ```

### Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + S` | Save document |
| `Ctrl + Shift + S` | Save as new file |
| `Ctrl + O` | Open file |
| `Ctrl + N` | New document |
| `Ctrl + B` | Recompile document |
| `Escape` | Cancel active compilation |
| `Ctrl + F` | Find in editor |
| `Ctrl + H` | Find and Replace |
| `Ctrl + P` | Project files manager |
| `Ctrl + ,` | Settings and preferences |
| `Ctrl + \` | Toggle sidebar |
| `Ctrl + J` | Toggle diagnostics drawer |
| `Ctrl + +` / `Ctrl + -` | Zoom in / out in preview |
| `Ctrl + 0` | Reset preview zoom |
| `Double-Click Preview` | Jump editor to line (SyncTeX) |

---

## Configuration & Custom Themes

Preferences are stored in `laketex_config.json` next to the executable:

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
  "language": "en"
}
```

### Adding Custom Themes

Drop `.json` or `.yaml` files into a `themes/` folder next to `laketex.exe`. For example, `themes/my_theme.json`:

```json
{
  "name": "My Custom Theme",
  "dark_mode": true,
  "colors": {
    "background": "#1e1e2e",
    "panel_background": "#181825",
    "editor_background": "#11111b",
    "text": "#cdd6f4",
    "accent": "#cba6f7",
    "syntax_command": "#89b4fa",
    "syntax_argument": "#a6e3a1",
    "syntax_package": "#f9e2af",
    "syntax_tag_property": "#f38ba8",
    "syntax_property_number": "#fab387",
    "syntax_property_boolean": "#fab387",
    "syntax_math": "#f9e2af",
    "syntax_comment": "#6c7086"
  }
}
```

---

## Building from Source

Requirements: Rust stable toolchain (1.80+).

```bash
# Clone the repository
git clone https://github.com/greavettey/laketex.git
cd laketex

# Run tests
cargo test

# Build release executable
cargo build --release
```

The compiled executable will be in `target/release/laketex.exe`.

---

## Documentation

- **[System Architecture](docs/ARCHITECTURE.md)**: Notes on the parser, compiler worker, and PDFium bindings.
- **[REST API Reference](docs/API_REFERENCE.md)**: Endpoints and payload schemas.
- **[Theming Guide](docs/THEMING_GUIDE.tex)**: Theme key-value tokens and palette configuration.
