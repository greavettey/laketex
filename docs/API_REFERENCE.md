# Laketex REST API Reference Manual

Laketex includes an embedded headless HTTP server built for headless compilation, continuous integration (CI/CD) pipelines, and programmatic document automation.

The server can be started in two modes:
1. **Dedicated Headless Daemon**:
   ```bash
   laketex serve --port 3030 --host 127.0.0.1
   ```
2. **Background Mode Alongside GUI**:
   ```bash
   laketex --with-api --api-port 3030
   ```

All API request and response bodies use standard `application/json` with UTF-8 encoding.

---

## Endpoint Summary

| Method | Endpoint | Mode | Description |
| :--- | :--- | :--- | :--- |
| `GET` | [`/health`](#1-get-health) | Both | Check server health, version `0.2.3`, stateful flag, and detected engines |
| `GET` | [`/api/help`](#2-get-apihelp) | Both | Quick endpoint overview, launch modes, and CLI command directory |
| `GET` | [`/api/spec`](#3-get-apispec) | Both | Machine-readable AI agent JSON specification (OpenAPI-style schemas) |
| `GET` | [`/api/document`](#4-get-apidocument) | Stateful | Read active GUI editor buffer, path, dirty status, word/line count, diagnostics |
| `POST` | [`/api/document`](#5-post-apidocument) | Stateful | Replace or surgically patch (`line_range` / `search_replace`) active editor buffer |
| `POST` | [`/api/navigate`](#6-post-apinavigate) | Stateful | Jump editor caret, switch viewer page/mode/zoom, or open console drawer tabs |
| `POST` | [`/api/compile`](#7-post-apicompile) | Both | Compile LaTeX source to base64 PDF (updates live GUI in stateful mode) |
| `POST` | [`/api/check`](#8-post-apicheck) | Both | Fast Tectonic syntax linting and diagnostic error reporting |
| `POST` | [`/api/outline`](#9-post-apioutline) | Both | Extract structural outline (sections, equations, tables) as JSON |
| `POST` | [`/api/packages`](#10-post-apipackages) | Both | Inspect and parse imported `\usepackage` declarations and metadata |
| `POST` | [`/api/debug`](#11-post-apidebug) | Both | Deep render audit for matrix flaws, math notation bugs, and PDF integrity |
| `GET` | [`/api/dependencies`](#12-get-apidependencies) | Both | List external host tool dependencies (Pygments, etc.) and installation status |
| `GET` | [`/api/docs`](#13-get-apidocs) | Both | List and retrieve built-in offline LaTeX documentation manuals |

---

## 1. GET `/health`

Verifies that the Laketex headless service is running, reports version `0.2.3`, indicates whether live stateful synchronization is active, and enumerates detected TeX engine backends.

### Request
```bash
curl -X GET http://127.0.0.1:3030/health
```

### Response (`200 OK`)
```json
{
  "status": "online",
  "app": "Laketex",
  "version": "0.2.3",
  "stateful": false,
  "api_spec_endpoint": "/api/spec",
  "help_endpoint": "/api/help",
  "api_spec_cli_command": "laketex api-spec",
  "engines": [
    {
      "engine": "native",
      "display_name": "Native (Pure Rust)",
      "available": true,
      "path": null,
      "version": "0.2.3"
    },
    {
      "engine": "tectonic",
      "display_name": "Tectonic",
      "available": true,
      "path": "embedded:tectonic",
      "version": "0.15.0"
    }
  ]
}
```

---

## 2. GET `/api/help`

Returns a quick JSON directory of all available endpoints, launch modes, and CLI aliases.

### Request
```bash
curl -X GET http://127.0.0.1:3030/api/help
# Aliases: GET /help, GET /
```

### Response (`200 OK`)
```json
{
  "app": "Laketex",
  "version": "0.2.3",
  "stateful_gui_mode_active": false,
  "agent_api_spec": {
    "description": "Complete machine-readable JSON specification of all endpoints, schemas, security invariants, and agent workflows.",
    "http_endpoint": "GET /api/spec (aliases: /api/schema, /api/agent)",
    "cli_command": "laketex api-spec (aliases: agent-spec, schema, endpoints; pass --compact for single-line JSON)"
  },
  "launch_modes": {
    "headless_daemon": "laketex serve --port 3030",
    "gui_with_stateless_api": "laketex --with-api --api-port 3030",
    "gui_with_stateful_live_api": "laketex --with-api --with-state --api-port 3030"
  },
  "endpoints": [
    { "method": "GET", "path": "/api/spec", "summary": "Full machine-readable agent API specification" },
    { "method": "GET", "path": "/health", "summary": "Server health, stateful flag, and detected engines" },
    { "method": "GET", "path": "/api/document", "summary": "[--with-state] Read active GUI editor buffer" },
    { "method": "POST", "path": "/api/document", "summary": "[--with-state] Replace or patch active GUI editor buffer" },
    { "method": "POST", "path": "/api/navigate", "summary": "[--with-state] Move caret, switch page/mode, or open drawer tabs" },
    { "method": "POST", "path": "/api/compile", "summary": "Compile LaTeX source to base64 PDF" }
  ]
}
```

---

## 3. GET `/api/spec`

Emits the complete, machine-readable JSON specification of all headless and stateful REST API endpoints, request/response JSON schemas, security policies, and recommended autonomous agent workflows.

### Request
```bash
curl -X GET http://127.0.0.1:3030/api/spec
# Aliases: GET /api/schema, GET /api/agent
# CLI equivalent: laketex api-spec (or laketex api-spec --compact)
```

### Response (`200 OK`)
```json
{
  "schema_version": "1.0.0",
  "app": "Laketex",
  "version": "0.2.3",
  "description": "Laketex Lightweight LaTeX Studio & Autonomous Agent REST API Specification",
  "stateful_gui_mode_active": false,
  "security": {
    "host_binding": "127.0.0.1 (Loopback only)",
    "csrf_protection": "Browser Cross-Origin and Referer requests strictly rejected",
    "shell_escape_policy": "Disabled by default; requires explicit --allow-shell-escape flag"
  },
  "endpoints": [ ... ],
  "recommended_agent_workflows": [ ... ]
}
```

---

## 4. GET `/api/document` (Stateful Live Sync)

Reads the active editor text buffer, bound file path, dirty status, line and word count, active compiler engine, and compiler diagnostics from the live desktop GUI.

> **Requires Stateful Mode**: Start Laketex with `laketex --with-api --with-state`. If invoked in stateless mode, returns `400 Bad Request`.

### Request
```bash
curl -X GET http://127.0.0.1:3030/api/document
```

### Response (`200 OK`)
```json
{
  "file_name": "paper.tex",
  "file_path": "C:\\projects\\paper\\paper.tex",
  "is_dirty": true,
  "source": "\\documentclass{article}\n\\begin{document}\nHello World\n\\end{document}",
  "line_count": 4,
  "word_count": 2,
  "engine": "Native",
  "view_mode": "PdfRaster",
  "current_page": 1,
  "zoom": 1.0,
  "diagnostics": []
}
```

---

## 5. POST `/api/document` (Stateful Live Sync)

Replaces or surgically patches the active editor buffer in the live GUI. Changes immediately appear in the editor window and trigger preview recompilation.

> **Requires Stateful Mode**: Start Laketex with `laketex --with-api --with-state`.  
> **Aliases**: `PUT /api/document`, `PATCH /api/document`.

### Request Fields:
- `source` *(string, optional)*: Replaces the entire active editor buffer.
- `edits` *(array, optional)*: Surgical edit operations applied in order:
  - `{"type": "line_range", "start_line": 5, "end_line": 8, "new_text": "\\section{Results}\nNew text..."}`
  - `{"type": "search_replace", "search": "old formula", "replace": "new formula", "all": false}`
- `save` *(boolean, optional, default: `false`)*: Saves the modified document directly to disk (requires a bound file path).
- `compile` *(boolean, optional, default: `true` if content changed)*: Triggers automatic background recompilation.
- `jump_to_line` *(usize, optional)*: Automatically scrolls the editor caret to the given 1-based line number.
- `engine` *(string, optional)*: Switches the active compiler engine (`"native"`, `"tectonic"`).

### Example: Surgical Line Range Patch
```bash
curl -X POST http://127.0.0.1:3030/api/document \
  -H "Content-Type: application/json" \
  -d '{
    "edits": [
      {
        "type": "line_range",
        "start_line": 2,
        "end_line": 2,
        "new_text": "\\title{Refined Quantum Derivations}"
      }
    ],
    "compile": true,
    "jump_to_line": 2
  }'
```

### Response (`200 OK`)
```json
{
  "success": true,
  "snapshot": {
    "file_name": "paper.tex",
    "is_dirty": true,
    "line_count": 4,
    "engine": "Native",
    "view_mode": "PdfRaster"
  }
}
```

---

## 6. POST `/api/navigate` (Stateful Live Sync)

Manipulates the user's GUI viewport, editor caret line, active preview page, zoom level, view mode, and collapsible drawer/sidebar panels without altering document text.

> **Requires Stateful Mode**: Start Laketex with `laketex --with-api --with-state`.

### Request Fields:
- `line` *(usize, optional)*: 1-based source line to jump the editor caret to.
- `page` *(usize, optional)*: 1-based page number to display in the viewer.
- `view_mode` *(string, optional)*: `"NativeVector"` / `"native"` or `"PdfRaster"` / `"pdf"`.
- `zoom` *(f32, optional)*: Viewer zoom factor, clamped to `0.25..=4.0`.
- `show_drawer` *(boolean, optional)*: Open (`true`) or close (`false`) the bottom console drawer.
- `drawer_tab` *(string, optional)*: Specific drawer tab to display (`"diagnostics"`, `"audit"`, `"logs"`, `"api"`, or `"audit_log"`). Automatically opens the drawer.
- `show_sidebar` *(boolean, optional)*: Open (`true`) or close (`false`) the left outline sidebar.

### Example Request
```bash
curl -X POST http://127.0.0.1:3030/api/navigate \
  -H "Content-Type: application/json" \
  -d '{
    "line": 42,
    "page": 2,
    "drawer_tab": "audit_log",
    "show_drawer": true
  }'
```

### Response (`200 OK`)
```json
{
  "success": true,
  "snapshot": {
    "current_page": 2,
    "zoom": 1.0,
    "view_mode": "PdfRaster"
  }
}
```

---

## 7. POST `/api/compile`

Compiles a LaTeX document string into a binary PDF and returns structured diagnostics along with Base64-encoded PDF bytes.

### Request Body
```json
{
  "source": "\\documentclass{article}\\title{Quantum}\\begin{document}\\maketitle\\section{Hello}World\\end{document}",
  "engine": "native"
}
```

#### Fields:
- `source` *(string, required)*: Full LaTeX document source code.
- `engine` *(string, optional, default: `"native"`)*: Compiler engine to use:
  - `"native"`: Pure-Rust in-process engine (zero-install, instant < 70 ms).
  - `"tectonic"`: Tectonic engine with automated CTAN cache.
  - `"pdflatex"`, `"xelatex"`, `"lualatex"`: Local TeX distributions if installed.

### Example Request
```bash
curl -X POST http://127.0.0.1:3030/api/compile \
  -H "Content-Type: application/json" \
  -d '{
    "source": "\\documentclass{article}\\begin{document}Hello from REST API\\end{document}",
    "engine": "native"
  }'
```

### Response (`200 OK`)
```json
{
  "success": true,
  "duration_ms": 62,
  "diagnostics": [],
  "pdf_base64": "JVBERi0xLjUKJcfsj6IKNCAwIG9iai...",
  "stdout": "",
  "stderr": "",
  "error_report_markdown": null
}
```

*Note: If compilation fails or has diagnostics, `error_report_markdown` contains the complete formatted Markdown error report with code context and suggested fixes.*

### Saving Output to File:
To decode and save the compiled PDF directly from the command line:
```bash
curl -s -X POST http://127.0.0.1:3030/api/compile \
  -H "Content-Type: application/json" \
  -d '{"source": "\\documentclass{article}\\begin{document}Laketex Document\\end{document}"}' \
  | jq -r .pdf_base64 | base64 --decode > output.pdf
```

---

## 8. POST `/api/check`

Lints the LaTeX source and returns error diagnostics, line numbers, and warnings without generating PDF bytes.

### Request Body
```json
{
  "source": "\\documentclass{article}\\begin{document}\\undefinedcmd\\end{document}"
}
```

### Response (`200 OK`)
```json
{
  "success": false,
  "diagnostics": [
    {
      "severity": "Error",
      "message": "Undefined control sequence: \\undefinedcmd",
      "line": 1
    }
  ],
  "duration_ms": 48,
  "error_report_markdown": "#  Laketex Compilation Error & Diagnostic Report\n...",
  "error_report": {
    "app_version": "0.2.3",
    "document_name": "api_document.tex",
    "error_count": 1,
    "warning_count": 0,
    "diagnostics": [
      {
        "severity": "Error",
        "message": "Undefined control sequence: \\undefinedcmd",
        "line": 1,
        "category": "Undefined Command / Macro",
        "environment_intent": "Declaring document configuration in preamble",
        "surrounding_context": [
          { "line_number": 1, "content": "\\documentclass{article}\\begin{document}\\undefinedcmd\\end{document}", "is_target": true }
        ],
        "suggested_fix": "The command was not recognized by the TeX engine..."
      }
    ]
  }
}
```

---

## 9. POST `/api/outline`

Extracts the document's hierarchical outline tree without compiling a full PDF. Useful for table-of-contents generators, navigation viewers, and document analyzers.

### Request Body
```json
{
  "source": "\\documentclass{article}\\begin{document}\\section{First}\\subsection{Sub}\\begin{equation}x=1\\end{equation}\\end{document}"
}
```

### Response (`200 OK`)
```json
{
  "items": [
    {
      "kind": "Section",
      "title": "First",
      "line": 1,
      "level": 1
    },
    {
      "kind": "Subsection",
      "title": "Sub",
      "line": 1,
      "level": 2
    },
    {
      "kind": "Equation",
      "title": "Equation Block",
      "line": 1,
      "level": 3
    }
  ]
}
```

---

## 10. POST `/api/packages`

Detects all `\usepackage` declarations in the preamble, separating package names and option arguments.

### Request Body
```json
{
  "source": "\\documentclass{article}\\usepackage{amsmath,amssymb}\\usepackage[dvipsnames]{xcolor}\\begin{document}\\end{document}"
}
```

### Response (`200 OK`)
```json
{
  "packages": [
    {
      "name": "amsmath",
      "options": []
    },
    {
      "name": "amssymb",
      "options": []
    },
    {
      "name": "xcolor",
      "options": ["dvipsnames"]
    }
  ]
}
```

---

## 11. POST `/api/debug`

Scans the LaTeX source code and generates a rendered PDF preview to perform an in-depth audit of rendering issues, syntax problems, and visual degradations. It explicitly targets matrices, math notation, missing math delimiters, and PDF integrity.

> **Aliases**: `POST /debug`

### Request Body
```json
{
  "source": "\\documentclass{article}\\begin{document}\\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}\\end{document}",
  "engine": "native"
}
```

#### Fields:
- `source` *(string, required)*: LaTeX document source code.
- `engine` *(string, optional, default: `"native"`)*: `"native"` or `"tectonic"`.

### Example Request
```bash
curl -X POST http://127.0.0.1:3030/api/debug \
  -H "Content-Type: application/json" \
  -d '{
    "source": "\\documentclass{article}\\begin{document}\\begin{pmatrix} 1 & 2 \\\\ 3 \\end{pmatrix}\\end{document}"
  }'
```

### Response (`200 OK`)
```json
{
  "document_name": "api_document.tex",
  "timestamp": "UTC 17:41:40 (Day 20748)",
  "total_lines": 3,
  "total_issues": 3,
  "error_count": 1,
  "warning_count": 2,
  "degraded_count": 0,
  "matrix_issue_count": 3,
  "math_issue_count": 0,
  "pdf_status": "Valid PDF Generated (88332 bytes)",
  "issues": [
    {
      "line": 1,
      "command": "\\begin{pmatrix}",
      "category": "Matrix Math Mode",
      "severity": "Error",
      "issue": "Matrix environment '\\begin{pmatrix}' is used outside of math mode. Standard LaTeX and native rendering engines reject bare matrices in paragraph prose.",
      "snippet": "\\begin{pmatrix} 1 & 2 \\\\ 3 \\end{pmatrix}",
      "suggested_fix": "Enclose the matrix inside a display equation: '\\begin{equation} ... \\end{equation}' or '\\[ ... \\]'."
    },
    {
      "line": 1,
      "command": "\\begin{pmatrix}",
      "category": "Matrix Dependency",
      "severity": "Warning",
      "issue": "Matrix environment '\\begin{pmatrix}' is an AMS-LaTeX extension, but '\\usepackage{amsmath}' was not detected in the preamble.",
      "snippet": "\\begin{pmatrix} 1 & 2 \\\\ 3 \\end{pmatrix}",
      "suggested_fix": "Add '\\usepackage{amsmath}' to the document preamble before '\\begin{document}'."
    },
    {
      "line": 1,
      "command": "&",
      "category": "Matrix Column Count",
      "severity": "Warning",
      "issue": "Mismatched column count in matrix '\\begin{pmatrix}': Row 1 defines 2 column(s), but Row 2 provides 1 column(s). This causes bracket misalignment and distorted spacing.",
      "snippet": "3 \\end{pmatrix}",
      "suggested_fix": "Add or remove column separators ('&') so all rows contain exactly 2 columns."
    }
  ]
}
```

---

## 12. GET `/api/dependencies`

Inspects external system and Python host tools required by advanced LaTeX packages (such as `pygmentize` for `minted` or `pythontex`).

### Request
```bash
curl -X GET http://127.0.0.1:3030/api/dependencies
```

### Response (`200 OK`)
```json
[
  {
    "name": "Pygments",
    "binary_name": "pygmentize",
    "latex_package": "minted",
    "description": "Python syntax highlighter required by 'minted' with -shell-escape",
    "installed": true,
    "binary_path": "C:\\Python311\\Scripts\\pygmentize.exe",
    "pip_package": "Pygments",
    "install_command": "pip install Pygments"
  }
]
```

### Related Dependency Endpoints:
- **`POST /api/dependencies/check`**: Scans a specific LaTeX source string (`{"source": "..."}`) and returns only the dependencies required by that document.
- **`POST /api/dependencies/install`**: Triggers automated installation of a package tool via pip or conda-forge. Requires explicit consent in payload:
  ```bash
  curl -X POST http://127.0.0.1:3030/api/dependencies/install \
    -H "Content-Type: application/json" \
    -d '{"package": "Pygments", "consent": true}'
  ```

---

## 13. GET `/api/docs`

Lists or retrieves the built-in offline LaTeX manuals bundled directly inside the Laketex executable.

### 1. List Available Manuals (`GET /api/docs`)
```bash
curl -X GET http://127.0.0.1:3030/api/docs
```
Response (`200 OK`):
```json
[
  { "kind": "Manual", "title": "Laketex User Manual", "filename": "MANUAL.tex", "lines": 420 },
  { "kind": "Theming", "title": "Theming and Styling Guide", "filename": "THEMING_GUIDE.tex", "lines": 280 },
  { "kind": "Api", "title": "REST API Reference", "filename": "API_REFERENCE.tex", "lines": 350 },
  { "kind": "Architecture", "title": "System Architecture Specification", "filename": "ARCHITECTURE.tex", "lines": 310 }
]
```

### 2. Retrieve Full Manual Source (`POST /api/docs`)
```bash
curl -X POST http://127.0.0.1:3030/api/docs \
  -H "Content-Type: application/json" \
  -d '{"doc": "manual"}'
```
Response (`200 OK`):
```json
{
  "kind": "Manual",
  "title": "Laketex User Manual",
  "filename": "MANUAL.tex",
  "content": "\\documentclass{article}..."
}
```

---

## Error Handling

When an unexpected internal error or bad payload is delivered, the server responds with an appropriate HTTP status code (`400 Bad Request` or `500 Internal Server Error`) and JSON error descriptor:

```json
{
  "error": "Failed to parse JSON body: missing field `source`"
}
```
