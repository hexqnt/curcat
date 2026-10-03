//! Editable auto-placement configuration and normalized runtime parameters.

use crate::util::clamp_f32_or_default;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Parameters that govern auto-placement of points.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoPlaceConfig {
    pub hold_activation_secs: f32,
    pub distance_min: f32,
    pub distance_max: f32,
    pub distance_per_speed: f32,
    pub time_min_secs: f32,
    pub time_max_secs: f32,
    pub time_per_speed: f32,
    pub pause_speed_threshold: f32,
    pub pause_timeout_ms: u32,
    pub dedup_radius: f32,
    pub speed_smoothing: f32,
}

/// Finite auto-placement settings with ordered distance and time bounds; deserialize only `AutoPlaceConfig`.
#[derive(Debug, Clone, Copy)]
pub struct AutoPlaceParameters(AutoPlaceConfig);

impl AutoPlaceParameters {
    pub const fn hold_activation_secs(&self) -> f32 {
        self.0.hold_activation_secs
    }

    pub const fn speed_smoothing(&self) -> f32 {
        self.0.speed_smoothing
    }

    pub fn is_pause_speed(&self, speed: f32) -> bool {
        speed < self.0.pause_speed_threshold
    }

    pub fn pause_timeout(&self) -> Duration {
        Duration::from_millis(u64::from(self.0.pause_timeout_ms))
    }

    /// Require both movement and elapsed time; speed adjusts thresholds within the configured bounds.
    pub fn accepts_next_point(&self, distance: f32, elapsed: Duration, speed: f32) -> bool {
        let config = self.0;
        let distance_threshold =
            (speed * config.distance_per_speed).clamp(config.distance_min, config.distance_max);
        let time_threshold = if speed <= f32::EPSILON {
            config.time_max_secs
        } else {
            (config.time_per_speed / speed).clamp(config.time_min_secs, config.time_max_secs)
        };
        distance >= config.dedup_radius
            && distance >= distance_threshold
            && elapsed.as_secs_f32() >= time_threshold
    }
}

impl Default for AutoPlaceConfig {
    fn default() -> Self {
        Self {
            hold_activation_secs: 1.25,
            distance_min: 2.5,
            distance_max: 24.0,
            distance_per_speed: 0.01,
            time_min_secs: 0.05,
            time_max_secs: 0.28,
            time_per_speed: 28.0,
            pause_speed_threshold: 6.0,
            pause_timeout_ms: 160,
            dedup_radius: 1.5,
            speed_smoothing: 0.25,
        }
    }
}

impl AutoPlaceConfig {
    /// Clamp values to keep auto-placement stable and predictable.
    pub fn sanitized(&self) -> AutoPlaceParameters {
        let defaults = Self::default();
        let hold_activation_secs = clamp_f32_or_default(
            self.hold_activation_secs,
            0.1,
            10.0,
            defaults.hold_activation_secs,
        );
        let distance_min =
            clamp_f32_or_default(self.distance_min, 0.1, 200.0, defaults.distance_min);
        let distance_max = clamp_f32_or_default(
            self.distance_max,
            distance_min,
            1_000.0,
            defaults.distance_max,
        );
        let distance_per_speed = clamp_f32_or_default(
            self.distance_per_speed,
            0.0,
            1.0,
            defaults.distance_per_speed,
        );
        let time_min_secs =
            clamp_f32_or_default(self.time_min_secs, 0.01, 2.0, defaults.time_min_secs);
        let time_max_secs = clamp_f32_or_default(
            self.time_max_secs,
            time_min_secs,
            3.0,
            defaults.time_max_secs,
        );
        let time_per_speed =
            clamp_f32_or_default(self.time_per_speed, 0.1, 1_000.0, defaults.time_per_speed);
        let pause_speed_threshold = clamp_f32_or_default(
            self.pause_speed_threshold,
            0.0,
            1_000.0,
            defaults.pause_speed_threshold,
        );
        let pause_timeout_ms = self.pause_timeout_ms.clamp(0, 10_000);
        let dedup_radius =
            clamp_f32_or_default(self.dedup_radius, 0.0, 200.0, defaults.dedup_radius);
        let speed_smoothing =
            clamp_f32_or_default(self.speed_smoothing, 0.0, 1.0, defaults.speed_smoothing);
        AutoPlaceParameters(Self {
            hold_activation_secs,
            distance_min,
            distance_max,
            distance_per_speed,
            time_min_secs,
            time_max_secs,
            time_per_speed,
            pause_speed_threshold,
            pause_timeout_ms,
            dedup_radius,
            speed_smoothing,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, ExportConfig};

    #[test]
    fn point_acceptance_obeys_speed_adjusted_distance_and_time_boundaries() {
        let parameters = AutoPlaceConfig::default().sanitized();
        for (speed, distance, millis, accepted) in [
            (0.0, 2.49, 1000, false),
            (0.0, 2.5, 279, false),
            (0.0, 2.5, 280, true),
            (1000.0, 9.99, 100, false),
            (1000.0, 10.0, 49, false),
            (1000.0, 10.0, 50, true),
            (10_000.0, 23.99, 1000, false),
            (10_000.0, 24.0, 50, true),
        ] {
            assert_eq!(
                parameters.accepts_next_point(distance, Duration::from_millis(millis), speed),
                accepted,
                "{speed}, {distance}, {millis}"
            );
        }
        assert!(!parameters.accepts_next_point(f32::NAN, Duration::from_secs(1), 0.0));
    }

    #[test]
    fn deduplication_spacing_and_pause_threshold_are_independent_of_speed_limits() {
        let parameters = AutoPlaceConfig {
            dedup_radius: 10.0,
            ..AutoPlaceConfig::default()
        }
        .sanitized();
        assert!(!parameters.accepts_next_point(9.0, Duration::from_secs(1), 0.0));
        assert!(parameters.accepts_next_point(10.0, Duration::from_secs(1), 0.0));
        assert!(parameters.is_pause_speed(5.99));
        assert!(!parameters.is_pause_speed(6.0));
        assert_eq!(parameters.pause_timeout(), Duration::from_millis(160));
    }

    #[test]
    fn nan_config_values_use_defaults_and_keep_dependent_bounds_ordered() {
        let config: AppConfig = toml::from_str(
            r"
[export]
auto_rel_tolerance = nan
[auto_place]
hold_activation_secs = nan
distance_min = 200.0
distance_max = nan
time_min_secs = 2.0
time_max_secs = nan
",
        )
        .unwrap();
        assert_eq!(
            config.export.auto_rel_tolerance_sanitized(),
            f64::from(ExportConfig::default().auto_rel_tolerance)
        );
        let parameters = config.auto_place();
        let sanitized = &parameters.0;
        assert_eq!(
            sanitized.hold_activation_secs,
            AutoPlaceConfig::default().hold_activation_secs
        );
        assert_eq!(sanitized.distance_max, sanitized.distance_min);
        assert_eq!(sanitized.time_max_secs, sanitized.time_min_secs);
        assert_eq!(
            std::time::Duration::from_secs_f32(sanitized.hold_activation_secs),
            std::time::Duration::from_secs_f32(AutoPlaceConfig::default().hold_activation_secs)
        );
    }
}
