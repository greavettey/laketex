use laketex::document::DEFAULT_DOCUMENT_TEMPLATE;
use laketex::viewer::parity::VisualParityComparator;
use std::path::Path;

#[test]
fn test_visual_parity_between_tectonic_and_native() {
    let out_dir = Path::new("target/parity_reports/test_run");
    let report = VisualParityComparator::compare_latex_source(
        DEFAULT_DOCUMENT_TEMPLATE,
        "Laketex_Default_Template",
        0,
        1000,
        Some(out_dir),
    )
    .expect("Visual parity comparison pipeline must succeed");

    println!("Visual Parity Report Summary:");
    println!("  Document: {}", report.document_name);
    println!("  Dimensions: {} x {} px", report.rendered_width_px, report.rendered_height_px);
    println!("  Total pixels: {}", report.total_pixels);
    println!("  Matching pixels: {} ({:.2}%)", report.matching_pixels, report.visual_parity_score);
    println!("  Mismatch pixels: {}", report.mismatch_pixels);
    println!("  PSNR: {:.2} dB", report.peak_signal_to_noise_ratio);
    println!("  MSE: {:.2}", report.mean_squared_error);
    println!("  Tectonic size: {} bytes, Native size: {} bytes", report.tectonic_pdf_bytes, report.native_pdf_bytes);
    println!("  Tectonic compile: {} ms, Native compile: {} ms", report.tectonic_compile_ms, report.native_compile_ms);

    // Verify artifacts on disk
    assert!(out_dir.join("tectonic_render.png").exists(), "Tectonic render PNG must exist");
    assert!(out_dir.join("native_render.png").exists(), "Native render PNG must exist");
    assert!(out_dir.join("diff_comparison.png").exists(), "Visual diff PNG must exist");
    assert!(out_dir.join("visual_parity_report.json").exists(), "JSON parity report must exist");

    // Both engines must produce non-empty valid documents
    assert!(report.tectonic_pdf_bytes > 1000);
    assert!(report.native_pdf_bytes > 1000);
    assert!(report.total_pixels > 100_000);
}
