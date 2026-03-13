use crate::interp::InterpAlgorithm;
use crate::snap::{SnapFeatureSource, SnapThresholdKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiLanguage {
    En,
    Ru,
}

impl UiLanguage {
    pub const ALL: [Self; 2] = [Self::En, Self::Ru];

    pub fn detect_system() -> Self {
        for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = std::env::var(var)
                && let Some(lang) = Self::from_locale_tag(&value)
            {
                return lang;
            }
        }
        Self::En
    }

    pub fn from_locale_tag(tag: &str) -> Option<Self> {
        let normalized = tag
            .split('.')
            .next()
            .unwrap_or(tag)
            .split('@')
            .next()
            .unwrap_or(tag)
            .to_ascii_lowercase();
        if normalized.starts_with("ru") {
            Some(Self::Ru)
        } else if normalized.is_empty() {
            None
        } else {
            Some(Self::En)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextKey {
    File,
    Appearance,
    OpenImage,
    OpenImageHover,
    PasteImage,
    PasteImageHover,
    LoadProject,
    LoadProjectHover,
    SaveProject,
    SaveProjectHover,
    HideSide,
    ShowSide,
    HideSidePanel,
    ShowSidePanel,
    SidePanelPosition,
    Left,
    Right,
    ToggleSidePanelHover,
    PointsStats,
    PointsStatsHover,
    Filters,
    FiltersHover,
    AutoTrace,
    AutoTraceHover,
    ImageInfo,
    ImageInfoHover,
    TransformsTogether,
    Rotate90Ccw,
    Rotate90Cw,
    FlipHorizontally,
    FlipVertically,
    FlipH,
    FlipV,
    Zoom,
    ZoomHover,
    ZoomPresetsHover,
    Fit,
    FitHover,
    ResetView,
    ResetViewHover,
    PanWithMiddleButton,
    MmbPan,
    MmbPanHover,
    ClearPoints,
    ClearPointsHover,
    Undo,
    UndoHover,
    LanguageSwitcherHover,
    PointInput,
    Free,
    ContrastSnap,
    CenterlineSnap,
    FreeHover,
    ContrastSnapHover,
    CenterlineSnapHover,
    FeatureSource,
    FeatureSourceHover,
    ThresholdMode,
    ThresholdModeHover,
    GradientOnly,
    FeatureScore,
    GradientOnlyHover,
    FeatureScoreHover,
    Threshold,
    ThresholdHigherHint,
    CenterlineDetectsFlat,
    CenterlineDetectsFlatHover,
    StrengthThreshold,
    StrengthThresholdHover,
    HigherOnlyWellDefined,
    BestResultsColorSample,
    PreviewCircleHint,
    ShowPointConnections,
    ShowPointConnectionsHover,
    SearchRadiusPx,
    SearchRadiusHover,
    RadiusUsedToLookForCandidates,
    CurveColor,
    CurveColorHover,
    PickFromImage,
    PickFromImageHover,
    Tolerance,
    ToleranceHover,
    SnapOverlayColor,
    SnapOverlayColorHover,
    Calibration,
    CoordinateSystem,
    CoordinateSystemHover,
    Cartesian,
    Polar,
    CoordSystemForCalibrationExport,
    Snap15,
    Snap15Hover,
    ShowCalibrationOverlay,
    ShowCalibrationOverlayHover,
    Unit,
    UnitHover,
    Scale,
    ScaleHover,
    AxisValueTypeHover,
    AxisScaleHover,
    XAxis,
    YAxis,
    XAxisCalibrationHover,
    YAxisCalibrationHover,
    Origin,
    PickOrigin,
    PickOriginHover,
    Center,
    CenterOriginHover,
    Radius,
    Angle,
    RadiusScaleHover,
    RadiusScaleChoiceHover,
    AngleUnit,
    AngleUnitHover,
    Direction,
    DirectionHover,
    MappingOk,
    MappingOkHover,
    MappingIncomplete,
    MappingIncompleteHover,
    MappingOkAxisHover,
    MappingIncompleteAxisHover,
    RadiusCalibrationHover,
    AngleCalibrationHover,
    ExportPoints,
    InterpolatedCurve,
    RawPickedPoints,
    InterpolatedCurveHover,
    RawPickedPointsHover,
    Interpolation,
    InterpolationHover,
    InterpolationAlgorithmHover,
    Samples,
    SamplesHover,
    Count,
    Auto,
    AutoSamplesHover,
    ExtraColumns,
    ExtraColumnsHover,
    IncludeDistanceToPrev,
    IncludeDistanceToPrevHover,
    IncludeAngleDeg,
    IncludeAngleDegHover,
    IncludeCartesianColumns,
    IncludeCartesianColumnsHover,
    ExportCsv,
    ExportJson,
    ExportRon,
    ExportExcel,
    AddPointsBeforeExport,
    CompleteCalibrationBeforeExportCartesian,
    CompleteCalibrationBeforeExportPolar,
    ExportToFormat,
    ImageFiltersWindow,
    FiltersAffectDisplayOnly,
    Brightness,
    Contrast,
    Gamma,
    Invert,
    ThresholdEnabled,
    Level,
    BlurRadius,
    ResetFilters,
    LoadImageToAdjustFilters,
    AutoTraceWindow,
    AutoTraceIntro,
    TraceFromClick,
    LoadImageFirst,
    CompleteCalibrationBeforeTracing,
    AutoTraceCartesianOnly,
    SelectSnapBeforeTracing,
    ClickStartPoint,
    DirectionShort,
    StepPx,
    SearchRadiusShort,
    MaxPoints,
    GapTolerance,
    GapToleranceHover,
    MinSpacingPx,
    ImageInfoWindow,
    PointsInfoWindow,
    FileSection,
    ImageSection,
    Source,
    Name,
    Path,
    Size,
    SizeUnknown,
    Modified,
    ModifiedUnknown,
    NoFileMetadataForImage,
    Dimensions,
    AspectRatio,
    AspectRatioNa,
    Pixels,
    RgbaMemoryEstimate,
    CurrentZoom,
    LoadImageToInspectMetadata,
    Points,
    Placed,
    AddPointsToSeeStats,
    CalibratedPairsNeedsAxes,
    Ranges,
    CalibrationSection,
    Geometry,
    NoData,
    CalibrateAxisToSeeNumericValues,
    XAxisLength,
    YAxisLength,
    XAxisNotSet,
    YAxisNotSet,
    AngleBetweenAxes,
    AddBothAxesToMeasureOrthogonality,
    OriginNotSet,
    RadiusPoints,
    RadiusPointsSetNeedOrigin,
    RadiusPointsNotSet,
    AngleValues,
    AnglePointsSetValuesInvalid,
    AnglePointsNotSet,
    PixelBounds,
    Span,
    NoPointsForGeometryStats,
    AverageStep,
    TotalPolylineLength,
    ProjectWarningsWindow,
    ProjectWarningsIntro,
    ContinueAnyway,
    Cancel,
    StatusPoints,
    OpenProjectDialogTitle,
    CurcatProjectFilterLabel,
    SaveProjectDialogTitle,
    DefaultProjectName,
    OpenImageDialogTitle,
    ImageFilterAll,
    PickedLabel,
    LoadingImageWithName,
    LoadingImage,
    DropHint,
    Version,
}

impl TextKey {
    #[cfg(test)]
    pub const ALL: [Self; 243] = [
        Self::File,
        Self::Appearance,
        Self::OpenImage,
        Self::OpenImageHover,
        Self::PasteImage,
        Self::PasteImageHover,
        Self::LoadProject,
        Self::LoadProjectHover,
        Self::SaveProject,
        Self::SaveProjectHover,
        Self::HideSide,
        Self::ShowSide,
        Self::HideSidePanel,
        Self::ShowSidePanel,
        Self::SidePanelPosition,
        Self::Left,
        Self::Right,
        Self::ToggleSidePanelHover,
        Self::PointsStats,
        Self::PointsStatsHover,
        Self::Filters,
        Self::FiltersHover,
        Self::AutoTrace,
        Self::AutoTraceHover,
        Self::ImageInfo,
        Self::ImageInfoHover,
        Self::TransformsTogether,
        Self::Rotate90Ccw,
        Self::Rotate90Cw,
        Self::FlipHorizontally,
        Self::FlipVertically,
        Self::FlipH,
        Self::FlipV,
        Self::Zoom,
        Self::ZoomHover,
        Self::ZoomPresetsHover,
        Self::Fit,
        Self::FitHover,
        Self::ResetView,
        Self::ResetViewHover,
        Self::PanWithMiddleButton,
        Self::MmbPan,
        Self::MmbPanHover,
        Self::ClearPoints,
        Self::ClearPointsHover,
        Self::Undo,
        Self::UndoHover,
        Self::LanguageSwitcherHover,
        Self::PointInput,
        Self::Free,
        Self::ContrastSnap,
        Self::CenterlineSnap,
        Self::FreeHover,
        Self::ContrastSnapHover,
        Self::CenterlineSnapHover,
        Self::FeatureSource,
        Self::FeatureSourceHover,
        Self::ThresholdMode,
        Self::ThresholdModeHover,
        Self::GradientOnly,
        Self::FeatureScore,
        Self::GradientOnlyHover,
        Self::FeatureScoreHover,
        Self::Threshold,
        Self::ThresholdHigherHint,
        Self::CenterlineDetectsFlat,
        Self::CenterlineDetectsFlatHover,
        Self::StrengthThreshold,
        Self::StrengthThresholdHover,
        Self::HigherOnlyWellDefined,
        Self::BestResultsColorSample,
        Self::PreviewCircleHint,
        Self::ShowPointConnections,
        Self::ShowPointConnectionsHover,
        Self::SearchRadiusPx,
        Self::SearchRadiusHover,
        Self::RadiusUsedToLookForCandidates,
        Self::CurveColor,
        Self::CurveColorHover,
        Self::PickFromImage,
        Self::PickFromImageHover,
        Self::Tolerance,
        Self::ToleranceHover,
        Self::SnapOverlayColor,
        Self::SnapOverlayColorHover,
        Self::Calibration,
        Self::CoordinateSystem,
        Self::CoordinateSystemHover,
        Self::Cartesian,
        Self::Polar,
        Self::CoordSystemForCalibrationExport,
        Self::Snap15,
        Self::Snap15Hover,
        Self::ShowCalibrationOverlay,
        Self::ShowCalibrationOverlayHover,
        Self::Unit,
        Self::UnitHover,
        Self::Scale,
        Self::ScaleHover,
        Self::AxisValueTypeHover,
        Self::AxisScaleHover,
        Self::XAxis,
        Self::YAxis,
        Self::XAxisCalibrationHover,
        Self::YAxisCalibrationHover,
        Self::Origin,
        Self::PickOrigin,
        Self::PickOriginHover,
        Self::Center,
        Self::CenterOriginHover,
        Self::Radius,
        Self::Angle,
        Self::RadiusScaleHover,
        Self::RadiusScaleChoiceHover,
        Self::AngleUnit,
        Self::AngleUnitHover,
        Self::Direction,
        Self::DirectionHover,
        Self::MappingOk,
        Self::MappingOkHover,
        Self::MappingIncomplete,
        Self::MappingIncompleteHover,
        Self::MappingOkAxisHover,
        Self::MappingIncompleteAxisHover,
        Self::RadiusCalibrationHover,
        Self::AngleCalibrationHover,
        Self::ExportPoints,
        Self::InterpolatedCurve,
        Self::RawPickedPoints,
        Self::InterpolatedCurveHover,
        Self::RawPickedPointsHover,
        Self::Interpolation,
        Self::InterpolationHover,
        Self::InterpolationAlgorithmHover,
        Self::Samples,
        Self::SamplesHover,
        Self::Count,
        Self::Auto,
        Self::AutoSamplesHover,
        Self::ExtraColumns,
        Self::ExtraColumnsHover,
        Self::IncludeDistanceToPrev,
        Self::IncludeDistanceToPrevHover,
        Self::IncludeAngleDeg,
        Self::IncludeAngleDegHover,
        Self::IncludeCartesianColumns,
        Self::IncludeCartesianColumnsHover,
        Self::ExportCsv,
        Self::ExportJson,
        Self::ExportRon,
        Self::ExportExcel,
        Self::AddPointsBeforeExport,
        Self::CompleteCalibrationBeforeExportCartesian,
        Self::CompleteCalibrationBeforeExportPolar,
        Self::ExportToFormat,
        Self::ImageFiltersWindow,
        Self::FiltersAffectDisplayOnly,
        Self::Brightness,
        Self::Contrast,
        Self::Gamma,
        Self::Invert,
        Self::ThresholdEnabled,
        Self::Level,
        Self::BlurRadius,
        Self::ResetFilters,
        Self::LoadImageToAdjustFilters,
        Self::AutoTraceWindow,
        Self::AutoTraceIntro,
        Self::TraceFromClick,
        Self::LoadImageFirst,
        Self::CompleteCalibrationBeforeTracing,
        Self::AutoTraceCartesianOnly,
        Self::SelectSnapBeforeTracing,
        Self::ClickStartPoint,
        Self::DirectionShort,
        Self::StepPx,
        Self::SearchRadiusShort,
        Self::MaxPoints,
        Self::GapTolerance,
        Self::GapToleranceHover,
        Self::MinSpacingPx,
        Self::ImageInfoWindow,
        Self::PointsInfoWindow,
        Self::FileSection,
        Self::ImageSection,
        Self::Source,
        Self::Name,
        Self::Path,
        Self::Size,
        Self::SizeUnknown,
        Self::Modified,
        Self::ModifiedUnknown,
        Self::NoFileMetadataForImage,
        Self::Dimensions,
        Self::AspectRatio,
        Self::AspectRatioNa,
        Self::Pixels,
        Self::RgbaMemoryEstimate,
        Self::CurrentZoom,
        Self::LoadImageToInspectMetadata,
        Self::Points,
        Self::Placed,
        Self::AddPointsToSeeStats,
        Self::CalibratedPairsNeedsAxes,
        Self::Ranges,
        Self::CalibrationSection,
        Self::Geometry,
        Self::NoData,
        Self::CalibrateAxisToSeeNumericValues,
        Self::XAxisLength,
        Self::YAxisLength,
        Self::XAxisNotSet,
        Self::YAxisNotSet,
        Self::AngleBetweenAxes,
        Self::AddBothAxesToMeasureOrthogonality,
        Self::OriginNotSet,
        Self::RadiusPoints,
        Self::RadiusPointsSetNeedOrigin,
        Self::RadiusPointsNotSet,
        Self::AngleValues,
        Self::AnglePointsSetValuesInvalid,
        Self::AnglePointsNotSet,
        Self::PixelBounds,
        Self::Span,
        Self::NoPointsForGeometryStats,
        Self::AverageStep,
        Self::TotalPolylineLength,
        Self::ProjectWarningsWindow,
        Self::ProjectWarningsIntro,
        Self::ContinueAnyway,
        Self::Cancel,
        Self::StatusPoints,
        Self::OpenProjectDialogTitle,
        Self::CurcatProjectFilterLabel,
        Self::SaveProjectDialogTitle,
        Self::DefaultProjectName,
        Self::OpenImageDialogTitle,
        Self::ImageFilterAll,
        Self::PickedLabel,
        Self::LoadingImageWithName,
        Self::LoadingImage,
        Self::DropHint,
        Self::Version,
    ];
}

#[derive(Debug, Clone, Copy)]
pub struct I18n {
    lang: UiLanguage,
}

impl I18n {
    pub const fn new(lang: UiLanguage) -> Self {
        Self { lang }
    }

    pub const fn text(self, key: TextKey) -> &'static str {
        let en = en_text(key);
        let ru = ru_text(key);
        choose_text(self.lang, en, ru)
    }

    pub fn format_status_picking(self, label: &str) -> String {
        match self.lang {
            UiLanguage::En => format!("{label}… (Esc to cancel)"),
            UiLanguage::Ru => format!("{label}… (Esc чтобы отменить)"),
        }
    }

    pub fn format_picked(self, label: &str) -> String {
        format!("{}: {label}", self.text(TextKey::PickedLabel))
    }

    pub fn format_points_count(self, points_count: usize) -> String {
        format!("{}: {points_count}", self.text(TextKey::StatusPoints))
    }

    pub fn format_loading_image(self, description: &str) -> String {
        format!(
            "{}: {description}…",
            self.text(TextKey::LoadingImageWithName)
        )
    }

    pub fn format_fit_view(self, percent: f32) -> String {
        match self.lang {
            UiLanguage::En => format!("Fit view: {:.0}%", percent),
            UiLanguage::Ru => format!("Подогнать вид: {:.0}%", percent),
        }
    }

    pub fn format_auto_trace_added(self, count: usize) -> String {
        match self.lang {
            UiLanguage::En => format!("Auto-trace added {count} points."),
            UiLanguage::Ru => format!("Авто-трассировка добавила {count} точек."),
        }
    }

    pub fn format_sample_count_tuned(self, count: usize) -> String {
        match self.lang {
            UiLanguage::En => format!("Sample count auto-tuned to {count}."),
            UiLanguage::Ru => format!("Число семплов автоматически подобрано: {count}."),
        }
    }

    pub fn format_loaded_name(self, name: &str) -> String {
        match self.lang {
            UiLanguage::En => format!("Loaded {name}"),
            UiLanguage::Ru => format!("Загружено: {name}"),
        }
    }

    pub fn format_exported(self, format_label: &str) -> String {
        match self.lang {
            UiLanguage::En => format!("{format_label} exported."),
            UiLanguage::Ru => format!("Экспортировано в {format_label}."),
        }
    }

    pub fn format_export_failed(self, format_label: &str, err: &str) -> String {
        match self.lang {
            UiLanguage::En => format!("{format_label} export failed: {err}"),
            UiLanguage::Ru => format!("Ошибка экспорта {format_label}: {err}"),
        }
    }

    pub fn format_source(self, value: &str) -> String {
        format!("{}: {value}", self.text(TextKey::Source))
    }

    pub fn format_name(self, value: &str) -> String {
        format!("{}: {value}", self.text(TextKey::Name))
    }

    pub fn format_path(self, value: &str) -> String {
        format!("{}: {value}", self.text(TextKey::Path))
    }

    pub fn format_size(self, value: &str) -> String {
        format!("{}: {value}", self.text(TextKey::Size))
    }

    pub fn format_modified(self, value: &str) -> String {
        format!("{}: {value}", self.text(TextKey::Modified))
    }

    pub fn format_dimensions(self, w: usize, h: usize) -> String {
        format!("{}: {w} × {h} px", self.text(TextKey::Dimensions))
    }

    pub fn format_aspect_ratio(self, value: &str) -> String {
        format!("{}: {value}", self.text(TextKey::AspectRatio))
    }

    pub fn format_pixels(self, total: u64, mega_pixels: f64) -> String {
        match self.lang {
            UiLanguage::En => format!(
                "{}: {total} ({mega_pixels:.2} MP)",
                self.text(TextKey::Pixels)
            ),
            UiLanguage::Ru => format!(
                "{}: {total} ({mega_pixels:.2} МП)",
                self.text(TextKey::Pixels)
            ),
        }
    }

    pub fn format_rgba_memory_estimate(self, readable: &str, bytes: u64) -> String {
        match self.lang {
            UiLanguage::En => format!(
                "{}: {readable} ({bytes} bytes)",
                self.text(TextKey::RgbaMemoryEstimate)
            ),
            UiLanguage::Ru => format!(
                "{}: {readable} ({bytes} байт)",
                self.text(TextKey::RgbaMemoryEstimate)
            ),
        }
    }

    pub fn format_current_zoom(self, zoom: &str) -> String {
        format!("{}: {zoom}", self.text(TextKey::CurrentZoom))
    }

    pub fn format_placed_points(self, total: usize) -> String {
        format!("{}: {total}", self.text(TextKey::Placed))
    }

    pub fn format_calibrated_pairs(self, calibrated: usize) -> String {
        format!(
            "{}: {calibrated}",
            self.text(TextKey::CalibratedPairsNeedsAxes)
        )
    }

    pub fn format_axis_range(self, label: &str, min: &str, max: &str, span: &str) -> String {
        format!("{label}: {min} … {max} (Δ {span})")
    }

    pub fn format_axis_pixels(self, label: &str, min: f32, max: f32, span: f32) -> String {
        match self.lang {
            UiLanguage::En => format!("{label} pixels: {min:.1} … {max:.1} (Δ {span:.1} px)"),
            UiLanguage::Ru => format!("{label} пиксели: {min:.1} … {max:.1} (Δ {span:.1} px)"),
        }
    }

    pub fn format_axis_pixels_only(self, label: &str, min: f32, max: f32, span: f32) -> String {
        format!("{label} (px): {min:.1} … {max:.1} (Δ {span:.1} px)")
    }

    pub fn format_x_axis_length(self, len: f32) -> String {
        format!("{}: {len:.1} px", self.text(TextKey::XAxisLength))
    }

    pub fn format_y_axis_length(self, len: f32) -> String {
        format!("{}: {len:.1} px", self.text(TextKey::YAxisLength))
    }

    pub fn format_axes_angle(self, actual: f32, delta: f32) -> String {
        match self.lang {
            UiLanguage::En => {
                format!(
                    "{}: {actual:.2}° (offset {delta:.2}° from 90°)",
                    self.text(TextKey::AngleBetweenAxes)
                )
            }
            UiLanguage::Ru => {
                format!(
                    "{}: {actual:.2}° (смещение {delta:.2}° от 90°)",
                    self.text(TextKey::AngleBetweenAxes)
                )
            }
        }
    }

    pub fn format_origin_coords(self, x: f32, y: f32) -> String {
        format!("{}: @ ({x:.1}, {y:.1})", self.text(TextKey::Origin))
    }

    pub fn format_radius_points(self, d1: f32, d2: f32) -> String {
        format!(
            "{}: R1 {d1:.1} px, R2 {d2:.1} px",
            self.text(TextKey::RadiusPoints)
        )
    }

    pub fn format_angle_values(self, v1: &str, v2: &str, unit: &str) -> String {
        format!("{}: {v1} … {v2} {unit}", self.text(TextKey::AngleValues))
    }

    pub fn format_pixel_bounds(self, x_min: f32, x_max: f32, y_min: f32, y_max: f32) -> String {
        format!(
            "{}: x {x_min:.1}…{x_max:.1}, y {y_min:.1}…{y_max:.1}",
            self.text(TextKey::PixelBounds)
        )
    }

    pub fn format_span(self, x: f32, y: f32) -> String {
        format!("{}: {x:.1} × {y:.1} px", self.text(TextKey::Span))
    }

    pub fn format_average_step(self, avg: f32) -> String {
        format!("{}: {avg:.1} px", self.text(TextKey::AverageStep))
    }

    pub fn format_total_polyline_length(self, total: f32) -> String {
        format!("{}: {total:.1} px", self.text(TextKey::TotalPolylineLength))
    }

    pub fn format_loading_image_row(self, description: &str) -> String {
        match self.lang {
            UiLanguage::En => format!("Loading image: {description}…"),
            UiLanguage::Ru => format!("Загрузка изображения: {description}…"),
        }
    }

    pub fn format_version(self, version: &str) -> String {
        format!("{} {version}", self.text(TextKey::Version))
    }

    pub const fn interp_algorithm_label(self, algo: InterpAlgorithm) -> &'static str {
        match (self.lang, algo) {
            (UiLanguage::En, InterpAlgorithm::Linear) => "Linear",
            (UiLanguage::En, InterpAlgorithm::StepHold) => "Step (previous)",
            (UiLanguage::En, InterpAlgorithm::NaturalCubic) => "Natural cubic spline",
            (UiLanguage::Ru, InterpAlgorithm::Linear) => "Линейная",
            (UiLanguage::Ru, InterpAlgorithm::StepHold) => "Ступенчатая (пред.)",
            (UiLanguage::Ru, InterpAlgorithm::NaturalCubic) => "Натуральный кубический сплайн",
        }
    }

    pub const fn snap_feature_source_label(self, source: SnapFeatureSource) -> &'static str {
        match (self.lang, source) {
            (UiLanguage::En, SnapFeatureSource::LumaGradient) => "Luma gradient",
            (UiLanguage::En, SnapFeatureSource::ColorMatch) => "Color mask",
            (UiLanguage::En, SnapFeatureSource::Hybrid) => "Gradient + color",
            (UiLanguage::Ru, SnapFeatureSource::LumaGradient) => "Градиент яркости",
            (UiLanguage::Ru, SnapFeatureSource::ColorMatch) => "Цветовая маска",
            (UiLanguage::Ru, SnapFeatureSource::Hybrid) => "Градиент + цвет",
        }
    }

    pub const fn snap_threshold_kind_label(self, kind: SnapThresholdKind) -> &'static str {
        match (self.lang, kind) {
            (_, SnapThresholdKind::Gradient) => self.text(TextKey::GradientOnly),
            (_, SnapThresholdKind::Score) => self.text(TextKey::FeatureScore),
        }
    }
}

