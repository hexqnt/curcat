use crate::config::AppConfig;
use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use image::GenericImageView;
use image::ImageReader;
use image::Limits;
use std::io::Cursor;

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
        let [w, h] = self.size;
        if w == 0 || h == 0 {
            return;
        }
        let mut rotated = ColorImage::new([h, w], vec![Color32::TRANSPARENT; w * h]);
        for y in 0..h {
            for x in 0..w {
                let src_idx = y * w + x;
                let new_x = h - 1 - y;
                let new_y = x;
                let dst_idx = new_y * h + new_x;
                rotated.pixels[dst_idx] = self.pixels.pixels[src_idx];
            }
        }
        self.pixels = rotated;
        self.refresh_texture();
    }

    pub fn rotate_90_ccw(&mut self) {
        let [w, h] = self.size;
        if w == 0 || h == 0 {
            return;
        }
        let mut rotated = ColorImage::new([h, w], vec![Color32::TRANSPARENT; w * h]);
        for y in 0..h {
            for x in 0..w {
                let src_idx = y * w + x;
                let new_x = y;
                let new_y = w - 1 - x;
                let dst_idx = new_y * h + new_x;
                rotated.pixels[dst_idx] = self.pixels.pixels[src_idx];
            }
        }
        self.pixels = rotated;
        self.refresh_texture();
    }
}

pub fn load_image_from_bytes(
    ctx: &Context,
    cfg: &AppConfig,
    bytes: &[u8],
) -> anyhow::Result<LoadedImage> {
    let il = cfg.effective_image_limits();
    let mut limits = Limits::default();
    limits.max_image_width = Some(il.image_dim);
    limits.max_image_height = Some(il.image_dim);
    limits.max_alloc = Some(il.alloc_bytes);

    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    reader.limits(limits);
    let img = reader.decode()?;

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
