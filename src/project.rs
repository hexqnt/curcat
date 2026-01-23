use anyhow::{Context as _, anyhow, bail};
use bincode::config::{Config, standard};
use crc32fast::Hasher;
use lz4_flex::block::{compress_prepend_size, decompress_size_prepended};
use pathdiff::diff_paths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{AxisUnit, ScaleKind};

/// Magic signature prefix for project files
pub const PROJECT_MAGIC: &[u8; 6] = b"CURCAT";
/// Current binary project format version.
pub const PROJECT_VERSION: u32 = 1;

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

fn bincode_config() -> impl Config {
    standard().with_little_endian()
}

/// Compute CRC32 of an image file.
pub fn compute_image_crc32(path: &Path) -> anyhow::Result<u32> {
    let mut file = fs::File::open(path)
        .with_context(|| format!("Failed to open image file for checksum: {}", path.display()))?;
    let mut hasher = Hasher::new();
    let mut buf = vec![0u8; 32 * 1024].into_boxed_slice();
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher.finalize())
}

/// Build an image path relative to the project file location.
pub fn make_relative_image_path(project_path: &Path, image_path: &Path) -> Option<PathBuf> {
    let project_dir = project_path.parent()?;
    diff_paths(image_path, project_dir)
}

fn encode_payload(payload: &ProjectPayload) -> anyhow::Result<Vec<u8>> {
    bincode::serde::encode_to_vec(payload, bincode_config())
        .context("Failed to serialize project payload")
}

fn decode_payload(bytes: &[u8]) -> anyhow::Result<ProjectPayload> {
    let (payload, _): (ProjectPayload, usize) =
        bincode::serde::decode_from_slice(bytes, bincode_config())
            .context("Failed to deserialize project payload")?;
    Ok(payload)
}

fn build_temp_path(target: &Path) -> PathBuf {
    let parent = target
        .parent()
        .map_or_else(|| Path::new(".").to_path_buf(), Path::to_path_buf);
    let base = target.file_name().map_or_else(
        || "curcat_project".to_string(),
        |s| s.to_string_lossy().into_owned(),
    );
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut candidate = parent.join(format!(".{base}.{nanos}.tmp"));
    let mut counter = 0u32;
    while candidate.exists() {
        counter = counter.wrapping_add(1);
        candidate = parent.join(format!(".{base}.{nanos}.{counter}.tmp"));
    }
    candidate
}

fn replace_file(tmp_path: &Path, target: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        match fs::rename(tmp_path, target) {
            Ok(()) => Ok(()),
            Err(err) => {
                if matches!(
                    err.kind(),
                    io::ErrorKind::AlreadyExists | io::ErrorKind::PermissionDenied
                ) && target.exists()
                {
                    let _ = fs::remove_file(target);
                    fs::rename(tmp_path, target)
                } else {
                    Err(err)
                }
            }
        }
    }
    #[cfg(not(windows))]
    {
        fs::rename(tmp_path, target)
    }
}

fn write_atomic(path: &Path, data: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("Failed to create {}", parent.display()))?;
    let tmp_path = build_temp_path(path);
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .with_context(|| format!("Failed to create temp file {}", tmp_path.display()))?;
        file.write_all(data)
            .with_context(|| format!("Failed to write {}", tmp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("Failed to sync {}", tmp_path.display()))?;
    }
    let rename_result = replace_file(&tmp_path, path)
        .with_context(|| format!("Failed to replace {} with temp file", path.display()));
    if rename_result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    rename_result
}

/// Save a project with compression and an atomic temp-file swap.
pub fn save_project(path: &Path, payload: &ProjectPayload) -> anyhow::Result<()> {
    let encoded = encode_payload(payload)?;
    let compressed = compress_prepend_size(&encoded);
    let mut buffer = Vec::with_capacity(PROJECT_MAGIC.len() + 4 + compressed.len());
    buffer.extend_from_slice(PROJECT_MAGIC);
    buffer.extend_from_slice(&PROJECT_VERSION.to_le_bytes());
    buffer.extend_from_slice(&compressed);
    write_atomic(path, &buffer)
}

fn resolve_image_path(
    project_path: &Path,
    payload: &ProjectPayload,
) -> anyhow::Result<(ResolvedImage, Vec<ProjectWarning>)> {
    let mut warnings = Vec::new();
    let project_dir = project_path.parent().unwrap_or_else(|| Path::new("."));
    let expected_crc = payload.image_crc32;
    let mut candidates: Vec<(PathBuf, ImagePathSource)> = Vec::new();
    candidates.push((
        payload.absolute_image_path.clone(),
        ImagePathSource::Absolute,
    ));
    if let Some(rel) = payload.relative_image_path.as_ref() {
        candidates.push((project_dir.join(rel), ImagePathSource::Relative));
    }

    let mut chosen: Option<ResolvedImage> = None;

    for (path, source) in candidates {
        match compute_image_crc32(&path) {
            Ok(actual_crc) => {
                let checksum_matches = actual_crc == expected_crc;
                if checksum_matches {
                    return Ok((
                        ResolvedImage {
                            path,
                            checksum_matches: true,
                            source,
                            actual_checksum: Some(actual_crc),
                        },
                        warnings,
                    ));
                }
                warnings.push(ProjectWarning::ChecksumMismatch {
                    path: path.clone(),
                    source,
                    expected: expected_crc,
                    actual: actual_crc,
                });
                if chosen.is_none() {
                    chosen = Some(ResolvedImage {
                        path,
                        checksum_matches: false,
                        source,
                        actual_checksum: Some(actual_crc),
                    });
                }
            }
            Err(err) => {
                warnings.push(ProjectWarning::MissingImage {
                    path: path.clone(),
                    source,
                    reason: err.to_string(),
                });
            }
        }
    }

    chosen
        .map(|resolved| (resolved, warnings))
        .ok_or_else(|| anyhow!("Referenced image not found by absolute or relative path"))
}

