use inton::library::Library;
use inton_core::state::Project;

fn electronic() -> Vec<inton_core::tuning::Preset> {
    let lib = Library::factory();
    lib.entries
        .iter()
        .filter(|e| e.category == "Factory/Electronic/Detuned 12-note")
        .map(|e| lib.load(&e.id).unwrap())
        .collect()
}

#[test]
fn electronic_collection_is_original_searchable_and_spans_subtle_to_strong() {
    let presets = electronic();
    assert!(!presets.is_empty());
    let lib = Library::factory();
    assert_eq!(lib.search("detuned").len(), presets.len());
    let mut strengths = Vec::new();
    let mut signatures = std::collections::BTreeSet::new();
    for p in presets {
        assert!(p.attribution.contains("CC0-1.0"));
        assert!(p.tags.iter().any(|t| t == "electronic"));
        let t = p.prepare().unwrap();
        let offsets: Vec<_> = t
            .degrees_cents
            .iter()
            .enumerate()
            .map(|(i, c)| c - (i + 1) as f64 * 100.)
            .collect();
        assert!(offsets.iter().any(|v| *v > 1.) && offsets.iter().any(|v| *v < -1.));
        let strength = offsets.iter().map(|v| v.abs()).fold(0_f64, f64::max);
        assert!(strength <= 40.);
        strengths.push(strength);
        assert!(
            signatures.insert(
                offsets
                    .iter()
                    .map(|v| (v * 100.).round() as i32)
                    .collect::<Vec<_>>()
            )
        );
    }
    assert!(strengths.iter().any(|v| *v <= 6.));
    assert!(strengths.iter().any(|v| *v >= 30.));
}

#[test]
fn detunings_preserve_twelve_keys_octaves_a440_and_pitch_order() {
    let presets = electronic();
    assert!(!presets.is_empty());
    for p in presets {
        let t = p.prepare().unwrap();
        assert_eq!(t.count, 12);
        assert!((t.period - 2.).abs() < 1e-10);
        assert!((t.hz[69] - 440.).abs() < 1e-8);
        // C and A are fixed anchors; the remaining pitch classes bend around them.
        let c = 440. * 2_f64.powf(-9. / 12.);
        assert!((t.hz[60] - c).abs() < 1e-8);
        assert!(
            t.hz.windows(2)
                .all(|pair| pair[1] > pair[0] && pair[0] > 0.)
        );
        for note in 0..116 {
            assert!((t.hz[note + 12] / t.hz[note] - 2.).abs() < 1e-9);
        }
        for pair in t.degrees_cents.windows(2) {
            assert!(pair[1] - pair[0] >= 20.);
        }
    }
}

#[test]
fn new_scales_are_embedded_portably_and_scl_exports_match_the_factory() {
    let presets = electronic();
    assert!(!presets.is_empty());
    let records: serde_json::Value =
        serde_json::from_str(include_str!("../resources/library.json")).unwrap();
    for p in presets {
        let record = records
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source_id"].as_str() == p.source_id.as_deref())
            .unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join(record["scl_resource"].as_str().unwrap());
        assert_eq!(std::fs::read_to_string(path).unwrap(), p.scl_text);
        let mut project = Project::default();
        project.assign(3, p.clone()).unwrap();
        let recalled = Project::decode(&project.encode().unwrap()).unwrap();
        assert_eq!(recalled.slots[3].as_ref().unwrap(), &p);
        assert_eq!(
            recalled.slots[3].as_ref().unwrap().prepare().unwrap().hz,
            p.prepare().unwrap().hz
        );
    }
}

#[test]
fn interval_studies_distinguish_unsettled_thirds_from_unsettled_fifths() {
    let lib = Library::factory();
    let thirds = lib
        .load("factory:detune-frayed-thirds")
        .unwrap()
        .prepare()
        .unwrap();
    let fifths = lib
        .load("factory:detune-restless-fifths")
        .unwrap()
        .prepare()
        .unwrap();
    let pure_third = 1200. * 1.25_f64.log2();
    let pure_fifth = 1200. * 1.5_f64.log2();
    assert!((thirds.degrees_cents[6] - pure_fifth).abs() < 0.1);
    assert!((thirds.degrees_cents[3] - pure_third).abs() > 20.);
    assert!((fifths.degrees_cents[3] - pure_third).abs() < 0.5);
    assert!((fifths.degrees_cents[6] - pure_fifth).abs() > 10.);
}
