//! Framework-independent numerical building blocks with no global state.
//! Buffer and filter-table construction allocates and belongs outside the audio
//! callback. Processing reuses prepared storage without heap allocation or
//! deallocation; callers must also destroy that storage outside the callback.
#![forbid(unsafe_code)]
pub mod fractional_delay;

/// Convert amplitude decibels to a linear gain. The caller owns range validation.
#[inline]
pub fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// MIDI note frequency in twelve-tone equal temperament at A4 = 440 Hz.
#[inline]
pub fn note_frequency(note: u8) -> f32 {
    note_frequency_with_tuning(note, 0.0)
}

/// MIDI note frequency with an additional offset in semitones.
#[inline]
pub fn note_frequency_with_tuning(note: u8, tuning_semitones: f32) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 + tuning_semitones - 69.0) / 12.0)
}

#[cfg(test)]
mod tests;
