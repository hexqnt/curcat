use egui::{Pos2, pos2};

use super::behavior::SnapBehavior;
use super::maps::SnapMapLevel;
use crate::util::{i32_to_f32, saturating_f32_to_i32};

#[derive(Debug, Clone, Copy)]
pub(super) struct SnapCandidate {
    pub(super) pos: Pos2,
    score: f32,
    dist: f32,
}

pub(super) fn search_in_level(
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

    let min_x_usize = usize::try_from(min_x).ok()?;
    let max_x_usize = usize::try_from(max_x).ok()?;
    let x_end = max_x_usize.checked_add(1)?;

    for y in min_y..=max_y {
        let y_usize = usize::try_from(y).ok()?;
        let (gradient_row, color_row) = level.value_rows(y_usize)?;
        let gradient_cells = gradient_row.get(min_x_usize..x_end)?;
        let color_cells = color_row.get(min_x_usize..x_end)?;
        for (x, (&gradient, &color_similarity)) in
            (min_x..=max_x).zip(gradient_cells.iter().zip(color_cells))
        {
            let xf = i32_to_f32(x);
            let yf = i32_to_f32(y);
            let dx = xf - center_x;
            let dy = yf - center_y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq > radius_sq {
                continue;
            }
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

pub(super) fn refine_snap_position(
    level: &SnapMapLevel,
    approx: Pos2,
    behavior: SnapBehavior,
) -> Pos2 {
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
    let min_x = (ax - 1).max(0);
    let max_x = (ax + 1).min(width - 1);
    let min_y = (ay - 1).max(0);
    let max_y = (ay + 1).min(height - 1);
    let Ok(min_x_usize) = usize::try_from(min_x) else {
        return approx;
    };
    let Ok(max_x_usize) = usize::try_from(max_x) else {
        return approx;
    };
    let Some(x_end) = max_x_usize.checked_add(1) else {
        return approx;
    };

    let mut sum = 0.0;
    let mut sx = 0.0;
    let mut sy = 0.0;
    for py in min_y..=max_y {
        let Ok(py_usize) = usize::try_from(py) else {
            continue;
        };
        let Some((gradient_row, color_row)) = level.value_rows(py_usize) else {
            continue;
        };
        let Some(gradient_cells) = gradient_row.get(min_x_usize..x_end) else {
            continue;
        };
        let Some(color_cells) = color_row.get(min_x_usize..x_end) else {
            continue;
        };
        for (px, (&gradient, &color_similarity)) in
            (min_x..=max_x).zip(gradient_cells.iter().zip(color_cells))
        {
            let strength = behavior.feature_strength(gradient, color_similarity);
            if strength <= 0.0 {
                continue;
            }
            sum += strength;
            sx = strength.mul_add(i32_to_f32(px), sx);
            sy = strength.mul_add(i32_to_f32(py), sy);
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
