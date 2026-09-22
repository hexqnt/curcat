use egui::{Color32, ColorImage};
use rayon::prelude::*;
use std::simd::Select;
use std::simd::Simd;
use std::simd::StdFloat;
use std::simd::cmp::SimdPartialOrd;
use std::simd::num::SimdFloat;

use crate::pixel_simd::{F32x8, LANES, U32x8, unpack_rgba_block};

type U32x4 = Simd<u32, 4>;

// Мелкие изображения быстрее обрабатываются последовательно; размер блока
// кратен числу SIMD-полос и даёт Rayon достаточно крупных задач.
const FILTER_PARALLEL_THRESHOLD: usize = 262_144;
const FILTER_PARALLEL_CHUNK: usize = 32_768;
const CHANNEL_MAX: f32 = 255.0;
const INV_CHANNEL_MAX: f32 = 1.0 / CHANNEL_MAX;
const LUMA_RED: f32 = 0.2126;
const LUMA_GREEN: f32 = 0.7152;
const LUMA_BLUE: f32 = 0.0722;

/// Настройки фильтров, применяемых к отображаемому изображению.
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

/// Нормализованные настройки с закодированным состоянием threshold.
#[derive(Debug, Clone, Copy)]
struct SanitizedImageFilters {
    brightness: f32,
    contrast: f32,
    gamma: f32,
    invert: bool,
    threshold: Option<f32>,
    blur_radius: u32,
}

impl ImageFilters {
    fn sanitized(self) -> SanitizedImageFilters {
        SanitizedImageFilters {
            brightness: self.brightness.clamp(-1.0, 1.0),
            contrast: self.contrast.clamp(-1.0, 1.0),
            gamma: self.gamma.clamp(0.2, 5.0),
            invert: self.invert,
            threshold: self
                .threshold_enabled
                .then(|| self.threshold.clamp(0.0, 1.0)),
            blur_radius: self.blur_radius.min(24),
        }
    }

    pub fn is_identity(self) -> bool {
        self.sanitized().is_identity()
    }
}

impl SanitizedImageFilters {
    fn is_identity(self) -> bool {
        self.color_adjustments_are_identity() && self.blur_radius == 0
    }

    fn color_adjustments_are_identity(self) -> bool {
        self.brightness.abs() <= f32::EPSILON
            && self.contrast.abs() <= f32::EPSILON
            && (self.gamma - 1.0).abs() <= f32::EPSILON
            && !self.invert
            && self.threshold.is_none()
    }
}

/// Подготовленные параметры одного прохода по цветовым каналам.
#[derive(Debug, Clone, Copy)]
struct PreparedColorFilter {
    brightness: f32,
    contrast_factor: f32,
    inv_gamma: f32,
    invert: bool,
    threshold: Option<f32>,
}

impl PreparedColorFilter {
    fn from_sanitized(filters: SanitizedImageFilters) -> Self {
        Self {
            brightness: filters.brightness,
            contrast_factor: 1.0 + filters.contrast,
            inv_gamma: if (filters.gamma - 1.0).abs() <= f32::EPSILON {
                1.0
            } else {
                1.0 / filters.gamma
            },
            invert: filters.invert,
            threshold: filters.threshold,
        }
    }

    fn gamma_is_identity(self) -> bool {
        (self.inv_gamma - 1.0).abs() <= f32::EPSILON
    }
}

/// Применяет настройки фильтров к исходному изображению.
#[allow(clippy::many_single_char_names, clippy::suboptimal_flops)]
pub fn apply_image_filters(base: &ColorImage, filters: ImageFilters) -> ColorImage {
    if base.pixels.is_empty() {
        return base.clone();
    }
    let filters = filters.sanitized();
    if filters.is_identity() {
        return base.clone();
    }

    // Для прозрачных цветов round-trip через `from_rgba_unmultiplied` может
    // менять премультиплицированные каналы, поэтому пропускаем проход лишь
    // для гарантированно непрозрачного изображения.
    let can_skip_color_pass = filters.color_adjustments_are_identity()
        && base.pixels.iter().all(|pixel| pixel.a() == u8::MAX);

    let mut pixels = if filters.blur_radius > 0 {
        box_blur(base, filters.blur_radius)
    } else {
        base.pixels.clone()
    };

    if can_skip_color_pass {
        return ColorImage::new(base.size, pixels);
    }

    let color_filter = PreparedColorFilter::from_sanitized(filters);
    if color_filter.gamma_is_identity() {
        apply_gamma_one_filter(&mut pixels, color_filter);
    } else {
        apply_lut_filter(&mut pixels, color_filter);
    }

    ColorImage::new(base.size, pixels)
}

