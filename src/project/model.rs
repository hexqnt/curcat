use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::image::ImageTransformRecord;
use crate::types::{AngleDirection, AngleUnit, AxisUnit, CoordSystem, ScaleKind};

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

/// Saved calibration data for polar coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolarCalibrationRecord {
    /// Origin point in pixels.
    pub origin: Option<[f32; 2]>,
    /// Radius calibration data.
    pub radius: AxisCalibrationRecord,
    /// Angle calibration data.
    pub angle: AxisCalibrationRecord,
    /// Angle unit for calibration values.
    pub angle_unit: AngleUnit,
    /// Direction of increasing angle.
    pub angle_direction: AngleDirection,
}

impl Default for PolarCalibrationRecord {
    fn default() -> Self {
        Self {
            origin: None,
            radius: AxisCalibrationRecord {
                unit: AxisUnit::Float,
                scale: ScaleKind::Linear,
                p1: None,
                p2: None,
                v1_text: String::new(),
                v2_text: String::new(),
            },
            angle: AxisCalibrationRecord {
                unit: AxisUnit::Float,
                scale: ScaleKind::Linear,
                p1: None,
                p2: None,
                v1_text: String::new(),
                v2_text: String::new(),
            },
            angle_unit: AngleUnit::Degrees,
            angle_direction: AngleDirection::Cw,
        }
    }
}

/// Full calibration across both axes plus overlay flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationRecord {
    /// Active coordinate system.
    pub coord_system: CoordSystem,
    /// X-axis calibration.
    pub x: AxisCalibrationRecord,
    /// Y-axis calibration.
    pub y: AxisCalibrationRecord,
    /// Polar calibration (origin, radius, angle).
    pub polar: PolarCalibrationRecord,
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

/// Version 1 calibration payload (cartesian only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationRecordV1 {
    pub x: AxisCalibrationRecord,
    pub y: AxisCalibrationRecord,
    pub calibration_angle_snap: bool,
    pub show_calibration_segments: bool,
}

/// Version 1 project payload (before polar support).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPayloadV1 {
    pub absolute_image_path: PathBuf,
    pub relative_image_path: Option<PathBuf>,
    pub image_crc32: u32,
    pub transform: ImageTransformRecord,
    pub calibration: CalibrationRecordV1,
    pub points: Vec<PointRecord>,
    pub zoom: f32,
    pub pan: [f32; 2],
    pub title: Option<String>,
    pub description: Option<String>,
}

impl From<ProjectPayloadV1> for ProjectPayload {
    fn from(v1: ProjectPayloadV1) -> Self {
        Self {
            absolute_image_path: v1.absolute_image_path,
            relative_image_path: v1.relative_image_path,
            image_crc32: v1.image_crc32,
            transform: v1.transform,
            calibration: CalibrationRecord {
                coord_system: CoordSystem::Cartesian,
                x: v1.calibration.x,
                y: v1.calibration.y,
                polar: PolarCalibrationRecord::default(),
                calibration_angle_snap: v1.calibration.calibration_angle_snap,
                show_calibration_segments: v1.calibration.show_calibration_segments,
            },
            points: v1.points,
            zoom: v1.zoom,
            pan: v1.pan,
            title: v1.title,
            description: v1.description,
        }
    }
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
