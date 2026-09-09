//! Host parameter identities, ranges and persisted-field declarations.
use crate::state::{CurveState, PinnedNotesState, SpectrumRangeState, UiScaleState};
use nice_plug::prelude::*;
use spectral_dsp::MotionShape;

pub(crate) const ROUGH_FFT_SIZE: usize = 1024;
pub(crate) const COARSE_FFT_SIZE: usize = 2048;
pub(crate) const MIN_FFT_SIZE: usize = 4096;
pub(crate) const DEFAULT_FFT_SIZE: usize = 8192;
pub(crate) const MAX_FFT_SIZE: usize = 16_384;
pub(crate) const MAX_FREE_MOTION_RATE_HZ: f32 = 32.0;

#[derive(Clone, Copy, Debug, Default, Eq, Enum, PartialEq)]
pub(crate) enum FftQuality {
    #[id = "rough"]
    Rough,
    #[id = "coarse"]
    Coarse,
    #[id = "responsive"]
    Responsive,
    #[id = "balanced"]
    #[default]
    Balanced,
    #[id = "precise"]
    Precise,
}

impl FftQuality {
    pub(crate) fn size(self) -> usize {
        match self {
            Self::Rough => ROUGH_FFT_SIZE,
            Self::Coarse => COARSE_FFT_SIZE,
            Self::Responsive => MIN_FFT_SIZE,
            Self::Balanced => DEFAULT_FFT_SIZE,
            Self::Precise => MAX_FFT_SIZE,
        }
    }

    pub(crate) fn plan_index(self) -> usize {
        match self {
            Self::Rough => 0,
            Self::Coarse => 1,
            Self::Responsive => 2,
            Self::Balanced => 3,
            Self::Precise => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Enum, PartialEq)]
pub(crate) enum SpectralMotionShape {
    Ripple,
    Harmonic,
    #[default]
    Drift,
    Scan,
    Notch,
    Saw,
    // Keep original state indices 0..=6; Splash was stored as integer 6.
    Sprinkle,
    Cloud,
}

impl From<SpectralMotionShape> for MotionShape {
    fn from(value: SpectralMotionShape) -> Self {
        match value {
            SpectralMotionShape::Ripple => Self::Ripple,
            SpectralMotionShape::Harmonic => Self::Harmonic,
            SpectralMotionShape::Drift => Self::Drift,
            SpectralMotionShape::Scan => Self::Scan,
            SpectralMotionShape::Notch => Self::Notch,
            SpectralMotionShape::Saw => Self::Saw,
            SpectralMotionShape::Sprinkle => Self::Sprinkle,
            SpectralMotionShape::Cloud => Self::Cloud,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Enum, PartialEq)]
pub(crate) enum MotionDirection {
    #[default]
    Forward,
    Reverse,
    Alternate,
}

#[derive(Clone, Copy, Debug, Default, Eq, Enum, PartialEq)]
pub(crate) enum MotionRateDivision {
    #[id = "4-1"]
    #[name = "4/1"]
    FourWholeNotes,
    #[id = "3-1"]
    #[name = "3/1"]
    ThreeWholeNotes,
    #[id = "2-1"]
    #[name = "2/1"]
    TwoWholeNotes,
    #[id = "1-1d"]
    #[name = "1/1D"]
    WholeDotted,
    #[id = "1-1"]
    #[name = "1/1"]
    WholeNote,
    #[id = "1-2d"]
    #[name = "1/2D"]
    HalfDotted,
    #[id = "1-2"]
    #[name = "1/2"]
    Half,
    #[id = "1-4d"]
    #[name = "1/4D"]
    QuarterDotted,
    #[id = "1-2t"]
    #[name = "1/2T"]
    HalfTriplet,
    #[id = "1-4"]
    #[name = "1/4"]
    #[default]
    Quarter,
    #[id = "1-8d"]
    #[name = "1/8D"]
    EighthDotted,
    #[id = "1-4t"]
    #[name = "1/4T"]
    QuarterTriplet,
    #[id = "1-8"]
    #[name = "1/8"]
    Eighth,
    #[id = "1-16d"]
    #[name = "1/16D"]
    SixteenthDotted,
    #[id = "1-8t"]
    #[name = "1/8T"]
    EighthTriplet,
    #[id = "1-16"]
    #[name = "1/16"]
    Sixteenth,
    #[id = "1-16t"]
    #[name = "1/16T"]
    SixteenthTriplet,
    #[id = "1-32"]
    #[name = "1/32"]
    ThirtySecond,
    #[id = "1-64"]
    #[name = "1/64"]
    SixtyFourth,
}

impl MotionRateDivision {
    const ALL: [Self; 19] = [
        Self::FourWholeNotes,
        Self::ThreeWholeNotes,
        Self::TwoWholeNotes,
        Self::WholeDotted,
        Self::WholeNote,
        Self::HalfDotted,
        Self::Half,
        Self::QuarterDotted,
        Self::HalfTriplet,
        Self::Quarter,
        Self::EighthDotted,
        Self::QuarterTriplet,
        Self::Eighth,
        Self::SixteenthDotted,
        Self::EighthTriplet,
        Self::Sixteenth,
        Self::SixteenthTriplet,
        Self::ThirtySecond,
        Self::SixtyFourth,
    ];

