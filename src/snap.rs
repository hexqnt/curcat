use egui::{Color32, ColorImage, Pos2, pos2};
use rayon::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapFeatureSource {
    LumaGradient,
    ColorMatch,
    Hybrid,
}

impl SnapFeatureSource {
    pub const ALL: [Self; 3] = [Self::LumaGradient, Self::ColorMatch, Self::Hybrid];

    pub const fn label(self) -> &'static str {
        match self {
            Self::LumaGradient => "Luma gradient",
            Self::ColorMatch => "Color mask",
            Self::Hybrid => "Gradient + color",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapThresholdKind {
    Gradient,
    Score,
}

impl SnapThresholdKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Gradient => "Gradient only",
            Self::Score => "Feature score",
        }
    }
}

#[derive(Debug, Clone)]
struct SnapMapLevel {
    size: [usize; 2],
    scale: u32,
    gradient: Vec<f32>,
    color_similarity: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct SnapMapCache {
    levels: Vec<SnapMapLevel>,
}

impl SnapMapCache {
    pub fn build(image: &ColorImage, target: Color32, tolerance: f32) -> Option<Self> {
        if image.size[0] == 0 || image.size[1] == 0 {
            return None;
        }
        let mut levels = Vec::new();
        levels.push(SnapMapLevel::base_from_image(image, target, tolerance));
        while let Some(prev) = levels.last() {
            if prev.size[0] < 4 || prev.size[1] < 4 {
                break;
            }
            if let Some(next) = SnapMapLevel::downsample(prev) {
                levels.push(next);
            } else {
                break;
            }
        }
        Some(Self { levels })
    }

    pub fn find_point(
        &self,
        pixel_hint: Pos2,
        radius: f32,
        behavior: SnapBehavior,
    ) -> Option<Pos2> {
        if self.levels.is_empty() {
            return None;
        }
        let radius = radius.max(1.0);
        let (_, level) = self.level_for_radius(radius);
        let scale = u32_to_f32(level.scale);
        let coarse_center = pos2(pixel_hint.x / scale, pixel_hint.y / scale);
        let coarse_radius = (radius / scale).max(1.0);
        let coarse_candidate = search_in_level(level, coarse_center, coarse_radius, behavior)?;
        let coarse_base_pos = pos2(
            coarse_candidate.pos.x * scale,
            coarse_candidate.pos.y * scale,
        );
        let base_level = self.levels.first().unwrap();
        let refine_radius = (scale * 2.5).max(3.0);
        let refined_candidate =
            search_in_level(base_level, coarse_base_pos, refine_radius, behavior)
                .map_or(coarse_base_pos, |cand| cand.pos);
        Some(refine_snap_position(
            base_level,
            refined_candidate,
            behavior,
        ))
    }

    fn level_for_radius(&self, radius: f32) -> (usize, &SnapMapLevel) {
        assert!(!self.levels.is_empty(), "SnapMapCache without levels");
        let mut chosen = 0;
        for (idx, level) in self.levels.iter().enumerate() {
            let level_scale = u32_to_f32(level.scale);
            if radius / level_scale <= 12.0 || idx == self.levels.len() - 1 {
                chosen = idx;
                break;
            }
        }
        (chosen, &self.levels[chosen])
    }
}

impl SnapMapLevel {
    fn base_from_image(image: &ColorImage, target: Color32, tolerance: f32) -> Self {
        let size = image.size;
        let len = size[0] * size[1];
        let mut luminance = vec![0.0_f32; len];
        let mut color_similarity = vec![0.0_f32; len];
        luminance
            .par_iter_mut()
            .zip(color_similarity.par_iter_mut())
            .zip(image.pixels.par_iter())
            .for_each(|((lum, similarity), color)| {
                *lum = color_luminance(*color);
                *similarity = color_similarity_value(*color, target, tolerance);
            });
        let mut gradient = vec![0.0_f32; len];
        let width = size[0];
        let height = size[1];
        if width >= 3 && height >= 3 {
            let lum_slice = &luminance;
            gradient
                .par_chunks_mut(width)
                .enumerate()
                .for_each(|(y, row)| {
                    if y == 0 || y + 1 == height {
                        return;
                    }
                    for x in 1..(width - 1) {
                        let idx = y * width + x;
                        let gx = lum_slice[idx + 1] - lum_slice[idx - 1];
                        let gy = lum_slice[idx + width] - lum_slice[idx - width];
                        row[x] = gx.hypot(gy).min(255.0);
                    }
                });
        }

        Self {
            size,
            scale: 1,
            gradient,
            color_similarity,
        }
    }

