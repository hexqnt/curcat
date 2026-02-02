use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Describes where the current image data originated.
#[derive(Debug, Clone)]
pub enum ImageOrigin {
    File(PathBuf),
    DroppedBytes { suggested_name: Option<String> },
    Clipboard,
}

impl ImageOrigin {
    /// Human-readable label for UI display.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::File(_) => "File on disk",
            Self::DroppedBytes { .. } => "Dropped bytes",
            Self::Clipboard => "Clipboard",
        }
    }
}

/// Metadata describing a loaded image and its provenance.
#[derive(Debug, Clone)]
pub struct ImageMeta {
    origin: ImageOrigin,
    byte_len: Option<u64>,
    last_modified: Option<SystemTime>,
}

impl ImageMeta {
    /// Build metadata from a filesystem path (size and modified time when available).
    pub fn from_path(path: &Path) -> Self {
        let metadata = std::fs::metadata(path).ok();
        let (byte_len, last_modified) = metadata.map_or((None, None), |meta| {
            (Some(meta.len()), meta.modified().ok())
        });
        Self {
            origin: ImageOrigin::File(path.to_owned()),
            byte_len,
            last_modified,
        }
    }

    /// Build metadata for dropped bytes with optional name and modification time.
    pub fn from_dropped_bytes(
        name: Option<&str>,
        byte_len: usize,
        last_modified: Option<SystemTime>,
    ) -> Self {
        Self {
            origin: ImageOrigin::DroppedBytes {
                suggested_name: name.filter(|s| !s.is_empty()).map(ToOwned::to_owned),
            },
            byte_len: Some(byte_len as u64),
            last_modified,
        }
    }

    /// Build metadata for a clipboard image.
    pub const fn from_clipboard(byte_len: Option<u64>) -> Self {
        Self {
            origin: ImageOrigin::Clipboard,
            byte_len,
            last_modified: None,
        }
    }

    /// Best-effort display name for the image source.
    pub fn display_name(&self) -> String {
        match &self.origin {
            ImageOrigin::File(path) => path
                .file_name()
                .and_then(|s| s.to_str())
                .map_or_else(|| path.display().to_string(), ToOwned::to_owned),
            ImageOrigin::DroppedBytes { suggested_name } => suggested_name
                .as_deref()
                .map_or_else(|| "Unnamed drop".to_string(), str::to_owned),
            ImageOrigin::Clipboard => "Clipboard image".to_string(),
        }
    }

    /// Filesystem path when the image originated from disk.
    pub fn path(&self) -> Option<&Path> {
        match &self.origin {
            ImageOrigin::File(path) => Some(path.as_path()),
            ImageOrigin::DroppedBytes { .. } | ImageOrigin::Clipboard => None,
        }
    }

    /// Short label describing the origin.
    pub const fn source_label(&self) -> &'static str {
        self.origin.label()
    }

    /// Byte length of the image data when known.
    pub const fn byte_len(&self) -> Option<u64> {
        self.byte_len
    }

    /// Last modification timestamp when known.
    pub const fn last_modified(&self) -> Option<SystemTime> {
        self.last_modified
    }
}

/// Format a byte count with binary units (KiB, MiB, ...).
pub fn human_readable_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit_idx = 0;
    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{bytes} {}", UNITS[unit_idx])
    } else {
        format!("{value:.2} {}", UNITS[unit_idx])
    }
}

/// Format a `SystemTime` as a UTC timestamp string.
pub fn format_system_time(time: SystemTime) -> String {
    let datetime: DateTime<Utc> = DateTime::from(time);
    datetime.format("%Y-%m-%d %H:%M:%S %Z").to_string()
}

/// Return a simplified aspect ratio plus an approximate decimal ratio string.
pub fn describe_aspect_ratio(size: [usize; 2]) -> Option<String> {
    let [w, h] = size;
    if w == 0 || h == 0 {
        return None;
    }
    let divisor = gcd_usize(w, h);
    let simple_w = w / divisor;
    let simple_h = h / divisor;
    let approx = w as f64 / h as f64;
    Some(format!("{simple_w}:{simple_h} (~{approx:.3}:1)"))
}

/// Compute total pixel count with saturating multiplication.
pub fn total_pixel_count(size: [usize; 2]) -> u64 {
    let w = u64::try_from(size[0]).unwrap_or(u64::MAX);
    let h = u64::try_from(size[1]).unwrap_or(u64::MAX);
    w.saturating_mul(h)
}

const fn gcd_usize(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let tmp = a % b;
        a = b;
        b = tmp;
    }
    if a == 0 { 1 } else { a }
}