    pub(crate) fn beats(self) -> f32 {
        match self {
            Self::FourWholeNotes => 16.0,
            Self::ThreeWholeNotes => 12.0,
            Self::TwoWholeNotes => 8.0,
            Self::WholeDotted => 6.0,
            Self::WholeNote => 4.0,
            Self::HalfDotted => 3.0,
            Self::Half => 2.0,
            Self::HalfTriplet => 4.0 / 3.0,
            Self::QuarterDotted => 1.5,
            Self::Quarter => 1.0,
            Self::QuarterTriplet => 2.0 / 3.0,
            Self::EighthDotted => 0.75,
            Self::Eighth => 0.5,
            Self::EighthTriplet => 1.0 / 3.0,
            Self::SixteenthDotted => 0.375,
            Self::Sixteenth => 0.25,
            Self::SixteenthTriplet => 1.0 / 6.0,
            Self::ThirtySecond => 0.125,
            Self::SixtyFourth => 0.0625,
        }
    }

    pub(crate) fn closest_to_hz(rate_hz: f32, tempo_bpm: f32) -> Self {
        let beats_per_second = tempo_bpm.max(1.0) / 60.0;
        Self::ALL
            .into_iter()
            .min_by(|left, right| {
                let left_hz = beats_per_second / left.beats();
                let right_hz = beats_per_second / right.beats();
                (left_hz / rate_hz.max(0.001))
                    .ln()
                    .abs()
                    .total_cmp(&(right_hz / rate_hz.max(0.001)).ln().abs())
            })
            .unwrap_or_default()
    }

    pub(crate) fn rate_hz(self, tempo_bpm: f32) -> f32 {
        (tempo_bpm.max(1.0) / 60.0) / self.beats()
    }
}

#[derive(Params)]
pub(crate) struct SpectralParams {
    #[persist = "manual-curve-v1"]
    pub(crate) curve: CurveState,
    #[persist = "pinned-notes-v1"]
    pub(crate) pinned_notes: PinnedNotesState,
    #[persist = "ui-scale-v1"]
    pub(crate) ui_scale: UiScaleState,
    #[persist = "spectrum-range-v1"]
    pub(crate) spectrum_range: SpectrumRangeState,

