//! # PDF Document Viewer & Rasterization Subsystem
//!
//! Provides [`PdfRenderer`] for binding Google PDFium to rasterize compiled PDF pages into
//! high-DPI `egui::ColorImage` bitmaps with optional color inversion (dark mode), as well
//! as the interactive canvas widget ([`canvas::PdfCanvas`]) and visual parity comparison
//! harness ([`parity::VisualParityComparator`]).

pub mod canvas;
pub mod color_mode;
pub mod parity;

pub use color_mode::PdfColorMode;

use crate::error::{LakeTexError, Result};
use egui::ColorImage;
use image::RgbaImage;
use pdfium_render::prelude::*;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

static GLOBAL_PDFIUM: OnceLock<std::result::Result<Arc<Pdfium>, String>> = OnceLock::new();

/// High-DPI PDF rasterization engine powered by Google PDFium.
#[derive(Clone, Default)]
pub struct PdfRenderer;

impl PdfRenderer {
    /// Initializes PDFium lazily in a background thread for instant (<1ms) application startup.
    pub fn new() -> Self {
        // Trigger background initialization so PDFium is warm when compilation completes
        std::thread::Builder::new()
            .name("pdfium-loader".to_string())
            .spawn(|| {
                let _ = Self::global_pdfium();
            })
            .ok();
        Self
    }

    /// Returns the global PDFium instance, initializing it if not already loaded.
    pub fn global_pdfium() -> &'static std::result::Result<Arc<Pdfium>, String> {
        GLOBAL_PDFIUM.get_or_init(|| {
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
        })
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

    /// Returns true if the PDFium engine was successfully initialized.
    /// Ensures initialization has completed.
    pub fn is_ready(&self) -> bool {
        Self::global_pdfium().is_ok()
    }

    /// Returns true without blocking if PDFium is already loaded in the background.
    pub fn is_loaded_nonblocking(&self) -> bool {
        GLOBAL_PDFIUM.get().and_then(|r| r.as_ref().ok()).is_some()
    }

    /// Returns initialization error message if binding failed
    pub fn init_error(&self) -> Option<&str> {
        Self::global_pdfium().as_ref().err().map(|s| s.as_str())
    }

    /// Renders a specific page from PDF bytes to an `egui::ColorImage` with a specific color mode.
    pub fn render_page_to_color_image_with_mode(
        &self,
        pdf_bytes: &[u8],
        page_index: usize,
        target_width: u16,
        color_mode: PdfColorMode,
    ) -> Result<(ColorImage, usize)> {
        let pdfium = match Self::global_pdfium() {
            Ok(p) => p,
            Err(err) => {
                return Err(LakeTexError::PdfRender(format!(
                    "PDFium engine is not available: {err}"
                )))
            }
        };

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

        match color_mode {
            PdfColorMode::Normal => {}
            PdfColorMode::AcademicDark => {
                for pixel in rgba_img.pixels_mut() {
                    pixel[0] = 255 - pixel[0];
                    pixel[1] = 255 - pixel[1];
                    pixel[2] = 255 - pixel[2];
                }
            }
            PdfColorMode::Sepia => {
                for pixel in rgba_img.pixels_mut() {
                    let r = pixel[0] as f32;
                    let g = pixel[1] as f32;
                    let b = pixel[2] as f32;
                    let tr = (0.393 * r + 0.769 * g + 0.189 * b).min(255.0);
                    let tg = (0.349 * r + 0.686 * g + 0.168 * b).min(255.0);
                    let tb = (0.272 * r + 0.534 * g + 0.131 * b).min(255.0);
                    pixel[0] = tr as u8;
                    pixel[1] = tg as u8;
                    pixel[2] = tb as u8;
                }
            }
        }

        let width = rgba_img.width() as usize;
        let height = rgba_img.height() as usize;
        let pixels = rgba_img.into_raw();

        let color_image = ColorImage::from_rgba_unmultiplied([width, height], &pixels);
        Ok((color_image, page_count))
    }

    /// Renders a specific page from PDF bytes to an `egui::ColorImage`.
    pub fn render_page_to_color_image(
        &self,
        pdf_bytes: &[u8],
        page_index: usize,
        target_width: u16,
        invert_colors: bool,
    ) -> Result<(ColorImage, usize)> {
        let mode = if invert_colors {
            PdfColorMode::AcademicDark
        } else {
            PdfColorMode::Normal
        };
        self.render_page_to_color_image_with_mode(pdf_bytes, page_index, target_width, mode)
    }
}

fn log_init_error(msg: &str) {
    eprintln!("[Laketex] PDFium init notice: {msg}");
}
