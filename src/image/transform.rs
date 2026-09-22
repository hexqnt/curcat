use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

/// Image transform operation that can be replayed.
#[derive(Debug, Clone, Copy)]
pub enum ImageTransformOp {
    RotateCw,
    RotateCcw,
    FlipHorizontal,
    FlipVertical,
}

/// Accumulated rotation/flip state for the loaded image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageTransformRecord {
    pub rotation_quarters: u8,
    pub reflected: bool,
}

impl ImageTransformRecord {
    /// Identity transform (no rotation or reflection).
    pub const fn identity() -> Self {
        Self {
            rotation_quarters: 0,
            reflected: false,
        }
    }

    /// Apply a single transform operation to the accumulated state.
    pub const fn apply(&mut self, op: ImageTransformOp) {
        match op {
            ImageTransformOp::RotateCw => {
                self.rotation_quarters = (self.rotation_quarters + 1) % 4;
            }
            ImageTransformOp::RotateCcw => {
                self.rotation_quarters = (self.rotation_quarters + 3) % 4;
            }
            ImageTransformOp::FlipHorizontal => {
                self.rotation_quarters = (4 - self.rotation_quarters % 4) % 4;
                self.reflected = !self.reflected;
            }
            ImageTransformOp::FlipVertical => {
                self.rotation_quarters = (2 + 4 - self.rotation_quarters % 4) % 4;
                self.reflected = !self.reflected;
            }
        }
    }

    /// Expand stored state into a sequence of operations to reapply.
    pub fn replay_operations(self) -> Vec<ImageTransformOp> {
        let mut ops = Vec::new();
        for _ in 0..(self.rotation_quarters % 4) {
            ops.push(ImageTransformOp::RotateCw);
        }
        if self.reflected {
            ops.push(ImageTransformOp::FlipHorizontal);
        }
        ops
    }
}

/// Image data plus the egui texture handle that mirrors its pixels.
pub struct LoadedImage {
    pub size: [usize; 2],
    pub texture: TextureHandle,
    pub pixels: Arc<ColorImage>,
}

impl LoadedImage {
    fn refresh_texture(&mut self) {
        self.size = self.pixels.size;
        self.texture
            .set(Arc::clone(&self.pixels), TextureOptions::LINEAR);
    }

    /// Construct a `LoadedImage` from in-memory pixels and upload a texture.
    pub fn from_color_image(ctx: &Context, pixels: ColorImage) -> Self {
        let size = pixels.size;
        let pixels = Arc::new(pixels);
        let texture = ctx.load_texture("loaded_image", Arc::clone(&pixels), TextureOptions::LINEAR);
        Self {
            size,
            texture,
            pixels,
        }
    }

    /// Replace pixel data and refresh the texture.
    pub fn replace_pixels(&mut self, pixels: impl Into<Arc<ColorImage>>) {
        self.pixels = pixels.into();
        self.refresh_texture();
    }
}

/// Rotate the color image 90 degrees clockwise in-place.
pub fn rotate_color_image_cw(image: &mut ColorImage) {
    let [width, height] = image.size;
    if width == 0 || height == 0 {
        return;
    }
    let new_width = height;
    let total_pixels = width * height;
    let pixels = &image.pixels;
    let rotated_pixels = map_pixels(total_pixels, |idx| {
        let dx = idx % new_width;
        let dy = idx / new_width;
        let src_x = dy;
        let src_y = new_width - 1 - dx;
        let src_idx = src_y * width + src_x;
        pixels[src_idx]
    });
    *image = ColorImage::new([height, width], rotated_pixels);
}

/// Rotate the color image 90 degrees counter-clockwise in-place.
pub fn rotate_color_image_ccw(image: &mut ColorImage) {
    let [width, height] = image.size;
    if width == 0 || height == 0 {
        return;
    }
    let new_width = height;
    let total_pixels = width * height;
    let pixels = &image.pixels;
    let rotated_pixels = map_pixels(total_pixels, |idx| {
        let dx = idx % new_width;
        let dy = idx / new_width;
        let src_y = dx;
        let src_x = width - 1 - dy;
        let src_idx = src_y * width + src_x;
        pixels[src_idx]
    });
    *image = ColorImage::new([height, width], rotated_pixels);
}

