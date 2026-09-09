use super::*;
fn engine(shape: Shape) -> Engine {
    let mut e = Engine::new(48_000.0);
    e.configure(Config {
        shape,
        rate_hz: 13.7,
        size_octaves: 0.73,
        hop_samples: 256,
        ..Config::default()
    });
    e.set_source(
        0,
        Source {
            frequency_hz: 317.3,
            strength: 0.8,
            eligible: true,
        },
    );
    e
}
#[test]
fn partitions_preserve_decisions_and_windows() {
    fn render(block: usize, sync: bool) -> (u64, u64, Vec<f32>) {
        let mut e = engine(Shape::Sprinkle);
        e.configure(Config { sync, ..e.config });
        let mut result = Vec::new();
        let events = [137, 1931, 13_123, 25_000, 39_001, 49_921, 60_003];
        let mut at = 0;
        while at < 72_000 {
            if events.contains(&at) {
                e.trigger(0);
                let config = Config {
                    phase: if at % 2 == 0 { 0.99 } else { 0.01 },
                    rate_hz: if at > 25_000 { 7.3 } else { 13.7 },
                    ..e.config
                };
                e.configure(config);
            }
            let end = (at + block)
                .min((at / 256 + 1) * 256)
                .min(72_000)
                .min(events.iter().copied().find(|x| *x > at).unwrap_or(72_000));
            e.advance(end - at);
            at = end;
            if at % 256 == 0 {
                let frame = e.frame(46.875, 256.0 / 48_000.0);
                result.push(frame.attenuation_db(317.3, 24.0));
            }
        }
        (e.births, e.decision_hash, result)
    }
    for sync in [false, true] {
        let expected = render(1, sync);
        for block in [64, 257, 1024] {
            assert_eq!(render(block, sync), expected);
        }
    }
}
#[test]
fn sprinkle_has_clusters_rests_and_a_shared_rate_budget() {
    for sync in [false, true] {
        let mut e = Engine::new(1000.0);
        e.configure(Config {
            rate_hz: 4.0,
            hop_samples: 16,
            sync,
            ..Config::default()
        });
        for slot in 0..4 {
            e.set_source(
                slot,
                Source {
                    frequency_hz: 317.3 + slot as f32 * 91.17,
                    strength: 1.0,
                    eligible: true,
                },
            );
        }
        let mut times = Vec::new();
        let mut peak = 0;
        for sample in 0..60_000 {
            e.advance(1);
            for p in e.particles.iter().filter(|p| p.active && p.birth == sample) {
                times.push(p.birth);
                if sync {
                    // Quarter of a 4 Hz cycle = 62.5 samples, rounded up.
                    let distance = (p.birth as f64 / 62.5 - (p.birth as f64 / 62.5).round()).abs();
                    assert!(distance <= 1.0 / 62.5 + 1e-9);
                }
            }
            if sample % 16 == 15 {
                e.frame(1.0, 0.016);
                peak = peak.max(e.active_count());
            }
        }
        // Statistical tolerances set for 60 seconds at 4 Hz: average within
        // 20%, plus actual simultaneous starts, close pairs, and longer rests.
        assert!((192..=288).contains(&times.len()), "{} births", times.len());
        assert!(times.windows(2).any(|p| p[1] == p[0]));
        assert!(times.windows(2).any(|p| (1..100).contains(&(p[1] - p[0]))));
        assert!(times.windows(2).any(|p| p[1] - p[0] > 500));
        assert!(peak >= 4);
        assert_eq!(e.dropped, 0);
    }
}

#[test]
fn sprinkle_varies_gestures_avoids_repeat_sources_and_overflow_preserves_choices() {
    let mut e = engine(Shape::Sprinkle);
    let mut full = engine(Shape::Sprinkle);
    for slot in 1..4 {
        let source = Source {
            frequency_hz: 317.3 + slot as f32 * 80.0,
            strength: 0.8,
            eligible: true,
        };
        e.set_source(slot, source);
        full.set_source(slot, source);
    }
    let mut previous = None;
    let mut min_length = f64::INFINITY;
    let mut max_length = 0.0_f64;
    let mut min_attack = 1.0_f32;
    let mut max_attack = 0.0_f32;
    let mut min_width = 8.0_f32;
    let mut max_width = 0.0_f32;
    for index in 0..256 {
        e.particles.fill(Particle::default());
        e.attempts = 0;
        e.birth(index, None);
        full.birth(index, None);
        let p = e.particles[0];
        assert_ne!(p.source, previous);
        previous = p.source;
        assert_eq!(e.last_source, full.last_source);
        min_length = min_length.min(p.lifetime);
        max_length = max_length.max(p.lifetime);
        min_attack = min_attack.min(p.attack * p.lifetime as f32);
        max_attack = max_attack.max(p.attack * p.lifetime as f32);
        min_width = min_width.min(p.width);
        max_width = max_width.max(p.width);
    }
    assert_eq!(max_length, min_length);
    assert_eq!(max_attack, min_attack);
    assert!(max_width > min_width * 1.7);
    assert!(full.dropped > 0);
    // Once space is available, every captured variation agrees again.
    for engine in [&mut e, &mut full] {
        engine.particles.fill(Particle::default());
        engine.attempts = 0;
        engine.birth(256, None);
    }
    let (a, b) = (e.particles[0], full.particles[0]);
    assert_eq!(
        (a.source, a.lifetime, a.attack, a.width, a.octave),
        (b.source, b.lifetime, b.attack, b.width, b.octave)
    );
}

