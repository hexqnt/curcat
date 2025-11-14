use crate::config::AppConfig;
use anyhow::Context as _;
use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use image::GenericImageView;
use image::ImageReader;
use image::Limits;
use rayon::prelude::*;
use std::io::{BufRead, Cursor, Read, Seek};
use std::path::Path;

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

    pub fn rotate_90_cw(&mut self) {
        let [width, height] = self.size;
        if width == 0 || height == 0 {
            return;
        }
        let new_width = height;
        let total_pixels = width * height;
        let rotated_pixels: Vec<Color32> = (0..total_pixels)
            .into_par_iter()
            .map(|idx| {
                let dx = idx % new_width;
                let dy = idx / new_width;
                let src_x = dy;
                let src_y = new_width - 1 - dx;
                let src_idx = src_y * width + src_x;
                self.pixels.pixels[src_idx]
            })
            .collect();
        self.pixels = ColorImage::new([height, width], rotated_pixels);
        self.refresh_texture();
    }

    pub fn rotate_90_ccw(&mut self) {
        let [width, height] = self.size;
        if width == 0 || height == 0 {
            return;
        }
        let new_width = height;
        let total_pixels = width * height;
        let rotated_pixels: Vec<Color32> = (0..total_pixels)
            .into_par_iter()
            .map(|idx| {
                let dx = idx % new_width;
                let dy = idx / new_width;
                let src_y = dx;
                let src_x = width - 1 - dy;
                let src_idx = src_y * width + src_x;
                self.pixels.pixels[src_idx]
            })
            .collect();
        self.pixels = ColorImage::new([height, width], rotated_pixels);
        self.refresh_texture();
    }
}

fn decode_reader<R>(
    ctx: &Context,
    cfg: &AppConfig,
    mut reader: ImageReader<R>,
) -> anyhow::Result<LoadedImage>
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
    let color = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    let texture = ctx.load_texture("loaded_image", color.clone(), TextureOptions::LINEAR);
    Ok(LoadedImage {
        size: [w as usize, h as usize],
        texture,
        pixels: color,
    })
}

pub fn load_image_from_bytes(
    ctx: &Context,
    cfg: &AppConfig,
    bytes: &[u8],
) -> anyhow::Result<LoadedImage> {
    let cursor = Cursor::new(bytes);
    let reader = ImageReader::new(cursor)
        .with_guessed_format()
        .context("Failed to detect image format")?;
    decode_reader(ctx, cfg, reader)
}

pub fn load_image_from_path(
    ctx: &Context,
    cfg: &AppConfig,
    path: &Path,
) -> anyhow::Result<LoadedImage> {
    let reader = ImageReader::open(path)
        .with_context(|| format!("Failed to read {}", path.display()))?
        .with_guessed_format()
        .context("Failed to detect image format")?;
    decode_reader(ctx, cfg, reader)
}
