use super::CurcatApp;
use arboard::{Clipboard, Error as ClipboardError};
use egui::Context;

struct ClipboardCapture {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

impl CurcatApp {
    pub(crate) fn paste_image_from_clipboard(&mut self, _ctx: &Context) {
        self.project.pending_image_task = None;
        self.project.pending_image_limit_prompt = None;

        match capture_clipboard_image() {
            Ok(captured) => {
                self.start_loading_image_from_clipboard(
                    captured.width,
                    captured.height,
                    captured.rgba,
                );
            }
            Err(err) => self.set_status_error(err),
        }
    }
}

fn capture_clipboard_image() -> Result<ClipboardCapture, String> {
    let mut clipboard = Clipboard::new().map_err(format_clipboard_error)?;
    let data = clipboard.get_image().map_err(format_clipboard_error)?;

    if data.width == 0 || data.height == 0 {
        return Err("Paste failed: clipboard image is empty.".to_string());
    }

    let expected_len = data
        .width
        .checked_mul(data.height)
        .and_then(|px| px.checked_mul(4))
        .ok_or_else(|| "Paste failed: clipboard image dimensions are too large.".to_string())?;

    let bytes = data.bytes.into_owned();
    if bytes.len() < expected_len {
        return Err("Paste failed: clipboard image data is truncated.".to_string());
    }

    Ok(ClipboardCapture {
        width: data.width,
        height: data.height,
        rgba: bytes[..expected_len].to_vec(),
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