#[test]
fn partial_selection_is_sparse_unique_bounded_and_rolloff_weighted() {
    let flat = [1.0; MAX_PARTIALS];
    let steep = std::array::from_fn(|i| db_to_gain(-24.0 * ((i + 1) as f32).log2()));
    let mut counts = [0; 3];
    let mut flat_sum = 0;
    let mut steep_sum = 0;
    for index in 0..4096 {
        assert_eq!(select_partials(index, flat, 1, 100.0), [1, 0, 0]);
        for limit in [2, 4, 24] {
            let chosen = select_partials(index, flat, limit, 100.0);
            let nonzero: Vec<_> = chosen.into_iter().filter(|n| *n != 0).collect();
            assert!((1..=limit.min(3)).contains(&nonzero.len()));
            for (i, n) in nonzero.iter().enumerate() {
                assert!(*n as usize <= limit && !nonzero[..i].contains(n));
            }
        }
        let chosen = select_partials(index, flat, 24, 100.0);
        counts[chosen.iter().filter(|n| **n > 0).count() - 1] += 1;
        flat_sum += chosen[0] as u32;
        steep_sum += select_partials(index, steep, 24, 100.0)[0] as u32;
        assert!(select_partials(index, flat, 24, 3.0).iter().all(|n| *n < 3));
    }
    assert!((0.77..0.83).contains(&(counts[0] as f32 / 4096.0)));
    assert!(counts[1] > 500 && counts[2] > 50);
    assert!(flat_sum > 4 * steep_sum);
}

#[test]
fn sparse_partial_policy_applies_to_free_and_note_driven_sprinkles() {
    for supplied in [false, true] {
        for limit in [1, 24] {
            let mut e = Engine::new(48_000.0);
            e.configure(Config {
                partials: limit,
                partial_rolloff_db: 0.0,
                ..Config::default()
            });
            if supplied {
                e.set_source(
                    0,
                    Source {
                        frequency_hz: 317.3,
                        strength: 1.0,
                        eligible: true,
                    },
                );
            }
            let mut richer = false;
            for index in 0..128 {
                e.particles.fill(Particle::default());
                e.attempts = 0;
                e.birth(index, None);
                let p = e.particles[0];
                assert_eq!(p.source.is_some(), supplied);
                if limit == 1 {
                    assert_eq!(p.partials, [1, 0, 0]);
                }
                richer |= p.partials[1] != 0;
            }
            assert_eq!(richer, limit > 1);
        }
    }
}

#[test]
fn selected_partials_follow_tuning_without_rerolls_or_a_second_rolloff_gain() {
    let mut e = engine(Shape::Sprinkle);
    e.configure(Config {
        partials: 24,
        partial_rolloff_db: 0.0,
        scheduled: false,
        ..e.config
    });
    e.birth(0, Some(0));
    let p = e.particles[0];
    e.advance((p.lifetime * p.attack as f64 * 48_000.0).round() as usize);
    let before = e.frame(1.0, 0.01);
    e.configure(Config {
        partials: 1,
        partial_rolloff_db: 24.0,
        ..e.config
    });
    assert_eq!(e.particles[0].partials, p.partials);
    assert_eq!(e.particles[0].strength, p.strength);
    e.set_source(
        0,
        Source {
            frequency_hz: 350.0,
            ..e.sources[0]
        },
    );
    let after = e.frame(1.0, 0.0);
    for (slot, partial) in p
        .partials
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, n)| *n > 0)
    {
        let a = before.windows[slot * MAX_PARTICLES];
        let b = after.windows[slot * MAX_PARTICLES];
        assert!(a.amplitude > 0.0);
        assert_eq!(a.amplitude, before.windows[0].amplitude);
        assert_eq!(a.amplitude, b.amplitude);
        assert!(
            (a.center_log2_hz - (p.frequency_hz.log2() + p.octave + (partial as f32).log2())).abs()
                < 1e-6
        );
        assert!((b.center_log2_hz - a.center_log2_hz - (350.0_f32 / 317.3).log2()).abs() < 1e-6);
    }
}

