//! Validated SCL/KBM parsing for a complete 128-note tuning table.
//! No GUI, plugin framework, or MTS dependency. Parsing is not realtime-safe.
use std::ffi::{CString, c_char, c_int};
/// Largest accepted SCL or KBM text, in bytes.
pub const MAX_TUNING_BYTES: usize = 1_048_576;
/// A parsed scale mapped onto every MIDI note.
#[derive(Clone, Debug)]
pub struct Prepared {
    /// Each SCL interval in cents, in file order; the last entry is the period.
    pub degrees_cents: Vec<f64>,
    /// Frequency of each MIDI note, scaled so the reference note sounds at 440 Hz.
    pub hz: [f64; 128],
    /// Period as a frequency ratio, such as 2.0 for an octave.
    pub period: f64,
    /// Number of SCL intervals; equals `degrees_cents.len()`.
    pub count: usize,
    /// KBM reference note, in -256..=255; may lie outside the MIDI range.
    pub reference_note: i32,
    /// KBM root note (the note that plays scale degree zero).
    pub root_note: i32,
    /// Reference frequency declared by the KBM, in Hz.
    pub original_reference: f64,
}
unsafe extern "C" {
    fn oiko_prepare_tuning(
        scl: *const c_char,
        kbm: *const c_char,
        hz: *mut f64,
        period: *mut f64,
        count: *mut c_int,
        reference: *mut c_int,
        root: *mut c_int,
        original: *mut f64,
        error: *mut c_char,
        len: usize,
        degrees: *mut f64,
    ) -> c_int;
}

/// Parses SCL text and optional KBM text into a complete 128-note tuning.
///
/// Without a KBM, the scale starts on MIDI note 60 and note 69 is the 440 Hz
/// reference. Fails with a user-readable message when either text exceeds
/// `MAX_TUNING_BYTES`, declares more than 4096 entries, is malformed, leaves
/// any MIDI note unmapped, or produces a non-finite frequency.
pub fn prepare(scl_text: &str, kbm_text: Option<&str>) -> Result<Prepared, String> {
    // Bound user input before entering the parser, including declared allocation counts.
    // `index` is the line holding the declared count, after comments: SCL 1, KBM 0.
    for (text, index) in [(Some(scl_text), 1), (kbm_text, 0)] {
        if let Some(text) = text {
            if text.len() > MAX_TUNING_BYTES {
                return Err("Tuning file exceeds 1 MiB".into());
            }
            let lines: Vec<_> = text
                .lines()
                .filter(|l| {
                    !l.trim_start().starts_with('!') && (index == 1 || !l.trim().is_empty())
                })
                .collect();
            if index == 0
                && let Some(reference) = lines.get(4)
            {
                let reference = reference
                    .trim()
                    .parse::<i32>()
                    .map_err(|_| "Invalid KBM reference note")?;
                if !(-256..=255).contains(&reference) {
                    return Err("KBM reference note must be between -256 and 255".into());
                }
            }
            if let Some(line) = lines.get(index) {
                let n = line
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .parse::<i64>()
                    .map_err(|_| "Invalid SCL/KBM size")?;
                if !(0..=4096).contains(&n) {
                    return Err("SCL/KBM size must be at most 4096".into());
                }
            }
        }
    }
    let scl = CString::new(scl_text.as_bytes()).map_err(|_| "SCL contains a NUL byte")?;
    let kbm = kbm_text
        .map(|s| CString::new(s.as_bytes()))
        .transpose()
        .map_err(|_| "KBM contains a NUL byte")?;
    let mut t = Prepared {
        degrees_cents: vec![0.0; 4096],
        hz: [0.0; 128],
        period: 0.0,
        count: 0,
        reference_note: 0,
        root_note: 0,
        original_reference: 0.0,
    };
    let mut count = 0;
    let mut error = [0u8; 2048];
    let ok = unsafe {
        oiko_prepare_tuning(
            scl.as_ptr(),
            kbm.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            t.hz.as_mut_ptr(),
            &mut t.period,
            &mut count,
            &mut t.reference_note,
            &mut t.root_note,
            &mut t.original_reference,
            error.as_mut_ptr().cast(),
            error.len(),
            t.degrees_cents.as_mut_ptr(),
        )
    };
    if ok == 0 {
        return Err(String::from_utf8_lossy(
            &error[..error.iter().position(|b| *b == 0).unwrap_or(error.len())],
        )
        .into_owned());
    }
    t.count = count as usize;
    t.degrees_cents.truncate(t.count);
    Ok(t)
}
