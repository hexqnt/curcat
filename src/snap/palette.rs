use egui::{Color32, ColorImage};
use rayon::prelude::*;

/// Lightweight summary of an image's average color properties.
#[derive(Debug, Clone, Copy)]
struct ImageColorStats {
    avg_rgb: [f32; 3],
    avg_luma: f32,
    hue: f32,
    saturation: f32,
}

impl ImageColorStats {
    fn from_image(image: &ColorImage) -> Option<Self> {
        let total_pixels = image.pixels.len();
        if total_pixels == 0 {
            return None;
        }
        let step = (total_pixels / SNAP_COLOR_SAMPLE_TARGET).max(1);
        let (sum_r, sum_g, sum_b, sum_luma, samples) =
            if total_pixels <= SNAP_PARALLEL_STATS_MIN_PIXELS {
                let mut sum_r = 0.0_f32;
                let mut sum_g = 0.0_f32;
                let mut sum_b = 0.0_f32;
                let mut sum_luma = 0.0_f32;
                let mut samples = 0_usize;
                for color in image.pixels.iter().step_by(step) {
                    let [r, g, b, _] = color.to_array();
                    let rf = f32::from(r);
                    let gf = f32::from(g);
                    let bf = f32::from(b);
                    sum_r += rf;
                    sum_g += gf;
                    sum_b += bf;
                    sum_luma += srgb_luminance_components(rf, gf, bf);
                    samples += 1;
                }
                (sum_r, sum_g, sum_b, sum_luma, samples)
            } else {
                image
                    .pixels
                    .par_chunks(step)
                    .map(|chunk| {
                        let color = chunk[0];
                        let [r, g, b, _] = color.to_array();
                        let rf = f32::from(r);
                        let gf = f32::from(g);
                        let bf = f32::from(b);
                        let luma = srgb_luminance_components(rf, gf, bf);
                        (rf, gf, bf, luma, 1_usize)
                    })
                    .reduce(
                        || (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32, 0_usize),
                        |acc, val| {
                            (
                                acc.0 + val.0,
                                acc.1 + val.1,
                                acc.2 + val.2,
                                acc.3 + val.3,
                                acc.4 + val.4,
                            )
                        },
                    )
            };

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

fn srgb_luminance_components(r: f32, g: f32, b: f32) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
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

fn snap_color_score(color: Color32, stats: &ImageColorStats) -> f32 {
    let luma_diff = (srgb_luminance(color) - stats.avg_luma).abs() / 255.0;
    let [r, g, b, _] = color.to_array();
    let dr = f32::from(r) - stats.avg_rgb[0];
    let dg = f32::from(g) - stats.avg_rgb[1];
    let db = f32::from(b) - stats.avg_rgb[2];
    let color_diff = (dr * dr + dg * dg + db * db).sqrt() / SNAP_MAX_COLOR_DISTANCE;
    (luma_diff * 0.7 + color_diff * 0.3).clamp(0.0, 1.0)
}

fn rounded_u8(value: f32) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        value.round().clamp(0.0, f32::from(u8::MAX)) as u8
    }
}