#[test]
fn sprinkle_uses_captured_attack_release_with_free_live_and_pinned_sources() {
    for source in [None, Some(0), Some(128)] {
        for (attack_ms, release_ms) in [(0.0_f32, 0.0_f32), (25.0, 70.0), (5000.0, 5000.0)] {
            for (rate_hz, size_octaves) in [(0.01, 0.125), (32.0, 8.0)] {
                let mut e = Engine::new(1000.0);
                e.configure(Config {
                    scheduled: false,
                    hop_samples: 10,
                    attack_ms,
                    release_ms,
                    rate_hz,
                    size_octaves,
                    ..Config::default()
                });
                if let Some(slot) = source {
                    e.set_source(
                        slot,
                        Source {
                            frequency_hz: 317.3,
                            strength: 0.8,
                            eligible: true,
                        },
                    );
                }
                e.birth(0, source);
                let p = e.particles[0];
                let onset = (attack_ms * 0.001).max(0.002);
                let release = (release_ms * 0.001).max(0.002).max(0.02 - onset);
                assert!((p.lifetime - (onset + release) as f64).abs() < 2e-6);
                assert!((p.lifetime * p.attack as f64 - onset as f64).abs() < 2e-6);
                assert_eq!(e.frame(1.0, 0.0).windows[0].amplitude, 0.0);
                let peak_sample = (onset * 1000.0).round() as usize;
                e.advance(peak_sample);
                let peak = e.frame(1.0, 0.0).windows[0];
                assert!(peak.amplitude / p.strength > 0.999);
                // All timing edits affect future births, including quality changes.
                e.configure(Config {
                    attack_ms: 500.0,
                    release_ms: 250.0,
                    hop_samples: 256,
                    rate_hz: 13.0,
                    phase: 0.73,
                    size_octaves: 4.0,
                    ..e.config
                });
                assert_eq!(e.particles[0].attack, p.attack);
                assert_eq!(e.particles[0].lifetime, p.lifetime);
                if let Some(slot) = source {
                    e.release_source(slot);
                }
                let middle_sample = ((onset + release * 0.5) * 1000.0).round() as usize;
                e.advance(middle_sample - peak_sample);
                let middle = e.frame(1.0, 0.0).windows[0];
                assert!((middle.amplitude / p.strength - 0.125).abs() < 0.001);
                assert_eq!(middle.center_log2_hz, peak.center_log2_hz);
                e.advance((p.lifetime * 1000.0).ceil() as usize - middle_sample);
                assert_eq!(e.frame(1.0, 0.0).windows[0].amplitude, 0.0);
                e.birth(1, None);
                let next = e.particles[0];
                assert!((next.lifetime - 0.75).abs() < 2e-6);
                assert!((next.lifetime * next.attack as f64 - 0.5).abs() < 2e-6);
            }
        }
    }
}

#[test]
fn sprinkle_zero_times_respect_hop_floor_and_invalid_times_are_bounded() {
    for fft in [1024, 2048, 4096, 8192, 16384] {
        let mut e = engine(Shape::Sprinkle);
        e.configure(Config {
            attack_ms: 0.0,
            release_ms: 0.0,
            hop_samples: fft / 4,
            ..e.config
        });
        e.birth(0, Some(0));
        let p = e.particles[0];
        assert!((p.lifetime - fft as f64 / 96000.0).abs() < 2e-6);
        assert!((p.lifetime * p.attack as f64 - 0.002).abs() < 2e-6);
        assert!(p.attack > 0.0 && p.attack < 1.0);
    }
    let mut e = engine(Shape::Sprinkle);
    e.configure(Config {
        attack_ms: f32::NAN,
        release_ms: f32::INFINITY,
        ..e.config
    });
    assert_eq!((e.config.attack_ms, e.config.release_ms), (0.0, 70.0));
    e.configure(Config {
        attack_ms: -10.0,
        release_ms: 10000.0,
        ..e.config
    });
    assert_eq!((e.config.attack_ms, e.config.release_ms), (0.0, 5000.0));
}

#[test]
fn clouds_follow_low_notes_with_quieter_captured_strength() {
    for direction in [Direction::Forward, Direction::Reverse, Direction::Alternate] {
        let mut e = engine(Shape::Cloud);
        e.configure(Config {
            direction,
            scheduled: false,
            ..e.config
        });
        e.set_source(
            0,
            Source {
                frequency_hz: 88.0,
                strength: 1.0,
                eligible: true,
            },
        );
        e.birth(0, None);
        let p = e.particles[0];
        assert_eq!(p.source, Some(0));
        assert_eq!(p.octave, 0.0);
        assert_eq!(p.frequency_hz, 88.0);
        assert!(p.strength > 0.0 && p.strength < 0.5);
        e.set_source(
            0,
            Source {
                frequency_hz: 77.0,
                ..e.sources[0]
            },
        );
        assert_eq!(e.particles[0].frequency_hz, 77.0);
        assert_eq!(e.particles[0].strength, p.strength);
    }
}

