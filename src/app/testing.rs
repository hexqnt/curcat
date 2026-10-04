//! Narrow UI test and profiling interface, available only with the `testing` feature.

use super::{CurcatApp, PickedPoint};
use crate::{
    config::{AppConfig, UiConfig},
    image::{ImageMeta, LoadedImage},
};
use egui::{ColorImage, Pos2, Rect, pos2};
use std::path::Path;

pub use crate::i18n::UiLanguage;

pub struct AppState<'a> {
    pub image_rect: Option<Rect>,
    pub image_size: Option<[usize; 2]>,
    pub zoom: f32,
    pub points: usize,
    pub x1: Option<Pos2>,
    pub x1_text: &'a str,
    pub holding: bool,
    pub auto_placing: bool,
    pub picking: bool,
    pub guides: usize,
}

/// Build an empty fixture without reading user configuration or files.
#[must_use]
pub fn empty(ctx: &egui::Context, language: UiLanguage) -> CurcatApp {
    egui_extras::install_image_loaders(ctx);
    CurcatApp::from_config(AppConfig {
        smooth_zoom: false,
        ui: UiConfig {
            language: Some(language),
        },
        ..AppConfig::default()
    })
}

/// Build a calibrated in-memory fixture without reading user configuration or files.
#[must_use]
pub fn from_image(
    ctx: &egui::Context,
    image: ColorImage,
    points: impl IntoIterator<Item = Pos2>,
    language: UiLanguage,
) -> CurcatApp {
    let mut app = empty(ctx, language);
    let size = image.size;
    let image = LoadedImage::from_color_image(ctx, image);
    app.image.base_pixels = Some(std::sync::Arc::clone(&image.pixels));
    app.image.image = Some(image);
    let width = super::safe_usize_to_f32(size[0]);
    let height = super::safe_usize_to_f32(size[1]);
    app.calibration.cal_x.p1 = Some(pos2(width * 0.1, height * 0.9));
    app.calibration.cal_x.p2 = Some(pos2(width * 0.9, height * 0.9));
    app.calibration.cal_y.p1 = app.calibration.cal_x.p1;
    app.calibration.cal_y.p2 = Some(pos2(width * 0.1, height * 0.1));
    for axis in [&mut app.calibration.cal_x, &mut app.calibration.cal_y] {
        axis.v1_text = "0".into();
        axis.v2_text = "1".into();
    }
    app.points.points = points.into_iter().map(PickedPoint::new).collect();
    app
}

pub fn render(app: &mut CurcatApp, ui: &mut egui::Ui) {
    app.render(ui);
}

impl CurcatApp {
    #[must_use]
    pub fn inspect(&self) -> AppState<'_> {
        AppState {
            image_rect: self.image_rect,
            image_size: self.image.image.as_ref().map(|image| image.size),
            zoom: self.image.zoom,
            points: self.points.points.len(),
            x1: self.calibration.cal_x.p1,
            x1_text: &self.calibration.cal_x.v1_text,
            holding: self.interaction.auto_place_state.hold_started_at.is_some(),
            auto_placing: self.interaction.auto_place_state.active,
            picking: !matches!(self.calibration.pick_mode, super::PickMode::None),
            guides: self.calibration.snap_guides.iter().flatten().count(),
        }
    }

    #[must_use]
    pub fn point_pixel(&self, index: usize) -> Option<Pos2> {
        self.points.points.get(index).map(|point| point.pixel)
    }

    /// Whether image or project workers still need to be polled.
    #[must_use]
    pub const fn has_pending_tasks(&self) -> bool {
        self.project.pending_image_task.is_some() || self.project.pending_project_save.is_some()
    }

    /// Load a fixture image through the normal worker with a configured side limit.
    pub fn load_image_with_limit(&mut self, path: &Path, max_side: u32) {
        self.config.image_limits.image_dim = max_side;
        self.start_loading_image_from_path(path.to_path_buf());
    }

    /// Save the current fixture through the normal project worker using its source image.
    pub fn save_project(&mut self, project_path: &Path, image_path: &Path) {
        self.image.meta = Some(ImageMeta::from_path(image_path));
        self.handle_project_save(project_path);
    }

    /// Load a fixture project through normal parsing, warning, and apply paths.
    pub fn load_project(&mut self, path: &Path) {
        self.handle_project_load(path.to_path_buf());
    }
}
