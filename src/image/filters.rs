use egui::{Color32, ColorImage};
use std::simd::Select;
use std::simd::Simd;
use std::simd::cmp::SimdPartialOrd;
use std::simd::num::SimdFloat;

type U32x4 = Simd<u32, 4>;
const FILTER_SIMD_LANES: usize = 8;
type F32x8 = Simd<f32, FILTER_SIMD_LANES>;

/// Simple image filter settings applied to the displayed pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageFilters {
    pub brightness: f32,
    pub contrast: f32,
    pub gamma: f32,
    pub invert: bool,
    pub threshold: f32,
    pub threshold_enabled: bool,
    pub blur_radius: u32,
}

impl Default for ImageFilters {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 0.0,
            gamma: 1.0,
            invert: false,
            threshold: 0.5,
            threshold_enabled: false,
            blur_radius: 0,
        }
    }
}

impl ImageFilters {
    pub fn sanitized(self) -> Self {
        let brightness = self.brightness.clamp(-1.0, 1.0);
        let contrast = self.contrast.clamp(-1.0, 1.0);
        let gamma = self.gamma.clamp(0.2, 5.0);
        let threshold = self.threshold.clamp(0.0, 1.0);
        let blur_radius = self.blur_radius.min(24);
        Self {
            brightness,
            contrast,
            gamma,
            invert: self.invert,
            threshold,
            threshold_enabled: self.threshold_enabled,
            blur_radius,
        }
    }

    pub fn is_identity(self) -> bool {
        let sanitized = self.sanitized();
        sanitized.brightness.abs() <= f32::EPSILON
            && sanitized.contrast.abs() <= f32::EPSILON
            && (sanitized.gamma - 1.0).abs() <= f32::EPSILON
            && !sanitized.invert
            && !sanitized.threshold_enabled
            && sanitized.blur_radius == 0
    }
}

/// Apply the provided filter settings to a base image.
#[allow(clippy::many_single_char_names, clippy::suboptimal_flops)]
pub fn apply_image_filters(base: &ColorImage, filters: ImageFilters) -> ColorImage {
    if base.pixels.is_empty() {
        return base.clone();
    }
    let filters = filters.sanitized();
    if filters.is_identity() {
        return base.clone();
    }

    let mut pixels = if filters.blur_radius > 0 {
        box_blur(base, filters.blur_radius)
    } else {
        base.pixels.clone()
    };

    let contrast_factor = 1.0 + filters.contrast;
    let inv_gamma = if (filters.gamma - 1.0).abs() <= f32::EPSILON {
        1.0
    } else {
        1.0 / filters.gamma
    };

    if (inv_gamma - 1.0).abs() <= f32::EPSILON {
        apply_image_filters_simd_gamma1(&mut pixels, filters, contrast_factor);
    } else {
        for pixel in &mut pixels {
            apply_filter_scalar_pixel(pixel, filters, contrast_factor, inv_gamma);
        }
    }

    ColorImage::new(base.size, pixels)
}

