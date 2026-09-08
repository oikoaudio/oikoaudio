//! Host-persisted UI zoom. The serialized field remains a plain f32.
use nice_plug::params::persist::PersistentField;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

/// DEFAULT_PERCENT preserves each product's established initial zoom.
#[derive(Clone)]
pub struct UiScaleState<const DEFAULT_PERCENT: u32 = 100> {
    value: Arc<AtomicU32>,
}
impl<const DEFAULT_PERCENT: u32> Default for UiScaleState<DEFAULT_PERCENT> {
    fn default() -> Self {
        Self {
            value: Arc::new(AtomicU32::new(
                oiko_ui::scale::closest_ui_scale(DEFAULT_PERCENT as f32 / 100.0).to_bits(),
            )),
        }
    }
}
impl<const DEFAULT_PERCENT: u32> UiScaleState<DEFAULT_PERCENT> {
    pub fn get(&self) -> f32 {
        f32::from_bits(self.value.load(Ordering::Acquire))
    }
    pub fn set(&self, scale: f32) {
        self.value.store(
            oiko_ui::scale::closest_ui_scale(scale).to_bits(),
            Ordering::Release,
        );
    }
}
impl<'a, const DEFAULT_PERCENT: u32> PersistentField<'a, f32> for UiScaleState<DEFAULT_PERCENT> {
    fn set(&self, value: f32) {
        self.set(value);
    }
    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&f32) -> R,
    {
        f(&self.get())
    }
}
#[cfg(test)]
mod tests;
