use egui::{Color32, ColorImage, Pos2, pos2};
use rayon::prelude::*;
use std::simd::num::SimdFloat;
use std::simd::{Simd, StdFloat};

use super::behavior::SnapBehavior;
use super::color::{color_luminance, color_similarity_value};
use super::search::{refine_snap_position, search_in_level};
use crate::util::{clamp_index, u32_to_f32};

const SNAP_MAP_SIMD_LANES: usize = 8;
const SNAP_BASE_PAR_CHUNK: usize = 4096;
const LUMA_R_COEFF: f32 = 0.2126;
const LUMA_G_COEFF: f32 = 0.7152;
const LUMA_B_COEFF: f32 = 0.0722;
type F32x8 = Simd<f32, SNAP_MAP_SIMD_LANES>;

#[allow(clippy::suboptimal_flops)]
fn compute_luma_similarity_chunk(
    lum: &mut [f32],
    similarity: &mut [f32],
    colors: &[Color32],
    target: Color32,
    tol: f32,
) {
    let [tr, tg, tb, _] = target.to_array();
    let target_rgb = [f32::from(tr), f32::from(tg), f32::from(tb)];
    let zero = F32x8::splat(0.0);
    let one = F32x8::splat(1.0);
    let target_r = F32x8::splat(target_rgb[0]);
    let target_g = F32x8::splat(target_rgb[1]);
    let target_b = F32x8::splat(target_rgb[2]);
    let tolerance_vec = F32x8::splat(tol);
    let inv_tolerance = F32x8::splat(1.0 / tol);
    let luma_r = F32x8::splat(LUMA_R_COEFF);
    let luma_g = F32x8::splat(LUMA_G_COEFF);
    let luma_b = F32x8::splat(LUMA_B_COEFF);

    let mut i = 0usize;
    while i + SNAP_MAP_SIMD_LANES <= colors.len() {
        let mut r = [0.0_f32; SNAP_MAP_SIMD_LANES];
        let mut g = [0.0_f32; SNAP_MAP_SIMD_LANES];
        let mut b = [0.0_f32; SNAP_MAP_SIMD_LANES];
        for lane in 0..SNAP_MAP_SIMD_LANES {
            let [pr, pg, pb, _] = colors[i + lane].to_array();
            r[lane] = f32::from(pr);
            g[lane] = f32::from(pg);
            b[lane] = f32::from(pb);
        }

        let rf = F32x8::from_array(r);
        let gf = F32x8::from_array(g);
        let bf = F32x8::from_array(b);
        (rf * luma_r + gf * luma_g + bf * luma_b)
            .copy_to_slice(&mut lum[i..i + SNAP_MAP_SIMD_LANES]);

        let dr = rf - target_r;
        let dg = gf - target_g;
        let db = bf - target_b;
        let diff = (dr * dr + dg * dg + db * db).sqrt();
        (((tolerance_vec - diff).simd_max(zero) * inv_tolerance).simd_min(one))
            .copy_to_slice(&mut similarity[i..i + SNAP_MAP_SIMD_LANES]);

        i += SNAP_MAP_SIMD_LANES;
    }

    for lane in i..colors.len() {
        lum[lane] = color_luminance(colors[lane]);
        similarity[lane] = color_similarity_value(colors[lane], target, tol);
    }
}

#[allow(clippy::suboptimal_flops)]
fn downsample_cell_checked(
    prev: &SnapMapLevel,
    src_width: usize,
    src_height: usize,
    x: usize,
    y: usize,
) -> (f32, f32) {
    let mut g_sum = 0.0;
    let mut c_sum = 0.0;
    let mut count = 0.0;
    for dy in 0..2 {
        for dx in 0..2 {
            let sx = x * 2 + dx;
            let sy = y * 2 + dy;
            if sx < src_width && sy < src_height {
                let idx = sy * src_width + sx;
                g_sum += prev.gradient[idx];
                c_sum += prev.color_similarity[idx];
                count += 1.0;
            }
        }
    }
    if count > 0.0 {
        (g_sum / count, c_sum / count)
    } else {
        (0.0, 0.0)
    }
}