#[allow(clippy::many_single_char_names, clippy::suboptimal_flops)]
fn apply_gamma_one_filter(pixels: &mut [Color32], filter: PreparedColorFilter) {
    let zero = F32x8::splat(0.0);
    let one = F32x8::splat(1.0);
    let half = F32x8::splat(0.5);
    let brightness = F32x8::splat(filter.brightness);
    let contrast = F32x8::splat(filter.contrast_factor);
    let luma_r = F32x8::splat(LUMA_RED);
    let luma_g = F32x8::splat(LUMA_GREEN);
    let luma_b = F32x8::splat(LUMA_BLUE);
    let input_scale = F32x8::splat(INV_CHANNEL_MAX);
    let output_scale = F32x8::splat(CHANNEL_MAX);
    let alpha_mask = U32x8::splat(u32::MAX << 24);

    let apply_chunk = |pixels: &mut [Color32]| {
        let (chunks, remainder) = pixels.as_chunks_mut::<LANES>();
        for chunk in chunks {
            let channels = unpack_rgba_block(chunk);
            let mut rf = channels.red * input_scale;
            let mut gf = channels.green * input_scale;
            let mut bf = channels.blue * input_scale;

            rf = (rf + brightness).simd_clamp(zero, one);
            gf = (gf + brightness).simd_clamp(zero, one);
            bf = (bf + brightness).simd_clamp(zero, one);

            rf = ((rf - half) * contrast + half).simd_clamp(zero, one);
            gf = ((gf - half) * contrast + half).simd_clamp(zero, one);
            bf = ((bf - half) * contrast + half).simd_clamp(zero, one);

            if filter.invert {
                rf = one - rf;
                gf = one - gf;
                bf = one - bf;
            }

            if let Some(threshold) = filter.threshold {
                let luma = rf * luma_r + gf * luma_g + bf * luma_b;
                let mask = luma.simd_ge(F32x8::splat(threshold));
                rf = mask.select(one, zero);
                gf = mask.select(one, zero);
                bf = mask.select(one, zero);
            }

            let red: U32x8 = (rf * output_scale).round().cast();
            let green: U32x8 = (gf * output_scale).round().cast();
            let blue: U32x8 = (bf * output_scale).round().cast();
            let packed = red | (green << 8) | (blue << 16) | (channels.packed & alpha_mask);

            for (pixel, rgba) in chunk.iter_mut().zip(packed.to_array()) {
                let [r, g, b, a] = rgba.to_le_bytes();
                *pixel = Color32::from_rgba_unmultiplied(r, g, b, a);
            }
        }

        for pixel in remainder {
            apply_filter_scalar_pixel(pixel, filter);
        }
    };

    for_each_pixel_chunk(pixels, apply_chunk);
}

#[allow(clippy::many_single_char_names, clippy::suboptimal_flops)]
fn apply_lut_filter(pixels: &mut [Color32], filter: PreparedColorFilter) {
    let channel_lut: [f32; 256] = std::array::from_fn(|value| {
        transform_channel(
            f32::from(u8::try_from(value).expect("LUT index must fit into u8")) / CHANNEL_MAX,
            filter,
        )
    });

    let apply_chunk = |pixels: &mut [Color32]| {
        for pixel in pixels {
            let [r, g, b, a] = pixel.to_array();
            let mut rf = channel_lut[usize::from(r)];
            let mut gf = channel_lut[usize::from(g)];
            let mut bf = channel_lut[usize::from(b)];
            if let Some(threshold) = filter.threshold {
                let luma = LUMA_RED * rf + LUMA_GREEN * gf + LUMA_BLUE * bf;
                let value = if luma >= threshold { 1.0 } else { 0.0 };
                rf = value;
                gf = value;
                bf = value;
            }
            *pixel = Color32::from_rgba_unmultiplied(
                float_to_u8(rf),
                float_to_u8(gf),
                float_to_u8(bf),
                a,
            );
        }
    };

    for_each_pixel_chunk(pixels, apply_chunk);
}