#[allow(clippy::many_single_char_names, clippy::suboptimal_flops)]
fn apply_image_filters_simd_gamma1(
    pixels: &mut [Color32],
    filters: ImageFilters,
    contrast_factor: f32,
) {
    const INV_255: f32 = 1.0 / 255.0;
    let zero = F32x8::splat(0.0);
    let one = F32x8::splat(1.0);
    let half = F32x8::splat(0.5);
    let brightness = F32x8::splat(filters.brightness);
    let contrast = F32x8::splat(contrast_factor);
    let threshold = F32x8::splat(filters.threshold);
    let luma_r = F32x8::splat(0.2126);
    let luma_g = F32x8::splat(0.7152);
    let luma_b = F32x8::splat(0.0722);

    let mut chunks = pixels.chunks_exact_mut(FILTER_SIMD_LANES);
    for chunk in &mut chunks {
        let mut r = [0.0_f32; FILTER_SIMD_LANES];
        let mut g = [0.0_f32; FILTER_SIMD_LANES];
        let mut b = [0.0_f32; FILTER_SIMD_LANES];
        let mut a = [0_u8; FILTER_SIMD_LANES];

        for (lane, pixel) in chunk.iter().enumerate() {
            let [pr, pg, pb, pa] = pixel.to_array();
            r[lane] = f32::from(pr) * INV_255;
            g[lane] = f32::from(pg) * INV_255;
            b[lane] = f32::from(pb) * INV_255;
            a[lane] = pa;
        }

        let mut rf = F32x8::from_array(r);
        let mut gf = F32x8::from_array(g);
        let mut bf = F32x8::from_array(b);

        rf = (rf + brightness).simd_clamp(zero, one);
        gf = (gf + brightness).simd_clamp(zero, one);
        bf = (bf + brightness).simd_clamp(zero, one);

        rf = ((rf - half) * contrast + half).simd_clamp(zero, one);
        gf = ((gf - half) * contrast + half).simd_clamp(zero, one);
        bf = ((bf - half) * contrast + half).simd_clamp(zero, one);

        if filters.invert {
            rf = one - rf;
            gf = one - gf;
            bf = one - bf;
        }

        if filters.threshold_enabled {
            let luma = rf * luma_r + gf * luma_g + bf * luma_b;
            let mask = luma.simd_ge(threshold);
            rf = mask.select(one, zero);
            gf = mask.select(one, zero);
            bf = mask.select(one, zero);
        }

        let mut r_out = [0.0_f32; FILTER_SIMD_LANES];
        let mut g_out = [0.0_f32; FILTER_SIMD_LANES];
        let mut b_out = [0.0_f32; FILTER_SIMD_LANES];
        rf.copy_to_slice(&mut r_out);
        gf.copy_to_slice(&mut g_out);
        bf.copy_to_slice(&mut b_out);

        for lane in 0..FILTER_SIMD_LANES {
            chunk[lane] = Color32::from_rgba_unmultiplied(
                float_to_u8(r_out[lane]),
                float_to_u8(g_out[lane]),
                float_to_u8(b_out[lane]),
                a[lane],
            );
        }
    }

    for pixel in chunks.into_remainder() {
        apply_filter_scalar_pixel(pixel, filters, contrast_factor, 1.0);
    }
}

