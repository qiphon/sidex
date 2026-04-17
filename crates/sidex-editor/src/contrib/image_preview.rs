//! Image preview — built-in viewer for image files opened in the editor.
//!
//! When an image file is opened, this module provides zoom/pan controls,
//! fit-mode switching, background selection, and a status bar showing
//! dimensions, format, and file size.

use std::path::Path;

/// Supported image formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Webp,
    Svg,
    Bmp,
    Ico,
}

impl ImageFormat {
    /// Detects format from a file extension (case-insensitive).
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "gif" => Some(Self::Gif),
            "webp" => Some(Self::Webp),
            "svg" => Some(Self::Svg),
            "bmp" => Some(Self::Bmp),
            "ico" => Some(Self::Ico),
            _ => None,
        }
    }

    /// Short display label (e.g. `"PNG"`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Gif => "GIF",
            Self::Webp => "WEBP",
            Self::Svg => "SVG",
            Self::Bmp => "BMP",
            Self::Ico => "ICO",
        }
    }
}

/// How the image is scaled to fit the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageFitMode {
    /// Scale down to fit within the viewport, preserving aspect ratio.
    #[default]
    Fit,
    /// Scale to fill the viewport, cropping as needed.
    Fill,
    /// Show at the image's native resolution (1:1 pixels).
    Original,
}

/// Background behind the image (useful for images with transparency).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageBackground {
    #[default]
    Transparent,
    Checkerboard,
    White,
    Black,
}

/// Decoded image metadata ready for display.
#[derive(Debug, Clone)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    /// Original file size in bytes.
    pub file_size: u64,
    /// Handle into the GPU texture cache (assigned by the renderer).
    pub texture_id: Option<u64>,
}

/// Top-level state for the image preview pane.
#[derive(Debug, Clone)]
pub struct ImagePreview {
    pub path: String,
    pub zoom: f32,
    pub fit_mode: ImageFitMode,
    pub background: ImageBackground,
    pub image_data: Option<ImageData>,
}

const ZOOM_MIN: f32 = 0.05;
const ZOOM_MAX: f32 = 32.0;
const ZOOM_STEP: f32 = 1.2;

impl ImagePreview {
    /// Creates a new preview for the given file path.
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            zoom: 1.0,
            fit_mode: ImageFitMode::default(),
            background: ImageBackground::default(),
            image_data: None,
        }
    }

    /// Zoom in by one step.
    pub fn zoom_in(&mut self) { self.zoom = (self.zoom * ZOOM_STEP).min(ZOOM_MAX); }

    /// Zoom out by one step.
    pub fn zoom_out(&mut self) { self.zoom = (self.zoom / ZOOM_STEP).max(ZOOM_MIN); }

    /// Reset zoom to 100%.
    pub fn zoom_reset(&mut self) { self.zoom = 1.0; }

    /// Set an exact zoom level (clamped to valid range).
    pub fn set_zoom(&mut self, level: f32) { self.zoom = level.clamp(ZOOM_MIN, ZOOM_MAX); }

    /// Zoom percentage for display (e.g. `100`).
    #[must_use]
    pub fn zoom_percent(&self) -> u32 { (self.zoom * 100.0).round() as u32 }

    /// Status bar text: `"800 x 600 • PNG • 245 KB"`.
    #[must_use]
    pub fn status_text(&self) -> String {
        match &self.image_data {
            Some(data) => {
                let size = format_file_size(data.file_size);
                format!(
                    "{} x {} \u{2022} {} \u{2022} {}",
                    data.width,
                    data.height,
                    data.format.label(),
                    size,
                )
            }
            None => String::from("Loading\u{2026}"),
        }
    }

    /// Attach loaded image data to this preview.
    pub fn set_image_data(&mut self, data: ImageData) {
        self.image_data = Some(data);
    }
}

/// Loads image metadata from a file path.
///
/// This reads only the file size and infers format from the extension;
/// actual pixel decoding is handled by the GPU layer.
pub fn load_image(path: &Path) -> Result<ImageData, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| "file has no extension".to_string())?;

    let format = ImageFormat::from_extension(ext)
        .ok_or_else(|| format!("unsupported image format: {ext}"))?;

    let metadata = std::fs::metadata(path)
        .map_err(|e| format!("cannot read file: {e}"))?;

    Ok(ImageData {
        width: 0,
        height: 0,
        format,
        file_size: metadata.len(),
        texture_id: None,
    })
}

/// Returns `true` if the path's extension is a known image format.
#[must_use]
pub fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .and_then(ImageFormat::from_extension)
        .is_some()
}

fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_detection() {
        assert_eq!(ImageFormat::from_extension("png"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("JPG"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_extension("txt"), None);
    }

    #[test]
    fn is_image_file_check() {
        assert!(is_image_file(Path::new("photo.png")));
        assert!(is_image_file(Path::new("icon.ICO")));
        assert!(!is_image_file(Path::new("readme.md")));
        assert!(!is_image_file(Path::new("noext")));
    }

    #[test]
    fn zoom_controls() {
        let mut p = ImagePreview::new("/tmp/test.png");
        p.zoom_in();
        assert!(p.zoom > 1.0);
        p.zoom_reset();
        p.zoom_out();
        assert!(p.zoom < 1.0);
        p.set_zoom(0.001);
        assert!(p.zoom >= ZOOM_MIN);
        p.set_zoom(999.0);
        assert!(p.zoom <= ZOOM_MAX);
    }

    #[test]
    fn status_text_formatting() {
        let mut p = ImagePreview::new("/tmp/test.png");
        assert!(p.status_text().contains("Loading"));
        p.set_image_data(ImageData {
            width: 800, height: 600,
            format: ImageFormat::Png, file_size: 245 * 1024, texture_id: None,
        });
        assert_eq!(p.status_text(), "800 x 600 \u{2022} PNG \u{2022} 245 KB");
        assert_eq!(format_file_size(500), "500 B");
        assert_eq!(format_file_size(1_500_000), "1.4 MB");
    }
}