#[test]
fn particle_windows_can_open_below_200_hz_at_every_size_and_resolution() {
    for shape in [Shape::Sprinkle, Shape::Cloud] {
        for fft in [1024, 2048, 4096, 8192, 16384] {
            let mut workspace = MaskWorkspace::default();
            workspace.prepare(48_000.0, fft);
            let mut mask = Mask::new(fft / 2 + 1);
            let mut gains = vec![0.0; fft / 2 + 1];
            for size in [0.125, 1.0, 8.0] {
                let mut e = engine(shape);
                e.configure(Config {
                    scheduled: false,
                    size_octaves: size,
                    ..e.config
                });
                e.birth(0, Some(0));
                let p = e.particles[0];
                e.advance((p.lifetime * p.attack as f64 * 48_000.0).round() as usize);
                // Retuning follows the captured octave ratio into the bass.
                e.set_source(
                    0,
                    Source {
                        frequency_hz: 93.75 / p.octave.exp2(),
                        ..e.sources[0]
                    },
                );
                let mut frame = e.frame(48_000.0 / fft as f32, 0.0);
                frame.activity = 1.0;
                assert!(frame.windows[0].openness(93.75_f32.log2()) > 0.0);
                mask.gains(&frame, 24.0, &workspace, &mut gains);
                let bin = fft / 512; // 93.75 Hz at every tested resolution.
                assert!(gains[bin] > db_to_gain(-24.0) + 1e-4);
                assert!(gains.iter().all(|g| g.is_finite() && *g <= 1.0));
                mask.gains(&frame, 0.0, &workspace, &mut gains);
                assert!(gains.iter().all(|g| *g == 1.0));
            }
        }
    }
    // Wide skirts cross 200 Hz smoothly without a hidden low-frequency mask.
    let window = Window {
        center_log2_hz: 200.0_f32.log2(),
        radius_octaves: 4.0,
        amplitude: 1.0,
    };
    assert_eq!(window.openness(200.0_f32.log2()), 1.0);
    assert!(window.openness(100.0_f32.log2()) > 0.8);
    assert!((window.openness(199.99_f32.log2()) - window.openness(200.01_f32.log2())).abs() < 1e-6);
}

#[test]
fn sprinkle_spans_low_and_high_registers_with_a_gentle_bass_bias() {
    for root in [27.5_f32, 317.3, 440.0, 4186.01] {
        let mut counts = [0; 8];
        let mut forward_total = 0.0;
        let mut reverse_total = 0.0;
        for index in 0..4096 {
            let (octave, strength) = sprinkle_register(index, root, 20_000.0, Direction::Forward);
            let (reverse, _) = sprinkle_register(index, root, 20_000.0, Direction::Reverse);
            let hz = (root.log2() + octave).exp2();
            assert!((PARTICLE_MIN_HZ - 1e-3..=SPRINKLE_MAX_HZ + 1e-3).contains(&hz));
            assert_eq!(octave, octave.round());
            counts[((hz / PARTICLE_MIN_HZ).log2().floor() as usize).min(7)] += 1;
            forward_total += octave;
            reverse_total += reverse;
            assert!((PARTICLE_LOW_OPENING..=1.0).contains(&strength));
            if hz < 100.0 {
                assert!(strength < 0.35);
            }
            if hz >= PARTICLE_FULL_OPENING_HZ {
                assert_eq!(strength, 1.0);
            }
        }
        assert!(
            counts[..7].iter().all(|count| *count > 0),
            "{root}: {counts:?}"
        );
        assert!(counts[0] < counts[5], "{counts:?}");
        assert!(forward_total > reverse_total);
    }
}

#[test]
fn free_clouds_allow_bass_but_choose_it_less_often_than_log_uniform() {
    let mut low = 0;
    for index in 0..8192 {
        let hz = free_cloud_frequency(index, 20000.0);
        assert!((20.0..=20000.0).contains(&hz));
        low += usize::from(hz < 200.0);
    }
    // Log-uniform would put 1/3 of centers below 200 Hz.
    assert!((0.10..0.25).contains(&(low as f32 / 8192.0)), "{low}");
    assert!(register_strength(40.0_f32.log2()) < register_strength(100.0_f32.log2()));
    assert!(register_strength(100.0_f32.log2()) < register_strength(440.0_f32.log2()));
    // Source updates through the former cutoff do not introduce a gain step.
    assert!(
        (register_strength(199.99_f32.log2()) - register_strength(200.01_f32.log2())).abs() < 1e-4
    );
}

