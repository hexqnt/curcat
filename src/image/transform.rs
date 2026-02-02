use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use rayon::prelude::*;

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

#[allow(dead_code)]
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
        rotate_color_image_cw(&mut self.pixels);
        self.refresh_texture();
    }

    /// Rotate the image 90 degrees counter-clockwise, updating pixels and texture.
    pub fn rotate_90_ccw(&mut self) {
        rotate_color_image_ccw(&mut self.pixels);
        self.refresh_texture();
    }

    /// Mirror the image horizontally (left-right), updating pixels and texture.
    pub fn flip_horizontal(&mut self) {
        flip_color_image_horizontal(&mut self.pixels);
        self.refresh_texture();
    }

    /// Mirror the image vertically (top-bottom), updating pixels and texture.
    pub fn flip_vertical(&mut self) {
        flip_color_image_vertical(&mut self.pixels);
        self.refresh_texture();
    }

    /// Replace pixel data and refresh the texture.
    pub fn replace_pixels(&mut self, pixels: ColorImage) {
        self.pixels = pixels;
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

/// Mirror the color image horizontally (left-right).
pub fn flip_color_image_horizontal(image: &mut ColorImage) {
    let [width, height] = image.size;
    if width == 0 || height == 0 {
        return;
    }
    let total_pixels = width * height;
    let pixels = &image.pixels;
    let flipped_pixels = map_pixels(total_pixels, |idx| {
        let x = idx % width;
        let y = idx / width;
        let src_x = width - 1 - x;
        let src_idx = y * width + src_x;
        pixels[src_idx]
    });
    *image = ColorImage::new([width, height], flipped_pixels);
}

/// Mirror the color image vertically (top-bottom).
pub fn flip_color_image_vertical(image: &mut ColorImage) {
    let [width, height] = image.size;
    if width == 0 || height == 0 {
        return;
    }
    let total_pixels = width * height;
    let pixels = &image.pixels;
    let flipped_pixels = map_pixels(total_pixels, |idx| {
        let x = idx % width;
        let y = idx / width;
        let src_y = height - 1 - y;
        let src_idx = src_y * width + x;
        pixels[src_idx]
    });
    *image = ColorImage::new([width, height], flipped_pixels);
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