const fn choose_text(lang: UiLanguage, en: &'static str, ru: Option<&'static str>) -> &'static str {
    match lang {
        UiLanguage::En => en,
        UiLanguage::Ru => match ru {
            Some(value) => value,
            None => en,
        },
    }
}

const fn en_text(key: TextKey) -> &'static str {
    match key {
        TextKey::File => "File",
        TextKey::Appearance => "Appearance",
        TextKey::OpenImage => "Open image…",
        TextKey::OpenImageHover => {
            "Open an image (Ctrl+O). You can also drag & drop into the center."
        }
        TextKey::PasteImage => "Paste image",
        TextKey::PasteImageHover => "Paste image from clipboard (Ctrl+V)",
        TextKey::LoadProject => "Load project…",
        TextKey::LoadProjectHover => "Load a saved Curcat project (Ctrl+Shift+P)",
        TextKey::SaveProject => "Save project",
        TextKey::SaveProjectHover => "Save the current session as a Curcat project (Ctrl+S)",
        TextKey::HideSide => "Hide side",
        TextKey::ShowSide => "Show side",
        TextKey::HideSidePanel => "Hide side panel",
        TextKey::ShowSidePanel => "Show side panel",
        TextKey::SidePanelPosition => "Side panel position",
        TextKey::Left => "Left",
        TextKey::Right => "Right",
        TextKey::ToggleSidePanelHover => "Toggle side panel (Ctrl+B) and set position",
        TextKey::PointsStats => "Points stats",
        TextKey::PointsStatsHover => "Show stats for picked points",
        TextKey::Filters => "Filters",
        TextKey::FiltersHover => "Show image filters (Ctrl+Shift+F)",
        TextKey::AutoTrace => "Auto-trace",
        TextKey::AutoTraceHover => "Show auto-trace controls (Ctrl+Shift+T)",
        TextKey::ImageInfo => "Image info",
        TextKey::ImageInfoHover => "Show file & image details (Ctrl+I)",
        TextKey::TransformsTogether => "Transforms image, points, and calibration together.",
        TextKey::Rotate90Ccw => "Rotate 90° counter-clockwise.",
        TextKey::Rotate90Cw => "Rotate 90° clockwise.",
        TextKey::FlipHorizontally => "Flip horizontally.",
        TextKey::FlipVertically => "Flip vertically.",
        TextKey::FlipH => "Flip H",
        TextKey::FlipV => "Flip V",
        TextKey::Zoom => "Zoom:",
        TextKey::ZoomHover => "Choose a preset zoom level",
        TextKey::ZoomPresetsHover => "Zoom presets (percent)",
        TextKey::Fit => "Fit",
        TextKey::FitHover => "Fit the image into the viewport (Ctrl+F)",
        TextKey::ResetView => "Reset view",
        TextKey::ResetViewHover => "Reset zoom to 100% and pan to origin (Ctrl+R)",
        TextKey::PanWithMiddleButton => "Pan with middle mouse button",
        TextKey::MmbPan => "MMB pan",
        TextKey::MmbPanHover => "Enable/disable middle-button panning",
        TextKey::ClearPoints => "Clear points",
        TextKey::ClearPointsHover => "Clear all points (Ctrl+Shift+D)",
        TextKey::Undo => "Undo",
        TextKey::UndoHover => "Undo last point (Ctrl+Z)",
        TextKey::LanguageSwitcherHover => "UI language",
        TextKey::PointInput => "Point input",
        TextKey::Free => "Free",
        TextKey::ContrastSnap => "Contrast snap",
        TextKey::CenterlineSnap => "Centerline snap",
        TextKey::FreeHover => "Place points exactly where you click",
        TextKey::ContrastSnapHover => {
            "Snap to the nearest high-contrast area inside the search radius"
        }
        TextKey::CenterlineSnapHover => "Snap to the centerline of the color-matched curve",
        TextKey::FeatureSource => "Feature source",
        TextKey::FeatureSourceHover => {
            "Choose what the snapper looks at when searching for a candidate"
        }
        TextKey::ThresholdMode => "Threshold mode",
        TextKey::ThresholdModeHover => {
            "Select how the detector decides if a pixel is strong enough"
        }
        TextKey::GradientOnly => "Gradient only",
        TextKey::FeatureScore => "Feature score",
        TextKey::GradientOnlyHover => "Compare threshold against raw gradient strength",
        TextKey::FeatureScoreHover => "Compare threshold against combined feature score",
        TextKey::Threshold => "threshold",
        TextKey::ThresholdHigherHint => "Higher = snap only to strong candidates",
        TextKey::CenterlineDetectsFlat => "Centerline detects flat color interiors",
        TextKey::CenterlineDetectsFlatHover => {
            "Pick the curve color to help the detector focus on the intended line"
        }
        TextKey::StrengthThreshold => "Strength threshold",
        TextKey::StrengthThresholdHover => "Rejects weak centerline matches",
        TextKey::HigherOnlyWellDefined => "Higher = snap only to well-defined line centers",
        TextKey::BestResultsColorSample => {
            "Best results come from sampling the curve color before snapping."
        }
        TextKey::PreviewCircleHint => {
            "The preview circle in the image shows the area that will be scanned."
        }
        TextKey::ShowPointConnections => "Show point connections",
        TextKey::ShowPointConnectionsHover => {
            "Show or hide the lines between picked points (not calibration lines)."
        }
        TextKey::SearchRadiusPx => "Search radius (px)",
        TextKey::SearchRadiusHover => {
            "Measured in image pixels; smaller values keep snapping near the cursor"
        }
        TextKey::RadiusUsedToLookForCandidates => "Radius used to look for snap candidates",
        TextKey::CurveColor => "Curve color:",
        TextKey::CurveColorHover => "Target color for the curve",
        TextKey::PickFromImage => "Pick from image",
        TextKey::PickFromImageHover => "Click, then select a pixel on the image",
        TextKey::Tolerance => "tolerance",
        TextKey::ToleranceHover => "How far the pixel color may deviate from the picked color",
        TextKey::SnapOverlayColor => "Snap overlay color",
        TextKey::SnapOverlayColorHover => {
            "Choices are derived from the image to keep the snap preview visible"
        }
        TextKey::Calibration => "Calibration",
        TextKey::CoordinateSystem => "Coordinate system:",
        TextKey::CoordinateSystemHover => "Choose between Cartesian (X/Y) or Polar (angle/radius)",
        TextKey::Cartesian => "Cartesian",
        TextKey::Polar => "Polar",
        TextKey::CoordSystemForCalibrationExport => "Coordinate system for calibration and export",
        TextKey::Snap15 => "15° snap",
        TextKey::Snap15Hover => "Snap calibration lines to 15° steps while picking or dragging",
        TextKey::ShowCalibrationOverlay => "Show calibration overlay",
        TextKey::ShowCalibrationOverlayHover => {
            "Show or hide calibration lines and point labels on the image"
        }
        TextKey::Unit => "Unit:",
        TextKey::UnitHover => "Value type for the axis (Float/DateTime)",
        TextKey::Scale => "Scale:",
        TextKey::ScaleHover => "Axis scale (Linear/Log10)",
        TextKey::AxisValueTypeHover => "Choose the axis value type",
        TextKey::AxisScaleHover => "Choose the axis scale",
        TextKey::XAxis => "X axis",
        TextKey::YAxis => "Y axis",
        TextKey::XAxisCalibrationHover => "X axis calibration",
        TextKey::YAxisCalibrationHover => "Y axis calibration",
        TextKey::Origin => "Origin",
        TextKey::PickOrigin => "Pick Origin",
        TextKey::PickOriginHover => "Click, then pick the origin on the image",
        TextKey::Center => "Center",
        TextKey::CenterOriginHover => "Set origin to image center",
        TextKey::Radius => "Radius",
        TextKey::Angle => "Angle",
        TextKey::RadiusScaleHover => "Radius scale (Linear/Log10)",
        TextKey::RadiusScaleChoiceHover => "Choose the radius scale",
        TextKey::AngleUnit => "Angle unit:",
        TextKey::AngleUnitHover => "Units for angle values (degrees or radians)",
        TextKey::Direction => "Direction:",
        TextKey::DirectionHover => "Direction of increasing angle",
        TextKey::MappingOk => "Mapping: OK",
        TextKey::MappingOkHover => "Calibration complete — you can pick points and export",
        TextKey::MappingIncomplete => "Mapping: incomplete or invalid",
        TextKey::MappingIncompleteHover => "Provide two points and valid values to calibrate",
        TextKey::MappingOkAxisHover => "Calibration complete for this axis",
        TextKey::MappingIncompleteAxisHover => "Provide origin, two points, and valid values",
        TextKey::RadiusCalibrationHover => "Radius calibration",
        TextKey::AngleCalibrationHover => "Angle calibration",
        TextKey::ExportPoints => "Export points",
        TextKey::InterpolatedCurve => "Interpolated curve",
        TextKey::RawPickedPoints => "Raw picked points",
        TextKey::InterpolatedCurveHover => "Export evenly spaced samples of the curve",
        TextKey::RawPickedPointsHover => "Export only the points you clicked, in order",
        TextKey::Interpolation => "Interpolation:",
        TextKey::InterpolationHover => "Choose how to interpolate between control points",
        TextKey::InterpolationAlgorithmHover => {
            "Algorithm used to generate the interpolated samples"
        }
        TextKey::Samples => "Samples:",
        TextKey::SamplesHover => "Number of evenly spaced samples to export",
        TextKey::Count => "count",
        TextKey::Auto => "Auto",
        TextKey::AutoSamplesHover => {
            "Automatically choose a sample count based on curve smoothness"
        }
        TextKey::ExtraColumns => "Extra columns:",
        TextKey::ExtraColumnsHover => "Optional metrics for the picked points",
        TextKey::IncludeDistanceToPrev => "Include distance to previous point",
        TextKey::IncludeDistanceToPrevHover => {
            "Adds a column with distances between consecutive picked points"
        }
        TextKey::IncludeAngleDeg => "Include angle (deg)",
        TextKey::IncludeAngleDegHover => {
            "Adds a column with angles at each interior point (first/last stay empty)"
        }
        TextKey::IncludeCartesianColumns => "Include Cartesian x/y columns",
        TextKey::IncludeCartesianColumnsHover => {
            "Adds x and y columns computed from angle and radius"
        }
        TextKey::ExportCsv => "Export CSV…",
        TextKey::ExportJson => "Export JSON…",
        TextKey::ExportRon => "Export RON…",
        TextKey::ExportExcel => "Export Excel…",
        TextKey::AddPointsBeforeExport => "Add points before exporting to",
        TextKey::CompleteCalibrationBeforeExportCartesian => {
            "Complete both axis calibrations before exporting to"
        }
        TextKey::CompleteCalibrationBeforeExportPolar => {
            "Complete origin, radius, and angle calibration before exporting to"
        }
        TextKey::ExportToFormat => "Export data to",
        TextKey::ImageFiltersWindow => "Image filters",
        TextKey::FiltersAffectDisplayOnly => "Affects display and snapping only.",
        TextKey::Brightness => "brightness",
        TextKey::Contrast => "contrast",
        TextKey::Gamma => "gamma",
        TextKey::Invert => "invert",
        TextKey::ThresholdEnabled => "threshold",
        TextKey::Level => "level",
        TextKey::BlurRadius => "blur radius",
        TextKey::ResetFilters => "Reset filters",
        TextKey::LoadImageToAdjustFilters => "Load an image to adjust filters.",
        TextKey::AutoTraceWindow => "Auto-trace",
        TextKey::AutoTraceIntro => "Click once to trace a curve segment automatically.",
        TextKey::TraceFromClick => "Trace from click",
        TextKey::LoadImageFirst => "Load an image first.",
        TextKey::CompleteCalibrationBeforeTracing => "Complete calibration before tracing.",
        TextKey::AutoTraceCartesianOnly => "Auto-trace currently supports Cartesian mode only.",
        TextKey::SelectSnapBeforeTracing => {
            "Select Contrast snap or Centerline snap before tracing."
        }
        TextKey::ClickStartPoint => "Click, then pick a start point on the image.",
        TextKey::DirectionShort => "Direction",
        TextKey::StepPx => "step (px)",
        TextKey::SearchRadiusShort => "search radius (px)",
        TextKey::MaxPoints => "max points",
        TextKey::GapTolerance => "gap tolerance",
        TextKey::GapToleranceHover => "How many missed steps to tolerate before stopping.",
        TextKey::MinSpacingPx => "min spacing (px)",
        TextKey::ImageInfoWindow => "Image info",
        TextKey::PointsInfoWindow => "Points info",
        TextKey::FileSection => "File",
        TextKey::ImageSection => "Image",
        TextKey::Source => "Source",
        TextKey::Name => "Name",
        TextKey::Path => "Path",
        TextKey::Size => "Size",
        TextKey::SizeUnknown => "Unknown",
        TextKey::Modified => "Modified",
        TextKey::ModifiedUnknown => "Unknown",
        TextKey::NoFileMetadataForImage => "No captured file metadata for this image.",
        TextKey::Dimensions => "Dimensions",
        TextKey::AspectRatio => "Aspect ratio",
        TextKey::AspectRatioNa => "n/a",
        TextKey::Pixels => "Pixels",
        TextKey::RgbaMemoryEstimate => "RGBA memory estimate",
        TextKey::CurrentZoom => "Current zoom",
        TextKey::LoadImageToInspectMetadata => "Load an image to inspect its metadata.",
        TextKey::Points => "Points",
        TextKey::Placed => "Placed",
        TextKey::AddPointsToSeeStats => "Add points to see stats.",
        TextKey::CalibratedPairsNeedsAxes => "Calibrated pairs",
        TextKey::Ranges => "Ranges",
        TextKey::CalibrationSection => "Calibration",
        TextKey::Geometry => "Geometry",
        TextKey::NoData => "no data",
        TextKey::CalibrateAxisToSeeNumericValues => "Calibrate this axis to see numeric values.",
        TextKey::XAxisLength => "X axis length",
        TextKey::YAxisLength => "Y axis length",
        TextKey::XAxisNotSet => "X axis not set",
        TextKey::YAxisNotSet => "Y axis not set",
        TextKey::AngleBetweenAxes => "Angle between axes",
        TextKey::AddBothAxesToMeasureOrthogonality => {
            "Add both calibration axes to measure orthogonality."
        }
        TextKey::OriginNotSet => "Origin not set",
        TextKey::RadiusPoints => "Radius points",
        TextKey::RadiusPointsSetNeedOrigin => "Radius points set (origin needed for lengths).",
        TextKey::RadiusPointsNotSet => "Radius points not set",
        TextKey::AngleValues => "Angle values",
        TextKey::AnglePointsSetValuesInvalid => "Angle points set (values invalid).",
        TextKey::AnglePointsNotSet => "Angle points not set",
        TextKey::PixelBounds => "Pixel bounds",
        TextKey::Span => "Span",
        TextKey::NoPointsForGeometryStats => "No points for geometry stats.",
        TextKey::AverageStep => "Average step",
        TextKey::TotalPolylineLength => "Total polyline length",
        TextKey::ProjectWarningsWindow => "Project warnings",
        TextKey::ProjectWarningsIntro => "Issues detected while loading the project:",
        TextKey::ContinueAnyway => "Continue anyway",
        TextKey::Cancel => "Cancel",
        TextKey::StatusPoints => "Points",
        TextKey::OpenProjectDialogTitle => "Open project",
        TextKey::CurcatProjectFilterLabel => "Curcat project",
        TextKey::SaveProjectDialogTitle => "Save project",
        TextKey::DefaultProjectName => "project.curcat",
        TextKey::OpenImageDialogTitle => "Open image",
        TextKey::ImageFilterAll => "All images",
        TextKey::PickedLabel => "Picked",
        TextKey::LoadingImageWithName => "Loading image",
        TextKey::LoadingImage => "Loading image…",
        TextKey::DropHint => "Drop an image here, open a file, or paste from clipboard (Ctrl+V).",
        TextKey::Version => "Version",
    }
}