fn for_each_pixel_chunk(pixels: &mut [Color32], apply: impl Fn(&mut [Color32]) + Send + Sync) {
    if pixels.len() >= FILTER_PARALLEL_THRESHOLD {
        pixels
            .par_chunks_mut(FILTER_PARALLEL_CHUNK)
            .for_each(&apply);
    } else {
        apply(pixels);
    }
}

fn transform_channel(mut value: f32, filter: PreparedColorFilter) -> f32 {
    value = (value + filter.brightness).clamp(0.0, 1.0);
    value = (value - 0.5)
        .mul_add(filter.contrast_factor, 0.5)
        .clamp(0.0, 1.0);
    if !filter.gamma_is_identity() {
        value = value.powf(filter.inv_gamma).clamp(0.0, 1.0);
    }
    if filter.invert {
        value = 1.0 - value;
    }
    value
}

#[allow(clippy::many_single_char_names, clippy::suboptimal_flops)]
fn apply_filter_scalar_pixel(pixel: &mut Color32, filter: PreparedColorFilter) {
    let [r, g, b, a] = pixel.to_array();
    let mut rf = transform_channel(f32::from(r) / CHANNEL_MAX, filter);
    let mut gf = transform_channel(f32::from(g) / CHANNEL_MAX, filter);
    let mut bf = transform_channel(f32::from(b) / CHANNEL_MAX, filter);

    if let Some(threshold) = filter.threshold {
        let luma = LUMA_RED * rf + LUMA_GREEN * gf + LUMA_BLUE * bf;
        let v = if luma >= threshold { 1.0 } else { 0.0 };
        rf = v;
        gf = v;
        bf = v;
    }

    *pixel = Color32::from_rgba_unmultiplied(float_to_u8(rf), float_to_u8(gf), float_to_u8(bf), a);
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn float_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * CHANNEL_MAX).round() as u8
}

fn box_blur(image: &ColorImage, radius: u32) -> Vec<Color32> {
    let [width, height] = image.size;
    if radius == 0 || width == 0 || height == 0 {
        return image.pixels.clone();
    }
    let radius = radius as usize;
    let row_len = width;
    let mut horiz = vec![[0u8; 4]; width * height];
    if image.pixels.len() >= FILTER_PARALLEL_THRESHOLD {
        image
            .pixels
            .par_chunks_exact(row_len)
            .zip(horiz.par_chunks_exact_mut(row_len))
            .for_each_init(
                || vec![U32x4::splat(0); width + 1],
                |row_prefix, (src_row, horiz_row)| {
                    horizontal_blur_row(src_row, horiz_row, radius, row_prefix);
                },
            );
    } else {
        let mut row_prefix = vec![U32x4::splat(0); width + 1];
        for (src_row, horiz_row) in image
            .pixels
            .chunks_exact(row_len)
            .zip(horiz.chunks_exact_mut(row_len))
        {
            horizontal_blur_row(src_row, horiz_row, radius, &mut row_prefix);
        }
    }

    let mut out = vec![Color32::TRANSPARENT; width * height];
    let mut column_sums = vec![U32x4::splat(0); width];
    // Скользящее окно сохраняет последовательный доступ к строкам и не требует
    // отдельного prefix-буфера высотой с изображение для каждого столбца.
    for row in horiz.chunks_exact(row_len).take(radius.min(height - 1) + 1) {
        for (sum, &rgba) in column_sums.iter_mut().zip(row) {
            *sum += rgba_u32(rgba);
        }
    }

    for y in 0..height {
        let y0 = y.saturating_sub(radius);
        let y1 = (y + radius).min(height - 1);
        let count = u32::try_from(y1 - y0 + 1).unwrap_or(u32::MAX);
        for (pixel, &sum) in out[y * row_len..(y + 1) * row_len]
            .iter_mut()
            .zip(&column_sums)
        {
            let [r, g, b, a] = avg_rgba(sum, U32x4::splat(0), count);
            *pixel = Color32::from_rgba_unmultiplied(r, g, b, a);
        }

        if y >= radius {
            let leaving_row = &horiz[(y - radius) * row_len..(y - radius + 1) * row_len];
            for (sum, &rgba) in column_sums.iter_mut().zip(leaving_row) {
                *sum -= rgba_u32(rgba);
            }
        }
        let entering_y = y + radius + 1;
        if entering_y < height {
            let entering_row = &horiz[entering_y * row_len..(entering_y + 1) * row_len];
            for (sum, &rgba) in column_sums.iter_mut().zip(entering_row) {
                *sum += rgba_u32(rgba);
            }
        }
    }

    out
}

