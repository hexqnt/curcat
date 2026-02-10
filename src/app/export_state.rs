use crate::interp::InterpAlgorithm;

pub const SAMPLE_COUNT_MIN: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    Interpolated,
    RawPoints,
}

pub struct ExportState {
    pub(super) sample_count: usize,
    pub(super) export_kind: ExportKind,
    pub(super) interp_algorithm: InterpAlgorithm,
    pub(super) raw_include_distances: bool,
    pub(super) raw_include_angles: bool,
    pub(super) polar_export_include_cartesian: bool,
}