#[test]
fn free_sprinkles_share_phrase_roots_and_keep_harmonic_ratios() {
    let mut e = Engine::new(48_000.0);
    e.configure(Config {
        scheduled: false,
        ..Config::default()
    });
    for index in 0..16 {
        e.birth(index, None);
    }
    let root = e.particles[0].frequency_hz;
    assert!((FREE_ROOT_MIN_HZ..=FREE_ROOT_MIN_HZ * FREE_ROOT_RATIO).contains(&root));
    for p in &e.particles[..16] {
        assert_eq!(p.frequency_hz, root);
        assert!(p.source.is_none());
        assert_eq!(p.octave, p.octave.round());
        assert!(
            (PARTICLE_MIN_HZ - 1e-3..=SPRINKLE_MAX_HZ + 1e-3)
                .contains(&(p.frequency_hz.log2() + p.octave).exp2())
        );
        assert_eq!(p.partials, [1, 0, 0]);
    }
    e.phase = SPRINKLE_GROUP_CYCLES;
    e.birth(16, None);
    assert_ne!(e.particles[16].frequency_hz, root);
    assert_eq!(e.particles[0].frequency_hz, root);
    let next_root = e.particles[16].frequency_hz;
    e.configure(Config {
        phase: 0.73,
        ..e.config
    });
    e.birth(17, None);
    assert_eq!(e.particles[17].frequency_hz, next_root);
}

#[test]
fn both_shapes_use_notes_when_present_and_resume_free_births_after_release() {
    for shape in [Shape::Sprinkle, Shape::Cloud] {
        let mut e = Engine::new(48_000.0);
        e.configure(Config {
            shape,
            scheduled: false,
            ..Config::default()
        });
        e.birth(0, None);
        let free = e.particles[0];
        e.set_note_input_present(true);
        e.set_source(
            0,
            Source {
                frequency_hz: 317.3,
                strength: 0.6,
                eligible: true,
            },
        );
        e.birth(1, None);
        assert_eq!(e.particles[1].source, Some(0));
        assert_eq!(e.particles[1].frequency_hz, 317.3);
        if shape == Shape::Cloud {
            assert_eq!(e.particles[1].octave, 0.0);
        }
        e.set_source(
            0,
            Source {
                frequency_hz: 491.17,
                ..e.sources[0]
            },
        );
        assert_eq!(e.particles[1].frequency_hz, 491.17);
        assert_eq!(e.particles[0].frequency_hz, free.frequency_hz);
        e.release_source(0);
        // Notes still present but unavailable (muted/unmapped) cannot fall back.
        e.birth(2, None);
        assert_eq!(e.births, 2);
        e.set_note_input_present(false);
        e.birth(3, None);
        assert_eq!(e.births, 3);
        assert_eq!(e.particles[2].source, None);
        assert_eq!(e.particles[1].frequency_hz, 491.17);
        e.reset();
        assert!(!e.note_input_present);
    }
}

#[test]
fn free_births_and_note_handover_are_independent_of_block_partition() {
    fn run(block: usize, shape: Shape) -> (u64, u64, Vec<f32>) {
        let mut e = Engine::new(48_000.0);
        e.configure(Config {
            shape,
            rate_hz: 13.0,
            hop_samples: 256,
            ..Config::default()
        });
        let mut masks = Vec::new();
        let mut start = 0;
        while start < 96_000 {
            if start == 24_137 {
                e.set_note_input_present(true);
                e.set_source(
                    0,
                    Source {
                        frequency_hz: 317.3,
                        strength: 0.7,
                        eligible: true,
                    },
                );
                e.trigger(0);
            } else if start == 72_193 {
                e.release_source(0);
                e.set_note_input_present(false);
            }
            let next_event = if start < 24_137 {
                24_137
            } else if start < 72_193 {
                72_193
            } else {
                96_000
            };
            let end = (start + block).min(next_event).min((start / 256 + 1) * 256);
            e.advance(end - start);
            start = end;
            if start % 256 == 0 {
                let f = e.frame(46.875, 256.0 / 48_000.0);
                masks.extend([55.0, 317.3, 1000.0, 8000.0].map(|hz| f.attenuation_db(hz, 24.0)));
            }
        }
        (e.births, e.decision_hash, masks)
    }
    for shape in [Shape::Sprinkle, Shape::Cloud] {
        let reference = run(1024, shape);
        assert!(reference.0 > 10);
        for block in [1, 64, 257] {
            assert_eq!(run(block, shape), reference);
        }
    }
}

