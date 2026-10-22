//! # Visual Parity Comparison & Automated Benchmarking
//!
//! Provides automated visual parity testing between the LakeTex pure-Rust native engine
//! and reference TeX compilers (such as Tectonic).
//!
//! Workflow:
//! 1. Compiles identical LaTeX document source through both engines.
//! 2. Rasterizes both resulting PDF byte streams into identical-dimension RGB pixel buffers.
//! 3. Calculates per-pixel delta matrices, Mean Squared Error (MSE), and Peak Signal-to-Noise Ratio (PSNR).
//! 4. Generates side-by-side comparison images and high-visibility magenta discrepancy heatmaps.
//! 5. Emits structured JSON parity reports ([`VisualParityReport`]).

use crate::compiler::engine::{CompilerEngine, EngineKind};
use crate::error::{LakeTexError, Result};
use crate::viewer::PdfRenderer;
use egui::{Color32, ColorImage};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Metrics report comparing visual fidelity between two compilation outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualParityReport {
    /// Name or title of the tested document
    pub document_name: String,
    /// 1-based page index compared
    pub page_number: usize,
    /// Requested raster target width in pixels
    pub target_width_px: u32,
    /// Final aligned width in pixels
    pub rendered_width_px: usize,
    /// Final aligned height in pixels
    pub rendered_height_px: usize,
    /// Total pixels evaluated
    pub total_pixels: usize,
    /// Number of matching pixels within perceptual threshold
    pub matching_pixels: usize,
    /// Number of mismatching pixels
    pub mismatch_pixels: usize,
    /// Visual parity score percentage (0.0% to 100.0%)
    pub visual_parity_score: f32,
    /// Mean Squared Error across color channels
    pub mean_squared_error: f64,
    /// Peak Signal-to-Noise Ratio in decibels (dB)
    pub peak_signal_to_noise_ratio: f64,
    /// Byte size of reference Tectonic PDF
    pub tectonic_pdf_bytes: usize,
    /// Byte size of native pure-Rust PDF
    pub native_pdf_bytes: usize,
    /// Compilation duration for Tectonic in milliseconds
    pub tectonic_compile_ms: u128,
    /// Compilation duration for Native engine in milliseconds
    pub native_compile_ms: u128,
    /// Output directory where comparison PNG artifacts were saved
    pub output_dir: Option<String>,
}

/// Visual parity comparison engine.
pub struct VisualParityComparator;

impl VisualParityComparator {
    /// Compares Tectonic rendering vs Native Engine rendering on a given LaTeX document.
    pub fn compare_latex_source(
        latex_source: &str,
        document_name: &str,
        page_index: usize,
        target_width: u16,
        output_dir: Option<&Path>,
    ) -> Result<VisualParityReport> {
        Self::compare_latex_source_with_dir(
            latex_source,
            document_name,
            page_index,
            target_width,
            output_dir,
            None,
        )
    }

