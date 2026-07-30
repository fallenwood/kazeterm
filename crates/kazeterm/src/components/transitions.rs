use std::time::Duration;

use gpui::{Pixels, Size};

pub(crate) const UI_TRANSITION_FRAMES: u32 = 12;
pub(crate) const UI_TRANSITION_FRAME_DURATION: Duration = Duration::from_millis(15);

pub(crate) fn interpolate_f32(start: f32, target: f32, progress: f32) -> f32 {
  let progress = gpui::ease_in_out(progress.clamp(0.0, 1.0));
  start + (target - start) * progress
}

pub(crate) fn interpolate_pixels(start: Pixels, target: Pixels, progress: f32) -> Pixels {
  start + (target - start) * gpui::ease_in_out(progress.clamp(0.0, 1.0))
}

pub(crate) fn interpolate_size(
  start: Size<Pixels>,
  target: Size<Pixels>,
  progress: f32,
) -> Size<Pixels> {
  Size {
    width: interpolate_pixels(start.width, target.width, progress),
    height: interpolate_pixels(start.height, target.height, progress),
  }
}

#[cfg(test)]
mod tests {
  use gpui::{px, size};

  use super::*;

  #[test]
  fn interpolation_eases_between_endpoints() {
    let start = size(px(800.0), px(600.0));
    let target = size(px(1200.0), px(900.0));

    assert_eq!(interpolate_size(start, target, 0.0), start);
    assert_eq!(interpolate_size(start, target, 1.0), target);
    assert_eq!(
      interpolate_size(start, target, 0.5),
      size(px(1000.0), px(750.0))
    );
  }
}
