mod app;
mod config;
mod export;
mod image;
mod interp;
mod project;
mod snap;
mod types;

use app::CurcatApp;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let initial_image_path: Option<PathBuf> = std::env::args_os().nth(1).map(PathBuf::from);
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Curcat — Graph Digitizer",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(CurcatApp::new_with_initial_path(
                &cc.egui_ctx,
                initial_image_path.as_deref(),
            )))
        }),
    )
}
