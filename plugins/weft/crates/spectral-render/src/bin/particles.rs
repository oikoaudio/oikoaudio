//! Offline listening harness. Input WAV remains the only audio source.
//! Usage: particles INPUT.wav OUTPUT.wav sprinkle|cloud|reverse RATE SIZE [Hz,...]
use realfft::{RealFftPlanner, num_complex::Complex32};
use spectral_dsp::{
    MaskWorkspace,
    particles::{Config, Direction, Engine, Mask, Shape, Source},
    processing::build_dual_synthesis_window,
};
use std::{error::Error, f32::consts::TAU, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if !(6..=7).contains(&args.len()) {
        return Err(
            "Usage: particles INPUT.wav OUTPUT.wav sprinkle|cloud|reverse RATE SIZE [Hz,...]"
                .into(),
        );
    }
    let (shape, direction) = match args[3].as_str() {
        "sprinkle" => (Shape::Sprinkle, Direction::Forward),
        "cloud" => (Shape::Cloud, Direction::Forward),
        "reverse" => (Shape::Cloud, Direction::Reverse),
        _ => return Err("Unknown shape".into()),
    };
    let mut reader = hound::WavReader::open(&args[1])?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => reader
            .samples::<i32>()
            .map(|s| s.map(|v| v as f32 / 2.0_f32.powi(spec.bits_per_sample as i32 - 1)))
            .collect::<Result<_, _>>()?,
    };
    if ![1, 2].contains(&spec.channels) || samples.iter().any(|v| !v.is_finite()) {
        return Err("Expected finite mono or stereo audio".into());
    }
    let fft = 8192;
    let hop = fft / 4;
    let channels = spec.channels as usize;
    let length = samples.len() / channels;
    let rate: f32 = args[4].parse()?;
    let size: f32 = args[5].parse()?;
    if !rate.is_finite() || !size.is_finite() {
        return Err("Rate and Size must be finite".into());
    }
    let mut engine = Engine::new(spec.sample_rate as f32);
    engine.configure(Config {
        shape,
        direction,
        rate_hz: rate.min(spec.sample_rate as f32 / hop as f32 * 0.32),
        size_octaves: size,
        hop_samples: hop,
        ..Config::default()
    });
    if let Some(pitches) = args.get(6) {
        for (slot, text) in pitches.split(',').enumerate() {
            let frequency: f32 = text.parse()?;
            if slot >= 256 || !frequency.is_finite() || frequency <= 0.0 {
                return Err("Expected up to 256 positive source frequencies".into());
            }
            engine.set_source(
                slot,
                Source {
                    frequency_hz: frequency,
                    strength: 1.0,
                    eligible: true,
                },
            );
        }
    }
    let mut workspace = MaskWorkspace::default();
    workspace.prepare(spec.sample_rate as f32, fft);
    let mut particle_mask = Mask::new(fft / 2 + 1);
    let mut gains = vec![1.0; fft / 2 + 1];
    let window: Vec<f32> = (0..fft)
        .map(|i| 0.5 - 0.5 * (TAU * i as f32 / fft as f32).cos())
        .collect();
    let mut synthesis = vec![0.0; fft];
    build_dual_synthesis_window(&window, &mut synthesis, 4);
    let mut planner = RealFftPlanner::new();
    let forward = planner.plan_fft_forward(fft);
    let inverse = planner.plan_fft_inverse(fft);
    let mut forward_scratch = forward.make_scratch_vec();
    let mut inverse_scratch = inverse.make_scratch_vec();
    let mut real = vec![0.0; fft];
    let mut complex = vec![Complex32::default(); fft / 2 + 1];
    // Causal frames end at the engine's hop timestamp. Leading zero history
    // gives the same source timing convention as the live processor.
    let mut output = vec![0.0; (length + 2 * fft) * channels];
    let mut worst = std::time::Duration::ZERO;
    for end in (hop..length + fft).step_by(hop) {
        let begin = std::time::Instant::now();
        engine.advance(hop);
        let frame = engine.frame(
            spec.sample_rate as f32 / fft as f32,
            hop as f32 / spec.sample_rate as f32,
        );
        particle_mask.gains(&frame, 24.0, &workspace, &mut gains);
        for channel in 0..channels {
            for (i, value) in real.iter_mut().enumerate() {
                let source = end as isize + i as isize - fft as isize;
                *value = if source >= 0 {
                    samples
                        .get(source as usize * channels + channel)
                        .copied()
                        .unwrap_or(0.0)
                } else {
                    0.0
                } * window[i];
            }
            forward.process_with_scratch(&mut real, &mut complex, &mut forward_scratch)?;
            for (bin, gain) in complex.iter_mut().zip(&gains) {
                *bin *= gain;
            }
            inverse.process_with_scratch(&mut complex, &mut real, &mut inverse_scratch)?;
            for (i, value) in real.iter().enumerate() {
                output[(end + i) * channels + channel] += value * synthesis[i] / fft as f32;
            }
        }
        worst = worst.max(begin.elapsed());
    }
    if let Some(parent) = Path::new(&args[2])
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut writer = hound::WavWriter::create(
        &args[2],
        hound::WavSpec {
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
            ..spec
        },
    )?;
    // Remove fixed analysis latency for aligned listening; preserve the STFT tail.
    for sample in &output[fft * channels..(length + 2 * fft) * channels] {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;
    println!(
        "shape={shape:?}, direction={direction:?}, births={}, decision_hash={:016x}, worst_hop_us={:.1}, hop_deadline_us={:.1}; depth=24dB, no normalization",
        engine.births,
        engine.decision_hash,
        worst.as_secs_f64() * 1e6,
        hop as f64 / spec.sample_rate as f64 * 1e6
    );
    Ok(())
}