#[test]
fn birth_age_and_tuning_ratios_are_preserved() {
    let mut e = engine(Shape::Sprinkle);
    e.advance(137);
    e.trigger(0);
    let p = e.particles[0];
    assert_eq!(p.birth, 137);
    assert_eq!(p.octave, p.octave.round());
    assert!(
        (PARTICLE_MIN_HZ - 1e-3..=SPRINKLE_MAX_HZ + 1e-3)
            .contains(&(p.frequency_hz.log2() + p.octave).exp2())
    );
    e.set_source(
        0,
        Source {
            frequency_hz: 491.17,
            strength: 0.8,
            eligible: true,
        },
    );
    assert_eq!(e.particles[0].octave, p.octave);
    assert_eq!(e.particles[0].lifetime, p.lifetime);
    assert_eq!(e.particles[0].strength, p.strength);
    assert_eq!(e.particles[0].frequency_hz, 491.17);
    e.advance(119);
    let f = e.frame(46.875, 256.0 / 48_000.0);
    assert!(f.windows[0].amplitude > 0.0);
    assert!((f.windows[0].center_log2_hz - (491.17_f32.log2() + p.octave)).abs() < 1e-6);
}
#[test]
fn pool_and_attempt_budget_drop_without_stealing_or_rerolling() {
    let mut e = engine(Shape::Sprinkle);
    for _ in 0..64 {
        e.trigger(0);
    }
    let first = e.particles[0];
    for _ in 0..1000 {
        e.trigger(0);
    }
    assert_eq!(e.births, 64);
    assert_eq!(e.dropped, 1000);
    assert_eq!(e.active_count(), 64);
    assert_eq!(e.particles[0].birth, first.birth);
    assert_eq!(e.particles[0].lifetime, first.lifetime);
    e.advance(256);
    e.frame(46.875, 256.0 / 48_000.0);
    e.trigger(0);
    assert_eq!(e.births, 64);
    assert_eq!(e.dropped, 1001);
}
#[test]
fn reference_matches_prepared_masks_and_zero_depth_is_exact() {
    for fft in [1024, 2048, 4096, 8192, 16384] {
        let mut workspace = MaskWorkspace::default();
        workspace.prepare(48_000.0, fft);
        let mut mask = Mask::new(fft / 2 + 1);
        let mut gains = vec![0.0; fft / 2 + 1];
        for (shape, direction) in [
            (Shape::Sprinkle, Direction::Forward),
            (Shape::Cloud, Direction::Forward),
            (Shape::Cloud, Direction::Reverse),
            (Shape::Cloud, Direction::Alternate),
        ] {
            let mut e = engine(shape);
            e.configure(Config {
                direction,
                partials: 24,
                partial_rolloff_db: 0.0,
                ..e.config
            });
            for slot in 0..64 {
                e.set_source(
                    slot,
                    Source {
                        frequency_hz: 25.0 * (slot as f32 * 0.1).exp2(),
                        strength: 1.0,
                        eligible: true,
                    },
                );
                e.trigger(slot);
            }
            for _ in 0..40 {
                e.advance(fft / 4);
                let frame = e.frame(48_000.0 / fft as f32, fft as f32 / 192_000.0);
                mask.gains(&frame, 60.0, &workspace, &mut gains);
                for (bin, gain) in gains.iter().copied().enumerate().skip(1) {
                    let reference =
                        db_to_gain(-frame.attenuation_db(bin as f32 * 48_000.0 / fft as f32, 60.0));
                    assert!(
                        (gain - reference).abs() < 2e-6,
                        "{shape:?} fft {fft} bin {bin}"
                    );
                    assert!(gain.is_finite() && (0.001 - 1e-7..=1.0).contains(&gain));
                }
                mask.gains(&frame, 0.0, &workspace, &mut gains);
                assert!(gains.iter().all(|g| *g == 1.0));
            }
        }
    }
}
#[test]
fn sprinkle_varies_only_window_opening_and_keeps_background_through_note_gaps() {
    let mut e = engine(Shape::Sprinkle);
    e.configure(Config {
        scheduled: false,
        ..e.config
    });
    for _ in 0..300 {
        e.advance(256);
        e.frame(46.875, 256.0 / 48_000.0);
    }
    let depth = 36.0;
    let baseline = e.frame(46.875, 0.0).attenuation_db(20.0, depth);
    assert!((baseline - depth).abs() < 6e-5);
    let mut strengths = Vec::new();
    for cycle in 0..3 {
        e.set_source(
            0,
            Source {
                frequency_hz: 317.3,
                strength: 1.0,
                eligible: true,
            },
        );
        e.trigger(0);
        strengths.push(e.particles.iter().find(|p| p.active).unwrap().strength);
        // Alternate natural releases and a choke; all windows finish before
        // the next note, with incoming audio's background never reopening.
        if cycle == 1 {
            e.retire_source(0);
        } else {
            e.release_source(0);
        }
        for _ in 0..900 {
            e.advance(256);
            let f = e.frame(46.875, 256.0 / 48_000.0);
            assert_eq!(f.attenuation_db(20.0, depth), baseline);
        }
        assert_eq!(e.active_count(), 0);
        assert_eq!(e.frame(46.875, 0.0).attenuation_db(440.0, depth), baseline);
    }
    assert!(
        strengths
            .iter()
            .all(|s| (PARTICLE_LOW_OPENING * 0.85..=1.0).contains(s))
    );
    assert!(strengths.windows(2).any(|s| s[0] != s[1]));
}

