//! # PDF Document Viewer & Rasterization Subsystem
//!
//! Provides [`PdfRenderer`] for binding Google PDFium to rasterize compiled PDF pages into
//! high-DPI `egui::ColorImage` bitmaps with optional color inversion (dark mode), as well
//! as the interactive canvas widget ([`canvas::PdfCanvas`]) and visual parity comparison
//! harness ([`parity::VisualParityComparator`]).

pub mod canvas;
pub mod parity;

use crate::error::{LakeTexError, Result};
use egui::ColorImage;
use image::RgbaImage;
use pdfium_render::prelude::*;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

static GLOBAL_PDFIUM: OnceLock<std::result::Result<Arc<Pdfium>, String>> = OnceLock::new();

/// High-DPI PDF rasterization engine powered by Google PDFium.
pub struct PdfRenderer {
    pdfium: Option<Arc<Pdfium>>,
    init_error: Option<String>,
}

impl Default for PdfRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfRenderer {
    /// Attempts to bind PDFium from bundled binaries, local executable directory, or system PATH.
    pub fn new() -> Self {
        let global_res = GLOBAL_PDFIUM.get_or_init(|| {
            match pdfium_bundled::bind_pdfium_silent() {
                Ok(p) => Ok(Arc::new(p)),
                Err(e1) => {
                    let e1_str = e1.to_string();
                    let candidate = Self::find_pdfium_library();
                    if let Some(path) = candidate {
                        match pdfium_bundled::bind_pdfium_from_path(&path) {
                            Ok(p) => Ok(Arc::new(p)),
                            Err(e2) => {
                                let e2_str = e2.to_string();
                                log_init_error(&format!("bundled: {e1_str}, local: {e2_str}"));
                                Err(format!("bundled: {e1_str}, local: {e2_str}"))
                            }
                        }
                    } else {
                        log_init_error(&format!("bundled: {e1_str}"));
                        Err(format!("bundled: {e1_str}"))
                    }
                }
            }
        });

        match global_res {
            Ok(arc_pdfium) => Self {
                pdfium: Some(arc_pdfium.clone()),
                init_error: None,
            },
            Err(err) => Self {
                pdfium: None,
                init_error: Some(format!("PDFium engine init failed: {err}")),
            },
        }
    }

    fn find_pdfium_library() -> Option<PathBuf> {
        let lib_name = if cfg!(windows) {
            "pdfium.dll"
        } else if cfg!(target_os = "macos") {
            "libpdfium.dylib"
        } else {
            "libpdfium.so"
        };

        if let Ok(mut exe_dir) = std::env::current_exe() {
            exe_dir.pop();
            let p = exe_dir.join(lib_name);
            if p.is_file() {
                return Some(p);
            }
        }

        if let Some(bin_dir) = crate::compiler::CompilerEngine::laketex_bin_dir() {
            let p = bin_dir.join(lib_name);
            if p.is_file() {
                return Some(p);
            }
        }

        let cwd_p = PathBuf::from(lib_name);
        if cwd_p.is_file() {
            return Some(cwd_p);
        }

        None
    }

    /// Returns true if the PDFium engine was successfully initialized
    pub fn is_ready(&self) -> bool {
        self.pdfium.is_some()
    }

    /// Returns initialization error message if binding failed
    pub fn init_error(&self) -> Option<&str> {
        self.init_error.as_deref()
    }

    /// Renders a specific page from PDF bytes to an `egui::ColorImage`.
    pub fn render_page_to_color_image(
        &self,
        pdf_bytes: &[u8],
        page_index: usize,
        target_width: u16,
        invert_colors: bool,
    ) -> Result<(ColorImage, usize)> {
        let pdfium = self
            .pdfium
            .as_ref()
            .ok_or_else(|| LakeTexError::PdfRender("PDFium engine is not available".to_string()))?;

        let doc = pdfium
            .load_pdf_from_byte_slice(pdf_bytes, None)
            .map_err(|e| LakeTexError::PdfRender(format!("Failed to load PDF: {e}")))?;

        let page_count = doc.pages().len() as usize;
        if page_count == 0 {
            return Err(LakeTexError::PdfRender("PDF document has 0 pages".to_string()));
        }

        let target_page_idx = (page_index.min(page_count - 1)) as i32;
        let page = doc
            .pages()
            .get(target_page_idx)
            .map_err(|e| LakeTexError::PdfRender(format!("Failed to get page {target_page_idx}: {e}")))?;

        let target_width_px = (target_width.max(600)) as i32;
        let render_config = PdfRenderConfig::new().set_target_width(target_width_px);

        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| LakeTexError::PdfRender(format!("Page render error: {e}")))?;

        let dynamic_img = bitmap
            .as_image()
            .map_err(|e| LakeTexError::PdfRender(format!("Bitmap conversion error: {e}")))?;

        let mut rgba_img: RgbaImage = dynamic_img.to_rgba8();

        if invert_colors {
            for pixel in rgba_img.pixels_mut() {
                pixel[0] = 255 - pixel[0];
                pixel[1] = 255 - pixel[1];
                pixel[2] = 255 - pixel[2];
            }
        }

        let width = rgba_img.width() as usize;
        let height = rgba_img.height() as usize;
        let pixels = rgba_img.into_raw();

        let color_image = ColorImage::from_rgba_unmultiplied([width, height], &pixels);
        Ok((color_image, page_count))
    }
}

fn log_init_error(msg: &str) {
    eprintln!("[Laketex] PDFium init notice: {msg}");
}
