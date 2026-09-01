use gpui::{Context, Pixels, Task, Window, px};

use super::main_window::MainWindow;
use super::transitions::{TransitionSpec, interpolate_f32, interpolate_pixels};

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
    let animation = cx.global::<::config::Config>().animation;
    self.animate_vertical_tabbar_to(target_width, &animation, window, cx);
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
    self.animate_vertical_tabbar_to(target_width, &config.animation, window, cx);
    self.animate_ui_change_with_config(&config.animation, window, cx);
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
    animation: &::config::AnimationConfig,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let start_width = self.vertical_tabbar_render_width;
    if start_width == target_width {
      self.vertical_tabbar_animation = Task::ready(());
      cx.notify();
      return;
    }

    let Some(transition) = TransitionSpec::from_config(animation) else {
      self.vertical_tabbar_animation = Task::ready(());
      self.vertical_tabbar_render_width = target_width;
      cx.notify();
      return;
    };

    cx.notify();
    self.vertical_tabbar_animation = cx.spawn_in(window, async move |this, cx| {
      for frame in 1..=transition.frames {
        cx.background_executor()
          .timer(transition.frame_duration)
          .await;

        let progress = transition.progress(frame);
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

  pub(crate) fn animate_ui_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let animation = cx.global::<::config::Config>().animation;
    self.animate_ui_change_with_config(&animation, window, cx);
  }

  fn animate_ui_change_with_config(
    &mut self,
    animation: &::config::AnimationConfig,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let Some(transition) = TransitionSpec::from_config(animation) else {
      self.ui_transition_animation = Task::ready(());
      self.ui_transition_opacity = 1.0;
      cx.notify();
      return;
    };

    let start_opacity = transition.fade_start_opacity;
    self.ui_transition_opacity = start_opacity;
    cx.notify();

    self.ui_transition_animation = cx.spawn_in(window, async move |this, cx| {
      for frame in 1..=transition.frames {
        cx.background_executor()
          .timer(transition.frame_duration)
          .await;

        let progress = transition.progress(frame);
        let opacity = interpolate_f32(start_opacity, 1.0, progress);
        if this
          .update(cx, |main_window, cx| {
            main_window.ui_transition_opacity = opacity;
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
