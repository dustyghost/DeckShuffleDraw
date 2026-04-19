use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use eframe::egui::{self, ColorImage, TextureHandle, Vec2};
use image::imageops::FilterType;

#[derive(Debug)]
pub struct DecodedCard {
    pub path: PathBuf,
    pub size: [usize; 2],
    pub rgba: Vec<u8>,
}

pub fn decode_image_from_path(path: &Path, max_size: Vec2) -> Result<DecodedCard, String> {
    let bytes = fs::read(path).map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    let image = image::load_from_memory(&bytes)
        .map_err(|error| format!("Failed to decode {}: {error}", path.display()))?;
    let resized = resize_for_display(image, max_size);
    Ok(DecodedCard {
        path: path.to_path_buf(),
        size: [resized.width() as usize, resized.height() as usize],
        rgba: resized.to_rgba8().into_raw(),
    })
}

pub fn load_texture_from_decoded(ctx: &egui::Context, decoded_card: DecodedCard) -> TextureHandle {
    let texture_name = decoded_card.path.display().to_string();
    let color_image = ColorImage::from_rgba_unmultiplied(decoded_card.size, &decoded_card.rgba);
    ctx.load_texture(texture_name, color_image, egui::TextureOptions::LINEAR)
}

pub fn start_background_loader(
    ctx: egui::Context,
    image_paths: Vec<PathBuf>,
    max_size: Vec2,
) -> Receiver<Result<DecodedCard, String>> {
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        for path in image_paths {
            if sender.send(decode_image_from_path(&path, max_size)).is_err() {
                break;
            }

            ctx.request_repaint();
        }
    });

    receiver
}

pub fn fit_size(image_size: Vec2, available: Vec2) -> Vec2 {
    if image_size.x <= 0.0 || image_size.y <= 0.0 || available.x <= 0.0 || available.y <= 0.0 {
        return image_size;
    }

    let scale = (available.x / image_size.x)
        .min(available.y / image_size.y)
        .max(0.0);

    image_size * scale
}

pub fn find_image_paths(image_dir: &Path) -> Vec<PathBuf> {
    let mut images: Vec<_> = fs::read_dir(image_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png"))
                .unwrap_or(false)
        })
        .collect();

    images.sort();
    images
}

fn resize_for_display(image: image::DynamicImage, max_size: Vec2) -> image::DynamicImage {
    let max_width = max_size.x.max(1.0).round() as u32;
    let max_height = max_size.y.max(1.0).round() as u32;

    if image.width() <= max_width && image.height() <= max_height {
        image
    } else {
        image.resize(max_width, max_height, FilterType::Lanczos3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_test_dir(name: &str) -> PathBuf {
        let unique = format!(
            "gma_runner_{name}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).expect("temp dir should be created");
        path
    }

    #[test]
    fn fit_size_preserves_aspect_ratio() {
        let fitted = fit_size(egui::vec2(800.0, 1200.0), egui::vec2(400.0, 400.0));

        assert!((fitted.x - 266.66666).abs() < 0.01);
        assert!((fitted.y - 400.0).abs() < f32::EPSILON);
    }

    #[test]
    fn fit_size_returns_original_for_non_positive_bounds() {
        let original = egui::vec2(800.0, 1200.0);

        assert_eq!(fit_size(original, egui::vec2(0.0, 400.0)), original);
        assert_eq!(fit_size(original, egui::vec2(400.0, 0.0)), original);
    }

    #[test]
    fn find_image_paths_filters_supported_extensions_and_sorts() {
        let dir = temp_test_dir("find_image_paths");
        fs::write(dir.join("b-card.png"), b"png").expect("png file should be written");
        fs::write(dir.join("a-card.JPG"), b"jpg").expect("jpg file should be written");
        fs::write(dir.join("notes.txt"), b"text").expect("text file should be written");

        let paths = find_image_paths(&dir);

        assert_eq!(paths.len(), 2);
        assert_eq!(
            paths[0].file_name().and_then(|name| name.to_str()),
            Some("a-card.JPG")
        );
        assert_eq!(
            paths[1].file_name().and_then(|name| name.to_str()),
            Some("b-card.png")
        );
    }
}
