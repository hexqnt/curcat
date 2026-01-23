use egui::{Pos2, pos2};

use super::behavior::SnapBehavior;
use super::maps::SnapMapLevel;
use super::util::{i32_to_f32, saturating_f32_to_i32};

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
