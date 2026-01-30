use crate::config::AppConfig;
use anyhow::Context as _;
use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use image::{GenericImageView, ImageReader, Limits};
use rayon::prelude::*;
use std::io::{BufRead, Cursor, Read, Seek};
use std::path::Path;

/// Minimum pixel count before parallelizing per-pixel transforms.
const PARALLEL_PIXEL_THRESHOLD: usize = 262_144; // 512x512

fn map_pixels(total_pixels: usize, f: impl Fn(usize) -> Color32 + Sync + Send) -> Vec<Color32> {
    if total_pixels >= PARALLEL_PIXEL_THRESHOLD {
        (0..total_pixels).into_par_iter().map(f).collect()
    } else {
        let mut out = Vec::with_capacity(total_pixels);
        for idx in 0..total_pixels {
            out.push(f(idx));
        }
        out
    }
}

/// Image data plus the egui texture handle that mirrors its pixels.
pub struct LoadedImage {
    pub size: [usize; 2],
    pub texture: TextureHandle,
    pub pixels: ColorImage,
}

impl LoadedImage {
    fn refresh_texture(&mut self) {
        self.size = self.pixels.size;
        self.texture
            .set(self.pixels.clone(), TextureOptions::LINEAR);
    }

    /// Construct a `LoadedImage` from in-memory pixels and upload a texture.
    pub fn from_color_image(ctx: &Context, pixels: ColorImage) -> Self {
        let size = pixels.size;
        let texture = ctx.load_texture("loaded_image", pixels.clone(), TextureOptions::LINEAR);
        Self {
            size,
            texture,
            pixels,
        }
    }

    /// Rotate the image 90 degrees clockwise, updating pixels and texture.
    pub fn rotate_90_cw(&mut self) {
        let [width, height] = self.size;
        if width == 0 || height == 0 {
            return;
        }
        let new_width = height;
        let total_pixels = width * height;
        let pixels = &self.pixels.pixels;
        let rotated_pixels = map_pixels(total_pixels, |idx| {
            let dx = idx % new_width;
            let dy = idx / new_width;
            let src_x = dy;
            let src_y = new_width - 1 - dx;
            let src_idx = src_y * width + src_x;
            pixels[src_idx]
        });
        self.pixels = ColorImage::new([height, width], rotated_pixels);
        self.refresh_texture();
    }

    /// Rotate the image 90 degrees counter-clockwise, updating pixels and texture.
    pub fn rotate_90_ccw(&mut self) {
        let [width, height] = self.size;
        if width == 0 || height == 0 {
            return;
        }
        let new_width = height;
        let total_pixels = width * height;
        let pixels = &self.pixels.pixels;
        let rotated_pixels = map_pixels(total_pixels, |idx| {
            let dx = idx % new_width;
            let dy = idx / new_width;
            let src_y = dx;
            let src_x = width - 1 - dy;
            let src_idx = src_y * width + src_x;
            pixels[src_idx]
        });
        self.pixels = ColorImage::new([height, width], rotated_pixels);
        self.refresh_texture();
    }

    /// Mirror the image horizontally (left-right), updating pixels and texture.
    pub fn flip_horizontal(&mut self) {
        let [width, height] = self.size;
        if width == 0 || height == 0 {
            return;
        }
        let total_pixels = width * height;
        let pixels = &self.pixels.pixels;
        let flipped_pixels = map_pixels(total_pixels, |idx| {
            let x = idx % width;
            let y = idx / width;
            let src_x = width - 1 - x;
            let src_idx = y * width + src_x;
            pixels[src_idx]
        });
        self.pixels = ColorImage::new([width, height], flipped_pixels);
        self.refresh_texture();
    }

    /// Mirror the image vertically (top-bottom), updating pixels and texture.
    pub fn flip_vertical(&mut self) {
        let [width, height] = self.size;
        if width == 0 || height == 0 {
            return;
        }
        let total_pixels = width * height;
        let pixels = &self.pixels.pixels;
        let flipped_pixels = map_pixels(total_pixels, |idx| {
            let x = idx % width;
            let y = idx / width;
            let src_y = height - 1 - y;
            let src_idx = src_y * width + x;
            pixels[src_idx]
        });
        self.pixels = ColorImage::new([width, height], flipped_pixels);
        self.refresh_texture();
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn color_id(id: u8) -> Color32 {
        Color32::from_rgb(id, 0, 0)
    }

    fn ids_from_image(image: &ColorImage) -> Vec<u8> {
        image
            .pixels
            .iter()
            .map(|c| c.to_srgba_unmultiplied()[0])
            .collect()
    }

    fn test_image() -> ColorImage {
        ColorImage::new(
            [3, 2],
            vec![
                color_id(1),
                color_id(2),
                color_id(3),
                color_id(4),
                color_id(5),
                color_id(6),
            ],
        )
    }

    #[test]
    fn rotate_90_cw_maps_pixels() {
        let ctx = Context::default();
        let mut image = LoadedImage::from_color_image(&ctx, test_image());
        image.rotate_90_cw();
        assert_eq!(image.size, [2, 3]);
        assert_eq!(ids_from_image(&image.pixels), vec![4, 1, 5, 2, 6, 3]);
    }

    #[test]
    fn rotate_90_ccw_maps_pixels() {
        let ctx = Context::default();
        let mut image = LoadedImage::from_color_image(&ctx, test_image());
        image.rotate_90_ccw();
        assert_eq!(image.size, [2, 3]);
        assert_eq!(ids_from_image(&image.pixels), vec![3, 6, 2, 5, 1, 4]);
    }

    #[test]
    fn flip_horizontal_maps_pixels() {
        let ctx = Context::default();
        let mut image = LoadedImage::from_color_image(&ctx, test_image());
        image.flip_horizontal();
        assert_eq!(image.size, [3, 2]);
        assert_eq!(ids_from_image(&image.pixels), vec![3, 2, 1, 6, 5, 4]);
    }

    #[test]
    fn flip_vertical_maps_pixels() {
        let ctx = Context::default();
        let mut image = LoadedImage::from_color_image(&ctx, test_image());
        image.flip_vertical();
        assert_eq!(image.size, [3, 2]);
        assert_eq!(ids_from_image(&image.pixels), vec![4, 5, 6, 1, 2, 3]);
    }
}
