use crate::util::rounded_u8;
use egui::{Color32, ColorImage};
use rayon::prelude::*;
use std::simd::Simd;
use std::simd::num::SimdFloat;

/// Lightweight summary of an image's average color properties.
#[derive(Debug, Clone, Copy)]
struct ImageColorStats {
    avg_rgb: [f32; 3],
    avg_luma: f32,
    hue: f32,
    saturation: f32,
}

const PALETTE_SIMD_LANES: usize = 8;
const SNAP_PARALLEL_SAMPLE_BLOCKS: usize = 32;
const LUMA_R_COEFF: f32 = 0.2126;
const LUMA_G_COEFF: f32 = 0.7152;
const LUMA_B_COEFF: f32 = 0.0722;
type F32x8 = Simd<f32, PALETTE_SIMD_LANES>;

#[derive(Debug, Clone, Copy, Default)]
struct SampleAccum {
    sum_r: f32,
    sum_g: f32,
    sum_b: f32,
    sum_luma: f32,
    samples: usize,
}

impl SampleAccum {
    const fn zero() -> Self {
        Self {
            sum_r: 0.0,
            sum_g: 0.0,
            sum_b: 0.0,
            sum_luma: 0.0,
            samples: 0,
        }
    }

    fn merged(self, other: Self) -> Self {
        Self {
            sum_r: self.sum_r + other.sum_r,
            sum_g: self.sum_g + other.sum_g,
            sum_b: self.sum_b + other.sum_b,
            sum_luma: self.sum_luma + other.sum_luma,
            samples: self.samples + other.samples,
        }
    }
}

#[allow(clippy::suboptimal_flops)]
fn accumulate_sampled_colors_simd(pixels: &[Color32], step: usize) -> SampleAccum {
    if pixels.is_empty() {
        return SampleAccum::zero();
    }

    let mut red_sum_vec = F32x8::splat(0.0);
    let mut green_sum_vec = F32x8::splat(0.0);
    let mut blue_sum_vec = F32x8::splat(0.0);
    let mut luma_sum_vec = F32x8::splat(0.0);
    let luma_r = F32x8::splat(LUMA_R_COEFF);
    let luma_g = F32x8::splat(LUMA_G_COEFF);
    let luma_b = F32x8::splat(LUMA_B_COEFF);
    let lane_span = step.saturating_mul(PALETTE_SIMD_LANES);

    let mut offset = 0usize;
    let mut samples = 0usize;
    while lane_span > 0 && offset + lane_span <= pixels.len() {
        let mut r = [0.0_f32; PALETTE_SIMD_LANES];
        let mut g = [0.0_f32; PALETTE_SIMD_LANES];
        let mut b = [0.0_f32; PALETTE_SIMD_LANES];
        for lane in 0..PALETTE_SIMD_LANES {
            let [pr, pg, pb, _] = pixels[offset + lane * step].to_array();
            r[lane] = f32::from(pr);
            g[lane] = f32::from(pg);
            b[lane] = f32::from(pb);
        }

        let rf = F32x8::from_array(r);
        let gf = F32x8::from_array(g);
        let bf = F32x8::from_array(b);
        red_sum_vec += rf;
        green_sum_vec += gf;
        blue_sum_vec += bf;
        luma_sum_vec += rf * luma_r + gf * luma_g + bf * luma_b;
        samples += PALETTE_SIMD_LANES;
        offset += lane_span;
    }

    let mut sum_r = red_sum_vec.reduce_sum();
    let mut sum_g = green_sum_vec.reduce_sum();
    let mut sum_b = blue_sum_vec.reduce_sum();
    let mut sum_luma = luma_sum_vec.reduce_sum();

    while offset < pixels.len() {
        let [r, g, b, _] = pixels[offset].to_array();
        let rf = f32::from(r);
        let gf = f32::from(g);
        let bf = f32::from(b);
        sum_r += rf;
        sum_g += gf;
        sum_b += bf;
        sum_luma += srgb_luminance_components(rf, gf, bf);
        samples += 1;
        offset = offset.saturating_add(step);
    }

    SampleAccum {
        sum_r,
        sum_g,
        sum_b,
        sum_luma,
        samples,
    }
}

