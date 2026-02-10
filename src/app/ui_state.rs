use crate::export::{ExportFormat, ExportPayload};
use egui_file_dialog::FileDialog;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidePanelPosition {
    Left,
    Right,
}

#[allow(clippy::struct_excessive_bools)]
pub struct UiState {
    pub(super) side_open: bool,
    pub(super) side_position: SidePanelPosition,
    pub(super) info_window_open: bool,
    pub(super) points_info_window_open: bool,
    pub(super) image_filters_window_open: bool,
    pub(super) auto_trace_window_open: bool,
    pub(super) last_status: Option<String>,
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
