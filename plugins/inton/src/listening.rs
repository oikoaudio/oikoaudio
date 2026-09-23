//! Optional presentation guidance. Never changes embedded project data.
use inton_core::tuning::Preset;
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize)]
struct FactoryEntry {
    #[serde(flatten)]
    preset: Preset,
    listening_hint: String,
}
static FACTORY: LazyLock<Vec<FactoryEntry>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../resources/library.json"))
        .expect("validated factory scales and listening ideas")
});
fn factory(p: &Preset) -> Option<&'static FactoryEntry> {
    FACTORY.iter().find(|entry| {
        let f = &entry.preset;
        p.source_id
            .as_ref()
            .map_or(p.display_name == f.display_name, |id| {
                Some(id) == f.source_id.as_ref()
            })
            && f.scl_text == p.scl_text
            && f.kbm_text == p.kbm_text
    })
}
/// Text to show for `p`: the installed factory description for factory scales, otherwise
/// the preset's own; empty when it only repeats the display name.
pub fn description(p: &Preset) -> &str {
    let text = factory(p).map_or(p.description.as_str(), |f| f.preset.description.as_str());
    if text.trim() == p.display_name.trim() {
        ""
    } else {
        text
    }
}
/// Listening exercise for an unmodified factory scale, or an empty string if it has none.
pub fn hint(p: &Preset) -> &'static str {
    factory(p).map_or("", |f| f.listening_hint.as_str())
}

/// General orientation only where the resolved scale and default mapping support it.
pub fn hint_for(p: &Preset, t: &inton_core::tuning::Prepared) -> &'static str {
    let specific = hint(p);
    if !specific.is_empty() {
        return specific;
    }
    if p.kbm_text.is_none() && t.count == 7 && (t.period.log2() - 1.).abs() < 1e-9 {
        "Seven notes per octave: a heptatonic scale. Count seven consecutive MIDI keys, including black and white keys, to reach the octave. Try a drone on different degrees; the note you treat as home changes the character."
    } else {
        ""
    }
}
