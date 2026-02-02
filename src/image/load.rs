use crate::config::AppConfig;
use anyhow::Context as _;
use egui::ColorImage;
use image::{GenericImageView, ImageReader, Limits};
use std::io::{BufRead, Cursor, Read, Seek};
use std::path::Path;

fn decode_reader_to_color<R>(
    cfg: &AppConfig,
    mut reader: ImageReader<R>,
) -> anyhow::Result<ColorImage>
where
    R: Read + Seek + BufRead,
{
    let il = cfg.effective_image_limits();
    let mut limits = Limits::default();
    limits.max_image_width = Some(il.image_dim);
    limits.max_image_height = Some(il.image_dim);
    limits.max_alloc = Some(il.alloc_bytes);
    reader.limits(limits);
    let img = reader.decode().context("Failed to decode image data")?;

    let (w, h) = img.dimensions();
    let total_pixels = u64::from(w) * u64::from(h);
    if total_pixels > il.total_pixels {
        anyhow::bail!(
            "Image too large: {}x{} (~{} MP) exceeds limit (~{} MP)",
            w,
            h,
            total_pixels / 1_000_000,
            il.total_pixels / 1_000_000
        );
    }

    let rgba = img.to_rgba8();
    Ok(ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        &rgba,
    ))
}

/// Load and decode an image from a filesystem path using configured limits.
pub fn decode_image_from_path(cfg: &AppConfig, path: &Path) -> anyhow::Result<ColorImage> {
    let reader = ImageReader::open(path)
        .with_context(|| format!("Failed to read {}", path.display()))?
        .with_guessed_format()
        .context("Failed to detect image format")?;
    decode_reader_to_color(cfg, reader)
}

/// Load and decode an image from raw bytes using configured limits.
pub fn decode_image_from_bytes(cfg: &AppConfig, bytes: Vec<u8>) -> anyhow::Result<ColorImage> {
    let cursor = Cursor::new(bytes);
    let reader = ImageReader::new(cursor)
        .with_guessed_format()
        .context("Failed to detect image format")?;
    decode_reader_to_color(cfg, reader)
}