    fn downsample(prev: &Self) -> Option<Self> {
        let [w, h] = prev.size;
        if w < 2 || h < 2 {
            return None;
        }
        let new_w = w.div_ceil(2);
        let new_h = h.div_ceil(2);
        if new_w < 2 || new_h < 2 {
            return None;
        }
        let mut gradient = vec![0.0; new_w * new_h];
        let mut color_similarity = vec![0.0; new_w * new_h];
        gradient
            .par_chunks_mut(new_w)
            .zip(color_similarity.par_chunks_mut(new_w))
            .enumerate()
            .for_each(|(y, (grad_row, color_row))| {
                for x in 0..new_w {
                    let mut g_sum = 0.0;
                    let mut c_sum = 0.0;
                    let mut count = 0.0;
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let sx = x * 2 + dx;
                            let sy = y * 2 + dy;
                            if sx < w && sy < h {
                                let idx = sy * w + sx;
                                g_sum += prev.gradient[idx];
                                c_sum += prev.color_similarity[idx];
                                count += 1.0;
                            }
                        }
                    }
                    grad_row[x] = if count > 0.0 { g_sum / count } else { 0.0 };
                    color_row[x] = if count > 0.0 { c_sum / count } else { 0.0 };
                }
            });
        Some(Self {
            size: [new_w, new_h],
            scale: prev.scale * 2,
            gradient,
            color_similarity,
        })
    }

    fn gradient_at(&self, x: i32, y: i32) -> f32 {
        if self.gradient.is_empty() {
            return 0.0;
        }
        let xi = clamp_index(x, self.size[0]);
        let yi = clamp_index(y, self.size[1]);
        self.gradient[yi * self.size[0] + xi]
    }

    fn color_similarity_at(&self, x: i32, y: i32) -> f32 {
        if self.color_similarity.is_empty() {
            return 0.0;
        }
        let xi = clamp_index(x, self.size[0]);
        let yi = clamp_index(y, self.size[1]);
        self.color_similarity[yi * self.size[0] + xi]
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SnapBehavior {
    Contrast {
        feature_source: SnapFeatureSource,
        threshold_kind: SnapThresholdKind,
        threshold: f32,
    },
    Centerline {
        threshold: f32,
    },
}

impl SnapBehavior {
    fn feature_strength(self, gradient: f32, color_similarity: f32) -> f32 {
        match self {
            Self::Contrast { feature_source, .. } => match feature_source {
                SnapFeatureSource::LumaGradient => gradient.clamp(0.0, 255.0),
                SnapFeatureSource::ColorMatch => (color_similarity * 255.0).clamp(0.0, 255.0),
                SnapFeatureSource::Hybrid => {
                    let grad_strength = gradient.clamp(0.0, 255.0);
                    let color_strength = (color_similarity * 255.0).clamp(0.0, 255.0);
                    0.6 * grad_strength + 0.4 * color_strength
                }
            },
            Self::Centerline { .. } => {
                let color_strength = (color_similarity * 255.0).clamp(0.0, 255.0);
                if color_strength <= f32::EPSILON {
                    return 0.0;
                }
                let grad_norm = (gradient / 255.0).clamp(0.0, 1.0);
                color_strength * (1.0 - grad_norm)
            }
        }
    }

    fn threshold_passes(self, gradient: f32, feature_strength: f32) -> bool {
        match self {
            Self::Contrast {
                threshold_kind,
                threshold,
                ..
            } => match threshold_kind {
                SnapThresholdKind::Gradient => gradient >= threshold,
                SnapThresholdKind::Score => feature_strength >= threshold,
            },
            Self::Centerline { threshold } => feature_strength >= threshold,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SnapCandidate {
    pos: Pos2,
    score: f32,
    dist: f32,
}

fn search_in_level(
    level: &SnapMapLevel,
    center: Pos2,
    radius: f32,
    behavior: SnapBehavior,
) -> Option<SnapCandidate> {
    if radius <= 0.0 || level.size[0] < 3 || level.size[1] < 3 {
        return None;
    }
    let width = i32::try_from(level.size[0]).ok()?;
    let height = i32::try_from(level.size[1]).ok()?;
    let radius = radius.max(1.0);
    let radius_sq = radius * radius;
    let reach = saturating_f32_to_i32(radius.ceil());
    let center_x = center.x.clamp(1.0, i32_to_f32(width - 2));
    let center_y = center.y.clamp(1.0, i32_to_f32(height - 2));
    let cx = saturating_f32_to_i32(center_x.round());
    let cy = saturating_f32_to_i32(center_y.round());
    let min_x = (cx - reach).max(1);
    let max_x = (cx + reach).min(width - 2);
    let min_y = (cy - reach).max(1);
    let max_y = (cy + reach).min(height - 2);
    let mut best: Option<SnapCandidate> = None;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let xf = i32_to_f32(x);
            let yf = i32_to_f32(y);
            let dx = xf - center_x;
            let dy = yf - center_y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq > radius_sq {
                continue;
            }
            let gradient = level.gradient_at(x, y);
            let color_similarity = level.color_similarity_at(x, y);
            let feature_strength = behavior.feature_strength(gradient, color_similarity);
            if feature_strength <= 0.0 {
                continue;
            }
            if !behavior.threshold_passes(gradient, feature_strength) {
                continue;
            }
            let dist = dist_sq.sqrt();
            let closeness = (1.0 - dist / radius).max(0.05);
            let score = feature_strength * closeness;
            let candidate = SnapCandidate {
                pos: pos2(xf, yf),
                score,
                dist,
            };
            let update = best.as_ref().is_none_or(|existing| {
                score > existing.score + 0.1
                    || ((score - existing.score).abs() <= 0.1 && dist < existing.dist)
            });
            if update {
                best = Some(candidate);
            }
        }
    }

    best
}

fn refine_snap_position(level: &SnapMapLevel, approx: Pos2, behavior: SnapBehavior) -> Pos2 {
    if level.size[0] < 3 || level.size[1] < 3 {
        return approx;
    }
    let Ok(width) = i32::try_from(level.size[0]) else {
        return approx;
    };
    let Ok(height) = i32::try_from(level.size[1]) else {
        return approx;
    };
    let ax = saturating_f32_to_i32(approx.x.clamp(1.0, i32_to_f32(width - 2)).round());
    let ay = saturating_f32_to_i32(approx.y.clamp(1.0, i32_to_f32(height - 2)).round());

    let mut sum = 0.0;
    let mut sx = 0.0;
    let mut sy = 0.0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let px = (ax + dx).clamp(0, width - 1);
            let py = (ay + dy).clamp(0, height - 1);
            let strength = behavior
                .feature_strength(level.gradient_at(px, py), level.color_similarity_at(px, py));
            if strength <= 0.0 {
                continue;
            }
            sum += strength;
            sx += strength * i32_to_f32(px);
            sy += strength * i32_to_f32(py);
        }
    }
    if sum > 0.0 {
        pos2(
            (sx / sum).clamp(0.0, i32_to_f32(width - 1)),
            (sy / sum).clamp(0.0, i32_to_f32(height - 1)),
        )
    } else {
        approx
    }
}

