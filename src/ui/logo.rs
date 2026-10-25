//! # Application Logo & Branding Assets
//!
//! Bundles the official `assets/LaketexLogo.png` asset directly into the compiled executable
//! on all platforms via `include_bytes!`.
//!
//! Provides utilities for:
//! - Setting the OS window and taskbar application icon via [`egui::IconData`].
//! - Decoding and caching the bundled PNG into an [`egui::TextureHandle`] for runtime UI rendering.
//! - Rendering the LakeTex logo across the toolbar, preferences, and about dialogs.

use egui::{Color32, ColorImage, IconData, Rect, Response, TextureHandle, TextureOptions, Ui, Vec2};

/// Raw PNG bytes of the official LakeTex logo embedded into the binary at compile time.
pub const LOGO_PNG_BYTES: &[u8] = include_bytes!("../../assets/LaketexLogo.png");

/// Loads and decodes the bundled PNG logo into an [`egui::ColorImage`].
pub fn load_logo_color_image() -> ColorImage {
    let dynamic_img = image::load_from_memory(LOGO_PNG_BYTES)
        .expect("assets/LaketexLogo.png must be a valid PNG image bundled into the binary");
    let rgba = dynamic_img.to_rgba8();
    let (width, height) = rgba.dimensions();
    ColorImage::from_rgba_unmultiplied([width as usize, height as usize], rgba.as_raw())
}

/// Loads and decodes the bundled PNG logo into [`egui::IconData`] for window/taskbar icons,
/// downscaled to 256x256 to avoid multi-megabyte window icon allocations.
pub fn app_icon_data() -> IconData {
    let dynamic_img = image::load_from_memory(LOGO_PNG_BYTES)
        .expect("assets/LaketexLogo.png must be a valid PNG image bundled into the binary");
    let resized = dynamic_img.resize_exact(256, 256, image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    let (width, height) = rgba.dimensions();
    IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    }
}

/// Loads or retrieves the cached logo [`TextureHandle`] in the active egui context.
pub fn get_or_load_logo_texture(ctx: &egui::Context) -> TextureHandle {
    ctx.load_texture(
        "laketex_brand_logo",
        load_logo_color_image(),
        TextureOptions::LINEAR,
    )
}

/// Renders the LakeTex logo image widget with a specified square size and optional rounded corners.
pub fn render_logo(ui: &mut Ui, texture: &TextureHandle, size: Vec2, corner_radius: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if corner_radius > 0.0 {
            // Render with clip rect / rounded corners
            painter.rect_filled(rect, corner_radius, Color32::TRANSPARENT);
        }
        painter.image(
            texture.id(),
            rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    response
}

/// Constructs an [`egui::text::LayoutJob`] for the "Laketex" brand label with an italicized 'L'.
pub fn brand_layout_job(size: f32, color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        "L",
        0.0,
        egui::text::TextFormat {
            font_id: egui::FontId::proportional(size),
            color,
            italics: true,
            ..Default::default()
        },
    );
    job.append(
        "aketex",
        0.0,
        egui::text::TextFormat {
            font_id: egui::FontId::proportional(size),
            color,
            italics: false,
            ..Default::default()
        },
    );
    job
}

/// Renders the official "Laketex" brand label with an italicized 'L' as a non-selectable widget.
pub fn render_brand_label(ui: &mut Ui, size: f32, color: Color32) -> Response {
    ui.add(egui::Label::new(brand_layout_job(size, color)).selectable(false))
}

