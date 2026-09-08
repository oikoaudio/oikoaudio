use spectral_dsp::MANUAL_MASK_POINTS;
use std::fmt::Write;

pub(crate) fn encode(curve: &[f32; MANUAL_MASK_POINTS], sample_rate: f32) -> String {
    let mut text = format!("OIKO_WEFT_CURVE 1 {sample_rate}\n");
    for value in curve {
        let _ = writeln!(text, "{value}");
    }
    text
}

pub(crate) fn decode(
    text: &str,
    sample_rate: f32,
) -> Result<[f32; MANUAL_MASK_POINTS], &'static str> {
    if text.len() > 1_000_000 {
        return Err("Curve data is too large");
    }
    let mut fields = text.split_whitespace();
    if fields.next() != Some("OIKO_WEFT_CURVE") || fields.next() != Some("1") {
        return Err("Clipboard does not contain a Weft curve");
    }
    let source_rate: f32 = fields
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or("Missing sample rate")?;
    if !source_rate.is_finite() || !(8_000.0..=768_000.0).contains(&source_rate) {
        return Err("Invalid sample rate");
    }
    let mut source = [0.0_f32; MANUAL_MASK_POINTS];
    for value in &mut source {
        *value = fields
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or("Incomplete curve data")?;
        if !value.is_finite() || value.abs() > crate::curve::CURVE_TRANSFORM_STORAGE_LIMIT_DB {
            return Err("Invalid curve value");
        }
    }
    if fields.next().is_some() {
        return Err("Unexpected curve data");
    }
    if sample_rate == source_rate {
        return Ok(source);
    }
    Ok(std::array::from_fn(|i| {
        let position = (i as f32 * sample_rate / source_rate).min((MANUAL_MASK_POINTS - 1) as f32);
        let left = position.floor() as usize;
        let right = (left + 1).min(MANUAL_MASK_POINTS - 1);
        source[left] + (source[right] - source[left]) * (position - left as f32)
    }))
}

#[cfg(test)]
mod tests;
