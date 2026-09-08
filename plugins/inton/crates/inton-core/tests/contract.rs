use inton_core::engine::Parameter;
use inton_core::{
    engine::{Engine, Parameters},
    state::Project,
    tuning::Preset,
};
fn scale(n: usize, period: f64) -> Preset {
    let mut text = format!("Test\n{n}\n");
    for i in 1..=n {
        text += &format!("{:.12}\n", 1200.0 * period.log2() * i as f64 / n as f64);
    }
    Preset::new(format!("{n} divisions of {period}"), text, None)
}
fn close(a: f64, b: f64) {
    assert!((a / b - 1.0).abs() < 1e-8, "{a} != {b}");
}
#[test]
fn coercion_and_stable_defaults() {
    let mut p = Parameters::default();
    assert_eq!(p.values(), [0.0, 0.0, 440.0, 0.0, 1.0]);
    p.set(Parameter::Position, 1.9);
    assert_eq!(p.position, 1);
    p.set(Parameter::Position, 35.0);
    assert_eq!(p.position, 31);
    p.set(Parameter::Position, -2.0);
    assert_eq!(p.position, 0);
    p.set(Parameter::Transpose, -1.9);
    assert_eq!(p.transpose, -1);
    p.set(Parameter::Transpose, 99.0);
    assert_eq!(p.transpose, 24);
    p.set(Parameter::Reference, f64::NAN);
    assert_eq!(p.reference, 440.0);
}
#[test]
fn arbitrary_periods_and_default_reference() {
    for (n, period) in [
        (12, 2.0),
        (19, 2.0),
        (22, 2.0),
        (31, 2.0),
        (53, 2.0),
        (13, 3.0),
        (13, 8.0),
        (17, 4.0),
        (11, 2.7),
    ] {
        let t = scale(n, period).prepare().unwrap();
        close(t.hz[69], 440.0);
        close(t.hz[60 + n] / t.hz[60], period);
    }
}
#[test]
fn instant_empty_clear_and_atomic_assignment() {
    let mut p = Project::default();
    p.assign(1, scale(22, 2.0)).unwrap();
    let mut e = Engine::new(&p).unwrap();
    let mut par = Parameters {
        position: 1,
        ..Default::default()
    };
    e.update(par, &p, false);
    let held = e.frequencies();
    par.position = 4;
    e.update(par, &p, false);
    e.advance(5.0);
    assert_eq!(e.frequencies(), held);
    assert!(e.empty);
    assert!(
        p.assign(1, Preset::new("bad".into(), "bad".into(), None))
            .is_err()
    );
    assert_eq!(
        p.slots[1].as_ref().unwrap().display_name,
        "22 divisions of 2"
    );
    p.clear(1).unwrap();
    assert!(p.slots[0].is_some());
    assert!(p.slots[1].is_none());
}
#[test]
fn logarithmic_morph_duration_capture_and_retarget() {
    let mut p = Project::default();
    p.assign(1, scale(22, 2.0)).unwrap();
    p.assign(2, scale(13, 3.0)).unwrap();
    let mut e = Engine::new(&p).unwrap();
    let a = e.frequencies();
    let mut par = Parameters {
        position: 1,
        morph_ms: 2000.0,
        ..Default::default()
    };
    e.update(par, &p, false);
    e.advance(0.8);
    let halfway = e.frequencies();
    let b = p.slots[1].as_ref().unwrap().prepare().unwrap().hz;
    close(halfway[60], a[60].powf(0.6) * b[60].powf(0.4));
    par.morph_ms = 5000.0;
    e.update(par, &p, false);
    e.advance(1.2);
    close(e.frequencies()[60], b[60]);
    par.position = 0;
    par.morph_ms = 2000.0;
    e.update(par, &p, false);
    e.advance(0.8);
    let current = e.frequencies();
    par.position = 2;
    e.update(par, &p, false);
    assert_eq!(e.frequencies(), current);
    e.advance(1.0);
    let c = p.slots[2].as_ref().unwrap().prepare().unwrap().hz;
    close(e.frequencies()[60], (current[60] * c[60]).sqrt());
}
#[test]
fn empty_mid_morph_freezes_current_tuning() {
    let mut p = Project::default();
    p.assign(1, scale(22, 2.0)).unwrap();
    let mut e = Engine::new(&p).unwrap();
    let mut par = Parameters {
        position: 1,
        morph_ms: 2000.0,
        ..Default::default()
    };
    e.update(par, &p, false);
    e.advance(0.8);
    let held = e.frequencies();
    par.position = 31;
    e.update(par, &p, false);
    e.advance(9.0);
    assert_eq!(held, e.frequencies());
}
#[test]
fn simultaneous_params_and_independent_reference() {
    let mut p = Project::default();
    p.assign(2, scale(22, 2.0)).unwrap();
    let mut e = Engine::new(&p).unwrap();
    let mut par = Parameters {
        position: 2,
        reference: 432.0,
        transpose: 12,
        ..Default::default()
    };
    e.update(par, &p, false);
    let base = p.slots[2].as_ref().unwrap().prepare().unwrap().hz;
    close(e.frequencies()[60], base[60] * (432.0 / 440.0) * 2.0);
    par.morph_ms = 10000.0;
    par.reference = 440.0;
    e.update(par, &p, false);
    close(e.frequencies()[60], base[60] * 2.0);
}
#[test]
fn embedded_portability_version_and_held_recall() {
    let mut p = Project::default();
    p.assign(7, scale(13, 8.0)).unwrap();
    p.parameters.position = 7;
    let bytes = p.encode().unwrap();
    let restored = Project::decode(&bytes).unwrap();
    assert_eq!(restored.slots.len(), 32);
    close(
        Engine::new(&restored).unwrap().frequencies()[82]
            / Engine::new(&restored).unwrap().frequencies()[69],
        8.0,
    );
    assert!(Project::decode(b"{\"version\":999}").is_err());
}
#[test]
fn kbm_reference_root_and_unmapped_rejection() {
    let mut p = scale(12, 2.0);
    p.kbm_text = Some("0\n0\n127\n60\n60\n256.0\n12\n".into());
    let t = p.prepare().unwrap();
    close(t.hz[60], 440.0);
    assert_eq!(t.reference_note, 60);
    close(t.original_reference, 256.0);
    p.kbm_text = Some("2\n0\n127\n60\n60\n256.0\n12\n0\nx\n".into());
    assert!(p.prepare().is_err());
}
#[test]
fn valid_empty_scala_description_and_malformed_limits() {
    let p = Preset::new(
        "empty description".into(),
        "! comment\n\n2\n700.0\n2/1\n".into(),
        None,
    );
    assert!(p.prepare().is_ok());
    for text in [
        "bad\n999999999999\n2/1\n",
        "bad\n2\n0/1\n2/1\n",
        "bad\n2\n3/0\n2/1\n",
        "bad\n2\n100.0\n",
    ] {
        assert!(
            Preset::new("bad".into(), text.into(), None)
                .prepare()
                .is_err()
        );
    }
}
#[test]
fn oversized_kbm_degree_is_rejected_before_scale_expansion() {
    let p = Preset::new(
        "tiny period".into(),
        "Tiny\n1\n0.001\n".into(),
        Some("1\n0\n127\n60\n60\n440\n1\n5000\n".into()),
    );
    assert!(p.prepare().is_err());
}
#[test]
fn held_tuning_state_is_bit_exact_across_json_roundtrip() {
    let p = Project {
        held_log: Some(vec![3.4598213596328002; 128]),
        parameters: Parameters {
            position: 31,
            ..Default::default()
        },
        ..Default::default()
    };
    let bytes = p.encode().unwrap();
    assert_eq!(bytes, Project::decode(&bytes).unwrap().encode().unwrap());
}