/// Generates a multi-resolution Windows `.ico` binary payload (16, 32, 48, 64, 128, 256 px)
/// from the bundled `assets/LaketexLogo.png`.
pub fn generate_ico_bytes() -> Vec<u8> {
    let dynamic_img = image::load_from_memory(LOGO_PNG_BYTES)
        .expect("assets/LaketexLogo.png must be a valid PNG image bundled into the binary");

    let sizes: &[u32] = &[16, 32, 48, 64, 128, 256];
    let mut payloads: Vec<(u32, Vec<u8>)> = Vec::with_capacity(sizes.len());

    for &size in sizes {
        let resized = dynamic_img
            .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
            .to_rgba8();

        if size == 256 {
            // 256x256 is stored as PNG per Windows Vista+ ICO specification
            let mut png_bytes = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut png_bytes);
            resized
                .write_to(&mut cursor, image::ImageFormat::Png)
                .expect("Failed to encode 256x256 ICO PNG layer");
            payloads.push((size, png_bytes));
        } else {
            // Sizes < 256 stored as 32-bit BGRA BITMAPINFOHEADER DIB + 1-bit AND mask for rc.exe compatibility
            let xor_bytes_len = (size * size * 4) as usize;
            let and_row_bytes = ((size + 31) / 32 * 4) as usize;
            let and_bytes_len = and_row_bytes * (size as usize);
            let mut dib = Vec::with_capacity(40 + xor_bytes_len + and_bytes_len);

            // BITMAPINFOHEADER (40 bytes)
            dib.extend_from_slice(&40u32.to_le_bytes()); // biSize
            dib.extend_from_slice(&(size as i32).to_le_bytes()); // biWidth
            dib.extend_from_slice(&((size * 2) as i32).to_le_bytes()); // biHeight (doubled for XOR + AND)
            dib.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
            dib.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
            dib.extend_from_slice(&0u32.to_le_bytes()); // biCompression (BI_RGB)
            dib.extend_from_slice(&((xor_bytes_len + and_bytes_len) as u32).to_le_bytes()); // biSizeImage
            dib.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
            dib.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
            dib.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
            dib.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

            // XOR mask: bottom-up BGRA rows
            for y in (0..size).rev() {
                for x in 0..size {
                    let [r, g, b, a] = resized.get_pixel(x, y).0;
                    dib.extend_from_slice(&[b, g, r, a]);
                }
            }

            // AND mask: bottom-up 1-bit transparency rows padded to 32-bit boundaries
            for y in (0..size).rev() {
                let mut row = vec![0u8; and_row_bytes];
                for x in 0..size {
                    let a = resized.get_pixel(x, y).0[3];
                    if a == 0 {
                        row[(x / 8) as usize] |= 1 << (7 - (x % 8));
                    }
                }
                dib.extend_from_slice(&row);
            }

            payloads.push((size, dib));
        }
    }

    let count = payloads.len() as u16;
    let header_size = 6 + (count as usize) * 16;
    let total_payload_size: usize = payloads.iter().map(|(_, b)| b.len()).sum();
    let mut ico = Vec::with_capacity(header_size + total_payload_size);

    // ICONDIR header (6 bytes)
    ico.extend_from_slice(&0u16.to_le_bytes()); // Reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // Type = 1 (ICO)
    ico.extend_from_slice(&count.to_le_bytes()); // Count

    // ICONDIRENTRY records (16 bytes each)
    let mut current_offset = header_size as u32;
    for (size, data) in &payloads {
        let dim_byte = if *size >= 256 { 0u8 } else { *size as u8 };
        ico.push(dim_byte); // bWidth
        ico.push(dim_byte); // bHeight
        ico.push(0u8); // bColorCount
        ico.push(0u8); // bReserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // wPlanes
        ico.extend_from_slice(&32u16.to_le_bytes()); // wBitCount
        ico.extend_from_slice(&(data.len() as u32).to_le_bytes()); // dwBytesInRes
        ico.extend_from_slice(&current_offset.to_le_bytes()); // dwImageOffset
        current_offset += data.len() as u32;
    }

    // Image payloads
    for (_, data) in payloads {
        ico.extend_from_slice(&data);
    }

    ico
}

