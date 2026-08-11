use egui::{Color32, ColorImage, Pos2, pos2};
use rayon::prelude::*;
use std::simd::StdFloat;
use std::simd::num::SimdFloat;

use super::behavior::SnapBehavior;
use super::color::{color_luminance, color_similarity_value};
use super::search::{refine_snap_position, search_in_level};
use crate::pixel_simd::{F32x8, LANES, unpack_rgba_block};
use crate::util::u32_to_f32;

// Построение карт упирается в пропускную способность памяти; большее число
// мелких задач замедляет типичные изображения из-за планировщика Rayon.
const SNAP_BUILD_PARALLEL_TASKS: usize = 4;
const LUMA_R_COEFF: f32 = 0.2126;
const LUMA_G_COEFF: f32 = 0.7152;
const LUMA_B_COEFF: f32 = 0.0722;

#[allow(clippy::suboptimal_flops)]
fn compute_luma_similarity_chunk(
    lum: &mut [f32],
    similarity: &mut [f32],
    colors: &[Color32],
    target: Color32,
    tol: f32,
) {
    assert_eq!(
        lum.len(),
        colors.len(),
        "luminance and color chunks must have the same length"
    );
    assert_eq!(
        similarity.len(),
        colors.len(),
        "similarity and color chunks must have the same length"
    );

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

    let (color_chunks, color_remainder) = colors.as_chunks::<LANES>();
    let (lum_chunks, lum_remainder) = lum.as_chunks_mut::<LANES>();
    let (similarity_chunks, similarity_remainder) = similarity.as_chunks_mut::<LANES>();

    for ((color_chunk, lum_chunk), similarity_chunk) in
        color_chunks.iter().zip(lum_chunks).zip(similarity_chunks)
    {
        let channels = unpack_rgba_block(color_chunk);
        let rf = channels.red;
        let gf = channels.green;
        let bf = channels.blue;
        (rf * luma_r + gf * luma_g + bf * luma_b).copy_to_slice(lum_chunk);

        let dr = rf - target_r;
        let dg = gf - target_g;
        let db = bf - target_b;
        let diff = (dr * dr + dg * dg + db * db).sqrt();
        (((tolerance_vec - diff).simd_max(zero) * inv_tolerance).simd_min(one))
            .copy_to_slice(similarity_chunk);
    }

    for ((lum, similarity), color) in lum_remainder
        .iter_mut()
        .zip(similarity_remainder)
        .zip(color_remainder)
    {
        *lum = color_luminance(*color);
        *similarity = color_similarity_value(*color, target, tol);
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

/// Усредняет восемь соседних блоков 2×2 из двух строк.
#[inline]
fn average_2x2_blocks(values: &[f32], row_width: usize, top: usize) -> F32x8 {
    let bottom = top + row_width;
    let (top_left, top_right) = F32x8::from_slice(&values[top..top + LANES])
        .deinterleave(F32x8::from_slice(&values[top + LANES..top + LANES * 2]));
    let (bottom_left, bottom_right) = F32x8::from_slice(&values[bottom..bottom + LANES])
        .deinterleave(F32x8::from_slice(
            &values[bottom + LANES..bottom + LANES * 2],
        ));
    (top_left + top_right + bottom_left + bottom_right) * F32x8::splat(0.25)
}

#[allow(clippy::suboptimal_flops)]
fn compute_gradient_row(row: &mut [f32], lum: &[f32], row_base: usize, width: usize) {
    assert_eq!(row.len(), width, "gradient row width mismatch");
    assert!(
        row_base >= width && row_base + width * 2 <= lum.len(),
        "gradient row must have neighboring luminance rows"
    );

    let max_gradient = F32x8::splat(255.0);
    let inner_end = width - 1;
    let prev_row = &lum[row_base - width..row_base];
    let curr_row = &lum[row_base..row_base + width];
    let next_row = &lum[row_base + width..row_base + width * 2];
    let mut x = 1usize;
    while x + LANES <= inner_end {
        let gx = F32x8::from_slice(&curr_row[x + 1..x + 1 + LANES])
            - F32x8::from_slice(&curr_row[x - 1..x - 1 + LANES]);
        let gy =
            F32x8::from_slice(&next_row[x..x + LANES]) - F32x8::from_slice(&prev_row[x..x + LANES]);
        ((gx * gx + gy * gy).sqrt().simd_min(max_gradient)).copy_to_slice(&mut row[x..x + LANES]);
        x += LANES;
    }

    let left = &curr_row[x - 1..inner_end - 1];
    let right = &curr_row[x + 1..=inner_end];
    let up = &prev_row[x..inner_end];
    let down = &next_row[x..inner_end];
    for ((((pixel, &left), &right), &up), &down) in row[x..inner_end]
        .iter_mut()
        .zip(left)
        .zip(right)
        .zip(up)
        .zip(down)
    {
        let gx = right - left;
        let gy = down - up;
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
            let Some(next) = SnapMapLevel::downsample(prev) else {
                break;
            };
            levels.push(next);
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
        let base_level = self.levels.first()?;
        let radius = radius.max(1.0);
        let level = self.level_for_radius(radius);
        let scale = u32_to_f32(level.scale);
        let coarse_center = pos2(pixel_hint.x / scale, pixel_hint.y / scale);
        let coarse_radius = (radius / scale).max(1.0);
        let coarse_candidate = search_in_level(level, coarse_center, coarse_radius, behavior)?;
        let coarse_base_pos = pos2(
            coarse_candidate.pos.x * scale,
            coarse_candidate.pos.y * scale,
        );
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

    fn level_for_radius(&self, radius: f32) -> &SnapMapLevel {
        for level in &self.levels {
            let level_scale = u32_to_f32(level.scale);
            if radius / level_scale <= 12.0 {
                return level;
            }
        }
        self.levels
            .last()
            .expect("SnapMapCache must contain at least one level")
    }
}

impl SnapMapLevel {
    fn base_from_image(image: &ColorImage, target: Color32, tolerance: f32) -> Self {
        let size = image.size;
        let len = size[0] * size[1];
        let tol = tolerance.max(1.0);
        let mut luminance = vec![0.0_f32; len];
        let mut color_similarity = vec![0.0_f32; len];
        let parallel_chunk = len.div_ceil(SNAP_BUILD_PARALLEL_TASKS);
        luminance
            .par_chunks_mut(parallel_chunk)
            .zip(color_similarity.par_chunks_mut(parallel_chunk))
            .zip(image.pixels.par_chunks(parallel_chunk))
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
            let rows_per_task = height.div_ceil(SNAP_BUILD_PARALLEL_TASKS);
            gradient
                .par_chunks_mut(width)
                .with_min_len(rows_per_task)
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
        let [width, height] = prev.size;
        if width < 2 || height < 2 {
            return None;
        }
        let new_width = width.div_ceil(2);
        let new_height = height.div_ceil(2);
        if new_width < 2 || new_height < 2 {
            return None;
        }
        let mut gradient = vec![0.0; new_width * new_height];
        let mut color_similarity = vec![0.0; new_width * new_height];
        let interior_width = width / 2;
        let interior_height = height / 2;
        let rows_per_task = new_height.div_ceil(SNAP_BUILD_PARALLEL_TASKS);
        gradient
            .par_chunks_mut(new_width)
            .zip(color_similarity.par_chunks_mut(new_width))
            .with_min_len(rows_per_task)
            .enumerate()
            .for_each(|(y, (grad_row, color_row))| {
                let src_y = y * 2;
                if y < interior_height {
                    let mut dst_x = 0usize;
                    while dst_x + LANES <= interior_width {
                        let top = src_y * width + dst_x * 2;
                        average_2x2_blocks(&prev.gradient, width, top)
                            .copy_to_slice(&mut grad_row[dst_x..dst_x + LANES]);
                        average_2x2_blocks(&prev.color_similarity, width, top)
                            .copy_to_slice(&mut color_row[dst_x..dst_x + LANES]);
                        dst_x += LANES;
                    }

                    for cell_x in dst_x..interior_width {
                        let src_x = cell_x * 2;
                        let top = src_y * width + src_x;
                        let bottom = top + width;
                        grad_row[cell_x] = (prev.gradient[top]
                            + prev.gradient[top + 1]
                            + prev.gradient[bottom]
                            + prev.gradient[bottom + 1])
                            * 0.25;
                        color_row[cell_x] = (prev.color_similarity[top]
                            + prev.color_similarity[top + 1]
                            + prev.color_similarity[bottom]
                            + prev.color_similarity[bottom + 1])
                            * 0.25;
                    }
                }

                let border_start = if y < interior_height {
                    interior_width
                } else {
                    0
                };
                for (offset, (grad_cell, color_cell)) in grad_row[border_start..]
                    .iter_mut()
                    .zip(&mut color_row[border_start..])
                    .enumerate()
                {
                    let cell_x = border_start + offset;
                    let (gradient_avg, similarity_avg) =
                        downsample_cell_checked(prev, width, height, cell_x, y);
                    *grad_cell = gradient_avg;
                    *color_cell = similarity_avg;
                }
            });
        Some(Self {
            size: [new_width, new_height],
            scale: prev.scale * 2,
            gradient,
            color_similarity,
        })
    }

    pub(super) fn value_rows(&self, y: usize) -> Option<(&[f32], &[f32])> {
        if y >= self.size[1] {
            return None;
        }
        let start = y.checked_mul(self.size[0])?;
        let end = start.checked_add(self.size[0])?;
        Some((
            self.gradient.get(start..end)?,
            self.color_similarity.get(start..end)?,
        ))
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
