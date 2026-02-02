//! Helpers for snap-map creation, color analysis, and snapping workflow.

use super::{CurcatApp, PointInputMode, SnapBuildJob, safe_usize_to_f32};
use crate::snap::{SnapBehavior, SnapMapCache, derive_snap_overlay_palette};
use egui::{Color32, ColorImage, Pos2, Vec2};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;

/// Square size (in pixels) used for the snap color swatch preview.
pub const SNAP_SWATCH_SIZE: f32 = 22.0;

impl CurcatApp {
    /// Default overlay palette used when the image analysis yields no colors.
    ///
    /// The set favors high-contrast tones that remain visible over most charts.
    pub(crate) fn default_snap_overlay_choices() -> Vec<Color32> {
        vec![
            Color32::from_rgb(236, 214, 96),
            Color32::from_rgb(66, 123, 176),
            Color32::from_rgb(184, 102, 128),
            Color32::from_rgb(72, 138, 96),
        ]
    }

    /// Refresh overlay palette choices from the current image and keep the selection stable.
    pub(crate) fn refresh_snap_overlay_palette(&mut self) {
        let previous_choice = self
            .snap
            .snap_overlay_choices
            .get(self.snap.snap_overlay_choice)
            .copied()
            .or(Some(self.snap.snap_overlay_color));
        let analyzed = self.image.image.as_ref().map_or_else(Vec::new, |img| {
            Self::analyze_image_for_snap_colors(&img.pixels)
        });
        let derived_choices = if analyzed.is_empty() {
            Self::default_snap_overlay_choices()
        } else {
            analyzed
        };
        let new_index = if let Some(prev) = previous_choice
            && let Some(idx) = derived_choices.iter().position(|color| *color == prev)
        {
            idx
        } else {
            0
        };
        self.snap.snap_overlay_choice = new_index;
        self.snap.snap_overlay_color = derived_choices
            .get(new_index)
            .copied()
            .unwrap_or(self.snap.snap_overlay_color);
        self.snap.snap_overlay_choices = derived_choices;
    }

    /// Extract candidate overlay colors from the image for snap previews.
    pub(crate) fn analyze_image_for_snap_colors(image: &ColorImage) -> Vec<Color32> {
        derive_snap_overlay_palette(image)
    }

    /// Invalidate cached snap maps so the next query rebuilds them.
    pub(crate) fn mark_snap_maps_dirty(&mut self) {
        self.snap.snap_maps_dirty = true;
        self.snap.snap_maps = None;
        self.snap.pending_snap_job = None;
    }

    /// Kick off a background job that builds snap maps for the current image.
    pub(crate) fn start_snap_job(&mut self) {
        if self.snap.pending_snap_job.is_some() || !self.snap.snap_maps_dirty {
            return;
        }
        let Some(image) = &self.image.image else {
            self.snap.snap_maps_dirty = false;
            self.snap.snap_maps = None;
            return;
        };
        let color_image = image.pixels.clone();
        let overlay_color = self.snap.snap_target_color;
        let tolerance = self.snap.snap_color_tolerance;
        let (tx, rx) = mpsc::channel();
        // Build the cache off-thread to avoid blocking the UI while scanning pixels.
        thread::spawn(move || {
            let result = SnapMapCache::build(&color_image, overlay_color, tolerance);
            let _ = tx.send(result);
        });
        self.snap.pending_snap_job = Some(SnapBuildJob { rx });
        self.snap.snap_maps_dirty = false;
    }

    /// Poll the snap-map build job without blocking and apply the result if ready.
    pub(crate) fn poll_snap_build_job(&mut self) {
        let Some(job) = self.snap.pending_snap_job.take() else {
            return;
        };
        match job.rx.try_recv() {
            Ok(result) => {
                self.snap.snap_maps = result;
            }
            Err(TryRecvError::Empty) => {
                self.snap.pending_snap_job = Some(job);
            }
            Err(TryRecvError::Disconnected) => {
                self.snap.snap_maps = None;
            }
        }
    }

    /// Ensure snap maps exist by polling/starting background work as needed.
    pub(crate) fn ensure_snap_maps(&mut self) {
        self.poll_snap_build_job();
        if self.snap.snap_maps.is_some() {
            return;
        }
        if self.snap.snap_maps_dirty && self.snap.pending_snap_job.is_none() {
            self.start_snap_job();
        }
        self.poll_snap_build_job();
    }

    /// Return a snapped pixel location if the current input mode requests it.
    pub(crate) fn snap_pixel_if_requested(&mut self, pixel_hint: Pos2) -> Pos2 {
        self.compute_snap_candidate(pixel_hint)
            .unwrap_or(pixel_hint)
    }