/// Load a project: validate header, decompress, and resolve the image path.
pub fn load_project(path: &Path) -> anyhow::Result<ProjectLoadOutcome> {
    let bytes =
        fs::read(path).with_context(|| format!("Failed to read project {}", path.display()))?;
    let header_len = PROJECT_MAGIC.len() + std::mem::size_of::<u32>();
    if bytes.len() < header_len {
        bail!("Project file too small or missing header");
    }
    let (magic, rest) = bytes.split_at(PROJECT_MAGIC.len());
    if magic != PROJECT_MAGIC {
        bail!("Not a Curcat project file: magic signature mismatch");
    }
    let (version_bytes, compressed) = rest.split_at(std::mem::size_of::<u32>());
    let version = u32::from_le_bytes(version_bytes.try_into().unwrap_or_default());
    if version != PROJECT_VERSION {
        bail!("Unsupported project version {version}. Supported version: {PROJECT_VERSION}");
    }
    let decompressed =
        decompress_size_prepended(compressed).context("Failed to decompress project payload")?;
    let payload = decode_payload(&decompressed)?;
    let (chosen_image, warnings) = resolve_image_path(path, &payload)?;
    Ok(ProjectLoadOutcome {
        payload,
        chosen_image,
        warnings,
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("curcat_{label}_{nanos}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn sample_payload(image_path: &Path, image_crc32: u32) -> ProjectPayload {
        ProjectPayload {
            absolute_image_path: image_path.to_path_buf(),
            relative_image_path: image_path.file_name().map(PathBuf::from),
            image_crc32,
            transform: ImageTransformRecord::identity(),
            calibration: CalibrationRecord {
                x: AxisCalibrationRecord {
                    unit: AxisUnit::Float,
                    scale: ScaleKind::Linear,
                    p1: Some([0.0, 0.0]),
                    p2: Some([10.0, 0.0]),
                    v1_text: "0".to_string(),
                    v2_text: "10".to_string(),
                },
                y: AxisCalibrationRecord {
                    unit: AxisUnit::Float,
                    scale: ScaleKind::Linear,
                    p1: Some([0.0, 0.0]),
                    p2: Some([0.0, 10.0]),
                    v1_text: "0".to_string(),
                    v2_text: "10".to_string(),
                },
                calibration_angle_snap: false,
                show_calibration_segments: true,
            },
            points: vec![PointRecord {
                pixel: [1.0, 2.0],
                x_numeric: Some(1.0),
                y_numeric: Some(2.0),
            }],
            zoom: 1.0,
            pan: [0.0, 0.0],
            title: Some("Test".to_string()),
            description: Some("Project roundtrip".to_string()),
        }
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = unique_temp_dir("roundtrip");
        let image_path = dir.join("image.bin");
        fs::write(&image_path, b"image-bytes").expect("write image");
        let crc = compute_image_crc32(&image_path).expect("checksum");
        let payload = sample_payload(&image_path, crc);
        let project_path = dir.join("project.curcat");
        save_project(&project_path, &payload).expect("save project");

        let outcome = load_project(&project_path).expect("load project");
        assert!(outcome.warnings.is_empty());
        assert_eq!(outcome.payload.image_crc32, payload.image_crc32);
        assert_eq!(outcome.payload.points.len(), payload.points.len());
        assert_eq!(outcome.chosen_image.path, image_path);
        assert!(outcome.chosen_image.checksum_matches);
    }

    #[test]
    fn load_warns_on_checksum_mismatch() {
        let dir = unique_temp_dir("checksum");
        let image_path = dir.join("image.bin");
        fs::write(&image_path, b"original").expect("write image");
        let crc = compute_image_crc32(&image_path).expect("checksum");
        let payload = sample_payload(&image_path, crc);
        let project_path = dir.join("project.curcat");
        save_project(&project_path, &payload).expect("save project");

        fs::write(&image_path, b"modified").expect("modify image");
        let outcome = load_project(&project_path).expect("load project");
        assert!(!outcome.chosen_image.checksum_matches);
        assert!(
            outcome
                .warnings
                .iter()
                .any(|w| matches!(w, ProjectWarning::ChecksumMismatch { .. }))
        );
    }

    #[test]
    fn replay_operations_restores_transform() {
        let mut record = ImageTransformRecord::identity();
        let ops = [
            ImageTransformOp::RotateCw,
            ImageTransformOp::FlipHorizontal,
            ImageTransformOp::RotateCcw,
            ImageTransformOp::FlipVertical,
        ];
        for op in ops {
            record.apply(op);
        }
        let replay = record.replay_operations();
        let mut rebuilt = ImageTransformRecord::identity();
        for op in replay {
            rebuilt.apply(op);
        }
        assert_eq!(rebuilt, record);
    }
}
