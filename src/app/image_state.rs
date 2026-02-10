use crate::image::{ImageFilters, ImageMeta, ImageTransformRecord, LoadedImage};
use egui::{ColorImage, Pos2, Vec2};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy)]
pub enum ZoomAnchor {
    ViewportCenter,
    ViewportPos(Pos2),
}

#[derive(Debug, Clone, Copy)]
pub enum ZoomIntent {
    Anchor(ZoomAnchor),
    TargetPan(Vec2),
}

pub enum ImageLoadRequest {
    Path(PathBuf),
    Bytes(Vec<u8>),
}

pub struct PendingImageTask {
    pub(super) rx: Receiver<ImageLoadResult>,
    pub(super) meta: PendingImageMeta,
}

pub enum ImageLoadResult {
    Success(ColorImage),
    Error(String),
}

#[derive(Clone)]
pub enum PendingImageMeta {
    Path {
        path: PathBuf,
    },
    DroppedBytes {
        name: Option<String>,
        byte_len: usize,
        last_modified: Option<SystemTime>,
    },
}

impl PendingImageMeta {
    pub(super) fn description(&self) -> String {
        match self {
            Self::Path { path } => path
                .file_name()
                .and_then(|s| s.to_str())
                .map_or_else(|| path.display().to_string(), str::to_string),
            Self::DroppedBytes { name, .. } => name
                .as_deref()
                .map_or_else(|| "dropped bytes".to_string(), str::to_string),
        }
    }

    pub(super) fn into_image_meta(self) -> ImageMeta {
        match self {
            Self::Path { path } => ImageMeta::from_path(&path),
            Self::DroppedBytes {
                name,
                byte_len,
                last_modified,
            } => ImageMeta::from_dropped_bytes(name.as_deref(), byte_len, last_modified),
        }
    }
}

pub struct ImageState {
    pub(super) image: Option<LoadedImage>,
    pub(super) base_pixels: Option<ColorImage>,
    pub(super) filters: ImageFilters,
    pub(super) meta: Option<ImageMeta>,
    pub(super) transform: ImageTransformRecord,
    pub(super) pan: Vec2,
    pub(super) last_viewport_size: Option<Vec2>,
    pub(super) skip_pan_sync_once: bool,
    pub(super) pending_fit_on_load: bool,
    pub(super) zoom: f32,
    pub(super) zoom_target: f32,
    pub(super) zoom_intent: ZoomIntent,
    pub(super) touch_pan_active: bool,
    pub(super) touch_pan_last: Option<Pos2>,
}