    /// Compute the best snap candidate based on the current input mode.
    pub(crate) fn compute_snap_candidate(&mut self, pixel_hint: Pos2) -> Option<Pos2> {
        match self.snap.point_input_mode {
            PointInputMode::Free => None,
            PointInputMode::ContrastSnap => self.find_contrast_point(pixel_hint),
            PointInputMode::CenterlineSnap => self.find_centerline_point(pixel_hint),
        }
    }

    /// Find a high-contrast snap target near the hint position.
    pub(crate) fn find_contrast_point(&mut self, pixel_hint: Pos2) -> Option<Pos2> {
        let behavior = SnapBehavior::Contrast {
            feature_source: self.snap.snap_feature_source,
            threshold_kind: self.snap.snap_threshold_kind,
            threshold: self.snap.contrast_threshold,
        };
        self.find_snap_point_with_radius(pixel_hint, self.snap.contrast_search_radius, behavior)
    }

    /// Find the centerline of a stroke near the hint position.
    pub(crate) fn find_centerline_point(&mut self, pixel_hint: Pos2) -> Option<Pos2> {
        let behavior = SnapBehavior::Centerline {
            threshold: self.snap.centerline_threshold,
        };
        self.find_snap_point_with_radius(pixel_hint, self.snap.contrast_search_radius, behavior)
    }

    /// Find a snap point within a radius using the specified snap behavior.
    pub(crate) fn find_snap_point_with_radius(
        &mut self,
        pixel_hint: Pos2,
        radius: f32,
        behavior: SnapBehavior,
    ) -> Option<Pos2> {
        self.ensure_snap_maps();
        if self.snap.snap_maps.is_none()
            && let Some(image) = &self.image.image
        {
            // Fall back to a synchronous build when a result is needed immediately.
            let color_image = image.pixels.clone();
            let overlay_color = self.snap.snap_target_color;
            let tolerance = self.snap.snap_color_tolerance;
            self.snap.snap_maps = SnapMapCache::build(&color_image, overlay_color, tolerance);
            self.snap.pending_snap_job = None;
            self.snap.snap_maps_dirty = false;
        }
        let cache = self.snap.snap_maps.as_ref()?;
        cache.find_point(pixel_hint, radius, behavior)
    }

    /// Sample a pixel color, clamping the position to the image bounds.
    pub(crate) fn sample_image_color(&self, pixel: Pos2) -> Option<Color32> {
        let image = self.image.image.as_ref()?;
        let [w, h] = image.pixels.size;
        if w == 0 || h == 0 {
            return None;
        }
        let clamp_coord = |coord: f32, len: usize| -> usize {
            if len == 0 {
                return 0;
            }
            let max = safe_usize_to_f32(len - 1);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                coord.round().clamp(0.0, max) as usize
            }
        };
        let x = clamp_coord(pixel.x, w);
        let y = clamp_coord(pixel.y, h);
        let idx = y * w + x;
        image.pixels.pixels.get(idx).copied()
    }

    /// Pick a curve color from the image and invalidate snap maps accordingly.
    pub(crate) fn pick_curve_color_at(&mut self, pixel: Pos2) {
        if let Some(color) = self.sample_image_color(pixel) {
            self.snap.snap_target_color = color;
            self.mark_snap_maps_dirty();
            self.set_status(format!(
                "Picked curve color #{:02X}{:02X}{:02X}",
                color[0], color[1], color[2]
            ));
        } else {
            self.set_status("Unable to pick color at cursor.");
        }
    }

    /// Snap calibration angles to fixed increments and clamp to the image rectangle.
    pub(crate) fn snap_calibration_angle(
        &self,
        candidate: Pos2,
        anchor: Option<Pos2>,
        image_size: Vec2,
    ) -> Pos2 {
        if !self.calibration.calibration_angle_snap {
            return candidate;
        }
        let Some(anchor) = anchor else {
            return candidate;
        };
        let delta = candidate - anchor;
        let len = delta.length();
        if len <= f32::EPSILON {
            return candidate;
        }
        let angle = delta.y.atan2(delta.x);
        let snapped_angle =
            (angle / super::CAL_ANGLE_SNAP_STEP_RAD).round() * super::CAL_ANGLE_SNAP_STEP_RAD;
        let snapped_delta = Vec2::new(snapped_angle.cos() * len, snapped_angle.sin() * len);
        let mut snapped = anchor + snapped_delta;
        snapped.x = snapped.x.clamp(0.0, image_size.x);
        snapped.y = snapped.y.clamp(0.0, image_size.y);
        snapped
    }
}
