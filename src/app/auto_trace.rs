use super::{CurcatApp, PickedPoint};
use crate::i18n::UiLanguage;
use crate::types::CoordSystem;
use crate::util::safe_usize_to_f32;
use egui::{Pos2, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoTraceDirection {
    Forward,
    Backward,
    Both,
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
    #[allow(clippy::too_many_lines)]
    pub(crate) fn auto_trace_from(&mut self, pixel_hint: Pos2) {
        if self.image.image.is_none() {
            self.set_status(match self.ui.language {
                UiLanguage::En => "Auto-trace requires an image.",
                UiLanguage::Ru => "Для авто-трассировки нужно изображение.",
            });
            return;
        }
        if !self.calibration_ready() {
            self.set_status(match self.ui.language {
                UiLanguage::En => "Auto-trace requires completed calibration.",
                UiLanguage::Ru => "Для авто-трассировки нужна завершённая калибровка.",
            });
            return;
        }
        if !matches!(self.calibration.coord_system, CoordSystem::Cartesian) {
            self.set_status(match self.ui.language {
                UiLanguage::En => "Auto-trace currently supports Cartesian calibration only.",
                UiLanguage::Ru => {
                    "Авто-трассировка сейчас поддерживает только декартову калибровку."
                }
            });
            return;
        }
        let Some(behavior) = self.current_snap_behavior() else {
            self.set_status(match self.ui.language {
                UiLanguage::En => "Auto-trace requires snapping (Contrast/Centerline).",
                UiLanguage::Ru => {
                    "Для авто-трассировки нужен режим привязки (Contrast/Centerline)."
                }
            });
            return;
        };
        let cfg = self.interaction.auto_trace_cfg.sanitized();
        let size = self.image.image.as_ref().map_or([0, 0], |img| img.size);
        let axis_dir = self
            .calibration
            .cal_x
            .mapping()
            .and_then(|map| {
                let (p1, p2) = map.endpoints();
                let delta = p2 - p1;
                let len = delta.length();
                if len <= f32::EPSILON {
                    return None;
                }
                let mut dir = delta / len;
                let (v1, v2) = map.values();
                if v2.to_scalar_seconds() < v1.to_scalar_seconds() {
                    dir = -dir;
                }
                Some(dir)
            })
            .unwrap_or(Vec2::X);
        let Some(start) = self.find_snap_point_with_radius(pixel_hint, cfg.search_radius, behavior)
        else {
            self.set_status(match self.ui.language {
                UiLanguage::En => "Auto-trace failed: no snap candidate near the click.",
                UiLanguage::Ru => {
                    "Авто-трассировка не удалась: рядом с кликом нет кандидата привязки."
                }
            });
            return;
        };

        let walk = TraceWalk {
            axis_dir,
            size,
            cfg,
        };
        let mut points = walk.collect(start, |probe| {
            self.find_snap_point_with_radius(probe, cfg.search_radius, behavior)
        });

        retain_separated_points(&mut points, cfg.dedup_radius);

        if points.is_empty() {
            self.set_status(match self.ui.language {
                UiLanguage::En => "Auto-trace found no points.",
                UiLanguage::Ru => "Авто-трассировка не нашла точек.",
            });
            return;
        }

        let added = points.len();
        self.points
            .points
            .extend(points.into_iter().map(PickedPoint::new));
        self.mark_points_dirty();
        self.set_status(self.i18n().format_auto_trace_added(added));
    }
}

#[derive(Clone, Copy)]
enum TraceSense {
    Forward,
    Backward,
}

impl TraceSense {
    const fn sign(self) -> f32 {
        match self {
            Self::Forward => 1.0,
            Self::Backward => -1.0,
        }
    }
}

/// Геометрия прохода не зависит от UI; поиск кандидата передаётся вызывающей стороной.
struct TraceWalk {
    axis_dir: Vec2,
    size: [usize; 2],
    cfg: AutoTraceConfig,
}

impl TraceWalk {
    fn collect(&self, start: Pos2, mut find: impl FnMut(Pos2) -> Option<Pos2>) -> Vec<Pos2> {
        let mut points = Vec::new();
        match self.cfg.direction {
            AutoTraceDirection::Forward => {
                points.push(start);
                self.extend(start, TraceSense::Forward, &mut points, &mut find);
            }
            AutoTraceDirection::Backward => {
                points.push(start);
                self.extend(start, TraceSense::Backward, &mut points, &mut find);
            }
            AutoTraceDirection::Both => {
                self.extend(start, TraceSense::Backward, &mut points, &mut find);
                points.reverse();
                points.push(start);
                self.extend(start, TraceSense::Forward, &mut points, &mut find);
            }
        }
        points
    }

    fn extend(
        &self,
        start: Pos2,
        sense: TraceSense,
        points: &mut Vec<Pos2>,
        find: &mut impl FnMut(Pos2) -> Option<Pos2>,
    ) {
        let [width, height] = self.size;
        if width == 0 || height == 0 {
            return;
        }
        let cfg = self.cfg;
        let mut anchor = start;
        let mut probe = start;
        let mut misses = 0u32;
        let sign = sense.sign();
        let step = self.axis_dir * cfg.step_px * sign;
        let max_x = safe_usize_to_f32(width - 1);
        let max_y = safe_usize_to_f32(height - 1);

        for _ in 0..cfg.max_points {
            probe += step;
            if probe.x < 0.0 || probe.x > max_x || probe.y < 0.0 || probe.y > max_y {
                break;
            }
            if let Some(pos) = find(probe) {
                let progress = (pos - anchor).dot(self.axis_dir) * sign;
                let rejected =
                    progress < cfg.min_advance || (pos - anchor).length() <= cfg.dedup_radius;
                if !rejected {
                    points.push(pos);
                    anchor = pos;
                    probe = anchor;
                    misses = 0;
                    continue;
                }
            }
            misses = misses.saturating_add(1);
            if misses > cfg.max_misses {
                break;
            }
        }
    }
}

fn retain_separated_points(points: &mut Vec<Pos2>, radius: f32) {
    let mut last = None;
    points.retain(|&point| {
        if last.is_none_or(|last: Pos2| (last - point).length() > radius) {
            last = Some(point);
            true
        } else {
            false
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    fn trace_walk(direction: AutoTraceDirection) -> TraceWalk {
        TraceWalk {
            axis_dir: Vec2::X,
            size: [11, 11],
            cfg: AutoTraceConfig {
                direction,
                step_px: 2.0,
                ..AutoTraceConfig::default()
            },
        }
    }

    #[test]
    fn tracing_preserves_direction_order_and_image_boundaries() {
        for (direction, expected, expected_probes) in [
            (
                AutoTraceDirection::Forward,
                vec![5.0, 7.0, 9.0],
                vec![7.0, 9.0],
            ),
            (
                AutoTraceDirection::Backward,
                vec![5.0, 3.0, 1.0],
                vec![3.0, 1.0],
            ),
            (
                AutoTraceDirection::Both,
                vec![1.0, 3.0, 5.0, 7.0, 9.0],
                vec![3.0, 1.0, 7.0, 9.0],
            ),
        ] {
            let walk = trace_walk(direction);
            let mut probes = Vec::new();
            let points = walk.collect(pos2(5.0, 1.0), |probe| {
                probes.push(probe.x);
                Some(probe)
            });
            assert_eq!(
                points,
                expected
                    .into_iter()
                    .map(|x| pos2(x, 1.0))
                    .collect::<Vec<_>>()
            );
            assert_eq!(probes, expected_probes);
        }
    }

    #[test]
    fn tracing_resets_misses_on_success_and_counts_rejected_candidates() {
        let mut walk = trace_walk(AutoTraceDirection::Forward);
        walk.cfg.max_misses = 1;
        let mut candidates = [
            None,
            Some(pos2(4.0, 1.0)),
            Some(pos2(3.0, 1.0)),
            Some(pos2(5.0, 1.0)),
        ]
        .into_iter();
        let mut probes = Vec::new();
        let points = walk.collect(pos2(0.0, 1.0), |probe| {
            probes.push(probe.x);
            candidates.next().expect("unexpected extra probe")
        });
        assert_eq!(points, [pos2(0.0, 1.0), pos2(4.0, 1.0)]);
        assert_eq!(probes, [2.0, 4.0, 6.0, 8.0]);
    }

    #[test]
    fn tracing_limits_attempts_even_when_candidates_are_missing() {
        let mut walk = trace_walk(AutoTraceDirection::Forward);
        walk.cfg.max_points = 3;
        let mut attempts = 0;
        let points = walk.collect(pos2(0.0, 1.0), |_| {
            attempts += 1;
            None
        });
        assert_eq!(attempts, 3);
        assert_eq!(points, [pos2(0.0, 1.0)]);
    }

    #[test]
    fn tracing_follows_rotated_axis() {
        let mut walk = trace_walk(AutoTraceDirection::Both);
        walk.axis_dir = -Vec2::Y;
        let points = walk.collect(pos2(1.0, 5.0), Some);
        assert_eq!(
            points,
            [
                pos2(1.0, 9.0),
                pos2(1.0, 7.0),
                pos2(1.0, 5.0),
                pos2(1.0, 3.0),
                pos2(1.0, 1.0)
            ]
        );
    }

    #[test]
    fn deduplication_compares_with_last_retained_point() {
        let mut points = vec![
            pos2(0.0, 0.0),
            pos2(1.0, 0.0),
            pos2(2.0, 0.0),
            pos2(3.5, 0.0),
            pos2(4.0, 0.0),
        ];
        retain_separated_points(&mut points, 1.5);
        assert_eq!(points, [pos2(0.0, 0.0), pos2(2.0, 0.0), pos2(4.0, 0.0)]);
    }

    #[test]
    fn deduplication_handles_empty_and_repeated_points() {
        let mut points = Vec::new();
        retain_separated_points(&mut points, 0.0);
        assert_eq!(points, [] as [Pos2; 0]);
        points.extend([pos2(2.0, 3.0); 3]);
        retain_separated_points(&mut points, 0.0);
        assert_eq!(points, [pos2(2.0, 3.0)]);
    }
}
