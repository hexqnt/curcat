use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{AxisUnit, ScaleKind};

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
