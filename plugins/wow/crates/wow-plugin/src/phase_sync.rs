//! Detect transport discontinuities without making phase depend on host block size.
#[derive(Default)]
pub(crate) struct PhaseSync {
    expected_beat: Option<f64>,
    divisions: [Option<f64>; 2],
}

impl PhaseSync {
    /// Returns, per LFO, the transport phase in turns of its division (`divisions`
    /// in beats) when playback starts or jumps or that division changes; otherwise
    /// `None`. Always `None` while stopped, without a position, or for a
    /// free-running LFO.
    pub(crate) fn anchors(
        &mut self,
        playing: bool,
        position: Option<f64>,
        beats_per_sample: f64,
        samples: usize,
        divisions: [Option<f64>; 2],
    ) -> [Option<f64>; 2] {
        let position = position.filter(|p| p.is_finite() && playing);
        let jumped = position.is_some_and(|p| {
            self.expected_beat
                .is_none_or(|expected| (p - expected).abs() > (2.0 * beats_per_sample).max(1e-7))
        });
        let anchors = std::array::from_fn(|i| {
            let beats = divisions[i]?;
            let position = position?;
            (jumped || self.divisions[i] != divisions[i])
                .then(|| position.rem_euclid(beats) / beats)
        });
        self.expected_beat = position.map(|p| p + samples as f64 * beats_per_sample);
        self.divisions = divisions;
        anchors
    }
}

#[cfg(test)]
mod tests;
