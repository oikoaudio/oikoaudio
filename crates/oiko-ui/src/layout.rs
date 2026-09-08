//! Shared layout measurements. Calculate columns once and reuse them across rows.
use egui::{Rect, Vec2, pos2};
pub const HEADER_HEIGHT: f32 = 40.0;
pub const INSET: f32 = 12.0;
pub const GAP: f32 = 6.0;
pub const CONTROL_HEIGHT: f32 = 24.0;

/// Equal-width columns on shared vertical rails; row height and y may vary.
#[derive(Clone, Copy)]
pub struct Columns {
    left: f32,
    width: f32,
    gap: f32,
    count: usize,
}
impl Columns {
    pub fn equal(left: f32, total_width: f32, count: usize, gap: f32) -> Self {
        assert!(count > 0 && gap >= 0.0 && total_width >= gap * (count - 1) as f32);
        Self {
            left,
            width: (total_width - gap * (count - 1) as f32) / count as f32,
            gap,
            count,
        }
    }
    pub fn cell(self, column: usize, top: f32, height: f32) -> Rect {
        assert!(column < self.count);
        Rect::from_min_size(
            pos2(self.left + column as f32 * (self.width + self.gap), top),
            Vec2::new(self.width, height),
        )
    }
}

#[cfg(test)]
mod tests;