    #[id = "note_depth"]
    pub(crate) note_depth_db: FloatParam,
    #[id = "width"]
    pub(crate) width_cents: FloatParam,
    #[id = "note_attack"]
    pub(crate) note_attack_ms: FloatParam,
    #[id = "note_fade"]
    pub(crate) note_release_ms: FloatParam,
    #[id = "partials"]
    pub(crate) partials: IntParam,
    #[id = "harmonic_rolloff"]
    pub(crate) harmonic_rolloff_db: FloatParam,
    #[id = "velocity_sensitivity"]
    pub(crate) velocity_sensitivity_percent: FloatParam,
    #[id = "pitch_bend_range"]
    pub(crate) pitch_bend_range: IntParam,
    #[id = "quality"]
    pub(crate) quality: EnumParam<FftQuality>,
    #[id = "motion_shape"]
    pub(crate) motion_shape: EnumParam<SpectralMotionShape>,
    #[id = "motion_depth"]
    pub(crate) motion_depth_db: FloatParam,
    #[id = "motion_rate"]
    pub(crate) motion_rate_hz: FloatParam,
    #[id = "msync"]
    pub(crate) motion_sync: BoolParam,
    #[id = "mdiv"]
    pub(crate) motion_rate_division: EnumParam<MotionRateDivision>,
    #[id = "mphase"]
    pub(crate) motion_phase_percent: FloatParam,
    #[id = "motion_size"]
    pub(crate) motion_size_octaves: FloatParam,
    #[id = "motion_direction"]
    pub(crate) motion_direction: EnumParam<MotionDirection>,
    #[id = "output"]
    pub(crate) output_gain_db: FloatParam,
    #[id = "smooth"]
    pub(crate) smooth_spectral: BoolParam,
    #[id = "curve_depth"]
    pub(crate) curve_depth_percent: FloatParam,
    #[id = "curve_tilt"]
    pub(crate) curve_tilt_db_per_octave: FloatParam,
    #[id = "curve_shift"]
    pub(crate) curve_shift_semitones: FloatParam,
}

impl Default for SpectralParams {
    fn default() -> Self {
        Self {
            curve: CurveState::default(),
            pinned_notes: PinnedNotesState::default(),
            ui_scale: UiScaleState::default(),
            spectrum_range: SpectrumRangeState::default(),
            note_depth_db: FloatParam::new(
                "Note Depth",
                0.0,
                FloatRange::SymmetricalSkewed {
                    min: 0.0,
                    max: 90.0,
                    factor: 1.0,
                    center: 20.0,
                },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            width_cents: FloatParam::new(
                "Note Width",
                100.0,
                FloatRange::Skewed {
                    min: 10.0,
                    max: 1200.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" cents")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            note_attack_ms: FloatParam::new(
                "Note Attack",
                0.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 5000.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            note_release_ms: FloatParam::new(
                "Note Release",
                70.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 5000.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            partials: IntParam::new("Partials", 1, IntRange::Linear { min: 1, max: 24 }),
            harmonic_rolloff_db: FloatParam::new(
                "Partial Rolloff",
                6.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 24.0,
                },
            )
            .with_unit(" dB/oct")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            velocity_sensitivity_percent: FloatParam::new(
                "Velocity Sensitivity",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 100.0,
                },
            )
            .with_unit(" %")
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            pitch_bend_range: IntParam::new(
                "Pitch Bend Range",
                48,
                IntRange::Linear { min: 1, max: 96 },
            )
            .with_unit(" st"),
            quality: EnumParam::new("Resolution", FftQuality::Balanced).non_automatable(),
            motion_shape: EnumParam::new("Motion Shape", SpectralMotionShape::Drift),
            motion_depth_db: FloatParam::new(
                "Motion Depth",
                0.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 60.0,
                    // Keep the useful 0-30 dB range across the first two thirds of
                    // the control, then compress the deeper 30-60 dB range.
                    factor: 0.584_962_5,
                },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            motion_rate_hz: FloatParam::new(
                "Motion Rate",
                0.3,
                FloatRange::SymmetricalSkewed {
                    min: 0.01,
                    max: MAX_FREE_MOTION_RATE_HZ,
                    factor: 1.0,
                    center: 4.0,
                },
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            motion_sync: BoolParam::new("Motion Sync", true),
            motion_rate_division: EnumParam::new("Motion Division", MotionRateDivision::Half),
            motion_phase_percent: FloatParam::new(
                "Motion Phase",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 100.0,
                },
            )
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            motion_size_octaves: FloatParam::new(
                "Motion Size",
                1.0,
                FloatRange::Skewed {
                    min: 0.125,
                    max: 8.0,
                    // Keep roughly 60% of the travel focused on sizes up to two
                    // octaves while leaving room for very broad spectral shapes.
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" oct")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            motion_direction: EnumParam::new("Motion Direction", MotionDirection::Forward),
            output_gain_db: FloatParam::new(
                "Output",
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            smooth_spectral: BoolParam::new("Smooth", false),
            curve_depth_percent: FloatParam::new(
                "Curve Depth",
                100.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 800.0,
                },
            )
            .with_unit(" %")
            .hide()
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            curve_tilt_db_per_octave: FloatParam::new(
                "Curve Tilt",
                0.0,
                FloatRange::Linear {
                    min: -12.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB/oct")
            .hide()
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            curve_shift_semitones: FloatParam::new(
                "Curve Shift",
                0.0,
                FloatRange::Linear {
                    min: -48.0,
                    max: 48.0,
                },
            )
            .with_unit(" st")
            .hide()
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
        }
    }
}

#[cfg(test)]
mod tests;
