use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::types::{AxisUnit, ScaleKind};

/// Image transform operation that can be replayed.
#[derive(Debug, Clone, Copy)]
pub enum ImageTransformOp {
    RotateCw,
    RotateCcw,
    FlipHorizontal,
    FlipVertical,
}

/// Accumulated rotation/flip state for the loaded image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageTransformRecord {
    pub rotation_quarters: u8,
    pub reflected: bool,
}

impl ImageTransformRecord {
    /// Identity transform (no rotation or reflection).
    pub const fn identity() -> Self {
        Self {
            rotation_quarters: 0,
            reflected: false,
        }
    }

    /// Apply a single transform operation to the accumulated state.
    pub const fn apply(&mut self, op: ImageTransformOp) {
        match op {
            ImageTransformOp::RotateCw => {
                self.rotation_quarters = (self.rotation_quarters + 1) % 4;
            }
            ImageTransformOp::RotateCcw => {
                self.rotation_quarters = (self.rotation_quarters + 3) % 4;
            }
            ImageTransformOp::FlipHorizontal => {
                self.rotation_quarters = (4 - self.rotation_quarters % 4) % 4;
                self.reflected = !self.reflected;
            }
            ImageTransformOp::FlipVertical => {
                self.rotation_quarters = (2 + 4 - self.rotation_quarters % 4) % 4;
                self.reflected = !self.reflected;
            }
        }
    }

    /// Expand stored state into a sequence of operations to reapply.
    pub fn replay_operations(self) -> Vec<ImageTransformOp> {
        let mut ops = Vec::new();
        for _ in 0..(self.rotation_quarters % 4) {
            ops.push(ImageTransformOp::RotateCw);
        }
        if self.reflected {
            ops.push(ImageTransformOp::FlipHorizontal);
        }
        ops
    }
}

/// Saved calibration data for a single axis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisCalibrationRecord {
    /// Unit for numeric values along the axis.
    pub unit: AxisUnit,
    /// Scale type for the axis.
    pub scale: ScaleKind,
    /// First calibration point in pixels.
    pub p1: Option<[f32; 2]>,
    /// Second calibration point in pixels.
    pub p2: Option<[f32; 2]>,
    /// Raw text entered for the first calibration value.
    pub v1_text: String,
    /// Raw text entered for the second calibration value.
    pub v2_text: String,
}

/// Full calibration across both axes plus overlay flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationRecord {
    /// X-axis calibration.
    pub x: AxisCalibrationRecord,
    /// Y-axis calibration.
    pub y: AxisCalibrationRecord,
    /// Whether angle snapping is enabled while picking calibration points.
    pub calibration_angle_snap: bool,
    /// Whether to draw calibration lines/labels on the image.
    pub show_calibration_segments: bool,
}

/// Saved point with pixel coordinates and computed values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointRecord {
    /// Point position in image pixels.
    pub pixel: [f32; 2],
    /// Numeric X value (if calibration available).
    pub x_numeric: Option<f64>,
    /// Numeric Y value (if calibration available).
    pub y_numeric: Option<f64>,
}

/// Version 1 project payload (before compression).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPayload {
    /// Absolute path to the source image.
    pub absolute_image_path: PathBuf,
    /// Relative path to the source image (optional fallback).
    pub relative_image_path: Option<PathBuf>,
    /// CRC32 checksum of the image file.
    pub image_crc32: u32,
    /// Stored image transform state.
    pub transform: ImageTransformRecord,
    /// Calibration data for both axes.
    pub calibration: CalibrationRecord,
    /// Stored points.
    pub points: Vec<PointRecord>,
    /// Last zoom level.
    pub zoom: f32,
    /// Last pan offset of the scroll area.
    pub pan: [f32; 2],
    /// Reserved project title.
    pub title: Option<String>,
    /// Reserved project description.
    pub description: Option<String>,
}

/// Where the image path was resolved from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePathSource {
    Absolute,
    Relative,
}

/// Details about the resolved image when loading a project.
#[derive(Debug)]
pub struct ResolvedImage {
    /// Path chosen for loading the image.
    pub path: PathBuf,
    /// Whether the checksum matched the expected value.
    pub checksum_matches: bool,
    /// Source of the path (absolute vs relative).
    pub source: ImagePathSource,
    /// Actual checksum of the chosen image (if computed).
    pub actual_checksum: Option<u32>,
}

/// Warnings collected while resolving paths or checksums.
#[derive(Debug)]
pub enum ProjectWarning {
    MissingImage {
        /// Path that failed to resolve.
        path: PathBuf,
        /// Path source (absolute or relative).
        source: ImagePathSource,
        /// OS error or reason for failure.
        reason: String,
    },
    ChecksumMismatch {
        /// Path that was found on disk.
        path: PathBuf,
        /// Path source (absolute or relative).
        source: ImagePathSource,
        /// Expected CRC32 from the project file.
        expected: u32,
        /// Actual CRC32 of the file.
        actual: u32,
    },
}

/// Result of parsing a project file.
#[derive(Debug)]
pub struct ProjectLoadOutcome {
    /// Parsed project payload.
    pub payload: ProjectPayload,
    /// Image path that will be used for loading.
    pub chosen_image: ResolvedImage,
    /// Warnings raised during resolution.
    pub warnings: Vec<ProjectWarning>,
    /// Project format version read from the file.
    pub version: u32,
}