    /// Compares Tectonic rendering vs Native Engine rendering on a given LaTeX document within a specific working directory.
    pub fn compare_latex_source_with_dir(
        latex_source: &str,
        document_name: &str,
        page_index: usize,
        target_width: u16,
        output_dir: Option<&Path>,
        work_dir: Option<&Path>,
    ) -> Result<VisualParityReport> {
        // 1. Compile with Tectonic
        let tectonic_res = CompilerEngine::compile(latex_source, EngineKind::Tectonic, work_dir)?;
        let tectonic_pdf = tectonic_res
            .pdf_bytes
            .ok_or_else(|| LakeTexError::Engine(format!("Tectonic failed to produce PDF: {}", tectonic_res.stderr)))?;

        // 2. Compile with Native Pure-Rust Engine
        let native_res = CompilerEngine::compile(latex_source, EngineKind::Native, work_dir)?;
        let native_pdf = native_res
            .pdf_bytes
            .ok_or_else(|| LakeTexError::Engine(format!("Native engine failed to produce PDF: {}", native_res.stderr)))?;

        // 3. Render both to ColorImage bitmaps
        let renderer = PdfRenderer::new();
        if !renderer.is_ready() {
            return Err(LakeTexError::PdfRender("PDFium engine is unavailable for rendering".to_string()));
        }

        let (img_tectonic, _t_pages) = renderer.render_page_to_color_image(&tectonic_pdf, page_index, target_width, false)
            .map_err(|e| LakeTexError::PdfRender(format!("Tectonic PDF render error: {e}")))?;
        let (img_native, _n_pages) = renderer.render_page_to_color_image(&native_pdf, page_index, target_width, false)
            .map_err(|e| LakeTexError::PdfRender(format!("Native PDF render error: {e}")))?;

        // 4. Align dimensions for comparative pixel analysis
        let w = img_tectonic.width().min(img_native.width());
        let h = img_tectonic.height().min(img_native.height());
        let total_pixels = w * h;

        let mut matching_pixels = 0;
        let mut sum_squared_error = 0.0_f64;
        let mut diff_pixels = Vec::with_capacity(total_pixels);

        for y in 0..h {
            for x in 0..w {
                let p_t = img_tectonic[(x, y)];
                let p_n = img_native[(x, y)];

                let dr = (p_t.r() as i32 - p_n.r() as i32).abs();
                let dg = (p_t.g() as i32 - p_n.g() as i32).abs();
                let db = (p_t.b() as i32 - p_n.b() as i32).abs();

                let pixel_sq_err = ((dr * dr + dg * dg + db * db) as f64) / 3.0;
                sum_squared_error += pixel_sq_err;

                // Threshold tolerance of 25 accounts for subpixel font hinting and raster filtering differences
                let max_channel_diff = dr.max(dg).max(db);
                if max_channel_diff <= 25 {
                    matching_pixels += 1;
                    // Matching pixels rendered in subtle muted gray
                    let lum = (p_t.r() as u16 + p_t.g() as u16 + p_t.b() as u16) / 3;
                    let gray = (210 + (lum * 45) / 255) as u8;
                    diff_pixels.push(Color32::from_gray(gray));
                } else {
                    // Discrepancy highlighted in high-visibility bright magenta
                    diff_pixels.push(Color32::from_rgb(255, 20, 110));
                }
            }
        }

        let mismatch_pixels = total_pixels - matching_pixels;
        let visual_parity_score = ((matching_pixels as f32 / total_pixels as f32) * 100.0).clamp(0.0, 100.0);
        let mse = sum_squared_error / (total_pixels as f64).max(1.0);
        let psnr = if mse < 1e-5 {
            99.0
        } else {
            10.0 * ((255.0 * 255.0) / mse).log10()
        };

        let diff_image = ColorImage {
            size: [w, h],
            pixels: diff_pixels,
        };

        let report = VisualParityReport {
            document_name: document_name.to_string(),
            page_number: page_index + 1,
            target_width_px: target_width as u32,
            rendered_width_px: w,
            rendered_height_px: h,
            total_pixels,
            matching_pixels,
            mismatch_pixels,
            visual_parity_score,
            mean_squared_error: (mse * 100.0).round() / 100.0,
            peak_signal_to_noise_ratio: (psnr * 100.0).round() / 100.0,
            tectonic_pdf_bytes: tectonic_pdf.len(),
            native_pdf_bytes: native_pdf.len(),
            tectonic_compile_ms: tectonic_res.duration_ms,
            native_compile_ms: native_res.duration_ms,
            output_dir: output_dir.map(|p| p.display().to_string()),
        };

        // 5. If output directory requested, write image artifacts and JSON report
        if let Some(out_dir) = output_dir {
            std::fs::create_dir_all(out_dir).map_err(LakeTexError::Io)?;

            let tectonic_png_path = out_dir.join("tectonic_render.png");
            let native_png_path = out_dir.join("native_render.png");
            let diff_png_path = out_dir.join("diff_comparison.png");
            let json_path = out_dir.join("visual_parity_report.json");

            Self::save_color_image_to_png(&img_tectonic, &tectonic_png_path)?;
            Self::save_color_image_to_png(&img_native, &native_png_path)?;
            Self::save_color_image_to_png(&diff_image, &diff_png_path)?;

            let report_json = serde_json::to_string_pretty(&report)?;
            std::fs::write(&json_path, report_json).map_err(LakeTexError::Io)?;
        }

        Ok(report)
    }

    /// Saves an egui ColorImage to a PNG image file.
    pub fn save_color_image_to_png(img: &ColorImage, path: &Path) -> Result<()> {
        let rgba_bytes: Vec<u8> = img
            .pixels
            .iter()
            .flat_map(|c| [c.r(), c.g(), c.b(), c.a()])
            .collect();

        let rgba_img = image::RgbaImage::from_raw(img.width() as u32, img.height() as u32, rgba_bytes)
            .ok_or_else(|| LakeTexError::PdfRender("Failed to create RGBA buffer from ColorImage".to_string()))?;

        rgba_img
            .save(path)
            .map_err(|e| LakeTexError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

        Ok(())
    }
}
