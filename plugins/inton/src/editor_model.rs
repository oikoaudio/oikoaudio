//! Presentation state; fixed project slots and tuning data remain independent of layout.
use inton_core::{state::Project, tuning::Prepared};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const HEIGHT: u32 = 350;
pub use oiko_ui::scale::SCALE_STEPS;
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewPreferences {
    pub dark: bool,
    pub browser_open: bool,
    pub illustration: bool,
    pub listening_ideas: bool,
    pub scale: f64,
}
impl Default for ViewPreferences {
    fn default() -> Self {
        Self {
            dark: true,
            browser_open: false,
            illustration: true,
            listening_ideas: false,
            scale: 1.0,
        }
    }
}
impl ViewPreferences {
    pub fn nearest_scale(scale: f64) -> f64 {
        oiko_ui::scale::nearest_scale(scale)
    }
    pub fn width(self) -> u32 {
        420
    }
    pub fn height(self) -> u32 {
        HEIGHT
    }
    pub fn size(self, host_scale: f64) -> (u32, u32) {
        let scale = Self::nearest_scale(self.scale) * host_scale;
        (
            (self.width() as f64 * scale).round() as u32,
            (self.height() as f64 * scale).round() as u32,
        )
    }
}
#[derive(Default)]
pub struct SetView {
    pub destination: usize,
    pub(crate) revealed: BTreeSet<usize>,
}
impl SetView {
    pub fn rows(&self, project: &Project, _active: usize) -> Vec<usize> {
        (0..32)
            .filter(|&n| project.has_slot(n) || self.revealed.contains(&n))
            .collect()
    }
    pub fn add_slot(&mut self, project: &Project) -> Option<usize> {
        let n = (0..32).find(|n| !project.has_slot(*n) && !self.revealed.contains(n))?;
        self.revealed.insert(n);
        self.destination = n;
        Some(n)
    }
}
#[derive(Clone, Debug)]
pub struct ScaleShape {
    pub positions: Vec<f64>,
    pub period_ratio: f64,
    pub equal_division: bool,
}
fn compact_number(value: f64, precision: usize) -> String {
    if value != 0. && !(0.0001..1_000_000.).contains(&value.abs()) {
        return format!("{value:.precision$e}");
    }
    format!("{value:.precision$}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}
impl ScaleShape {
    pub fn summary(&self) -> String {
        let octaves = self.period_ratio.log2();
        let period = if octaves.round() >= 1. && (octaves - octaves.round()).abs() < 1e-9 {
            let n = octaves.round() as u32;
            format!("{n} {}", if n == 1 { "octave" } else { "octaves" })
        } else {
            let ratio = compact_number(self.period_ratio, 4);
            let approximate = ratio.parse::<f64>().is_ok_and(|rounded| {
                (rounded - self.period_ratio).abs() > self.period_ratio.abs() * 1e-10
            });
            format!("{}{ratio}:1 period", if approximate { "≈" } else { "" })
        };
        format!(
            "{} {} · {period}",
            self.positions.len(),
            if self.equal_division {
                "equal steps"
            } else {
                "notes"
            }
        )
    }
    pub fn period_detail(&self) -> String {
        format!(
            "Period {}:1 · {} cents",
            compact_number(self.period_ratio, 6),
            compact_number(1200. * self.period_ratio.log2(), 3)
        )
    }

    pub fn from_prepared(t: &Prepared) -> Self {
        let period = t.degrees_cents.last().copied().unwrap_or(0.0);
        let mut positions = vec![0.0];
        if period.abs() > 1e-9 {
            positions.extend(
                t.degrees_cents
                    .iter()
                    .take(t.count.saturating_sub(1))
                    .map(|c| (c / period).rem_euclid(1.0)),
            );
        }
        let equal_division = positions.len() == t.count
            && positions
                .iter()
                .enumerate()
                .all(|(i, c)| (c - i as f64 / t.count as f64).abs() < 1e-6);
        Self {
            positions,
            period_ratio: t.period,
            equal_division,
        }
    }
}

/// A KBM can skip or remap keys, so its scale length alone cannot describe keyboard spacing.
pub fn mapping_summary(preset: &inton_core::tuning::Preset, tuning: &Prepared) -> String {
    let mapping = if preset.kbm_text.is_some() {
        "Custom keyboard mapping".to_owned()
    } else {
        format!(
            "{}: +{} keys",
            if (tuning.period.log2() - 1.).abs() < 1e-9 {
                "Octave"
            } else {
                "Repeat"
            },
            tuning.count
        )
    };
    format!("{mapping} · root MIDI {}", tuning.root_note)
}

/// Step through occupied automation positions without renumbering or wrapping.
pub fn adjacent_slot(project: &Project, active: usize, forward: bool) -> Option<usize> {
    if forward {
        ((active + 1)..project.slots.len()).find(|&n| project.slots[n].is_some())
    } else {
        (0..active.min(project.slots.len()))
            .rev()
            .find(|&n| project.slots[n].is_some())
    }
}

/// Independent presentation choices; the persisted browser-open preference is unchanged.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum BrowserTab {
    #[default]
    Library,
    Set,
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum AssignmentIntent {
    #[default]
    Browse,
    Append,
    Replace(usize),
}
#[derive(Clone, Default)]
pub(crate) enum Inspection {
    #[default]
    Active,
    Slot(usize),
    Library(String),
}
impl Inspection {
    pub fn library(&self) -> Option<&String> {
        match self {
            Self::Library(id) => Some(id),
            _ => None,
        }
    }
    pub fn slot(&self) -> Option<usize> {
        match self {
            Self::Slot(slot) => Some(*slot),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Overlay {
    #[default]
    None,
    Settings,
    ListeningGuide,
    QuickGuide,
    Connection,
}
impl Overlay {
    pub fn is_guide(self) -> bool {
        matches!(self, Self::ListeningGuide | Self::QuickGuide)
    }
}