#[allow(clippy::many_single_char_names, clippy::suboptimal_flops)]
fn apply_filter_scalar_pixel(
    pixel: &mut Color32,
    filters: ImageFilters,
    contrast_factor: f32,
    inv_gamma: f32,
) {
    let [r, g, b, a] = pixel.to_array();
    let mut rf = f32::from(r) / 255.0;
    let mut gf = f32::from(g) / 255.0;
    let mut bf = f32::from(b) / 255.0;

    rf = (rf + filters.brightness).clamp(0.0, 1.0);
    gf = (gf + filters.brightness).clamp(0.0, 1.0);
    bf = (bf + filters.brightness).clamp(0.0, 1.0);

    rf = (rf - 0.5).mul_add(contrast_factor, 0.5).clamp(0.0, 1.0);
    gf = (gf - 0.5).mul_add(contrast_factor, 0.5).clamp(0.0, 1.0);
    bf = (bf - 0.5).mul_add(contrast_factor, 0.5).clamp(0.0, 1.0);

    if (inv_gamma - 1.0).abs() > f32::EPSILON {
        rf = rf.powf(inv_gamma).clamp(0.0, 1.0);
        gf = gf.powf(inv_gamma).clamp(0.0, 1.0);
        bf = bf.powf(inv_gamma).clamp(0.0, 1.0);
    }

    if filters.invert {
        rf = 1.0 - rf;
        gf = 1.0 - gf;
        bf = 1.0 - bf;
    }

    if filters.threshold_enabled {
        let luma = 0.2126 * rf + 0.7152 * gf + 0.0722 * bf;
        let v = if luma >= filters.threshold { 1.0 } else { 0.0 };
        rf = v;
        gf = v;
        bf = v;
    }

    *pixel = Color32::from_rgba_unmultiplied(float_to_u8(rf), float_to_u8(gf), float_to_u8(bf), a);
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn float_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn box_blur(image: &ColorImage, radius: u32) -> Vec<Color32> {
    let [width, height] = image.size;
    if radius == 0 || width == 0 || height == 0 {
        return image.pixels.clone();
    }
    let radius = radius as usize;
    let row_len = width;
    let mut horiz = vec![[0u8; 4]; width * height];
    let mut row_prefix = vec![U32x4::splat(0); width + 1];

    for y in 0..height {
        let row_start = y * row_len;
        row_prefix[0] = U32x4::splat(0);
        for x in 0..width {
            let [r, g, b, a] = image.pixels[row_start + x].to_array();
            let rgba = U32x4::from_array([u32::from(r), u32::from(g), u32::from(b), u32::from(a)]);
            row_prefix[x + 1] = row_prefix[x] + rgba;
        }
        for x in 0..width {
            let x0 = x.saturating_sub(radius);
            let x1 = (x + radius).min(width - 1);
            let count = u32::try_from(x1 - x0 + 1).unwrap_or(u32::MAX);
            let sum = row_prefix[x1 + 1];
            let base = row_prefix[x0];
            horiz[row_start + x] = avg_rgba(sum, base, count);
        }
    }

    let mut out = vec![Color32::TRANSPARENT; width * height];
    let mut col_prefix = vec![U32x4::splat(0); height + 1];
    for x in 0..width {
        col_prefix[0] = U32x4::splat(0);
        for y in 0..height {
            let idx = y * row_len + x;
            let [r, g, b, a] = horiz[idx];
            let rgba = U32x4::from_array([u32::from(r), u32::from(g), u32::from(b), u32::from(a)]);
            col_prefix[y + 1] = col_prefix[y] + rgba;
        }
        for y in 0..height {
            let y0 = y.saturating_sub(radius);
            let y1 = (y + radius).min(height - 1);
            let count = u32::try_from(y1 - y0 + 1).unwrap_or(u32::MAX);
            let sum = col_prefix[y1 + 1];
            let base = col_prefix[y0];
            let idx = y * row_len + x;
            let [r, g, b, a] = avg_rgba(sum, base, count);
            out[idx] = Color32::from_rgba_unmultiplied(r, g, b, a);
        }
    }

    out
}

fn avg_rgba(sum: U32x4, base: U32x4, count: u32) -> [u8; 4] {
    let avg = (sum - base + U32x4::splat(count / 2)) / U32x4::splat(count);
    avg.to_array()
        .map(|channel| u8::try_from(channel.min(u32::from(u8::MAX))).unwrap_or(u8::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mod_u8(value: usize) -> u8 {
        u8::try_from(value % 256).unwrap_or(0)
    }

    fn test_image(width: usize, height: usize) -> ColorImage {
        let mut pixels = Vec::with_capacity(width * height);
        for i in 0..(width * height) {
            let r = mod_u8(i * 37 + 13);
            let g = mod_u8(i * 19 + 97);
            let b = mod_u8(i * 53 + 41);
            let a = mod_u8(i * 29 + 73);
            pixels.push(Color32::from_rgba_unmultiplied(r, g, b, a));
        }
        ColorImage::new([width, height], pixels)
    }

    fn box_blur_scalar_reference(image: &ColorImage, radius: u32) -> Vec<Color32> {
        let [width, height] = image.size;
        if radius == 0 || width == 0 || height == 0 {
            return image.pixels.clone();
        }
        let radius = radius as usize;
        let row_len = width;
        let mut horiz = vec![[0u8; 4]; width * height];
        let mut row_prefix = vec![[0u32; 4]; width + 1];

        for y in 0..height {
            let row_start = y * row_len;
            row_prefix[0] = [0; 4];
            for x in 0..width {
                let [r, g, b, a] = image.pixels[row_start + x].to_array();
                let prev = row_prefix[x];
                row_prefix[x + 1] = [
                    prev[0] + u32::from(r),
                    prev[1] + u32::from(g),
                    prev[2] + u32::from(b),
                    prev[3] + u32::from(a),
                ];
            }
            for x in 0..width {
                let x0 = x.saturating_sub(radius);
                let x1 = (x + radius).min(width - 1);
                let count = u32::try_from(x1 - x0 + 1).unwrap_or(u32::MAX);
                let sum = row_prefix[x1 + 1];
                let base = row_prefix[x0];
                horiz[row_start + x] = [
                    avg_channel_scalar(sum[0], base[0], count),
                    avg_channel_scalar(sum[1], base[1], count),
                    avg_channel_scalar(sum[2], base[2], count),
                    avg_channel_scalar(sum[3], base[3], count),
                ];
            }
        }

        let mut out = vec![Color32::TRANSPARENT; width * height];
        let mut col_prefix = vec![[0u32; 4]; height + 1];
        for x in 0..width {
            col_prefix[0] = [0; 4];
            for y in 0..height {
                let idx = y * row_len + x;
                let [r, g, b, a] = horiz[idx];
                let prev = col_prefix[y];
                col_prefix[y + 1] = [
                    prev[0] + u32::from(r),
                    prev[1] + u32::from(g),
                    prev[2] + u32::from(b),
                    prev[3] + u32::from(a),
                ];
            }
            for y in 0..height {
                let y0 = y.saturating_sub(radius);
                let y1 = (y + radius).min(height - 1);
                let count = u32::try_from(y1 - y0 + 1).unwrap_or(u32::MAX);
                let sum = col_prefix[y1 + 1];
                let base = col_prefix[y0];
                let idx = y * row_len + x;
                out[idx] = Color32::from_rgba_unmultiplied(
                    avg_channel_scalar(sum[0], base[0], count),
                    avg_channel_scalar(sum[1], base[1], count),
                    avg_channel_scalar(sum[2], base[2], count),
                    avg_channel_scalar(sum[3], base[3], count),
                );
            }
        }

        out
    }

    fn avg_channel_scalar(sum: u32, base: u32, count: u32) -> u8 {
        let value = (sum - base + count / 2) / count;
        let clamped = value.min(u32::from(u8::MAX));
        u8::try_from(clamped).unwrap_or(u8::MAX)
    }

    fn apply_image_filters_scalar_reference(
        base: &ColorImage,
        filters: ImageFilters,
    ) -> ColorImage {
        if base.pixels.is_empty() {
            return base.clone();
        }
        let filters = filters.sanitized();
        if filters.is_identity() {
            return base.clone();
        }

        let mut pixels = if filters.blur_radius > 0 {
            box_blur_scalar_reference(base, filters.blur_radius)
        } else {
            base.pixels.clone()
        };
        let contrast_factor = 1.0 + filters.contrast;
        let inv_gamma = if (filters.gamma - 1.0).abs() <= f32::EPSILON {
            1.0
        } else {
            1.0 / filters.gamma
        };

        for pixel in &mut pixels {
            apply_filter_scalar_pixel(pixel, filters, contrast_factor, inv_gamma);
        }

        ColorImage::new(base.size, pixels)
    }

    #[test]
    fn simd_fast_path_matches_scalar_reference() {
        let image = test_image(11, 5);
        let filters = ImageFilters {
            brightness: 0.15,
            contrast: -0.22,
            gamma: 1.0,
            invert: true,
            threshold: 0.5,
            threshold_enabled: false,
            blur_radius: 0,
        };

        let simd = apply_image_filters(&image, filters);
        let scalar = apply_image_filters_scalar_reference(&image, filters);
        assert_eq!(simd.pixels, scalar.pixels);
    }

    #[test]
    fn simd_fast_path_threshold_matches_scalar_reference() {
        let image = test_image(17, 3);
        let filters = ImageFilters {
            brightness: -0.1,
            contrast: 0.4,
            gamma: 1.0,
            invert: false,
            threshold: 0.61,
            threshold_enabled: true,
            blur_radius: 0,
        };

        let simd = apply_image_filters(&image, filters);
        let scalar = apply_image_filters_scalar_reference(&image, filters);
        assert_eq!(simd.pixels, scalar.pixels);
    }

    #[test]
    fn box_blur_simd_channels_match_scalar_reference() {
        let image = test_image(13, 9);
        for radius in [1_u32, 2, 3, 4] {
            let simd = box_blur(&image, radius);
            let scalar = box_blur_scalar_reference(&image, radius);
            assert_eq!(simd, scalar);
        }
    }
}
