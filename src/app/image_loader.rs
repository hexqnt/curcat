use super::{
    CurcatApp, ImageLoadRequest, ImageLoadResult, PendingImageLimitPrompt, PendingImageMeta,
    PendingImageTask,
};
use crate::i18n::UiLanguage;
use crate::image::{
    ImageLoadOutcome, ImageLoadPolicy, LoadedImage, decode_image_from_bytes,
    decode_image_from_clipboard_rgba, decode_image_from_path,
};
use egui::{ColorImage, Context};
use std::path::Path;
use std::sync::mpsc::{self, TryRecvError};
use std::thread;

impl CurcatApp {
    pub(crate) fn start_loading_image_from_path(&mut self, path: std::path::PathBuf) {
        if !self.can_start_image_load(Some(path.as_path())) {
            return;
        }
        self.remember_image_dir_from_path(&path);
        let meta = PendingImageMeta::Path { path: path.clone() };
        self.start_image_load(ImageLoadRequest::Path(path), meta, ImageLoadPolicy::AskUser);
    }

    pub(crate) fn start_loading_image_from_bytes(
        &mut self,
        name: Option<String>,
        bytes: Vec<u8>,
        last_modified: Option<std::time::SystemTime>,
    ) {
        if !self.can_start_image_load(None) {
            return;
        }
        let meta = PendingImageMeta::DroppedBytes {
            name,
            byte_len: bytes.len(),
            last_modified,
        };
        self.start_image_load(
            ImageLoadRequest::Bytes(bytes),
            meta,
            ImageLoadPolicy::AskUser,
        );
    }

    pub(crate) fn start_loading_image_from_clipboard(
        &mut self,
        width: usize,
        height: usize,
        rgba: Vec<u8>,
    ) {
        if !self.can_start_image_load(None) {
            return;
        }
        let meta = PendingImageMeta::Clipboard {
            byte_len: rgba.len(),
        };
        self.start_image_load(
            ImageLoadRequest::ClipboardRgba {
                width,
                height,
                rgba,
            },
            meta,
            ImageLoadPolicy::AskUser,
        );
    }

    pub(crate) fn retry_image_load_with_policy(&mut self, policy: ImageLoadPolicy) {
        let Some(prompt) = self.project.pending_image_limit_prompt.take() else {
            return;
        };
        let (request, meta, policy) = prompt.retry_with_policy(policy);
        self.start_image_load(request, meta, policy);
    }

    pub(crate) fn reject_image_load_due_to_limits(&mut self) {
        if self.project.pending_image_limit_prompt.take().is_some() {
            self.project.pending_project_apply = None;
            self.set_status_warn(match self.ui.language {
                UiLanguage::En => "Image load canceled due to limits.",
                UiLanguage::Ru => "Загрузка изображения отменена из-за лимитов.",
            });
        }
    }

    fn start_image_load(
        &mut self,
        request: ImageLoadRequest,
        meta: PendingImageMeta,
        policy: ImageLoadPolicy,
    ) {
        let description = meta.description();
        let cfg = self.config.clone();
        let (tx, rx) = mpsc::channel();
        self.project.pending_image_limit_prompt = None;

        thread::spawn(move || {
            let msg = decode_request(&cfg, request, policy);
            let _ = tx.send(msg);
        });

        self.project.pending_image_task = Some(PendingImageTask { rx, meta });
        self.set_status(self.i18n().format_loading_image(&description));
    }

