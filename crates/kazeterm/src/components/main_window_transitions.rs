use gpui::{Context, Pixels, Task, Window, px};

use super::main_window::MainWindow;
use super::transitions::{
  UI_TRANSITION_FRAME_DURATION, UI_TRANSITION_FRAMES, interpolate_f32, interpolate_pixels,
};

const CONFIGURATION_TRANSITION_START_OPACITY: f32 = 0.82;

impl MainWindow {
  pub(crate) fn set_tab_bar_visible(
    &mut self,
    visible: bool,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.tab_bar_visible = visible;
    let target_width =
      self.vertical_tabbar_target_width(cx.global::<::config::Config>().tab.vertical);
    self.animate_vertical_tabbar_to(target_width, window, cx);
  }

  pub(crate) fn transition_configuration_change(
    &mut self,
    config: &::config::Config,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let min_width = px(
      config
        .tab
        .get_vertical_tabbar_min_width(config.font.ui_size),
    );
    let max_width = (window.bounds().size.width - px(160.0)).max(min_width);
    self.vertical_tabbar_width = self.vertical_tabbar_width.max(min_width).min(max_width);

    let target_width = self.vertical_tabbar_target_width(config.tab.vertical);
    self.animate_vertical_tabbar_to(target_width, window, cx);
    self.animate_configuration_fade(window, cx);
  }

  fn vertical_tabbar_target_width(&self, vertical_tabs: bool) -> Pixels {
    if vertical_tabs && self.tab_bar_visible {
      self.vertical_tabbar_width
    } else {
      Pixels::ZERO
    }
  }

  fn animate_vertical_tabbar_to(
    &mut self,
    target_width: Pixels,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let start_width = self.vertical_tabbar_render_width;
    if start_width == target_width {
      self.vertical_tabbar_animation = Task::ready(());
      cx.notify();
      return;
    }

    cx.notify();
    self.vertical_tabbar_animation = cx.spawn_in(window, async move |this, cx| {
      for frame in 1..=UI_TRANSITION_FRAMES {
        cx.background_executor()
          .timer(UI_TRANSITION_FRAME_DURATION)
          .await;

        let progress = frame as f32 / UI_TRANSITION_FRAMES as f32;
        let next_width = interpolate_pixels(start_width, target_width, progress);
        if this
          .update(cx, |main_window, cx| {
            main_window.vertical_tabbar_render_width = next_width;
            cx.notify();
          })
          .is_err()
        {
          return;
        }
      }
    });
  }

  fn animate_configuration_fade(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.configuration_transition_opacity = CONFIGURATION_TRANSITION_START_OPACITY;
    cx.notify();

    self.configuration_transition_animation = cx.spawn_in(window, async move |this, cx| {
      for frame in 1..=UI_TRANSITION_FRAMES {
        cx.background_executor()
          .timer(UI_TRANSITION_FRAME_DURATION)
          .await;

        let progress = frame as f32 / UI_TRANSITION_FRAMES as f32;
        let opacity = interpolate_f32(CONFIGURATION_TRANSITION_START_OPACITY, 1.0, progress);
        if this
          .update(cx, |main_window, cx| {
            main_window.configuration_transition_opacity = opacity;
            cx.notify();
          })
          .is_err()
        {
          return;
        }
      }
    });
  }
}