const fn ru_text(key: TextKey) -> Option<&'static str> {
    match key {
        TextKey::File => Some("Файл"),
        TextKey::Appearance => Some("Вид"),
        TextKey::OpenImage => Some("Открыть изображение…"),
        TextKey::OpenImageHover => {
            Some("Открыть изображение (Ctrl+O). Можно также перетащить его в центр.")
        }
        TextKey::PasteImage => Some("Вставить изображение"),
        TextKey::PasteImageHover => Some("Вставить изображение из буфера обмена (Ctrl+V)"),
        TextKey::LoadProject => Some("Загрузить проект…"),
        TextKey::LoadProjectHover => Some("Загрузить сохранённый проект Curcat (Ctrl+Shift+P)"),
        TextKey::SaveProject => Some("Сохранить проект"),
        TextKey::SaveProjectHover => Some("Сохранить текущую сессию как проект Curcat (Ctrl+S)"),
        TextKey::HideSide => Some("Скрыть панель"),
        TextKey::ShowSide => Some("Показать панель"),
        TextKey::HideSidePanel => Some("Скрыть боковую панель"),
        TextKey::ShowSidePanel => Some("Показать боковую панель"),
        TextKey::SidePanelPosition => Some("Положение боковой панели"),
        TextKey::Left => Some("Слева"),
        TextKey::Right => Some("Справа"),
        TextKey::ToggleSidePanelHover => {
            Some("Показать/скрыть боковую панель (Ctrl+B) и выбрать её сторону")
        }
        TextKey::PointsStats => Some("Статистика точек"),
        TextKey::PointsStatsHover => Some("Показать статистику выбранных точек"),
        TextKey::Filters => Some("Фильтры"),
        TextKey::FiltersHover => Some("Показать фильтры изображения (Ctrl+Shift+F)"),
        TextKey::AutoTrace => Some("Авто-трассировка"),
        TextKey::AutoTraceHover => Some("Показать настройки авто-трассировки (Ctrl+Shift+T)"),
        TextKey::ImageInfo => Some("Инфо об изображении"),
        TextKey::ImageInfoHover => Some("Показать информацию о файле и изображении (Ctrl+I)"),
        TextKey::TransformsTogether => Some("Преобразует изображение, точки и калибровку вместе."),
        TextKey::Rotate90Ccw => Some("Повернуть на 90° против часовой стрелки."),
        TextKey::Rotate90Cw => Some("Повернуть на 90° по часовой стрелке."),
        TextKey::FlipHorizontally => Some("Отразить по горизонтали."),
        TextKey::FlipVertically => Some("Отразить по вертикали."),
        TextKey::FlipH => Some("Горизонт."),
        TextKey::FlipV => Some("Вертик."),
        TextKey::Zoom => Some("Масштаб:"),
        TextKey::ZoomHover => Some("Выбрать предустановленный масштаб"),
        TextKey::ZoomPresetsHover => Some("Предустановки масштаба (в процентах)"),
        TextKey::Fit => Some("Вписать"),
        TextKey::FitHover => Some("Вписать изображение в область просмотра (Ctrl+F)"),
        TextKey::ResetView => Some("Сбросить вид"),
        TextKey::ResetViewHover => {
            Some("Сбросить масштаб до 100% и панорамирование к началу (Ctrl+R)")
        }
        TextKey::PanWithMiddleButton => Some("Панорамирование средней кнопкой мыши"),
        TextKey::MmbPan => Some("Панорам. СКМ"),
        TextKey::MmbPanHover => Some("Включить/выключить панорамирование средней кнопкой"),
        TextKey::ClearPoints => Some("Очистить точки"),
        TextKey::ClearPointsHover => Some("Очистить все точки (Ctrl+Shift+D)"),
        TextKey::Undo => Some("Отменить"),
        TextKey::UndoHover => Some("Отменить последнюю точку (Ctrl+Z)"),
        TextKey::LanguageSwitcherHover => Some("Язык интерфейса"),
        TextKey::PointInput => Some("Ввод точек"),
        TextKey::Free => Some("Свободно"),
        TextKey::ContrastSnap => Some("Привязка к контрасту"),
        TextKey::CenterlineSnap => Some("Привязка к центру линии"),
        TextKey::FreeHover => Some("Ставить точки ровно в месте клика"),
        TextKey::ContrastSnapHover => {
            Some("Привязка к ближайшей области высокого контраста в радиусе поиска")
        }
        TextKey::CenterlineSnapHover => Some("Привязка к центральной линии цветовой кривой"),
        TextKey::FeatureSource => Some("Источник признака"),
        TextKey::FeatureSourceHover => Some("Что анализатор использует при поиске кандидата"),
        TextKey::ThresholdMode => Some("Режим порога"),
        TextKey::ThresholdModeHover => Some("Как детектор решает, что пиксель достаточно сильный"),
        TextKey::GradientOnly => Some("Только градиент"),
        TextKey::FeatureScore => Some("Оценка признака"),
        TextKey::GradientOnlyHover => Some("Сравнивать порог с сырой силой градиента"),
        TextKey::FeatureScoreHover => Some("Сравнивать порог с объединённой оценкой признаков"),
        TextKey::Threshold => Some("порог"),
        TextKey::ThresholdHigherHint => {
            Some("Больше значение = привязка только к сильным кандидатам")
        }
        TextKey::CenterlineDetectsFlat => {
            Some("Поиск центральной линии распознаёт однородные цветовые области")
        }
        TextKey::CenterlineDetectsFlatHover => {
            Some("Выберите цвет кривой, чтобы детектор точнее искал нужную линию")
        }
        TextKey::StrengthThreshold => Some("Порог силы"),
        TextKey::StrengthThresholdHover => Some("Отбрасывает слабые совпадения центральной линии"),
        TextKey::HigherOnlyWellDefined => {
            Some("Больше значение = привязка только к хорошо выраженным центрам линий")
        }
        TextKey::BestResultsColorSample => {
            Some("Лучший результат обычно после выбора цвета кривой.")
        }
        TextKey::PreviewCircleHint => {
            Some("Круг предпросмотра на изображении показывает область сканирования.")
        }
        TextKey::ShowPointConnections => Some("Показывать соединения точек"),
        TextKey::ShowPointConnectionsHover => {
            Some("Показать/скрыть линии между выбранными точками (не калибровочными).")
        }
        TextKey::SearchRadiusPx => Some("Радиус поиска (px)"),
        TextKey::SearchRadiusHover => Some(
            "Измеряется в пикселях изображения; меньшие значения держат привязку ближе к курсору",
        ),
        TextKey::RadiusUsedToLookForCandidates => {
            Some("Радиус, используемый для поиска кандидатов привязки")
        }
        TextKey::CurveColor => Some("Цвет кривой:"),
        TextKey::CurveColorHover => Some("Целевой цвет кривой"),
        TextKey::PickFromImage => Some("Выбрать с изображения"),
        TextKey::PickFromImageHover => Some("Нажмите и выберите пиксель на изображении"),
        TextKey::Tolerance => Some("допуск"),
        TextKey::ToleranceHover => Some("Насколько цвет пикселя может отличаться от выбранного"),
        TextKey::SnapOverlayColor => Some("Цвет оверлея привязки"),
        TextKey::SnapOverlayColorHover => {
            Some("Варианты вычисляются из изображения для лучшей видимости предпросмотра")
        }
        TextKey::Calibration => Some("Калибровка"),
        TextKey::CoordinateSystem => Some("Система координат:"),
        TextKey::CoordinateSystemHover => {
            Some("Выберите декартову (X/Y) или полярную (угол/радиус) систему")
        }
        TextKey::Cartesian => Some("Декартова"),
        TextKey::Polar => Some("Полярная"),
        TextKey::CoordSystemForCalibrationExport => {
            Some("Система координат для калибровки и экспорта")
        }
        TextKey::Snap15 => Some("Привязка 15°"),
        TextKey::Snap15Hover => {
            Some("Привязывать калибровочные линии к шагу 15° при выборе или перетаскивании")
        }
        TextKey::ShowCalibrationOverlay => Some("Показывать калибровочный оверлей"),
        TextKey::ShowCalibrationOverlayHover => {
            Some("Показать или скрыть калибровочные линии и подписи точек на изображении")
        }
        TextKey::Unit => Some("Тип:"),
        TextKey::UnitHover => Some("Тип значения оси (Число/Дата-время)"),
        TextKey::Scale => Some("Шкала:"),
        TextKey::ScaleHover => Some("Шкала оси (Линейная/Лог10)"),
        TextKey::AxisValueTypeHover => Some("Выбрать тип значения оси"),
        TextKey::AxisScaleHover => Some("Выбрать шкалу оси"),
        TextKey::XAxis => Some("Ось X"),
        TextKey::YAxis => Some("Ось Y"),
        TextKey::XAxisCalibrationHover => Some("Калибровка оси X"),
        TextKey::YAxisCalibrationHover => Some("Калибровка оси Y"),
        TextKey::Origin => Some("Начало"),
        TextKey::PickOrigin => Some("Выбрать центр"),
        TextKey::PickOriginHover => Some("Нажмите, затем выберите начало координат на изображении"),
        TextKey::Center => Some("По центру"),
        TextKey::CenterOriginHover => Some("Установить начало координат в центр изображения"),
        TextKey::Radius => Some("Радиус"),
        TextKey::Angle => Some("Угол"),
        TextKey::RadiusScaleHover => Some("Шкала радиуса (Линейная/Лог10)"),
        TextKey::RadiusScaleChoiceHover => Some("Выбрать шкалу радиуса"),
        TextKey::AngleUnit => Some("Ед. угла:"),
        TextKey::AngleUnitHover => Some("Единицы угла (градусы или радианы)"),
        TextKey::Direction => Some("Направление:"),
        TextKey::DirectionHover => Some("Направление увеличения угла"),
        TextKey::MappingOk => Some("Преобразование: готово"),
        TextKey::MappingOkHover => {
            Some("Калибровка завершена — можно ставить точки и экспортировать")
        }
        TextKey::MappingIncomplete => Some("Преобразование: неполное или некорректное"),
        TextKey::MappingIncompleteHover => {
            Some("Задайте две точки и корректные значения для калибровки")
        }
        TextKey::MappingOkAxisHover => Some("Калибровка этой оси завершена"),
        TextKey::MappingIncompleteAxisHover => {
            Some("Укажите начало координат, две точки и корректные значения")
        }
        TextKey::RadiusCalibrationHover => Some("Калибровка радиуса"),
        TextKey::AngleCalibrationHover => Some("Калибровка угла"),
        TextKey::ExportPoints => Some("Экспорт точек"),
        TextKey::InterpolatedCurve => Some("Интерполированная кривая"),
        TextKey::RawPickedPoints => Some("Исходные выбранные точки"),
        TextKey::InterpolatedCurveHover => {
            Some("Экспортировать равномерно распределённые семплы кривой")
        }
        TextKey::RawPickedPointsHover => {
            Some("Экспортировать только точки, которые вы поставили, по порядку")
        }
        TextKey::Interpolation => Some("Интерполяция:"),
        TextKey::InterpolationHover => {
            Some("Выберите способ интерполяции между контрольными точками")
        }
        TextKey::InterpolationAlgorithmHover => {
            Some("Алгоритм, используемый для генерации интерполированных семплов")
        }
        TextKey::Samples => Some("Семплы:"),
        TextKey::SamplesHover => Some("Количество равномерных семплов для экспорта"),
        TextKey::Count => Some("кол-во"),
        TextKey::Auto => Some("Авто"),
        TextKey::AutoSamplesHover => {
            Some("Автоматически выбрать число семплов по гладкости кривой")
        }
        TextKey::ExtraColumns => Some("Доп. колонки:"),
        TextKey::ExtraColumnsHover => Some("Дополнительные метрики для выбранных точек"),
        TextKey::IncludeDistanceToPrev => Some("Добавить расстояние до предыдущей точки"),
        TextKey::IncludeDistanceToPrevHover => {
            Some("Добавляет колонку с расстояниями между соседними выбранными точками")
        }
        TextKey::IncludeAngleDeg => Some("Добавить угол (град.)"),
        TextKey::IncludeAngleDegHover => {
            Some("Добавляет колонку с углами в каждой внутренней точке (первая/последняя пустые)")
        }
        TextKey::IncludeCartesianColumns => Some("Добавить декартовы колонки x/y"),
        TextKey::IncludeCartesianColumnsHover => {
            Some("Добавляет колонки x и y, вычисленные из угла и радиуса")
        }
        TextKey::ExportCsv => Some("Экспорт CSV…"),
        TextKey::ExportJson => Some("Экспорт JSON…"),
        TextKey::ExportRon => Some("Экспорт RON…"),
        TextKey::ExportExcel => Some("Экспорт Excel…"),
        TextKey::AddPointsBeforeExport => Some("Добавьте точки перед экспортом в"),
        TextKey::CompleteCalibrationBeforeExportCartesian => {
            Some("Завершите калибровку обеих осей перед экспортом в")
        }
        TextKey::CompleteCalibrationBeforeExportPolar => {
            Some("Завершите калибровку начала, радиуса и угла перед экспортом в")
        }
        TextKey::ExportToFormat => Some("Экспортировать данные в"),
        TextKey::ImageFiltersWindow => Some("Фильтры изображения"),
        TextKey::FiltersAffectDisplayOnly => Some("Влияет только на отображение и привязку."),
        TextKey::Brightness => Some("яркость"),
        TextKey::Contrast => Some("контраст"),
        TextKey::Gamma => Some("гамма"),
        TextKey::Invert => Some("инверсия"),
        TextKey::ThresholdEnabled => Some("порог"),
        TextKey::Level => Some("уровень"),
        TextKey::BlurRadius => Some("радиус размытия"),
        TextKey::ResetFilters => Some("Сбросить фильтры"),
        TextKey::LoadImageToAdjustFilters => {
            Some("Загрузите изображение, чтобы настраивать фильтры.")
        }
        TextKey::AutoTraceWindow => Some("Авто-трассировка"),
        TextKey::AutoTraceIntro => {
            Some("Кликните один раз, чтобы автоматически протрассировать сегмент кривой.")
        }
        TextKey::TraceFromClick => Some("Трассировать от клика"),
        TextKey::LoadImageFirst => Some("Сначала загрузите изображение."),
        TextKey::CompleteCalibrationBeforeTracing => {
            Some("Завершите калибровку перед трассировкой.")
        }
        TextKey::AutoTraceCartesianOnly => {
            Some("Авто-трассировка пока поддерживает только декартов режим.")
        }
        TextKey::SelectSnapBeforeTracing => Some(
            "Выберите режим «Привязка по контрасту» или «Привязка к центру линии» перед трассировкой.",
        ),
        TextKey::ClickStartPoint => Some("Нажмите и выберите стартовую точку на изображении."),
        TextKey::DirectionShort => Some("Направление"),
        TextKey::StepPx => Some("шаг (px)"),
        TextKey::SearchRadiusShort => Some("радиус поиска (px)"),
        TextKey::MaxPoints => Some("макс. точек"),
        TextKey::GapTolerance => Some("допуск пропусков"),
        TextKey::GapToleranceHover => Some("Сколько пропущенных шагов допускать перед остановкой."),
        TextKey::MinSpacingPx => Some("мин. расстояние (px)"),
        TextKey::ImageInfoWindow => Some("Информация об изображении"),
        TextKey::PointsInfoWindow => Some("Информация о точках"),
        TextKey::FileSection => Some("Файл"),
        TextKey::ImageSection => Some("Изображение"),
        TextKey::Source => Some("Источник"),
        TextKey::Name => Some("Имя"),
        TextKey::Path => Some("Путь"),
        TextKey::Size => Some("Размер"),
        TextKey::SizeUnknown => Some("Неизвестно"),
        TextKey::Modified => Some("Изменён"),
        TextKey::ModifiedUnknown => Some("Неизвестно"),
        TextKey::NoFileMetadataForImage => {
            Some("Для этого изображения нет сохранённых метаданных файла.")
        }
        TextKey::Dimensions => Some("Размеры"),
        TextKey::AspectRatio => Some("Соотношение сторон"),
        TextKey::AspectRatioNa => Some("н/д"),
        TextKey::Pixels => Some("Пиксели"),
        TextKey::RgbaMemoryEstimate => Some("Оценка памяти RGBA"),
        TextKey::CurrentZoom => Some("Текущий масштаб"),
        TextKey::LoadImageToInspectMetadata => {
            Some("Загрузите изображение, чтобы посмотреть его метаданные.")
        }
        TextKey::Points => Some("Точки"),
        TextKey::Placed => Some("Поставлено"),
        TextKey::AddPointsToSeeStats => Some("Добавьте точки, чтобы увидеть статистику."),
        TextKey::CalibratedPairsNeedsAxes => Some("Калиброванных пар"),
        TextKey::Ranges => Some("Диапазоны"),
        TextKey::CalibrationSection => Some("Калибровка"),
        TextKey::Geometry => Some("Геометрия"),
        TextKey::NoData => Some("нет данных"),
        TextKey::CalibrateAxisToSeeNumericValues => {
            Some("Откалибруйте эту ось, чтобы увидеть числовые значения.")
        }
        TextKey::XAxisLength => Some("Длина оси X"),
        TextKey::YAxisLength => Some("Длина оси Y"),
        TextKey::XAxisNotSet => Some("Ось X не задана"),
        TextKey::YAxisNotSet => Some("Ось Y не задана"),
        TextKey::AngleBetweenAxes => Some("Угол между осями"),
        TextKey::AddBothAxesToMeasureOrthogonality => {
            Some("Добавьте обе калибровочные оси для оценки ортогональности.")
        }
        TextKey::OriginNotSet => Some("Начало координат не задано"),
        TextKey::RadiusPoints => Some("Точки радиуса"),
        TextKey::RadiusPointsSetNeedOrigin => {
            Some("Точки радиуса заданы (для длин нужно начало координат).")
        }
        TextKey::RadiusPointsNotSet => Some("Точки радиуса не заданы"),
        TextKey::AngleValues => Some("Значения угла"),
        TextKey::AnglePointsSetValuesInvalid => Some("Точки угла заданы (значения некорректны)."),
        TextKey::AnglePointsNotSet => Some("Точки угла не заданы"),
        TextKey::PixelBounds => Some("Границы пикселей"),
        TextKey::Span => Some("Размах"),
        TextKey::NoPointsForGeometryStats => Some("Нет точек для геометрической статистики."),
        TextKey::AverageStep => Some("Средний шаг"),
        TextKey::TotalPolylineLength => Some("Суммарная длина ломаной"),
        TextKey::ProjectWarningsWindow => Some("Предупреждения проекта"),
        TextKey::ProjectWarningsIntro => Some("При загрузке проекта обнаружены проблемы:"),
        TextKey::ContinueAnyway => Some("Продолжить"),
        TextKey::Cancel => Some("Отмена"),
        TextKey::StatusPoints => Some("Точек"),
        TextKey::OpenProjectDialogTitle => Some("Открыть проект"),
        TextKey::CurcatProjectFilterLabel => Some("Проект Curcat"),
        TextKey::SaveProjectDialogTitle => Some("Сохранить проект"),
        TextKey::DefaultProjectName => Some("project.curcat"),
        TextKey::OpenImageDialogTitle => Some("Открыть изображение"),
        TextKey::ImageFilterAll => Some("Все изображения"),
        TextKey::PickedLabel => Some("Выбрано"),
        TextKey::LoadingImageWithName => Some("Загрузка изображения"),
        TextKey::LoadingImage => Some("Загрузка изображения…"),
        TextKey::DropHint => Some(
            "Перетащите сюда изображение, откройте файл или вставьте из буфера обмена (Ctrl+V).",
        ),
        TextKey::Version => Some("Версия"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_keys_have_ru_translation() {
        for key in TextKey::ALL {
            assert!(ru_text(key).is_some(), "Missing RU translation for {key:?}");
        }
    }

    #[test]
    fn fallback_uses_en_when_ru_missing() {
        assert_eq!(choose_text(UiLanguage::Ru, "en", None), "en");
        assert_eq!(choose_text(UiLanguage::Ru, "en", Some("ru")), "ru");
        assert_eq!(choose_text(UiLanguage::En, "en", Some("ru")), "en");
    }

    #[test]
    fn detect_locale_prefers_ru_prefix() {
        assert_eq!(
            UiLanguage::from_locale_tag("ru_RU.UTF-8"),
            Some(UiLanguage::Ru)
        );
        assert_eq!(UiLanguage::from_locale_tag("ru"), Some(UiLanguage::Ru));
        assert_eq!(
            UiLanguage::from_locale_tag("en_US.UTF-8"),
            Some(UiLanguage::En)
        );
        assert_eq!(UiLanguage::from_locale_tag("C"), Some(UiLanguage::En));
        assert_eq!(UiLanguage::from_locale_tag(""), None);
    }
}
