/// Image feature source used to score candidate pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapFeatureSource {
    LumaGradient,
    ColorMatch,
    Hybrid,
}

impl SnapFeatureSource {
    /// Ordered list of feature sources exposed in the UI.
    pub const ALL: [Self; 3] = [Self::LumaGradient, Self::ColorMatch, Self::Hybrid];

    /// Human-friendly label for UI display.
    pub const fn label(self) -> &'static str {
        match self {
            Self::LumaGradient => "Luma gradient",
            Self::ColorMatch => "Color mask",
            Self::Hybrid => "Gradient + color",
        }
    }
}

/// Threshold interpretation when accepting snap candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapThresholdKind {
    Gradient,
    Score,
}

impl SnapThresholdKind {
    /// Human-friendly label for UI display.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Gradient => "Gradient only",
            Self::Score => "Feature score",
        }
    }
}

/// Behavior configuration for snapping (contrast or centerline).
#[derive(Debug, Clone, Copy)]
pub enum SnapBehavior {
    Contrast {
        feature_source: SnapFeatureSource,
        threshold_kind: SnapThresholdKind,
        threshold: f32,
    },
    Centerline {
        threshold: f32,
    },
}

impl SnapBehavior {
    pub(super) fn feature_strength(self, gradient: f32, color_similarity: f32) -> f32 {
        match self {
            Self::Contrast { feature_source, .. } => match feature_source {
                SnapFeatureSource::LumaGradient => gradient.clamp(0.0, 255.0),
                SnapFeatureSource::ColorMatch => (color_similarity * 255.0).clamp(0.0, 255.0),
                SnapFeatureSource::Hybrid => {
                    let grad_strength = gradient.clamp(0.0, 255.0);
                    let color_strength = (color_similarity * 255.0).clamp(0.0, 255.0);
                    0.6f32.mul_add(grad_strength, 0.4 * color_strength)
                }
            },
            Self::Centerline { .. } => {
                let color_strength = (color_similarity * 255.0).clamp(0.0, 255.0);
                if color_strength <= f32::EPSILON {
                    return 0.0;
                }
                let grad_norm = (gradient / 255.0).clamp(0.0, 1.0);
                color_strength * (1.0 - grad_norm)
            }
        }
    }

    pub(super) fn threshold_passes(self, gradient: f32, feature_strength: f32) -> bool {
        match self {
            Self::Contrast {
                threshold_kind,
                threshold,
                ..
            } => match threshold_kind {
                SnapThresholdKind::Gradient => gradient >= threshold,
                SnapThresholdKind::Score => feature_strength >= threshold,
            },
            Self::Centerline { threshold } => feature_strength >= threshold,
        }
    }
}
