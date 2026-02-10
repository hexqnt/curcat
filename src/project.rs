mod checksum;
mod io;
mod model;
mod path;

pub use checksum::compute_image_crc32;
pub use io::{load_project, save_project};
pub use model::{
    AxisCalibrationRecord, CalibrationRecord, ImagePathSource, PointRecord, PolarCalibrationRecord,
    ProjectLoadOutcome, ProjectPayload, ProjectWarning, ResolvedImage,
};
pub use path::make_relative_image_path;
#[allow(unused_imports)]
pub use crate::image::{ImageTransformOp, ImageTransformRecord};

#[cfg(test)]
mod tests;