impl ImageColorStats {
    #[allow(clippy::cast_precision_loss)]
    fn from_image(image: &ColorImage) -> Option<Self> {
        let total_pixels = image.pixels.len();
        if total_pixels == 0 {
            return None;
        }
        let step = (total_pixels / SNAP_COLOR_SAMPLE_TARGET).max(1);
        let simd_block = step.checked_mul(PALETTE_SIMD_LANES).unwrap_or(step);
        let parallel_chunk = simd_block
            .checked_mul(SNAP_PARALLEL_SAMPLE_BLOCKS)
            .unwrap_or(simd_block)
            .max(simd_block)
            .max(step);
        let accum = if total_pixels <= SNAP_PARALLEL_STATS_MIN_PIXELS {
            accumulate_sampled_colors_simd(&image.pixels, step)
        } else {
            image
                .pixels
                .par_chunks(parallel_chunk)
                .map(|chunk| accumulate_sampled_colors_simd(chunk, step))
                .reduce(SampleAccum::zero, SampleAccum::merged)
        };
        let (sum_r, sum_g, sum_b, sum_luma, samples) = (
            accum.sum_r,
            accum.sum_g,
            accum.sum_b,
            accum.sum_luma,
            accum.samples,
        );

        if samples == 0 {
            return None;
        }

        let sample_count = samples as f32;
        let avg_r = sum_r / sample_count;
        let avg_g = sum_g / sample_count;
        let avg_b = sum_b / sample_count;
        let avg_luma = sum_luma / sample_count;
        let (hue, saturation, _value) = rgb_to_hsv(
            (avg_r / 255.0).clamp(0.0, 1.0),
            (avg_g / 255.0).clamp(0.0, 1.0),
            (avg_b / 255.0).clamp(0.0, 1.0),
        );
        Some(Self {
            avg_rgb: [avg_r, avg_g, avg_b],
            avg_luma,
            hue,
            saturation,
        })
    }
}

const SNAP_COLOR_SAMPLE_TARGET: usize = 50_000;
const SNAP_MAX_COLOR_DISTANCE: f32 = 441.67294;
const SNAP_HUE_OFFSETS: [f32; 5] = [-45.0, -10.0, 15.0, 40.0, 70.0];
const SNAP_PARALLEL_STATS_MIN_PIXELS: usize = 8_192;

pub fn derive_snap_overlay_palette(image: &ColorImage) -> Vec<Color32> {
    let Some(stats) = ImageColorStats::from_image(image) else {
        return Vec::new();
    };
    let base_hue = if stats.saturation < 0.08 {
        if stats.avg_luma >= 128.0 { 215.0 } else { 35.0 }
    } else {
        wrap_hue(stats.hue + 180.0)
    };
    let saturation = (1.0 - stats.saturation)
        .mul_add(0.35, 0.45)
        .clamp(0.35, 0.7);
    let values = highlight_value_candidates(stats.avg_luma);
    let mut options: Vec<(Color32, f32)> = Vec::new();
    for (idx, offset) in SNAP_HUE_OFFSETS.iter().enumerate() {
        let hue = wrap_hue(base_hue + *offset);
        let value = values[idx % values.len()];
        let color = hsv_to_color32(hue, saturation, value);
        options.push((color, snap_color_score(color, &stats)));
    }
    for neutral in [
        Color32::from_rgb(240, 240, 240),
        Color32::from_rgb(32, 32, 32),
    ] {
        options.push((neutral, snap_color_score(neutral, &stats)));
    }
    options.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    options
        .into_iter()
        .map(|(color, _)| color)
        .take(4)
        .collect()
}

#[allow(clippy::suboptimal_flops)]
fn srgb_luminance_components(r: f32, g: f32, b: f32) -> f32 {
    LUMA_R_COEFF * r + LUMA_G_COEFF * g + LUMA_B_COEFF * b
}

