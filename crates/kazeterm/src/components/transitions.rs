use std::time::Duration;

use gpui::{Pixels, Size};

/// Default timing retained for deterministic transition tests.
#[cfg(test)]
pub(crate) const UI_TRANSITION_FRAMES: u32 = 12;
#[cfg(test)]
pub(crate) const UI_TRANSITION_FRAME_DURATION: Duration = Duration::from_millis(15);

#[derive(Clone, Copy, Debug)]
pub(crate) struct TransitionSpec {
  pub(crate) frames: u32,
  pub(crate) frame_duration: Duration,
  easing: ::config::AnimationEasing,
}

impl TransitionSpec {
  pub(crate) fn from_config(config: &::config::AnimationConfig) -> Option<Self> {
    let frames = config.get_frame_count();
    (frames > 0).then(|| Self {
      frames,
      frame_duration: config.get_frame_duration(),
      easing: config.easing,
    })
  }

  pub(crate) fn progress(self, frame: u32) -> f32 {
    self.easing.apply(frame as f32 / self.frames as f32)
  }
}

pub(crate) fn interpolate_pixels(start: Pixels, target: Pixels, progress: f32) -> Pixels {
  start + (target - start) * progress.clamp(0.0, 1.0)
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
  fn interpolation_reaches_expected_points() {
    let start = size(px(800.0), px(600.0));
    let target = size(px(1200.0), px(900.0));

    assert_eq!(interpolate_size(start, target, 0.0), start);
    assert_eq!(interpolate_size(start, target, 1.0), target);
    assert_eq!(
      interpolate_size(start, target, 0.5),
      size(px(1000.0), px(750.0))
    );
  }

  #[test]
  fn transition_spec_uses_configured_timing_and_easing() {
    let config = ::config::AnimationConfig {
      duration_ms: 200,
      frame_interval_ms: 20,
      easing: ::config::AnimationEasing::EaseIn,
      ..Default::default()
    };
    let transition = TransitionSpec::from_config(&config).unwrap();

    assert_eq!(transition.frames, 10);
    assert_eq!(transition.frame_duration, Duration::from_millis(20));
    assert_eq!(transition.progress(5), 0.25);
  }

  #[test]
  fn disabled_transition_has_no_spec() {
    let config = ::config::AnimationConfig {
      enabled: false,
      ..Default::default()
    };
    assert!(TransitionSpec::from_config(&config).is_none());
  }
}
