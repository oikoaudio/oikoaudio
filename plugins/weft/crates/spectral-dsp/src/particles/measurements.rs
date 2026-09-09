use super::*;
#[test]
#[ignore = "manual release-profile measurement of 64 particles for every shape"]
fn benchmark_particle_core() {
    use std::time::{Duration, Instant};
    println!("sample_rate,fft,shape,direction,active,worst_us");
    for sample_rate in [44100.0, 48000.0, 96000.0, 192000.0, 384000.0] {
        for fft in [1024, 2048, 4096, 8192, 16384] {
            for (shape, direction) in [
                (Shape::Sprinkle, Direction::Forward),
                (Shape::Cloud, Direction::Forward),
                (Shape::Cloud, Direction::Reverse),
                (Shape::Cloud, Direction::Alternate),
            ] {
                let mut e = Engine::new(sample_rate);
                e.configure(Config {
                    shape,
                    direction,
                    size_octaves: 8.0,
                    rate_hz: 32.0,
                    partials: 24,
                    partial_rolloff_db: 0.0,
                    hop_samples: fft / 4,
                    ..Config::default()
                });
                e.set_source(
                    0,
                    Source {
                        frequency_hz: 440.0,
                        strength: 1.0,
                        eligible: true,
                    },
                );
                let mut workspace = MaskWorkspace::default();
                workspace.prepare(sample_rate, fft);
                let mut mask = Mask::new(fft / 2 + 1);
                let mut gains = vec![1.0; fft / 2 + 1];
                let mut worst = Duration::ZERO;
                for repetition in 0..64 {
                    e.particles.fill(Particle::default());
                    e.attempts = 0;
                    let begin = Instant::now();
                    for index in 0..64 {
                        e.birth(repetition * 64 + index, None);
                    }
                    e.advance(fft / 4);
                    let frame = e.frame(sample_rate / fft as f32, fft as f32 / (4.0 * sample_rate));
                    assert_eq!(e.active_count(), MAX_PARTICLES);
                    mask.gains(&frame, 60.0, &workspace, &mut gains);
                    std::hint::black_box(&gains);
                    worst = worst.max(begin.elapsed());
                }
                println!(
                    "{sample_rate},{fft},{shape:?},{direction:?},64,{:.3}",
                    worst.as_secs_f64() * 1e6
                );
            }
        }
    }
}
