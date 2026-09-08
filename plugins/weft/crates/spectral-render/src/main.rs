use realfft::RealFftPlanner;
use realfft::num_complex::Complex32;
use spectral_dsp::{MANUAL_MASK_POINTS, MIDI_NOTES, MaskConfig, build_mask};
use std::f32::consts::TAU;
use std::path::PathBuf;

const SAMPLE_RATE: u32 = 48_000;
const FFT_SIZE: usize = 8192;
const OVERLAP: usize = 4;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/oiko-spectral-poc.wav"));
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let input = dense_test_signal(4.0);
    let mut notes = [0.0; MIDI_NOTES];
    // A minor: A3, C4, E4. Eight partials turn the mask into a playable
    // resonator-like spectral chord while retaining the input's phases.
    for note in [57, 60, 64] {
        notes[note] = 1.0;
    }
    let output = process_offline(&input, &notes);
    write_wav(&output_path, &output)?;

    let peak = output
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    let rms =
        (output.iter().map(|sample| sample * sample).sum::<f32>() / output.len() as f32).sqrt();
    println!(
        "rendered {} samples to {} (peak {:.3}, RMS {:.3})",
        output.len(),
        output_path.display(),
        peak,
        rms
    );
    Ok(())
}

fn process_offline(input: &[f32], notes: &[f32; MIDI_NOTES]) -> Vec<f32> {
    let hop = FFT_SIZE / OVERLAP;
    let mut output = vec![0.0; input.len() + FFT_SIZE];
    let window: Vec<f32> = (0..FFT_SIZE)
        .map(|index| 0.5 - 0.5 * (TAU * index as f32 / FFT_SIZE as f32).cos())
        .collect();
    let mut planner = RealFftPlanner::<f32>::new();
    let forward = planner.plan_fft_forward(FFT_SIZE);
    let inverse = planner.plan_fft_inverse(FFT_SIZE);
    let mut real = vec![0.0; FFT_SIZE];
    let mut complex = vec![Complex32::default(); FFT_SIZE / 2 + 1];
    let mut mask = vec![0.0; FFT_SIZE / 2 + 1];
    build_mask(
        &mut mask,
        &[0.0; MANUAL_MASK_POINTS],
        notes,
        MaskConfig {
            sample_rate: SAMPLE_RATE as f32,
            fft_size: FFT_SIZE,
            note_depth_db: 72.0,
            width_cents: 75.0,
            partials: 8,
            harmonic_rolloff_db_per_octave: 4.5,
            ..MaskConfig::default()
        },
    );
    let normalization = ((OVERLAP as f32 / 4.0) * 1.5).recip() / FFT_SIZE as f32;

    for start in (0..input.len()).step_by(hop) {
        for index in 0..FFT_SIZE {
            real[index] = input.get(start + index).copied().unwrap_or(0.0) * window[index];
        }
        forward
            .process(&mut real, &mut complex)
            .expect("valid FFT buffers");
        for (bin, gain) in complex.iter_mut().zip(mask.iter()) {
            *bin *= *gain;
        }
        inverse
            .process(&mut complex, &mut real)
            .expect("valid FFT buffers");
        for index in 0..FFT_SIZE {
            output[start + index] += real[index] * window[index] * normalization;
        }
    }
    output.truncate(input.len());
    output
}

fn dense_test_signal(seconds: f32) -> Vec<f32> {
    let len = (seconds * SAMPLE_RATE as f32) as usize;
    let mut state = 0x9e37_79b9_u32;
    (0..len)
        .map(|index| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = (state as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let time = index as f32 / SAMPLE_RATE as f32;
            let bed = [73.42_f32, 110.0, 146.83, 220.0, 293.66, 440.0]
                .iter()
                .enumerate()
                .map(|(voice, frequency)| {
                    (TAU * frequency * time + voice as f32 * 0.37).sin() / (voice + 1) as f32
                })
                .sum::<f32>();
            (bed * 0.16 + noise * 0.08) * fade(index, len)
        })
        .collect()
}

fn fade(index: usize, len: usize) -> f32 {
    let fade_samples = SAMPLE_RATE as usize / 20;
    let fade_in = (index as f32 / fade_samples as f32).min(1.0);
    let fade_out = ((len - index) as f32 / fade_samples as f32).min(1.0);
    fade_in * fade_out
}

fn write_wav(path: &PathBuf, samples: &[f32]) -> Result<(), hound::Error> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &sample in samples {
        writer.write_sample(sample)?;
        writer.write_sample(sample)?;
    }
    writer.finalize()
}

#[cfg(test)]
mod tests;
