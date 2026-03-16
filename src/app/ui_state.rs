use crate::export::{ExportFormat, ExportPayload};
use crate::i18n::UiLanguage;
use egui_file_dialog::FileDialog;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidePanelPosition {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    Info,
    Warn,
    Error,
}

pub struct StatusMessage {
    pub(super) text: String,
    pub(super) level: StatusLevel,
    pub(super) created_at: Instant,
}

#[allow(clippy::struct_excessive_bools)]
pub struct UiState {
    pub(super) language: UiLanguage,
    pub(super) side_open: bool,
    pub(super) side_position: SidePanelPosition,
    pub(super) info_window_open: bool,
    pub(super) points_info_window_open: bool,
    pub(super) image_filters_window_open: bool,
    pub(super) auto_trace_window_open: bool,
    pub(super) last_status: Option<StatusMessage>,
    pub(super) status_copy_feedback_until: Option<Instant>,
}

#[derive(Debug)]
pub enum NativeDialog {
    Open(FileDialog),
    OpenProject(FileDialog),
    SaveProject(FileDialog),
    SaveExport {
        dialog: FileDialog,
        payload: ExportPayload,
        format: ExportFormat,
    },
}