fn horizontal_blur_row(
    source: &[Color32],
    output: &mut [[u8; 4]],
    radius: usize,
    prefix: &mut [U32x4],
) {
    debug_assert_eq!(source.len(), output.len());
    debug_assert_eq!(prefix.len(), source.len() + 1);
    prefix[0] = U32x4::splat(0);
    for (x, pixel) in source.iter().enumerate() {
        prefix[x + 1] = prefix[x] + rgba_u32(pixel.to_array());
    }
    for (x, out_pixel) in output.iter_mut().enumerate() {
        let x0 = x.saturating_sub(radius);
        let x1 = (x + radius).min(source.len() - 1);
        let count = u32::try_from(x1 - x0 + 1).unwrap_or(u32::MAX);
        *out_pixel = avg_rgba(prefix[x1 + 1], prefix[x0], count);
    }
}

fn rgba_u32([r, g, b, a]: [u8; 4]) -> U32x4 {
    U32x4::from_array([u32::from(r), u32::from(g), u32::from(b), u32::from(a)])
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
        let color_filter = PreparedColorFilter::from_sanitized(filters);

        for pixel in &mut pixels {
            apply_filter_scalar_pixel(pixel, color_filter);
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
    fn gamma_lut_matches_scalar_reference() {
        let image = test_image(257, 3);
        let filters = ImageFilters {
            brightness: -0.13,
            contrast: 0.37,
            gamma: 2.2,
            invert: true,
            threshold: 0.5,
            threshold_enabled: false,
            blur_radius: 0,
        };

        let optimized = apply_image_filters(&image, filters);
        let scalar = apply_image_filters_scalar_reference(&image, filters);
        assert_eq!(optimized.pixels, scalar.pixels);
    }

    #[test]
    fn gamma_lut_threshold_matches_scalar_reference() {
        let image = test_image(257, 3);
        let filters = ImageFilters {
            brightness: 0.07,
            contrast: -0.19,
            gamma: 0.65,
            invert: false,
            threshold: 0.47,
            threshold_enabled: true,
            blur_radius: 0,
        };

        let optimized = apply_image_filters(&image, filters);
        let scalar = apply_image_filters_scalar_reference(&image, filters);
        assert_eq!(optimized.pixels, scalar.pixels);
    }

    #[test]
    fn parallel_gamma_lut_matches_scalar_reference() {
        let image = test_image(513, 513);
        let filters = ImageFilters {
            brightness: -0.09,
            contrast: 0.31,
            gamma: 1.8,
            invert: false,
            threshold: 0.5,
            threshold_enabled: false,
            blur_radius: 0,
        };

        let optimized = apply_image_filters(&image, filters);
        let scalar = apply_image_filters_scalar_reference(&image, filters);
        assert_eq!(optimized.pixels, scalar.pixels);
    }

    #[test]
    fn box_blur_simd_channels_match_scalar_reference() {
        for [width, height] in [[1, 1], [1, 17], [17, 1], [2, 3], [13, 9], [49, 3]] {
            let image = test_image(width, height);
            for radius in [0_u32, 1, 2, 3, 4, 24] {
                let simd = box_blur(&image, radius);
                let scalar = box_blur_scalar_reference(&image, radius);
                assert_eq!(simd, scalar, "size: {width}x{height}, radius: {radius}");
            }
        }
    }

    #[test]
    fn parallel_box_blur_matches_scalar_reference() {
        let image = test_image(513, 513);
        let optimized = box_blur(&image, 4);
        let scalar = box_blur_scalar_reference(&image, 4);
        assert_eq!(optimized, scalar);
    }
}