#[test]
fn extended_kbm_reference_preserves_mapping_and_bounds() {
    // An off-keyboard reference is still a pitch anchor. These are synthetic,
    // not artist tuning data; include a full-keyboard, nonascending export.
    let mut p = scale(128, 2.0_f64.powf(12800.0 / 1200.0));
    let keys = (0..128).map(|n| format!("{n}\n")).collect::<String>();
    for reference in [-256, -59, 255] {
        p.kbm_text = Some(format!("128\n0\n127\n60\n{reference}\n440.0\n128\n{keys}"));
        let t = p.prepare().unwrap();
        assert_eq!(t.reference_note, reference);
        for n in 0..128 {
            close(
                t.hz[n],
                440.0 * 2.0_f64.powf((n as f64 - reference as f64) / 12.0),
            );
        }
    }
    for reference in [-257, 256, i32::MIN, i32::MAX] {
        p.kbm_text = Some(format!("128\n0\n127\n60\n{reference}\n440.0\n128\n{keys}"));
        assert!(p.prepare().is_err());
    }
    // Repeated and negative degrees are legitimate in keyboard-table exports.
    p.scl_text = format!(
        "Synthetic keyboard table\n128\n{}12800.0\n",
        (1..128)
            .map(|n| format!(
                "{:.1}\n",
                if !(25..=99).contains(&n) {
                    0.0
                } else {
                    (n - 100) as f64 * 100.0
                }
            ))
            .collect::<String>()
    );
    p.kbm_text = Some(format!("128\n0\n127\n60\n-59\n440.0\n128\n{keys}"));
    let t = p.prepare().unwrap();
    for n in 0..128 {
        let relative = n as i32 - 60;
        let degree = relative.rem_euclid(128) as usize;
        let cents = if degree == 0 {
            0.0
        } else {
            t.degrees_cents[degree - 1]
        };
        let anchor_cents = -12800.0;
        close(
            t.hz[n],
            440.0
                * 2.0_f64.powf(
                    (cents + relative.div_euclid(128) as f64 * 12800.0 - anchor_cents) / 1200.0,
                ),
        );
    }
}

#[test]
fn invalid_project_is_rejected_at_decode_and_engine_boundaries() {
    for corrupt in [
        |p: &mut Project| p.slots.clear(),
        |p: &mut Project| p.parameters.position = 32,
        |p: &mut Project| p.parameters.reference = 0.0,
        |p: &mut Project| p.held_log = Some(vec![0.0; 127]),
        |p: &mut Project| p.version = 999,
        |p: &mut Project| p.slots[0].as_mut().unwrap().scl_text = "invalid".into(),
    ] {
        let mut project = Project::default();
        corrupt(&mut project);
        assert!(Project::decode(&project.encode().unwrap()).is_err());
        assert!(Engine::new(&project).is_err());
    }
}
