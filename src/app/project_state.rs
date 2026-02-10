use super::{
    AxisCalUi, CurcatApp, MAX_ZOOM, MIN_ZOOM, NativeDialog, PendingImageTask, PickMode,
    PickedPoint, PolarCalUi, ZoomIntent,
};
use crate::image::ImageTransformRecord;
use crate::project;
use crate::types::{AxisUnit, ScaleKind};
use egui::{Pos2, Vec2};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[derive(Debug)]
pub(super) struct ProjectSaveRequest {
    pub(super) target_path: PathBuf,
    pub(super) image_path: PathBuf,
    pub(super) transform: ImageTransformRecord,
    pub(super) calibration: project::CalibrationRecord,
    pub(super) points: Vec<project::PointRecord>,
    pub(super) zoom: f32,
    pub(super) pan: [f32; 2],
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
}

pub(super) struct PendingProjectSave {
    pub(super) rx: Receiver<ProjectSaveResult>,
}

pub(super) enum ProjectSaveResult {
    Success,
    Error(String),
}

#[derive(Debug)]
pub(super) struct ProjectApplyPlan {
    pub(super) payload: project::ProjectPayload,
    pub(super) image: project::ResolvedImage,
    pub(super) project_path: PathBuf,
    pub(super) version: u32,
}

#[derive(Debug)]
pub(super) struct ProjectLoadPrompt {
    pub(super) warnings: Vec<project::ProjectWarning>,
    pub(super) plan: ProjectApplyPlan,
}

pub struct ProjectState {
    pub(super) pending_image_task: Option<PendingImageTask>,
    pub(super) pending_project_apply: Option<ProjectApplyPlan>,
    pub(super) pending_project_save: Option<PendingProjectSave>,
    pub(super) project_prompt: Option<ProjectLoadPrompt>,
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    pub(super) active_dialog: Option<NativeDialog>,
    pub(super) last_project_dir: Option<PathBuf>,
    pub(super) last_project_path: Option<PathBuf>,
    pub(super) last_image_dir: Option<PathBuf>,
    pub(super) last_export_dir: Option<PathBuf>,
}

fn perform_project_save(request: ProjectSaveRequest) -> Result<(), String> {
    let ProjectSaveRequest {
        target_path,
        image_path,
        transform,
        calibration,
        points,
        zoom,
        pan,
        title,
        description,
    } = request;
    let absolute_image_path = std::fs::canonicalize(&image_path).unwrap_or(image_path);
    let image_crc32 =
        project::compute_image_crc32(&absolute_image_path).map_err(|err| err.to_string())?;
    let relative_image_path = project::make_relative_image_path(&target_path, &absolute_image_path)
        .or_else(|| absolute_image_path.file_name().map(PathBuf::from));
    let payload = project::ProjectPayload {
        absolute_image_path,
        relative_image_path,
        image_crc32,
        transform,
        calibration,
        points,
        zoom,
        pan,
        title,
        description,
    };
    project::save_project(&target_path, &payload).map_err(|err| err.to_string())
}

impl CurcatApp {
    fn axis_to_record(cal: &AxisCalUi) -> project::AxisCalibrationRecord {
        project::AxisCalibrationRecord {
            unit: cal.unit,
            scale: cal.scale,
            p1: cal.p1.map(|p| [p.x, p.y]),
            p2: cal.p2.map(|p| [p.x, p.y]),
            v1_text: cal.v1_text.clone(),
            v2_text: cal.v2_text.clone(),
        }
    }

    fn axis_from_record(record: &project::AxisCalibrationRecord) -> AxisCalUi {
        AxisCalUi {
            unit: record.unit,
            scale: record.scale,
            p1: record.p1.map(|p| Pos2::new(p[0], p[1])),
            p2: record.p2.map(|p| Pos2::new(p[0], p[1])),
            v1_text: record.v1_text.clone(),
            v2_text: record.v2_text.clone(),
        }
    }

    fn polar_to_record(polar: &PolarCalUi) -> project::PolarCalibrationRecord {
        project::PolarCalibrationRecord {
            origin: polar.origin.map(|p| [p.x, p.y]),
            radius: Self::axis_to_record(&polar.radius),
            angle: Self::axis_to_record(&polar.angle),
            angle_unit: polar.angle_unit,
            angle_direction: polar.angle_direction,
        }
    }

    fn polar_from_record(record: &project::PolarCalibrationRecord) -> PolarCalUi {
        let mut radius = Self::axis_from_record(&record.radius);
        let mut angle = Self::axis_from_record(&record.angle);
        radius.unit = AxisUnit::Float;
        angle.unit = AxisUnit::Float;
        angle.scale = ScaleKind::Linear;
        PolarCalUi {
            origin: record.origin.map(|p| Pos2::new(p[0], p[1])),
            radius,
            angle,
            angle_unit: record.angle_unit,
            angle_direction: record.angle_direction,
        }
    }

