use crate::params::{
    BoolParam, FloatParam, IntParam, InternalParamMut, Param,
    range::{FloatRange, IntRange},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn clipped_float_keeps_base_edit_and_deduplicates_effective_callback() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let p = FloatParam::new("Test", 0.8, FloatRange::Linear { min: 0.0, max: 1.0 }).with_callback(
        Arc::new(move |_| {
            counter.fetch_add(1, Ordering::Relaxed);
        }),
    );
    unsafe {
        assert!(p._internal_modulate_value(0.5));
        assert!(p._internal_set_plain_value(0.7));
        assert_eq!(p.unmodulated_plain_value(), 0.7);
        assert_eq!(p.unmodulated_normalized_value(), 0.7);
        assert_eq!(p.value(), 1.0);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert!(!p._internal_set_plain_value(0.7));
        assert!(p._internal_modulate_value(0.0));
        assert_eq!(p.value(), 0.7);
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }
}

#[test]
fn clipped_integer_and_boolean_keep_base_edits() {
    let integer = IntParam::new("Integer", 8, IntRange::Linear { min: 0, max: 10 });
    let boolean = BoolParam::new("Boolean", true);
    unsafe {
        integer._internal_modulate_value(0.5);
        assert!(integer._internal_set_plain_value(7));
        assert_eq!(integer.unmodulated_plain_value(), 7);
        assert_eq!(integer.value(), 10);
        integer._internal_modulate_value(0.0);
        assert_eq!(integer.value(), 7);
        boolean._internal_modulate_value(1.0);
        assert!(boolean._internal_set_plain_value(false));
        assert!(!boolean.unmodulated_plain_value());
        assert!(boolean.value());
        boolean._internal_modulate_value(0.0);
        assert!(!boolean.value());
    }
}

#[test]
fn lower_clamp_and_quantization_update_normalized_values() {
    let p = FloatParam::new("Float", 0.2, FloatRange::Linear { min: 0.0, max: 1.0 });
    let integer = IntParam::new("Integer", 5, IntRange::Linear { min: 0, max: 10 });
    unsafe {
        p._internal_modulate_value(-0.5);
        p._internal_set_plain_value(0.3);
        p._internal_modulate_value(0.0);
        assert_eq!(p.value(), 0.3);
        integer._internal_modulate_value(0.01);
        assert_eq!(integer.value(), 5);
        assert!((integer.modulated_normalized_value() - 0.51).abs() < 1e-6);
        integer._internal_modulate_value(0.02);
        assert!((integer.modulated_normalized_value() - 0.52).abs() < 1e-6);
    }
}
