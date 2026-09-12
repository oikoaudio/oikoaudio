//! Product rate limits and tempo divisions. Audio and editor use the same selection.
use nice_plug::prelude::*;

#[derive(Clone, Copy)]
pub(crate) struct RateRange {
    pub min: f32,
    pub max: f32,
}

pub(crate) const WOW: RateRange = RateRange {
    min: crate::MIN_RATE_HZ as f32,
    max: crate::MAX_RATE_HZ as f32,
};
pub(crate) const FLUTTER: RateRange = RateRange {
    min: crate::MIN_FLUTTER_RATE_HZ as f32,
    max: crate::MAX_FLUTTER_RATE_HZ as f32,
};

impl RateRange {
    pub(crate) fn division_indices(self, tempo: f32) -> (usize, usize) {
        let first = RateDivision::ALL
            .iter()
            .position(|d| d.rate_hz(tempo) >= self.min)
            .unwrap_or(RateDivision::ALL.len() - 1);
        let last = RateDivision::ALL
            .iter()
            .rposition(|d| d.rate_hz(tempo) <= self.max)
            .unwrap_or(0);
        (first.min(last), last)
    }
}

/// Ignore missing/non-finite host tempo. Retain the last valid tempo, initially 120 BPM.
/// The accepted domain guarantees both LFOs have in-range musical divisions.
pub(crate) fn host_tempo(tempo: Option<f64>, previous: f64) -> f64 {
    tempo
        .filter(|t| t.is_finite() && (1.0..=960.0).contains(t))
        .unwrap_or(previous)
}

#[derive(Clone, Copy, Debug, Default, Eq, Enum, PartialEq)]
pub(crate) enum RateDivision {
    #[id = "16-1"]
    #[name = "16/1"]
    SixteenWholeNotes,
    #[id = "8-1"]
    #[name = "8/1"]
    EightWholeNotes,
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
    #[id = "1-32d"]
    #[name = "1/32D"]
    ThirtySecondDotted,
    #[id = "1-16t"]
    #[name = "1/16T"]
    SixteenthTriplet,
    #[id = "1-32"]
    #[name = "1/32"]
    ThirtySecond,
    #[id = "1-64d"]
    #[name = "1/64D"]
    SixtyFourthDotted,
    #[id = "1-32t"]
    #[name = "1/32T"]
    ThirtySecondTriplet,
    #[id = "1-64"]
    #[name = "1/64"]
    SixtyFourth,
    #[id = "1-128"]
    #[name = "1/128"]
    OneTwentyEighth,
    #[id = "1-256"]
    #[name = "1/256"]
    TwoFiftySixth,
    #[id = "1-512"]
    #[name = "1/512"]
    FiveHundredTwelfth,
    #[id = "1-1024"]
    #[name = "1/1024"]
    OneThousandTwentyFourth,
    #[id = "1-2048"]
    #[name = "1/2048"]
    TwoThousandFortyEighth,
    #[id = "1-4096"]
    #[name = "1/4096"]
    FourThousandNinetySixth,
}

impl RateDivision {
    pub(crate) const ALL: [Self; 30] = [
        Self::SixteenWholeNotes,
        Self::EightWholeNotes,
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
        Self::ThirtySecondDotted,
        Self::SixteenthTriplet,
        Self::ThirtySecond,
        Self::SixtyFourthDotted,
        Self::ThirtySecondTriplet,
        Self::SixtyFourth,
        Self::OneTwentyEighth,
        Self::TwoFiftySixth,
        Self::FiveHundredTwelfth,
        Self::OneThousandTwentyFourth,
        Self::TwoThousandFortyEighth,
        Self::FourThousandNinetySixth,
    ];

    pub(crate) fn beats(self) -> f64 {
        match self {
            Self::ThirtySecondDotted => 0.1875,
            Self::SixtyFourthDotted => 0.09375,
            Self::ThirtySecondTriplet => 1.0 / 12.0,
            Self::FiveHundredTwelfth => 4.0 / 512.0,
            Self::OneThousandTwentyFourth => 4.0 / 1024.0,
            Self::TwoThousandFortyEighth => 4.0 / 2048.0,
            Self::FourThousandNinetySixth => 4.0 / 4096.0,

            Self::SixteenWholeNotes => 64.0,
            Self::EightWholeNotes => 32.0,
            Self::OneTwentyEighth => 0.03125,
            Self::TwoFiftySixth => 0.015625,
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

    pub(crate) fn closest_to_hz(rate_hz: f32, tempo_bpm: f32, range: RateRange) -> Self {
        let (first, last) = range.division_indices(tempo_bpm);
        Self::ALL[first..=last]
            .iter()
            .copied()
            .min_by(|left, right| {
                (left.rate_hz(tempo_bpm) / rate_hz.max(0.001))
                    .ln()
                    .abs()
                    .total_cmp(&(right.rate_hz(tempo_bpm) / rate_hz.max(0.001)).ln().abs())
            })
            .unwrap()
    }

    pub(crate) fn bounded(self, tempo_bpm: f32, range: RateRange) -> Self {
        let (first, last) = range.division_indices(tempo_bpm);
        Self::ALL[self.to_index().clamp(first, last)]
    }

    pub(crate) fn rate_hz(self, tempo_bpm: f32) -> f32 {
        self.rate_hz_precise(tempo_bpm as f64) as f32
    }
    pub(crate) fn rate_hz_precise(self, tempo_bpm: f64) -> f64 {
        (tempo_bpm.max(1.0) / 60.0) / self.beats()
    }
}

#[cfg(test)]
mod tests;
