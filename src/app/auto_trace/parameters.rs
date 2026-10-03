//! Editable trace settings and bounded parameters for the tracing algorithm.

use super::AutoTraceDirection;
use crate::util::clamp_f32_or_default;

/// Editable UI settings; the algorithm receives only normalized `TraceParameters`.
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

/// Finite distances and a nonzero search budget, constructed only by normalizing UI settings.
#[derive(Debug, Clone, Copy)]
pub(super) struct TraceParameters {
    direction: AutoTraceDirection,
    step_px: f32,
    search_radius_px: f32,
    max_steps_per_direction: usize,
    max_consecutive_misses: u32,
    min_advance_px: f32,
    min_spacing_px: f32,
}

impl From<AutoTraceConfig> for TraceParameters {
    fn from(config: AutoTraceConfig) -> Self {
        let defaults = AutoTraceConfig::default();
        let step_px = clamp_f32_or_default(config.step_px, 1.0, 80.0, defaults.step_px);
        Self {
            direction: config.direction,
            step_px,
            search_radius_px: clamp_f32_or_default(
                config.search_radius,
                2.0,
                120.0,
                defaults.search_radius,
            ),
            max_steps_per_direction: config.max_points.clamp(2, 20_000),
            max_consecutive_misses: config.max_misses.min(1_000),
            min_advance_px: clamp_f32_or_default(
                config.min_advance,
                0.1,
                step_px,
                defaults.min_advance,
            ),
            min_spacing_px: clamp_f32_or_default(
                config.dedup_radius,
                0.0,
                50.0,
                defaults.dedup_radius,
            ),
        }
    }
}

impl TraceParameters {
    pub(super) const fn direction(&self) -> AutoTraceDirection {
        self.direction
    }

    pub(super) const fn step_px(&self) -> f32 {
        self.step_px
    }

    pub(super) const fn search_radius_px(&self) -> f32 {
        self.search_radius_px
    }

    /// Count search attempts per direction, including misses; the initial point is outside this budget.
    pub(super) const fn max_steps_per_direction(&self) -> usize {
        self.max_steps_per_direction
    }

    pub(super) const fn max_consecutive_misses(&self) -> u32 {
        self.max_consecutive_misses
    }

    pub(super) const fn min_advance_px(&self) -> f32 {
        self.min_advance_px
    }

    pub(super) const fn min_spacing_px(&self) -> f32 {
        self.min_spacing_px
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nan_distances_use_defaults_and_zero_budget_stays_nonzero() {
        let parameters = TraceParameters::from(AutoTraceConfig {
            step_px: f32::NAN,
            search_radius: f32::NAN,
            min_advance: f32::NAN,
            dedup_radius: f32::NAN,
            max_points: 0,
            ..AutoTraceConfig::default()
        });
        assert_eq!(parameters.step_px(), 6.0);
        assert_eq!(parameters.search_radius_px(), 12.0);
        assert_eq!(parameters.min_advance_px(), 1.0);
        assert_eq!(parameters.min_spacing_px(), 1.5);
        assert_eq!(parameters.max_steps_per_direction(), 2);
    }

    #[test]
    fn extreme_settings_are_bounded_and_advance_does_not_exceed_step() {
        let parameters = TraceParameters::from(AutoTraceConfig {
            step_px: f32::NEG_INFINITY,
            search_radius: f32::INFINITY,
            min_advance: f32::INFINITY,
            dedup_radius: f32::NEG_INFINITY,
            max_points: usize::MAX,
            max_misses: u32::MAX,
            ..AutoTraceConfig::default()
        });
        assert_eq!(parameters.step_px(), 1.0);
        assert_eq!(parameters.search_radius_px(), 120.0);
        assert_eq!(parameters.min_advance_px(), 1.0);
        assert_eq!(parameters.min_spacing_px(), 0.0);
        assert_eq!(parameters.max_steps_per_direction(), 20_000);
        assert_eq!(parameters.max_consecutive_misses(), 1_000);
    }
}
