use egui::{Color32, ColorImage, Pos2, pos2};
use rayon::prelude::*;

use super::behavior::SnapBehavior;
use super::color::{color_luminance, color_similarity_value};
use super::search::{refine_snap_position, search_in_level};
use super::util::{clamp_index, u32_to_f32};

#[derive(Debug, Clone)]
pub(super) struct SnapMapLevel {
    pub(super) size: [usize; 2],
    scale: u32,
    gradient: Vec<f32>,
    color_similarity: Vec<f32>,
}

/// Cached multi-resolution maps for fast snapping searches.
#[derive(Debug, Clone)]
pub struct SnapMapCache {
    levels: Vec<SnapMapLevel>,
}

impl SnapMapCache {
    /// Build a multi-scale cache for the given image and target color.
    ///
    /// Returns `None` when the image is empty.
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

    /// Find the best snap candidate near `pixel_hint` within `radius`.
    ///
    /// The search is done on a coarse level first and refined on the base
    /// level to produce a stable, precise position.
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
                    for (x, pixel) in row[1..width - 1].iter_mut().enumerate() {
                        let idx = y * width + x + 1;
                        let gx = lum_slice[idx + 1] - lum_slice[idx - 1];
                        let gy = lum_slice[idx + width] - lum_slice[idx - width];
                        *pixel = gx.hypot(gy).min(255.0);
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

    pub(super) fn gradient_at(&self, x: i32, y: i32) -> f32 {
        if self.gradient.is_empty() {
            return 0.0;
        }
        let xi = clamp_index(x, self.size[0]);
        let yi = clamp_index(y, self.size[1]);
        self.gradient[yi * self.size[0] + xi]
    }

    pub(super) fn color_similarity_at(&self, x: i32, y: i32) -> f32 {
        if self.color_similarity.is_empty() {
            return 0.0;
        }
        let xi = clamp_index(x, self.size[0]);
        let yi = clamp_index(y, self.size[1]);
        self.color_similarity[yi * self.size[0] + xi]
    }
}
