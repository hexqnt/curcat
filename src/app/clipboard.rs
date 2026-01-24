use super::CurcatApp;
use crate::config::AppConfig;
use crate::image_info::{ImageMeta, human_readable_bytes};
use crate::image_util::LoadedImage;
use arboard::{Clipboard, Error as ClipboardError};
use egui::{ColorImage, Context};

struct ClipboardCapture {
    image: ColorImage,
    byte_len: usize,
}

struct ValidatedClipboardSize {
    width: usize,
    height: usize,
    expected_len: usize,
}

impl CurcatApp {
    pub(crate) fn paste_image_from_clipboard(&mut self, ctx: &Context) {
        self.project.pending_image_task = None;
        match capture_clipboard_image(&self.config) {
            Ok(captured) => {
                let meta = ImageMeta::from_clipboard(u64::try_from(captured.byte_len).ok());
                let name = meta.display_name();
                let loaded = LoadedImage::from_color_image(ctx, captured.image);
                self.set_loaded_image(loaded, Some(meta));
                self.set_status(format!("Loaded {name}"));
                self.image.pending_fit_on_load = self.project.pending_project_apply.is_none();
            }
            Err(err) => self.set_status(err),
        }
    }
}

fn capture_clipboard_image(cfg: &AppConfig) -> Result<ClipboardCapture, String> {
    let mut clipboard = Clipboard::new().map_err(format_clipboard_error)?;
    let data = clipboard.get_image().map_err(format_clipboard_error)?;
    let size = validate_clipboard_image(cfg, data.width, data.height)?;
    let bytes = data.bytes.into_owned();
    if bytes.len() < size.expected_len {
        return Err("Paste failed: clipboard image data is truncated.".to_string());
    }
    let image =
        ColorImage::from_rgba_unmultiplied([size.width, size.height], &bytes[..size.expected_len]);
    Ok(ClipboardCapture {
        image,
        byte_len: size.expected_len,
    })
}

fn validate_clipboard_image(
    cfg: &AppConfig,
    width: usize,
    height: usize,
) -> Result<ValidatedClipboardSize, String> {
    if width == 0 || height == 0 {
        return Err("Paste failed: clipboard image is empty.".to_string());
    }
    let limits = cfg.effective_image_limits();
    let width_u32 = u32::try_from(width).unwrap_or(u32::MAX);
    let height_u32 = u32::try_from(height).unwrap_or(u32::MAX);
    if width_u32 > limits.image_dim || height_u32 > limits.image_dim {
        return Err(format!(
            "Paste failed: clipboard image {width}x{height} exceeds the per-side limit ({} px).",
            limits.image_dim
        ));
    }

    let total_pixels = u64::try_from(width)
        .ok()
        .and_then(|w| u64::try_from(height).ok().and_then(|h| w.checked_mul(h)))
        .ok_or_else(|| {
            "Paste failed: clipboard dimensions are too large for this system.".to_string()
        })?;
    if total_pixels > limits.total_pixels {
        return Err(format!(
            "Paste failed: clipboard image too large: {width}x{height} (~{} MP) exceeds limit (~{} MP).",
            total_pixels / 1_000_000,
            limits.total_pixels / 1_000_000
        ));
    }

    let rgba_bytes = total_pixels.checked_mul(4).ok_or_else(|| {
        "Paste failed: clipboard image is too large to fit in memory.".to_string()
    })?;
    if rgba_bytes > limits.alloc_bytes {
        return Err(format!(
            "Paste failed: clipboard image needs about {} of RGBA data, over the configured limit ({}).",
            human_readable_bytes(rgba_bytes),
            human_readable_bytes(limits.alloc_bytes)
        ));
    }

    let expected_len = usize::try_from(rgba_bytes).map_err(|_| {
        "Paste failed: clipboard image does not fit in available memory.".to_string()
    })?;

    Ok(ValidatedClipboardSize {
        width,
        height,
        expected_len,
    })
}

fn format_clipboard_error(err: ClipboardError) -> String {
    match err {
        ClipboardError::ContentNotAvailable => {
            "Paste failed: clipboard does not contain an image.".to_string()
        }
        ClipboardError::ClipboardNotSupported => {
            "Paste failed: clipboard access is not supported in this environment.".to_string()
        }
        ClipboardError::ClipboardOccupied => {
            "Paste failed: clipboard is busy; try again in a moment.".to_string()
        }
        ClipboardError::ConversionFailure => {
            "Paste failed: clipboard image could not be converted.".to_string()
        }
        ClipboardError::Unknown { description } => {
            format!("Paste failed: {description}")
        }
        _ => {
            format!("Paste failed: {err}")
        }
    }
}
