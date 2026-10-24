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

| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `GET` | [`/health`](#1-get-health) | Check server health, version, and availability |
| `POST` | [`/api/compile`](#2-post-apicompile) | Compile LaTeX source code to PDF bytes (Base64) |
| `POST` | [`/api/outline`](#3-post-apioutline) | Extract structural outline (sections, equations, tables) as JSON |
| `POST` | [`/api/packages`](#4-post-apipackages) | Inspect and parse imported `\usepackage` packages |
| `POST` | [`/api/check`](#5-post-apicheck) | Lint and check LaTeX source for errors and warnings |
| `POST` | [`/api/debug`](#6-post-apidebug) | Audit LaTeX source and PDF preview for rendering issues (matrices, math) |

---

## 1. GET `/health`

Verifies that the Laketex headless service is running.

### Request
```bash
curl -X GET http://127.0.0.1:3030/health
```

### Response (`200 OK`)
```json
{
  "status": "ok",
  "app": "Laketex",
  "version": "0.1.0",
  "native_engine_available": true
}
```

---

## 2. POST `/api/compile`

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

## 3. POST `/api/outline`

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

## 4. POST `/api/packages`

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

## 5. POST `/api/check`

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
    "app_version": "0.1.0",
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

## 6. POST `/api/debug`

Scans the LaTeX source code and generates a rendered PDF preview to perform an in-depth audit of rendering issues, syntax problems, and visual degradations. It explicitly targets matrices, math notation, missing math delimiters, and PDF integrity.

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

## Error Handling

When an unexpected internal error or bad payload is delivered, the server responds with an appropriate HTTP status code (`400 Bad Request` or `500 Internal Server Error`) and JSON error descriptor:

```json
{
  "error": "Failed to parse JSON body: missing field `source`"
}
```
