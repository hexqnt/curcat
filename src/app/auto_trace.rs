use super::{CurcatApp, PickedPoint};
use crate::snap::SnapBehavior;
use crate::types::CoordSystem;
use crate::util::safe_usize_to_f32;
use egui::{Pos2, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoTraceDirection {
    Forward,
    Backward,
    Both,
}

impl AutoTraceDirection {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Forward => "Forward (+X)",
            Self::Backward => "Backward (-X)",
            Self::Both => "Both",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AutoTraceConfig {
    pub(crate) direction: AutoTraceDirection,
    pub(crate) step_px: f32,
    pub(crate) search_radius: f32,
    pub(crate) max_points: usize,
    pub(crate) max_misses: u32,
    pub(crate) min_advance: f32,
    pub(crate) dedup_radius: f32,
}

impl Default for AutoTraceConfig {
    fn default() -> Self {
        Self {
            direction: AutoTraceDirection::Forward,
            step_px: 6.0,
            search_radius: 12.0,
            max_points: 800,
            max_misses: 8,
            min_advance: 1.0,
            dedup_radius: 1.5,
        }
    }
}

impl AutoTraceConfig {
    pub(crate) fn sanitized(self) -> Self {
        let step_px = self.step_px.clamp(1.0, 80.0);
        let search_radius = self.search_radius.clamp(2.0, 120.0);
        let max_points = self.max_points.clamp(2, 20_000);
        let max_misses = self.max_misses.min(1_000);
        let min_advance = self.min_advance.clamp(0.1, step_px.max(0.1));
        let dedup_radius = self.dedup_radius.clamp(0.0, 50.0);
        Self {
            direction: self.direction,
            step_px,
            search_radius,
            max_points,
            max_misses,
            min_advance,
            dedup_radius,
        }
    }
}

impl CurcatApp {
    pub(crate) fn auto_trace_from(&mut self, pixel_hint: Pos2) {
        if self.image.image.is_none() {
            self.set_status("Auto-trace requires an image.");
            return;
        }
        if !self.calibration_ready() {
            self.set_status("Auto-trace requires completed calibration.");
            return;
        }
        if !matches!(self.calibration.coord_system, CoordSystem::Cartesian) {
            self.set_status("Auto-trace currently supports Cartesian calibration only.");
            return;
        }
        let Some(behavior) = self.current_snap_behavior() else {
            self.set_status("Auto-trace requires snapping (Contrast/Centerline).");
            return;
        };
        let cfg = self.interaction.auto_trace_cfg.sanitized();
        let size = self.image.image.as_ref().map_or([0, 0], |img| img.size);
        let axis_dir = self
            .calibration
            .cal_x
            .mapping()
            .and_then(|map| {
                let delta = map.p2 - map.p1;
                let len = delta.length();
                if len <= f32::EPSILON {
                    return None;
                }
                let mut dir = delta / len;
                if map.v2.to_scalar_seconds() < map.v1.to_scalar_seconds() {
                    dir = -dir;
                }
                Some(dir)
            })
            .unwrap_or(Vec2::X);
        let Some(start) = self.find_snap_point_with_radius(pixel_hint, cfg.search_radius, behavior)
        else {
            self.set_status("Auto-trace failed: no snap candidate near the click.");
            return;
        };

        let mut points = Vec::new();
        match cfg.direction {
            AutoTraceDirection::Forward => {
                points.push(start);
                points.extend(self.auto_trace_direction(start, axis_dir, 1.0, size, behavior, cfg));
            }
            AutoTraceDirection::Backward => {
                points.push(start);
                points
                    .extend(self.auto_trace_direction(start, axis_dir, -1.0, size, behavior, cfg));
            }
            AutoTraceDirection::Both => {
                let mut back =
                    self.auto_trace_direction(start, axis_dir, -1.0, size, behavior, cfg);
                let forward = self.auto_trace_direction(start, axis_dir, 1.0, size, behavior, cfg);
                back.reverse();
                points.extend(back);
                points.push(start);
                points.extend(forward);
            }
        }

        let mut deduped: Vec<Pos2> = Vec::new();
        for p in points {
            if deduped
                .last()
                .is_none_or(|last| (*last - p).length() > cfg.dedup_radius)
            {
                deduped.push(p);
            }
        }

        if deduped.is_empty() {
            self.set_status("Auto-trace found no points.");
            return;
        }

        for p in &deduped {
            self.points.points.push(PickedPoint::new(*p));
        }
        self.mark_points_dirty();
        self.set_status(format!("Auto-trace added {} points.", deduped.len()));
    }

    fn auto_trace_direction(
        &mut self,
        start: Pos2,
        axis_dir: Vec2,
        dir_sign: f32,
        size: [usize; 2],
        behavior: SnapBehavior,
        cfg: AutoTraceConfig,
    ) -> Vec<Pos2> {
        if size[0] == 0 || size[1] == 0 {
            return Vec::new();
        }
        let mut points = Vec::new();
        let mut anchor = start;
        let mut probe = start;
        let mut misses = 0u32;
        let step = axis_dir * cfg.step_px * dir_sign.signum();
        let max_x = safe_usize_to_f32(size[0].saturating_sub(1));
        let max_y = safe_usize_to_f32(size[1].saturating_sub(1));

        for _ in 0..cfg.max_points {
            probe += step;
            if probe.x < 0.0 || probe.x > max_x || probe.y < 0.0 || probe.y > max_y {
                break;
            }

            let candidate = self.find_snap_point_with_radius(probe, cfg.search_radius, behavior);
            if let Some(pos) = candidate {
                let progress = (pos - anchor).dot(axis_dir) * dir_sign.signum();
                if progress < cfg.min_advance {
                    misses = misses.saturating_add(1);
                    if misses > cfg.max_misses {
                        break;
                    }
                    continue;
                }
                if (pos - anchor).length() <= cfg.dedup_radius {
                    misses = misses.saturating_add(1);
                    if misses > cfg.max_misses {
                        break;
                    }
                    continue;
                }
                points.push(pos);
                anchor = pos;
                probe = anchor;
                misses = 0;
            } else {
                misses = misses.saturating_add(1);
                if misses > cfg.max_misses {
                    break;
                }
            }
        }

        points
    }
}