    fn build_project_save_request(
        &mut self,
        target_path: &Path,
    ) -> anyhow::Result<ProjectSaveRequest> {
        let Some(image_path) = self
            .image
            .meta
            .as_ref()
            .and_then(|m| m.path().map(Path::to_path_buf))
        else {
            anyhow::bail!("Cannot save project: image was not loaded from a file");
        };

        let (x_mapping, y_mapping) = self.cartesian_mappings();
        let polar_mapping = self.polar_mapping();
        self.ensure_point_numeric_cache(
            self.calibration.coord_system,
            x_mapping.as_ref(),
            y_mapping.as_ref(),
            polar_mapping.as_ref(),
        );

        let points = self
            .points
            .points
            .iter()
            .map(|p| project::PointRecord {
                pixel: [p.pixel.x, p.pixel.y],
                x_numeric: p.x_numeric,
                y_numeric: p.y_numeric,
            })
            .collect();

        let calibration = project::CalibrationRecord {
            coord_system: self.calibration.coord_system,
            x: Self::axis_to_record(&self.calibration.cal_x),
            y: Self::axis_to_record(&self.calibration.cal_y),
            polar: Self::polar_to_record(&self.calibration.polar_cal),
            calibration_angle_snap: self.calibration.calibration_angle_snap,
            show_calibration_segments: self.calibration.show_calibration_segments,
        };

        Ok(ProjectSaveRequest {
            target_path: target_path.to_path_buf(),
            image_path,
            transform: self.image.transform,
            calibration,
            points,
            zoom: self.image.zoom,
            pan: [self.image.pan.x, self.image.pan.y],
            title: self.project.title.clone(),
            description: self.project.description.clone(),
        })
    }

    pub(super) fn handle_project_save(&mut self, path: &Path) {
        if self.project.pending_project_save.is_some() {
            self.set_status("Project save already in progress.");
            return;
        }
        self.project.last_project_path = Some(path.to_path_buf());
        self.project.last_project_dir = path.parent().map(Path::to_path_buf);
        match self.build_project_save_request(path) {
            Ok(request) => self.start_project_save_job(request),
            Err(err) => self.set_status(format!("Project save failed: {err}")),
        }
    }