#[test]
fn release_finishes_choke_detaches_and_sprinkle_keeps_motion_depth() {
    let mut e = engine(Shape::Sprinkle);
    e.configure(Config {
        scheduled: false,
        ..e.config
    });
    e.trigger(0);
    e.advance(256);
    e.frame(46.875, 256.0 / 48_000.0);
    e.release_source(0);
    let before = e.births;
    for _ in 0..1500 {
        e.advance(256);
        e.frame(46.875, 256.0 / 48_000.0);
    }
    assert_eq!(e.births, before);
    assert_eq!(e.active_count(), 0);
    assert!((e.activity - 1.0).abs() < 1e-6);
    let empty = e.frame(46.875, 0.0);
    for depth in [0.0, 12.0, 24.0, 60.0] {
        assert!((empty.attenuation_db(440.0, depth) - depth).abs() < 6e-5);
    }
    e.set_source(
        0,
        Source {
            frequency_hz: 400.0,
            strength: 1.0,
            eligible: true,
        },
    );
    e.trigger(0);
    e.retire_source(0);
    e.set_source(
        0,
        Source {
            frequency_hz: 800.0,
            strength: 1.0,
            eligible: true,
        },
    );
    assert_eq!(
        e.particles.iter().find(|p| p.active).unwrap().frequency_hz,
        400.0
    );
    e.advance(961);
    e.frame(46.875, 961.0 / 48_000.0);
    assert!(
        e.particles
            .iter()
            .filter(|p| p.active)
            .all(|p| { p.frequency_hz == 800.0 && p.source == Some(0) })
    );
}
#[test]
fn clouds_swell_in_place_even_when_rate_phase_size_and_direction_change() {
    for direction in [Direction::Forward, Direction::Reverse, Direction::Alternate] {
        for index in 0..2 {
            let mut e = engine(Shape::Cloud);
            e.release_source(0);
            e.configure(Config {
                direction,
                scheduled: false,
                ..e.config
            });
            e.birth(index, None);
            let p = e.particles[0];
            let initial = e.frame(46.875, 0.0).windows[0];
            assert_eq!(initial.amplitude, 0.0);
            let mut amplitudes = [0.0; 9];
            for (i, amplitude) in amplitudes.iter_mut().enumerate() {
                let sample = ((i + 1) as f64 * 0.1 * p.lifetime * 48_000.0) as u64;
                e.advance((sample - e.elapsed_samples()) as usize);
                e.configure(Config {
                    rate_hz: 32.0,
                    attack_ms: 5000.0,
                    release_ms: 0.0,
                    phase: 0.73,
                    size_octaves: 8.0,
                    direction: if direction == Direction::Reverse {
                        Direction::Forward
                    } else {
                        Direction::Reverse
                    },
                    ..e.config
                });
                let w = e.frame(46.875, 0.01).windows[0];
                assert_eq!(w.center_log2_hz, initial.center_log2_hz);
                assert_eq!(w.radius_octaves, initial.radius_octaves);
                assert_eq!(e.particles[0].attack, p.attack);
                *amplitude = w.amplitude;
            }
            if p.attack == CLOUD_ATTACK {
                assert!(amplitudes[4] > amplitudes[0] && amplitudes[4] > amplitudes[8]);
            } else {
                assert!(amplitudes[7] > amplitudes[4] && amplitudes[4] > amplitudes[0]);
            }
            assert_eq!(e.births, 1);
        }
    }
}

#[test]
fn cloud_direction_selects_envelopes_without_changing_placement_or_lifetime() {
    for index in 0..64 {
        let mut centers = [0.0; 3];
        let mut lifetimes = [0.0; 3];
        for (i, direction) in [Direction::Forward, Direction::Reverse, Direction::Alternate]
            .into_iter()
            .enumerate()
        {
            let mut e = engine(Shape::Cloud);
            e.release_source(0);
            e.configure(Config {
                direction,
                ..e.config
            });
            e.birth(index, None);
            let p = e.particles[0];
            assert!((PARTICLE_MIN_HZ..=20_000.0).contains(&p.frequency_hz));
            centers[i] = p.frequency_hz;
            lifetimes[i] = p.lifetime;
            let reverse = direction == Direction::Reverse
                || (direction == Direction::Alternate && index & 1 == 1);
            assert_eq!(
                p.attack,
                if reverse {
                    REVERSE_ATTACK
                } else {
                    CLOUD_ATTACK
                }
            );
        }
        assert_eq!(centers, [centers[0]; 3]);
        assert_eq!(lifetimes, [lifetimes[0]; 3]);
    }
}

