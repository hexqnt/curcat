use anyhow::{Context as _, anyhow};
use pathdiff::diff_paths;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::checksum::compute_image_crc32;
use super::model::{ImagePathSource, ProjectPayload, ProjectWarning, ResolvedImage};

/// Build an image path relative to the project file location.
pub fn make_relative_image_path(project_path: &Path, image_path: &Path) -> Option<PathBuf> {
    let project_dir = project_path.parent()?;
    diff_paths(image_path, project_dir)
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

pub(super) fn write_atomic(path: &Path, data: &[u8]) -> anyhow::Result<()> {
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

pub(super) fn resolve_image_path(
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
