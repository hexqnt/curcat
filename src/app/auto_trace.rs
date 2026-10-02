use super::{CalibrationMapping, CurcatApp, PickedPoint};
use crate::i18n::UiLanguage;
use crate::util::safe_usize_to_f32;
use egui::{Pos2, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoTraceDirection {
    Forward,
    Backward,
    Both,
}

mod parameters;

pub use parameters::AutoTraceConfig;
use parameters::TraceParameters;

impl CurcatApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn auto_trace_from(&mut self, pixel_hint: Pos2) {
        let Some(size) = self.image.image.as_ref().map(|image| image.size) else {
            self.set_status(match self.ui.language {
                UiLanguage::En => "Auto-trace requires an image.",
                UiLanguage::Ru => "Для авто-трассировки нужно изображение.",
            });
            return;
        };
        let x_mapping = match self.calibration_mapping() {
            CalibrationMapping::Cartesian {
                x: Some(x),
                y: Some(_),
            } => x,
            CalibrationMapping::Cartesian { .. } | CalibrationMapping::Polar(None) => {
                self.set_status(match self.ui.language {
                    UiLanguage::En => "Auto-trace requires completed calibration.",
                    UiLanguage::Ru => "Для авто-трассировки нужна завершённая калибровка.",
                });
                return;
            }
            CalibrationMapping::Polar(Some(_)) => {
                self.set_status(match self.ui.language {
                    UiLanguage::En => "Auto-trace currently supports Cartesian calibration only.",
                    UiLanguage::Ru => {
                        "Авто-трассировка сейчас поддерживает только декартову калибровку."
                    }
                });
                return;
            }
        };
        let Some(behavior) = self.current_snap_behavior() else {
            self.set_status(match self.ui.language {
                UiLanguage::En => "Auto-trace requires snapping (Contrast/Centerline).",
                UiLanguage::Ru => {
                    "Для авто-трассировки нужен режим привязки (Contrast/Centerline)."
                }
            });
            return;
        };
        let parameters = TraceParameters::from(self.interaction.auto_trace_cfg);
        let axis_dir = x_mapping.increasing_direction();
        let Some(start) =
            self.find_snap_point_with_radius(pixel_hint, parameters.search_radius_px(), behavior)
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
            parameters,
        };
        let mut points = walk.collect(start, |probe| {
            self.find_snap_point_with_radius(probe, parameters.search_radius_px(), behavior)
        });

        retain_separated_points(&mut points, parameters.min_spacing_px());

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

/// Traversal geometry is independent of the UI; the caller supplies candidate search.
struct TraceWalk {
    axis_dir: Vec2,
    size: [usize; 2],
    parameters: TraceParameters,
}

impl TraceWalk {
    fn collect(&self, start: Pos2, mut find: impl FnMut(Pos2) -> Option<Pos2>) -> Vec<Pos2> {
        let mut points = Vec::new();
        match self.parameters.direction() {
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
        let parameters = self.parameters;
        let mut anchor = start;
        let mut probe = start;
        let mut misses = 0u32;
        let sign = sense.sign();
        let step = self.axis_dir * parameters.step_px() * sign;
        let max_x = safe_usize_to_f32(width - 1);
        let max_y = safe_usize_to_f32(height - 1);

        for _ in 0..parameters.max_steps_per_direction() {
            probe += step;
            if probe.x < 0.0 || probe.x > max_x || probe.y < 0.0 || probe.y > max_y {
                break;
            }
            if let Some(pos) = find(probe) {
                let progress = (pos - anchor).dot(self.axis_dir) * sign;
                let rejected = progress < parameters.min_advance_px()
                    || (pos - anchor).length() <= parameters.min_spacing_px();
                if !rejected {
                    points.push(pos);
                    anchor = pos;
                    probe = anchor;
                    misses = 0;
                    continue;
                }
            }
            misses = misses.saturating_add(1);
            if misses > parameters.max_consecutive_misses() {
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
            parameters: AutoTraceConfig {
                direction,
                step_px: 2.0,
                ..AutoTraceConfig::default()
            }
            .into(),
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
        walk.parameters = AutoTraceConfig {
            max_misses: 1,
            step_px: 2.0,
            ..AutoTraceConfig::default()
        }
        .into();
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
        walk.parameters = AutoTraceConfig {
            max_points: 3,
            step_px: 2.0,
            ..AutoTraceConfig::default()
        }
        .into();
        let mut attempts = 0;
        let points = walk.collect(pos2(0.0, 1.0), |_| {
            attempts += 1;
            None
        });
        assert_eq!(attempts, 3);
        assert_eq!(points, [pos2(0.0, 1.0)]);
    }

    #[test]
    fn tracing_budget_is_per_direction_and_excludes_start() {
        let mut walk = trace_walk(AutoTraceDirection::Both);
        walk.parameters = AutoTraceConfig {
            direction: AutoTraceDirection::Both,
            step_px: 2.0,
            max_points: 2,
            ..AutoTraceConfig::default()
        }
        .into();
        let mut probes = Vec::new();
        let points = walk.collect(pos2(5.0, 1.0), |probe| {
            probes.push(probe);
            Some(probe)
        });
        assert_eq!(
            probes,
            [
                pos2(3.0, 1.0),
                pos2(1.0, 1.0),
                pos2(7.0, 1.0),
                pos2(9.0, 1.0)
            ]
        );
        assert_eq!(
            points,
            [
                pos2(1.0, 1.0),
                pos2(3.0, 1.0),
                pos2(5.0, 1.0),
                pos2(7.0, 1.0),
                pos2(9.0, 1.0)
            ]
        );
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
