//! Multi-scale snapping helpers for locating curve pixels near a cursor.

mod behavior;
mod color;
mod maps;
mod search;
mod util;

pub use behavior::{SnapBehavior, SnapFeatureSource, SnapThresholdKind};
pub use maps::SnapMapCache;