fn srgb_luminance(color: Color32) -> f32 {
    let [r, g, b, _] = color.to_array();
    srgb_luminance_components(f32::from(r), f32::from(g), f32::from(b))
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if (max - r).abs() <= f32::EPSILON {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if (max - g).abs() <= f32::EPSILON {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let saturation = if max <= 0.0 { 0.0 } else { delta / max };
    (wrap_hue(hue), saturation, max)
}

fn hsv_to_color32(hue: f32, saturation: f32, value: f32) -> Color32 {
    let wrapped_hue = wrap_hue(hue);
    let chroma = value * saturation;
    let sector = (wrapped_hue / 60.0).rem_euclid(6.0);
    let secondary = chroma * (1.0 - ((sector % 2.0) - 1.0).abs());
    let match_value = value - chroma;
    let (r1, g1, b1) = if sector < 1.0 {
        (chroma, secondary, 0.0)
    } else if sector < 2.0 {
        (secondary, chroma, 0.0)
    } else if sector < 3.0 {
        (0.0, chroma, secondary)
    } else if sector < 4.0 {
        (0.0, secondary, chroma)
    } else if sector < 5.0 {
        (secondary, 0.0, chroma)
    } else {
        (chroma, 0.0, secondary)
    };
    let red = rounded_u8(((r1 + match_value) * 255.0).clamp(0.0, 255.0));
    let green = rounded_u8(((g1 + match_value) * 255.0).clamp(0.0, 255.0));
    let blue = rounded_u8(((b1 + match_value) * 255.0).clamp(0.0, 255.0));
    Color32::from_rgb(red, green, blue)
}

fn wrap_hue(hue: f32) -> f32 {
    if hue.is_finite() {
        hue.rem_euclid(360.0)
    } else {
        0.0
    }
}

fn highlight_value_candidates(avg_luma: f32) -> [f32; 3] {
    let normalized = (avg_luma / 255.0).clamp(0.0, 1.0);
    if normalized > 0.75 {
        [0.2, 0.35, 0.5]
    } else if normalized > 0.55 {
        [0.3, 0.48, 0.64]
    } else if normalized > 0.35 {
        [0.45, 0.62, 0.8]
    } else {
        [0.92, 0.78, 0.62]
    }
}

#[allow(clippy::suboptimal_flops)]
fn snap_color_score(color: Color32, stats: &ImageColorStats) -> f32 {
    let luma_diff = (srgb_luminance(color) - stats.avg_luma).abs() / 255.0;
    let [r, g, b, _] = color.to_array();
    let dr = f32::from(r) - stats.avg_rgb[0];
    let dg = f32::from(g) - stats.avg_rgb[1];
    let db = f32::from(b) - stats.avg_rgb[2];
    let color_diff = (dr * dr + dg * dg + db * db).sqrt() / SNAP_MAX_COLOR_DISTANCE;
    (luma_diff * 0.7 + color_diff * 0.3).clamp(0.0, 1.0)
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
            let r = mod_u8(i * 31 + 7);
            let g = mod_u8(i * 47 + 13);
            let b = mod_u8(i * 59 + 23);
            pixels.push(Color32::from_rgb(r, g, b));
        }
        ColorImage::new([width, height], pixels)
    }

    fn scalar_accum_sampled_colors(pixels: &[Color32], step: usize) -> SampleAccum {
        let mut sum_r = 0.0_f32;
        let mut sum_g = 0.0_f32;
        let mut sum_b = 0.0_f32;
        let mut sum_luma = 0.0_f32;
        let mut samples = 0usize;
        let mut idx = 0usize;
        while idx < pixels.len() {
            let [r, g, b, _] = pixels[idx].to_array();
            let rf = f32::from(r);
            let gf = f32::from(g);
            let bf = f32::from(b);
            sum_r += rf;
            sum_g += gf;
            sum_b += bf;
            sum_luma += srgb_luminance_components(rf, gf, bf);
            samples += 1;
            idx = idx.saturating_add(step);
        }
        SampleAccum {
            sum_r,
            sum_g,
            sum_b,
            sum_luma,
            samples,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn scalar_stats_reference(image: &ColorImage) -> Option<ImageColorStats> {
        let total_pixels = image.pixels.len();
        if total_pixels == 0 {
            return None;
        }
        let step = (total_pixels / SNAP_COLOR_SAMPLE_TARGET).max(1);
        let accum = scalar_accum_sampled_colors(&image.pixels, step);
        if accum.samples == 0 {
            return None;
        }
        let sample_count = accum.samples as f32;
        let avg_r = accum.sum_r / sample_count;
        let avg_g = accum.sum_g / sample_count;
        let avg_b = accum.sum_b / sample_count;
        let avg_luma = accum.sum_luma / sample_count;
        let (hue, saturation, _value) = rgb_to_hsv(
            (avg_r / 255.0).clamp(0.0, 1.0),
            (avg_g / 255.0).clamp(0.0, 1.0),
            (avg_b / 255.0).clamp(0.0, 1.0),
        );
        Some(ImageColorStats {
            avg_rgb: [avg_r, avg_g, avg_b],
            avg_luma,
            hue,
            saturation,
        })
    }

    fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps.max(f32::EPSILON)
    }

    #[test]
    fn sampled_accum_simd_matches_scalar() {
        let image = test_image(127, 17);
        for step in [1usize, 2, 3, 5, 9, 17, 31] {
            let simd = accumulate_sampled_colors_simd(&image.pixels, step);
            let scalar = scalar_accum_sampled_colors(&image.pixels, step);
            assert_eq!(simd.samples, scalar.samples);
            let rgb_eps = scalar
                .sum_r
                .abs()
                .max(scalar.sum_g.abs())
                .max(scalar.sum_b.abs())
                .mul_add(1.0e-5, 1.0);
            let luma_eps = scalar.sum_luma.abs().mul_add(1.0e-5, 2.0);
            assert!(approx_eq(simd.sum_r, scalar.sum_r, rgb_eps));
            assert!(approx_eq(simd.sum_g, scalar.sum_g, rgb_eps));
            assert!(approx_eq(simd.sum_b, scalar.sum_b, rgb_eps));
            assert!(approx_eq(simd.sum_luma, scalar.sum_luma, luma_eps));
        }
    }

    #[test]
    fn image_color_stats_simd_matches_scalar_reference() {
        let image = test_image(2048, 2);
        let simd = ImageColorStats::from_image(&image).expect("stats");
        let scalar = scalar_stats_reference(&image).expect("stats");
        assert!(approx_eq(simd.avg_rgb[0], scalar.avg_rgb[0], 5.0e-3));
        assert!(approx_eq(simd.avg_rgb[1], scalar.avg_rgb[1], 5.0e-3));
        assert!(approx_eq(simd.avg_rgb[2], scalar.avg_rgb[2], 5.0e-3));
        assert!(approx_eq(simd.avg_luma, scalar.avg_luma, 5.0e-3));
        assert!(approx_eq(simd.hue, scalar.hue, 5.0e-3));
        assert!(approx_eq(simd.saturation, scalar.saturation, 1.0e-6));
    }
}