fn color_luminance(color: Color32) -> f32 {
    let [r, g, b, _] = color.to_array();
    0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b)
}

fn color_similarity_value(color: Color32, target: Color32, tolerance: f32) -> f32 {
    let [tr, tg, tb, _] = target.to_array();
    let [r, g, b, _] = color.to_array();
    let dr = f32::from(r) - f32::from(tr);
    let dg = f32::from(g) - f32::from(tg);
    let db = f32::from(b) - f32::from(tb);
    let diff = (dr * dr + dg * dg + db * db).sqrt();
    let tol = tolerance.max(1.0);
    ((tol - diff).max(0.0) / tol).clamp(0.0, 1.0)
}

fn clamp_index(value: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let last = len - 1;
    let Ok(last_i32) = i32::try_from(last) else {
        return last;
    };
    let clamped = value.clamp(0, last_i32);
    usize::try_from(clamped).unwrap_or(last)
}

const fn u32_to_f32(value: u32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f32
    }
}

const fn i32_to_f32(value: i32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f32
    }
}

const fn saturating_f32_to_i32(value: f32) -> i32 {
    #[allow(clippy::cast_precision_loss)]
    const MAX: f32 = i32::MAX as f32;
    #[allow(clippy::cast_precision_loss)]
    const MIN: f32 = i32::MIN as f32;
    #[allow(clippy::cast_possible_truncation)]
    {
        if value.is_nan() {
            0
        } else {
            value.clamp(MIN, MAX).round() as i32
        }
    }
}
