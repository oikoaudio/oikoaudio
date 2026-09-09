use super::*;
#[test]
fn particle_observations_retain_last_frame_during_publication() {
    let display = AnalysisDisplay::default();
    let mut snapshot = ParticleMask::default();
    display.store_particles(&[0.5, 0.25, 0.5], 10.0, true);
    display.read_particles(&mut snapshot);
    assert!(snapshot.active);
    assert!((snapshot.attenuation_db(10.0) - 12.0412).abs() < 1e-4);
    display.particle_generation.fetch_add(1, Ordering::SeqCst);
    display.particle_words[1].store(1.0_f32.to_bits(), Ordering::SeqCst);
    display.read_particles(&mut snapshot);
    assert!((snapshot.attenuation_db(10.0) - 12.0412).abs() < 1e-4);
}
