use tauri::image::Image;
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Decode encoded image bytes (PNG/JPEG/GIF/WebP) into (rgba, width, height).
fn decode_rgba(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok((rgba.into_raw(), w, h))
}

/// Copy an image to the clipboard as a bitmap. `source` is an http(s) URL
/// (downloaded) or a local file path (read from disk).
#[tauri::command]
pub async fn copy_image(app: tauri::AppHandle, source: String) -> Result<(), String> {
    let bytes: Vec<u8> = if source.starts_with("http://") || source.starts_with("https://") {
        let resp = reqwest::get(&source).await.map_err(|e| e.to_string())?;
        resp.bytes().await.map_err(|e| e.to_string())?.to_vec()
    } else {
        std::fs::read(&source).map_err(|e| e.to_string())?
    };
    let (rgba, w, h) = decode_rgba(&bytes)?;
    let image = Image::new_owned(rgba, w, h);
    app.clipboard().write_image(&image).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_png_to_rgba() {
        let mut buf = std::io::Cursor::new(Vec::new());
        let img = image::RgbaImage::from_pixel(2, 3, image::Rgba([10, 20, 30, 255]));
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        let (rgba, w, h) = decode_rgba(&buf.into_inner()).unwrap();
        assert_eq!((w, h), (2, 3));
        assert_eq!(rgba.len(), (2 * 3 * 4) as usize);
        assert_eq!(&rgba[0..4], &[10, 20, 30, 255]);
    }

    #[test]
    fn rejects_non_image_bytes() {
        assert!(decode_rgba(b"not an image").is_err());
    }
}