#[allow(clippy::suboptimal_flops)]
fn compute_gradient_row(row: &mut [f32], lum: &[f32], row_base: usize, width: usize) {
    let max_gradient = F32x8::splat(255.0);
    let inner_end = width - 1;
    let mut x = 1usize;
    while x + SNAP_MAP_SIMD_LANES <= inner_end {
        let idx = row_base + x;
        let gx = F32x8::from_slice(&lum[idx + 1..idx + 1 + SNAP_MAP_SIMD_LANES])
            - F32x8::from_slice(&lum[idx - 1..idx - 1 + SNAP_MAP_SIMD_LANES]);
        let gy = F32x8::from_slice(&lum[idx + width..idx + width + SNAP_MAP_SIMD_LANES])
            - F32x8::from_slice(&lum[idx - width..idx - width + SNAP_MAP_SIMD_LANES]);
        ((gx * gx + gy * gy).sqrt().simd_min(max_gradient))
            .copy_to_slice(&mut row[x..x + SNAP_MAP_SIMD_LANES]);
        x += SNAP_MAP_SIMD_LANES;
    }

    for (offset, pixel) in row[x..inner_end].iter_mut().enumerate() {
        let local_x = x + offset;
        let idx = row_base + local_x;
        let gx = lum[idx + 1] - lum[idx - 1];
        let gy = lum[idx + width] - lum[idx - width];
        *pixel = gx.hypot(gy).min(255.0);
    }
}

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
        let base_level = &self.levels[0];
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
        let tol = tolerance.max(1.0);
        let mut luminance = vec![0.0_f32; len];
        let mut color_similarity = vec![0.0_f32; len];
        luminance
            .par_chunks_mut(SNAP_BASE_PAR_CHUNK)
            .zip(color_similarity.par_chunks_mut(SNAP_BASE_PAR_CHUNK))
            .zip(image.pixels.par_chunks(SNAP_BASE_PAR_CHUNK))
            .for_each(|((lum_chunk, similarity_chunk), color_chunk)| {
                compute_luma_similarity_chunk(
                    lum_chunk,
                    similarity_chunk,
                    color_chunk,
                    target,
                    tol,
                );
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
                    compute_gradient_row(row, lum_slice, y * width, width);
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
        let interior_w = w / 2;
        let interior_h = h / 2;
        let quarter = F32x8::splat(0.25);
        gradient
            .par_chunks_mut(new_w)
            .zip(color_similarity.par_chunks_mut(new_w))
            .enumerate()
            .for_each(|(y, (grad_row, color_row))| {
                let src_y = y * 2;
                if y < interior_h {
                    let mut x = 0usize;
                    while x + SNAP_MAP_SIMD_LANES <= interior_w {
                        let mut g00 = [0.0_f32; SNAP_MAP_SIMD_LANES];
                        let mut g01 = [0.0_f32; SNAP_MAP_SIMD_LANES];
                        let mut g10 = [0.0_f32; SNAP_MAP_SIMD_LANES];
                        let mut g11 = [0.0_f32; SNAP_MAP_SIMD_LANES];
                        let mut c00 = [0.0_f32; SNAP_MAP_SIMD_LANES];
                        let mut c01 = [0.0_f32; SNAP_MAP_SIMD_LANES];
                        let mut c10 = [0.0_f32; SNAP_MAP_SIMD_LANES];
                        let mut c11 = [0.0_f32; SNAP_MAP_SIMD_LANES];

                        for lane in 0..SNAP_MAP_SIMD_LANES {
                            let dst_x = x + lane;
                            let src_x = dst_x * 2;
                            let top = src_y * w + src_x;
                            let bottom = top + w;
                            g00[lane] = prev.gradient[top];
                            g01[lane] = prev.gradient[top + 1];
                            g10[lane] = prev.gradient[bottom];
                            g11[lane] = prev.gradient[bottom + 1];
                            c00[lane] = prev.color_similarity[top];
                            c01[lane] = prev.color_similarity[top + 1];
                            c10[lane] = prev.color_similarity[bottom];
                            c11[lane] = prev.color_similarity[bottom + 1];
                        }

                        ((F32x8::from_array(g00)
                            + F32x8::from_array(g01)
                            + F32x8::from_array(g10)
                            + F32x8::from_array(g11))
                            * quarter)
                            .copy_to_slice(&mut grad_row[x..x + SNAP_MAP_SIMD_LANES]);
                        ((F32x8::from_array(c00)
                            + F32x8::from_array(c01)
                            + F32x8::from_array(c10)
                            + F32x8::from_array(c11))
                            * quarter)
                            .copy_to_slice(&mut color_row[x..x + SNAP_MAP_SIMD_LANES]);
                        x += SNAP_MAP_SIMD_LANES;
                    }

                    for dst_x in x..interior_w {
                        let src_x = dst_x * 2;
                        let top = src_y * w + src_x;
                        let bottom = top + w;
                        grad_row[dst_x] = (prev.gradient[top]
                            + prev.gradient[top + 1]
                            + prev.gradient[bottom]
                            + prev.gradient[bottom + 1])
                            * 0.25;
                        color_row[dst_x] = (prev.color_similarity[top]
                            + prev.color_similarity[top + 1]
                            + prev.color_similarity[bottom]
                            + prev.color_similarity[bottom + 1])
                            * 0.25;
                    }
                }

                let border_start = if y < interior_h { interior_w } else { 0 };
                for x in border_start..new_w {
                    let (g, c) = downsample_cell_checked(prev, w, h, x, y);
                    grad_row[x] = g;
                    color_row[x] = c;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mod_u8(value: usize) -> u8 {
        u8::try_from(value % 256).unwrap_or(0)
    }

    fn test_image(width: usize, height: usize) -> ColorImage {
        let mut pixels = Vec::with_capacity(width * height);
        for i in 0..(width * height) {
            let r = mod_u8(i * 17 + 11);
            let g = mod_u8(i * 37 + 29);
            let b = mod_u8(i * 53 + 7);
            pixels.push(Color32::from_rgb(r, g, b));
        }
        ColorImage::new([width, height], pixels)
    }

    #[allow(clippy::suboptimal_flops)]
    fn base_from_image_scalar_reference(
        image: &ColorImage,
        target: Color32,
        tolerance: f32,
    ) -> SnapMapLevel {
        let size = image.size;
        let len = size[0] * size[1];
        let mut luminance = vec![0.0_f32; len];
        let mut color_similarity = vec![0.0_f32; len];
        let [tr, tg, tb, _] = target.to_array();
        let tol = tolerance.max(1.0);

        for (idx, color) in image.pixels.iter().copied().enumerate() {
            let [r, g, b, _] = color.to_array();
            let rf = f32::from(r);
            let gf = f32::from(g);
            let bf = f32::from(b);
            luminance[idx] = LUMA_R_COEFF * rf + LUMA_G_COEFF * gf + LUMA_B_COEFF * bf;
            let dr = rf - f32::from(tr);
            let dg = gf - f32::from(tg);
            let db = bf - f32::from(tb);
            let diff = (dr * dr + dg * dg + db * db).sqrt();
            color_similarity[idx] = ((tol - diff).max(0.0) / tol).clamp(0.0, 1.0);
        }

        let mut gradient = vec![0.0_f32; len];
        let width = size[0];
        let height = size[1];
        if width >= 3 && height >= 3 {
            for y in 1..(height - 1) {
                let row_start = y * width;
                for x in 1..(width - 1) {
                    let idx = row_start + x;
                    let gx = luminance[idx + 1] - luminance[idx - 1];
                    let gy = luminance[idx + width] - luminance[idx - width];
                    gradient[idx] = gx.hypot(gy).min(255.0);
                }
            }
        }

        SnapMapLevel {
            size,
            scale: 1,
            gradient,
            color_similarity,
        }
    }

    fn downsample_scalar_reference(prev: &SnapMapLevel) -> Option<SnapMapLevel> {
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
        for y in 0..new_h {
            for x in 0..new_w {
                let (g, c) = downsample_cell_checked(prev, w, h, x, y);
                gradient[y * new_w + x] = g;
                color_similarity[y * new_w + x] = c;
            }
        }
        Some(SnapMapLevel {
            size: [new_w, new_h],
            scale: prev.scale * 2,
            gradient,
            color_similarity,
        })
    }

    fn approx_eq_slice(lhs: &[f32], rhs: &[f32], eps: f32) -> bool {
        lhs.len() == rhs.len()
            && lhs
                .iter()
                .zip(rhs.iter())
                .all(|(a, b)| (*a - *b).abs() <= eps.max(f32::EPSILON))
    }

    #[test]
    fn base_level_simd_matches_scalar_reference() {
        let image = test_image(19, 11);
        let target = Color32::from_rgb(120, 33, 211);
        let tolerance = 43.0;
        let simd = SnapMapLevel::base_from_image(&image, target, tolerance);
        let scalar = base_from_image_scalar_reference(&image, target, tolerance);
        assert_eq!(simd.size, scalar.size);
        assert_eq!(simd.scale, scalar.scale);
        assert!(approx_eq_slice(&simd.gradient, &scalar.gradient, 1.0e-3));
        assert!(approx_eq_slice(
            &simd.color_similarity,
            &scalar.color_similarity,
            1.0e-6
        ));
    }

    #[test]
    fn downsample_simd_matches_scalar_reference() {
        let image = test_image(13, 9);
        let target = Color32::from_rgb(17, 201, 90);
        let tolerance = 28.0;
        let base = SnapMapLevel::base_from_image(&image, target, tolerance);
        let simd = SnapMapLevel::downsample(&base).expect("downsample");
        let scalar = downsample_scalar_reference(&base).expect("downsample");
        assert_eq!(simd.size, scalar.size);
        assert_eq!(simd.scale, scalar.scale);
        assert!(approx_eq_slice(&simd.gradient, &scalar.gradient, 1.0e-6));
        assert!(approx_eq_slice(
            &simd.color_similarity,
            &scalar.color_similarity,
            1.0e-6
        ));
    }
}