    pub(crate) fn poll_image_loader(&mut self, ctx: &Context) {
        let Some(task) = self.project.pending_image_task.take() else {
            return;
        };
        match task.rx.try_recv() {
            Ok(ImageLoadResult::Success(color)) => {
                let meta = task.meta.into_image_meta();
                let loaded_path = meta.path().map(Path::to_path_buf);
                self.finish_loaded_color_image(ctx, color, meta);
                self.apply_project_if_ready(loaded_path.as_deref());
            }
            Ok(ImageLoadResult::NeedsLimitDecision { request, info }) => {
                let label = task.meta.description();
                self.project.pending_image_limit_prompt = Some(PendingImageLimitPrompt {
                    request,
                    meta: task.meta,
                    info,
                });
                self.set_status_warn(match self.ui.language {
                    UiLanguage::En => {
                        format!("{label} exceeds configured limits. Choose how to continue.")
                    }
                    UiLanguage::Ru => {
                        format!("{label} превышает лимиты. Выберите, как продолжить загрузку.")
                    }
                });
            }
            Ok(ImageLoadResult::Error(err)) => {
                let label = task.meta.description();
                self.set_status_error(match self.ui.language {
                    UiLanguage::En => format!("Failed to load {label}: {err}"),
                    UiLanguage::Ru => format!("Не удалось загрузить {label}: {err}"),
                });
                self.project.pending_project_apply = None;
                self.project.pending_image_limit_prompt = None;
            }
            Err(TryRecvError::Empty) => {
                self.project.pending_image_task = Some(task);
            }
            Err(TryRecvError::Disconnected) => {
                let label = task.meta.description();
                self.set_status_error(match self.ui.language {
                    UiLanguage::En => format!("Loading {label} failed: worker disconnected."),
                    UiLanguage::Ru => format!("Ошибка загрузки {label}: рабочий поток отключился."),
                });
                self.project.pending_project_apply = None;
                self.project.pending_image_limit_prompt = None;
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
        self.set_status(self.i18n().format_loaded_name(&name));
        self.image.pending_fit_on_load = self.project.pending_project_apply.is_none();
    }

    pub(crate) fn remember_image_dir_from_path(&mut self, path: &Path) {
        let dir = path
            .parent()
            .map_or_else(|| std::path::PathBuf::from("."), Path::to_path_buf);
        self.project.last_image_dir = Some(dir);
    }

    pub(crate) fn remember_export_dir_from_path(&mut self, path: &Path) {
        let dir = path
            .parent()
            .map_or_else(|| std::path::PathBuf::from("."), Path::to_path_buf);
        self.project.last_export_dir = Some(dir);
    }

    fn can_start_image_load(&mut self, expected_project_image: Option<&Path>) -> bool {
        let Some(plan) = self.project.pending_project_apply.as_ref() else {
            return true;
        };
        if expected_project_image.is_some_and(|path| path == plan.image.path.as_path()) {
            return true;
        }
        self.set_status_warn(match self.ui.language {
            UiLanguage::En => "Project loading in progress. Wait until it finishes.",
            UiLanguage::Ru => "Идёт загрузка проекта. Дождитесь завершения.",
        });
        false
    }
}

fn decode_request(
    cfg: &crate::config::AppConfig,
    request: ImageLoadRequest,
    policy: ImageLoadPolicy,
) -> ImageLoadResult {
    match request {
        ImageLoadRequest::Path(path) => {
            let outcome = decode_image_from_path(cfg, &path, policy);
            map_outcome(ImageLoadRequest::Path(path), outcome)
        }
        ImageLoadRequest::Bytes(bytes) => {
            let outcome = decode_image_from_bytes(cfg, &bytes, policy);
            map_outcome(ImageLoadRequest::Bytes(bytes), outcome)
        }
        ImageLoadRequest::ClipboardRgba {
            width,
            height,
            rgba,
        } => {
            let outcome = decode_image_from_clipboard_rgba(cfg, width, height, &rgba, policy);
            map_outcome(
                ImageLoadRequest::ClipboardRgba {
                    width,
                    height,
                    rgba,
                },
                outcome,
            )
        }
    }
}

fn map_outcome(
    request: ImageLoadRequest,
    outcome: anyhow::Result<ImageLoadOutcome>,
) -> ImageLoadResult {
    match outcome {
        Ok(ImageLoadOutcome::Ready(color)) => ImageLoadResult::Success(color),
        Ok(ImageLoadOutcome::NeedsLimitDecision(info)) => {
            ImageLoadResult::NeedsLimitDecision { request, info }
        }
        Err(err) => ImageLoadResult::Error(err.to_string()),
    }
}
