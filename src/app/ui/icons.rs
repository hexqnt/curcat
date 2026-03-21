//! Набор UI-иконок на базе SVG из Tabler.
//!
//! Все ассеты подготавливаются как белые монохромные иконки и затем
//! тонируются цветами темы через egui.

use egui::{Color32, Image, ImageSource};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Icon {
    Menu,
    Stats,
    Info,
    Filters,
    SideToggle,
    RotateCcw,
    RotateCw,
    FlipH,
    FlipV,
    Fit,
    ResetView,
    Clear,
    Undo,
    ExportCsv,
    ExportJson,
    ExportRon,
    ExportXlsx,
    PickPoint,
    AutoPlace,
    AutoTrace,
    PickColor,
    DeletePoint,
    Pan,
    Zoom,
    PresetUnit,
    PresetPixels,
    Close,
    OpenImage,
    PasteImage,
    LoadProject,
    SaveProject,
}

pub const BUTTON_ICON_SIZE: f32 = 14.0;
pub const INLINE_ICON_SIZE: f32 = 13.0;
pub const BADGE_ICON_SIZE: f32 = 12.0;

pub const ICON_MENU: Icon = Icon::Menu;
pub const ICON_STATS: Icon = Icon::Stats;
pub const ICON_INFO: Icon = Icon::Info;
pub const ICON_FILTERS: Icon = Icon::Filters;
pub const ICON_SIDE_TOGGLE: Icon = Icon::SideToggle;
pub const ICON_ROTATE_CCW: Icon = Icon::RotateCcw;
pub const ICON_ROTATE_CW: Icon = Icon::RotateCw;
pub const ICON_FLIP_H: Icon = Icon::FlipH;
pub const ICON_FLIP_V: Icon = Icon::FlipV;
pub const ICON_FIT: Icon = Icon::Fit;
pub const ICON_RESET_VIEW: Icon = Icon::ResetView;
pub const ICON_CLEAR: Icon = Icon::Clear;
pub const ICON_UNDO: Icon = Icon::Undo;
pub const ICON_EXPORT_CSV: Icon = Icon::ExportCsv;
pub const ICON_EXPORT_JSON: Icon = Icon::ExportJson;
pub const ICON_EXPORT_RON: Icon = Icon::ExportRon;
pub const ICON_EXPORT_XLSX: Icon = Icon::ExportXlsx;
pub const ICON_PICK_POINT: Icon = Icon::PickPoint;
pub const ICON_AUTO_PLACE: Icon = Icon::AutoPlace;
pub const ICON_AUTO_TRACE: Icon = Icon::AutoTrace;
pub const ICON_PICK_COLOR: Icon = Icon::PickColor;
pub const ICON_DELETE_POINT: Icon = Icon::DeletePoint;
pub const ICON_PAN: Icon = Icon::Pan;
pub const ICON_ZOOM: Icon = Icon::Zoom;
pub const ICON_PRESET_UNIT: Icon = Icon::PresetUnit;
pub const ICON_PRESET_PIXELS: Icon = Icon::PresetPixels;
pub const ICON_CLOSE: Icon = Icon::Close;
pub const ICON_OPEN_IMAGE: Icon = Icon::OpenImage;
pub const ICON_PASTE_IMAGE: Icon = Icon::PasteImage;
pub const ICON_LOAD_PROJECT: Icon = Icon::LoadProject;
pub const ICON_SAVE_PROJECT: Icon = Icon::SaveProject;

/// Вернуть монохромную иконку фиксированного размера.
pub fn image(icon: Icon, size: f32) -> Image<'static> {
    Image::new(source(icon))
        .fit_to_exact_size(egui::vec2(size, size))
        .tint(Color32::WHITE)
}

const fn source(icon: Icon) -> ImageSource<'static> {
    match icon {
        Icon::Menu => egui::include_image!("../../../assets/icons/tabler/menu-2.svg"),
        Icon::Stats => egui::include_image!("../../../assets/icons/tabler/chart-dots.svg"),
        Icon::Info => egui::include_image!("../../../assets/icons/tabler/info-circle.svg"),
        Icon::Filters => {
            egui::include_image!("../../../assets/icons/tabler/adjustments-horizontal.svg")
        }
        Icon::SideToggle => {
            egui::include_image!("../../../assets/icons/tabler/arrows-horizontal.svg")
        }
        Icon::RotateCcw => egui::include_image!("../../../assets/icons/tabler/rotate-2.svg"),
        Icon::RotateCw => {
            egui::include_image!("../../../assets/icons/tabler/rotate-clockwise-2.svg")
        }
        Icon::FlipH => egui::include_image!("../../../assets/icons/tabler/flip-horizontal.svg"),
        Icon::FlipV => egui::include_image!("../../../assets/icons/tabler/flip-vertical.svg"),
        Icon::Fit => egui::include_image!("../../../assets/icons/tabler/maximize.svg"),
        Icon::ResetView => egui::include_image!("../../../assets/icons/tabler/zoom-reset.svg"),
        Icon::Clear | Icon::DeletePoint => {
            egui::include_image!("../../../assets/icons/tabler/trash.svg")
        }
        Icon::Undo => egui::include_image!("../../../assets/icons/tabler/arrow-back-up.svg"),
        Icon::ExportCsv => {
            egui::include_image!("../../../assets/icons/tabler/file-type-csv.svg")
        }
        Icon::ExportJson => egui::include_image!("../../../assets/icons/tabler/braces.svg"),
        Icon::ExportRon => egui::include_image!("../../../assets/icons/tabler/file-code.svg"),
        Icon::ExportXlsx => {
            egui::include_image!("../../../assets/icons/tabler/file-type-xls.svg")
        }
        Icon::PickPoint => egui::include_image!("../../../assets/icons/tabler/crosshair.svg"),
        Icon::AutoPlace => egui::include_image!("../../../assets/icons/tabler/point.svg"),
        Icon::AutoTrace => egui::include_image!("../../../assets/icons/tabler/route-2.svg"),
        Icon::PickColor => egui::include_image!("../../../assets/icons/tabler/color-picker.svg"),
        Icon::Pan => egui::include_image!("../../../assets/icons/tabler/hand-move.svg"),
        Icon::Zoom => egui::include_image!("../../../assets/icons/tabler/zoom-in.svg"),
        Icon::PresetUnit => {
            egui::include_image!("../../../assets/icons/tabler/ruler-measure.svg")
        }
        Icon::PresetPixels => egui::include_image!("../../../assets/icons/tabler/ruler-2.svg"),
        Icon::Close => egui::include_image!("../../../assets/icons/tabler/x.svg"),
        Icon::OpenImage => egui::include_image!("../../../assets/icons/tabler/photo.svg"),
        Icon::PasteImage => egui::include_image!("../../../assets/icons/tabler/clipboard.svg"),
        Icon::LoadProject => egui::include_image!("../../../assets/icons/tabler/folder-open.svg"),
        Icon::SaveProject => {
            egui::include_image!("../../../assets/icons/tabler/device-floppy.svg")
        }
    }
}