/// Generates an Apple `.icns` icon bundle payload (16x16 up to 1024x1024 Retina layers)
/// from the bundled `assets/LaketexLogo.png`.
pub fn generate_icns_bytes() -> Vec<u8> {
    let dynamic_img = image::load_from_memory(LOGO_PNG_BYTES)
        .expect("assets/LaketexLogo.png must be a valid PNG image bundled into the binary");

    // Pre-encode each unique resolution once
    let unique_sizes: &[u32] = &[16, 32, 64, 128, 256, 512, 1024];
    let mut size_map: std::collections::HashMap<u32, Vec<u8>> =
        std::collections::HashMap::with_capacity(unique_sizes.len());

    for &size in unique_sizes {
        let resized = dynamic_img
            .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let mut png_bytes = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_bytes);
        resized
            .write_to(&mut cursor, image::ImageFormat::Png)
            .expect("Failed to encode ICNS PNG layer");
        size_map.insert(size, png_bytes);
    }

    let chunks: &[(&[u8; 4], u32)] = &[
        (b"icp4", 16),
        (b"icp5", 32),
        (b"icp6", 64),
        (b"ic07", 128),
        (b"ic08", 256),
        (b"ic09", 512),
        (b"ic10", 1024),
        (b"ic11", 32),
        (b"ic12", 64),
        (b"ic13", 256),
        (b"ic14", 512),
    ];

    let body_size: usize = chunks
        .iter()
        .map(|&(_, s)| 8 + size_map[&s].len())
        .sum();
    let total_size = (8 + body_size) as u32;
    let mut icns = Vec::with_capacity(total_size as usize);

    icns.extend_from_slice(b"icns");
    icns.extend_from_slice(&total_size.to_be_bytes());

    for &(ostype, size) in chunks {
        let png_bytes = &size_map[&size];
        let chunk_len = (8 + png_bytes.len()) as u32;
        icns.extend_from_slice(ostype);
        icns.extend_from_slice(&chunk_len.to_be_bytes());
        icns.extend_from_slice(png_bytes);
    }

    icns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundled_logo_png_bytes_validity() {
        assert!(!LOGO_PNG_BYTES.is_empty(), "Bundled logo bytes must not be empty");
        assert!(LOGO_PNG_BYTES.starts_with(b"\x89PNG\r\n\x1a\n"), "Must be a valid PNG header");
        assert_eq!(LOGO_PNG_BYTES.len(), 69427);
    }

    #[test]
    fn test_load_logo_color_image_dimensions() {
        let color_image = load_logo_color_image();
        assert_eq!(color_image.size, [1276, 1276]);
        assert_eq!(color_image.pixels.len(), 1276 * 1276);
    }

    #[test]
    fn test_app_icon_data_format() {
        let icon = app_icon_data();
        assert_eq!(icon.width, 256);
        assert_eq!(icon.height, 256);
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }

    #[test]
    fn test_render_logo_in_egui_context() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let tex = get_or_load_logo_texture(ui.ctx());
                let resp = render_logo(ui, &tex, egui::vec2(24.0, 24.0), 4.0);
                assert_eq!(resp.rect.width(), 24.0);
                assert_eq!(resp.rect.height(), 24.0);
            });
        });
    }

    #[test]
    fn test_brand_layout_job_italicized_l() {
        let job = brand_layout_job(14.5, Color32::WHITE);
        assert_eq!(job.text, "Laketex");
        assert_eq!(job.sections.len(), 2);
        assert!(job.sections[0].format.italics, "The 'L' section must be italicized");
        assert!(!job.sections[1].format.italics, "The 'aketex' section must not be italicized");
    }

    #[test]
    fn test_generate_and_verify_ico_and_icns_assets() {
        let assets_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        std::fs::create_dir_all(&assets_dir).unwrap();
        let ico_path = assets_dir.join("laketex.ico");
        let icns_path = assets_dir.join("laketex.icns");

        if !ico_path.is_file() {
            let ico_bytes = generate_ico_bytes();
            std::fs::write(&ico_path, &ico_bytes).expect("Failed to write assets/laketex.ico");
        }
        if !icns_path.is_file() {
            let icns_bytes = generate_icns_bytes();
            std::fs::write(&icns_path, &icns_bytes).expect("Failed to write assets/laketex.icns");
        }

        let ico_bytes = std::fs::read(&ico_path).expect("assets/laketex.ico must be readable");
        assert!(ico_bytes.starts_with(&[0, 0, 1, 0, 6, 0]), "Valid 6-layer ICO header");
        assert!(ico_bytes.len() > 20_000, "ICO payload must be substantial");

        let icns_bytes = std::fs::read(&icns_path).expect("assets/laketex.icns must be readable");
        assert!(icns_bytes.starts_with(b"icns"), "Valid ICNS magic header");
        let recorded_len = u32::from_be_bytes([icns_bytes[4], icns_bytes[5], icns_bytes[6], icns_bytes[7]]) as usize;
        assert_eq!(recorded_len, icns_bytes.len(), "ICNS header length must match total byte size");
    }
}