/// Отражает изображение по горизонтали, переиспользуя буфер пикселей.
pub fn flip_color_image_horizontal(image: &mut ColorImage) {
    let [width, height] = image.size;
    if width == 0 || height == 0 {
        return;
    }
    let total_pixels = width * height;
    let pixels = &mut image.pixels[..total_pixels];
    if total_pixels >= PARALLEL_PIXEL_THRESHOLD {
        pixels.par_chunks_exact_mut(width).for_each(<[_]>::reverse);
    } else {
        pixels.chunks_exact_mut(width).for_each(<[_]>::reverse);
    }
    let pixels = std::mem::take(&mut image.pixels);
    *image = ColorImage::new([width, height], pixels);
}

/// Отражает изображение по вертикали, меняя местами строки без дополнительного буфера.
pub fn flip_color_image_vertical(image: &mut ColorImage) {
    let [width, height] = image.size;
    if width == 0 || height == 0 {
        return;
    }
    let total_pixels = width * height;
    let (top, rest) = image.pixels[..total_pixels].split_at_mut((height / 2) * width);
    // При нечётной высоте центральная строка остаётся на месте.
    let bottom = &mut rest[(height % 2) * width..];
    if total_pixels >= PARALLEL_PIXEL_THRESHOLD {
        top.par_chunks_exact_mut(width)
            .zip(bottom.par_chunks_exact_mut(width).rev())
            .for_each(|(top, bottom)| top.swap_with_slice(bottom));
    } else {
        for (top, bottom) in top
            .chunks_exact_mut(width)
            .zip(bottom.chunks_exact_mut(width).rev())
        {
            top.swap_with_slice(bottom);
        }
    }
    let pixels = std::mem::take(&mut image.pixels);
    *image = ColorImage::new([width, height], pixels);
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
        let mut image = test_image();
        rotate_color_image_cw(&mut image);
        assert_eq!(image.size, [2, 3]);
        assert_eq!(ids_from_image(&image), vec![4, 1, 5, 2, 6, 3]);
    }

    #[test]
    fn rotate_90_ccw_maps_pixels() {
        let mut image = test_image();
        rotate_color_image_ccw(&mut image);
        assert_eq!(image.size, [2, 3]);
        assert_eq!(ids_from_image(&image), vec![3, 6, 2, 5, 1, 4]);
    }

    #[test]
    fn flip_horizontal_maps_pixels() {
        let mut image = test_image();
        flip_color_image_horizontal(&mut image);
        assert_eq!(image.size, [3, 2]);
        assert_eq!(ids_from_image(&image), vec![3, 2, 1, 6, 5, 4]);
    }

    #[test]
    fn flip_vertical_maps_pixels() {
        let mut image = test_image();
        flip_color_image_vertical(&mut image);
        assert_eq!(image.size, [3, 2]);
        assert_eq!(ids_from_image(&image), vec![4, 5, 6, 1, 2, 3]);
    }

    #[test]
    fn flips_match_pixel_mapping_and_reuse_storage() {
        for [width, height] in [
            [1, 1],
            [1, 7],
            [7, 1],
            [3, 5],
            [4, 6],
            [513, 513],
            [512, 512],
        ] {
            let original = ColorImage::new(
                [width, height],
                (0..width * height)
                    .map(|index| color_id(u8::try_from(index % 251).unwrap()))
                    .collect(),
            );
            for horizontal in [false, true] {
                let mut image = original.clone();
                let storage = image.pixels.as_ptr();
                let flip = if horizontal {
                    flip_color_image_horizontal
                } else {
                    flip_color_image_vertical
                };
                flip(&mut image);
                assert_eq!(image.pixels.as_ptr(), storage);
                assert_eq!(image.size, original.size);
                for (index, pixel) in image.pixels.iter().enumerate() {
                    let (x, y) = (index % width, index / width);
                    let source = if horizontal {
                        y * width + width - 1 - x
                    } else {
                        (height - 1 - y) * width + x
                    };
                    assert_eq!(*pixel, original.pixels[source]);
                }
                flip(&mut image);
                assert_eq!(image.pixels, original.pixels);
            }
        }
    }

    #[test]
    fn replace_pixels_reuses_shared_buffer() {
        let ctx = Context::default();
        let mut loaded = LoadedImage::from_color_image(&ctx, test_image());
        let shared = Arc::new(test_image());

        loaded.replace_pixels(Arc::clone(&shared));

        assert!(Arc::ptr_eq(&loaded.pixels, &shared));
    }
}
