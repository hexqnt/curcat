use super::{CurcatApp, ImageLoadRequest, ImageLoadResult, PendingImageMeta, PendingImageTask};
use crate::image_util::{LoadedImage, decode_image_from_bytes, decode_image_from_path};
use egui::{ColorImage, Context};
use std::path::Path;
use std::sync::mpsc::{self, TryRecvError};
use std::thread;

impl CurcatApp {
    pub(crate) fn start_loading_image_from_path(&mut self, path: std::path::PathBuf) {
        self.remember_image_dir_from_path(&path);
        let meta = PendingImageMeta::Path { path: path.clone() };
        self.start_image_load(ImageLoadRequest::Path(path), meta);
    }

    pub(crate) fn start_loading_image_from_bytes(
        &mut self,
        name: Option<String>,
        bytes: Vec<u8>,
        last_modified: Option<std::time::SystemTime>,
    ) {
        let meta = PendingImageMeta::DroppedBytes {
            name,
            byte_len: bytes.len(),
            last_modified,
        };
        self.start_image_load(ImageLoadRequest::Bytes(bytes), meta);
    }

    fn start_image_load(&mut self, request: ImageLoadRequest, meta: PendingImageMeta) {
        let description = meta.description();
        let cfg = self.config.clone();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let result = match request {
                ImageLoadRequest::Path(path) => decode_image_from_path(&cfg, &path),
                ImageLoadRequest::Bytes(bytes) => decode_image_from_bytes(&cfg, bytes),
            };
            let msg = match result {
                Ok(color) => ImageLoadResult::Success(color),
                Err(err) => ImageLoadResult::Error(err.to_string()),
            };
            let _ = tx.send(msg);
        });
        self.pending_image_task = Some(PendingImageTask { rx, meta });
        self.set_status(format!("Loading {description}…"));
    }

    pub(crate) fn poll_image_loader(&mut self, ctx: &Context) {
        let Some(task) = self.pending_image_task.take() else {
            return;
        };
        match task.rx.try_recv() {
            Ok(ImageLoadResult::Success(color)) => {
                let meta = task.meta.into_image_meta();
                let loaded_path = meta.path().map(Path::to_path_buf);
                self.finish_loaded_color_image(ctx, color, meta);
                self.apply_project_if_ready(loaded_path.as_deref());
            }
            Ok(ImageLoadResult::Error(err)) => {
                let label = task.meta.description();
                self.set_status(format!("Failed to load {label}: {err}"));
                self.pending_project_apply = None;
            }
            Err(TryRecvError::Empty) => {
                self.pending_image_task = Some(task);
            }
            Err(TryRecvError::Disconnected) => {
                let label = task.meta.description();
                self.set_status(format!("Loading {label} failed: worker disconnected."));
                self.pending_project_apply = None;
            }
        }
    }

    fn finish_loaded_color_image(
        &mut self,
        ctx: &Context,
        color: ColorImage,
        meta: super::ImageMeta,
    ) {
        let name = meta.display_name();
        let loaded = LoadedImage::from_color_image(ctx, color);
        self.set_loaded_image(loaded, Some(meta));
        self.set_status(format!("Loaded {name}"));
        self.pending_fit_on_load = self.pending_project_apply.is_none();
    }

    pub(crate) fn remember_image_dir_from_path(&mut self, path: &Path) {
        let dir = path
            .parent()
            .map_or_else(|| std::path::PathBuf::from("."), Path::to_path_buf);
        self.last_image_dir = Some(dir);
    }

    pub(crate) fn remember_export_dir_from_path(&mut self, path: &Path) {
        let dir = path
            .parent()
            .map_or_else(|| std::path::PathBuf::from("."), Path::to_path_buf);
        self.last_export_dir = Some(dir);
    }
}