    fn start_project_save_job(&mut self, request: ProjectSaveRequest) {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = match perform_project_save(request) {
                Ok(()) => ProjectSaveResult::Success,
                Err(err) => ProjectSaveResult::Error(err),
            };
            let _ = tx.send(result);
        });
        self.project.pending_project_save = Some(PendingProjectSave { rx });
        self.set_status("Saving project…");
    }

    pub(super) fn poll_project_save_job(&mut self) {
        let Some(job) = self.project.pending_project_save.take() else {
            return;
        };
        match job.rx.try_recv() {
            Ok(ProjectSaveResult::Success) => {
                self.set_status("Project saved.");
            }
            Ok(ProjectSaveResult::Error(err)) => {
                self.set_status(format!("Project save failed: {err}"));
            }
            Err(TryRecvError::Empty) => {
                self.project.pending_project_save = Some(job);
            }
            Err(TryRecvError::Disconnected) => {
                self.set_status("Project save failed: worker disconnected.");
            }
        }
    }

    pub(super) fn handle_project_load(&mut self, path: PathBuf) {
        self.project.project_prompt = None;
        self.project.pending_project_apply = None;
        self.project.last_project_dir = path.parent().map(Path::to_path_buf);
        self.project.last_project_path = Some(path.clone());
        match project::load_project(&path) {
            Ok(outcome) => self.handle_loaded_project(path, outcome),
            Err(err) => self.set_status(format!("Failed to load project: {err}")),
        }
    }

    fn handle_loaded_project(&mut self, path: PathBuf, outcome: project::ProjectLoadOutcome) {
        let plan = ProjectApplyPlan {
            payload: outcome.payload,
            image: outcome.chosen_image,
            project_path: path,
            version: outcome.version,
        };
        if outcome.warnings.is_empty() {
            self.begin_applying_project(plan);
        } else {
            self.project.project_prompt = Some(ProjectLoadPrompt {
                warnings: outcome.warnings,
                plan,
            });
            self.set_status("Project has warnings. Confirm to continue loading.");
        }
    }

    pub(super) fn begin_applying_project(&mut self, plan: ProjectApplyPlan) {
        let image_path = plan.image.path.clone();
        self.project.project_prompt = None;
        let status = {
            let source_label = match plan.image.source {
                project::ImagePathSource::Absolute => "absolute path",
                project::ImagePathSource::Relative => "relative path",
            };
            if plan.image.checksum_matches {
                format!(
                    "Loading project v{} image from {source_label}…",
                    plan.version
                )
            } else {
                let expected = plan.payload.image_crc32;
                let actual = plan
                    .image
                    .actual_checksum
                    .map_or_else(|| "unknown".to_string(), |v| format!("{v:#010x}"));
                format!(
                    "Image checksum mismatch (expected {expected:#010x}, got {actual}). Loading from {source_label}…"
                )
            }
        };
        self.project.pending_project_apply = Some(plan);
        self.set_status(status);
        self.start_loading_image_from_path(image_path);
    }

    pub(super) fn apply_project_if_ready(&mut self, loaded_path: Option<&Path>) {
        let Some(plan) = self.project.pending_project_apply.take() else {
            return;
        };
        let Some(path) = loaded_path else {
            self.project.pending_project_apply = Some(plan);
            return;
        };
        if path != plan.image.path {
            self.project.pending_project_apply = Some(plan);
            return;
        }
        self.apply_project_state(plan);
    }

    fn apply_project_state(&mut self, plan: ProjectApplyPlan) {
        self.project.project_prompt = None;
        self.project.pending_project_apply = None;

        // Reapply transforms on freshly loaded image.
        self.image.transform = ImageTransformRecord::identity();
        let ops = plan.payload.transform.replay_operations();
        for op in ops {
            self.apply_image_transform(op, None);
        }
        self.image.transform = plan.payload.transform;

        self.image.zoom = plan.payload.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.image.pan = Vec2::new(plan.payload.pan[0], plan.payload.pan[1]);
        self.image.zoom_target = self.image.zoom;
        self.image.zoom_intent = ZoomIntent::TargetPan(self.image.pan);
        self.project.title.clone_from(&plan.payload.title);
        self.project
            .description
            .clone_from(&plan.payload.description);

        self.calibration.cal_x = Self::axis_from_record(&plan.payload.calibration.x);
        self.calibration.cal_y = Self::axis_from_record(&plan.payload.calibration.y);
        self.calibration.polar_cal = Self::polar_from_record(&plan.payload.calibration.polar);
        self.calibration.coord_system = plan.payload.calibration.coord_system;
        self.calibration.calibration_angle_snap = plan.payload.calibration.calibration_angle_snap;
        self.calibration.show_calibration_segments =
            plan.payload.calibration.show_calibration_segments;
        self.points.last_x_mapping = None;
        self.points.last_y_mapping = None;
        self.points.last_polar_mapping = None;
        self.points.last_coord_system = self.calibration.coord_system;
        self.calibration.pick_mode = PickMode::None;
        self.calibration.pending_value_focus = None;
        self.calibration.dragging_handle = None;
        self.image.touch_pan_active = false;
        self.image.touch_pan_last = None;

        self.points.points = plan
            .payload
            .points
            .iter()
            .map(|p| PickedPoint {
                pixel: Pos2::new(p.pixel[0], p.pixel[1]),
                x_numeric: p.x_numeric,
                y_numeric: p.y_numeric,
            })
            .collect();
        self.mark_points_dirty();
        self.mark_snap_maps_dirty();
        self.refresh_snap_overlay_palette();

        if let Some(parent) = plan.project_path.parent() {
            self.project.last_project_dir = Some(parent.to_path_buf());
        }
        self.project.last_project_path = Some(plan.project_path);
        self.remember_image_dir_from_path(&plan.image.path);

        if plan.image.checksum_matches {
            let source_label = match plan.image.source {
                project::ImagePathSource::Absolute => "absolute path",
                project::ImagePathSource::Relative => "relative path",
            };
            self.set_status(format!(
                "Project v{} loaded ({source_label}).",
                plan.version
            ));
        } else {
            let expected = plan.payload.image_crc32;
            let actual = plan
                .image
                .actual_checksum
                .map_or_else(|| "unknown".to_string(), |v| format!("{v:#010x}"));
            self.set_status(format!(
                "Project v{} loaded with checksum warning (expected {expected:#010x}, got {actual}).",
                plan.version
            ));
        }
    }

    pub(super) fn project_warning_text(warn: &project::ProjectWarning) -> String {
        let source_label = |source: &project::ImagePathSource| match source {
            project::ImagePathSource::Absolute => "Absolute path",
            project::ImagePathSource::Relative => "Relative path",
        };

        match warn {
            project::ProjectWarning::MissingImage {
                path,
                source,
                reason,
            } => format!(
                "Missing image ({}) at {}: {}",
                source_label(source),
                path.display(),
                reason
            ),
            project::ProjectWarning::ChecksumMismatch {
                path,
                source,
                expected,
                actual,
            } => format!(
                "Checksum mismatch ({}) at {}: expected {expected:#010x}, got {actual:#010x}",
                source_label(source),
                path.display()
            ),
        }
    }
}