#[test]
fn clouds_gate_without_sources_and_stopped_sync_only_allows_note_triggers() {
    let mut e = engine(Shape::Cloud);
    e.release_source(0);
    e.advance(256);
    let frame = e.frame(46.875, 256.0 / 48_000.0);
    assert!(frame.attenuation_db(440.0, 24.0) > 0.0);
    e.configure(Config {
        scheduled: false,
        shape: Shape::Sprinkle,
        ..e.config
    });
    e.set_source(
        0,
        Source {
            frequency_hz: 440.0,
            strength: 1.0,
            eligible: true,
        },
    );
    e.trigger(0);
    let births = e.births;
    for _ in 0..1000 {
        e.advance(256);
        e.frame(46.875, 256.0 / 48_000.0);
    }
    assert_eq!(e.births, births);
    assert_eq!(e.active_count(), 0);
}
#[test]
fn zero_endpoints_and_captured_lifetime_survive_rate_quality_and_seek() {
    let mut e = engine(Shape::Sprinkle);
    e.trigger(0);
    assert_eq!(e.frame(46.875, 0.0).windows[0].amplitude, 0.0);
    let lifetime = e.particles[0].lifetime;
    e.configure(Config {
        rate_hz: 32.0,
        hop_samples: 4096,
        ..e.config
    });
    assert_eq!(e.particles[0].lifetime, lifetime);
    e.advance(257);
    e.seek(153.7);
    let frame = e.frame(2.93, 257.0 / 48_000.0);
    assert!(frame.windows[0].amplitude > 0.0);
    e.advance(961);
    e.frame(2.93, 961.0 / 48_000.0);
    assert!(!e.particles[0].active);
    e.reset();
    assert_eq!(e.births, 0);
    assert_eq!(e.elapsed_samples(), 0);
}
#[test]
fn more_sources_do_not_multiply_scheduler_density() {
    let mut single = engine(Shape::Sprinkle);
    let mut chord = engine(Shape::Sprinkle);
    for slot in 1..12 {
        chord.set_source(
            slot,
            Source {
                frequency_hz: 350.0 + slot as f32,
                strength: 1.0,
                eligible: true,
            },
        );
    }
    for _ in 0..1000 {
        single.advance(256);
        chord.advance(256);
        single.frame(46.875, 256.0 / 48_000.0);
        chord.frame(46.875, 256.0 / 48_000.0);
    }
    assert_eq!(single.births, chord.births);
}

#[test]
fn reset_restores_fresh_decisions_and_masks_after_nondefault_configuration() {
    for sample_rate in [44_100.0, 48_000.0, 384_000.0] {
        let mut reset = Engine::new(sample_rate);
        reset.configure(Config {
            partials: MAX_PARTIALS,
            partial_rolloff_db: 24.0,
            phase: 0.75,
            sync: true,
            ..Config::default()
        });
        reset.set_note_input_present(true);
        reset.set_source(
            0,
            Source {
                frequency_hz: 317.3,
                strength: 0.8,
                eligible: true,
            },
        );
        reset.trigger(0);
        reset.advance(8192);
        reset.seek(153.7);
        reset.reset();
        let mut fresh = Engine::new(sample_rate);
        assert_eq!(reset.partial_weights, fresh.partial_weights);
        assert_eq!(reset.elapsed_samples(), 0);
        assert_eq!(reset.active_count(), 0);
        assert_eq!(
            (reset.births, reset.dropped, reset.decision_hash),
            (0, 0, 0)
        );
        for rolloff in [6.0, 0.0, 24.0] {
            let config = Config {
                partials: MAX_PARTIALS,
                partial_rolloff_db: rolloff,
                rate_hz: 32.0,
                ..Config::default()
            };
            reset.configure(config);
            fresh.configure(config);
            for _ in 0..128 {
                reset.advance(257);
                fresh.advance(257);
                let a = reset.frame(sample_rate / 1024.0, 257.0 / sample_rate);
                let b = fresh.frame(sample_rate / 1024.0, 257.0 / sample_rate);
                for hz in [20.0, 110.0, 440.0, 1731.0, 15_000.0] {
                    assert_eq!(a.attenuation_db(hz, 60.0), b.attenuation_db(hz, 60.0));
                }
                assert_eq!(
                    (reset.births, reset.dropped, reset.decision_hash),
                    (fresh.births, fresh.dropped, fresh.decision_hash)
                );
            }
        }
    }
}
