use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::Disableable;
use gpui_component::button::{Button, ButtonVariants};
use themeing::SettingsStore;

/// Events emitted by the about dialog
#[derive(Clone)]
pub enum AboutDialogEvent {
  CheckForUpdates,
}

/// Event emitted when the about dialog is closed
#[derive(Clone)]
pub struct AboutDialogCloseEvent;

pub struct AboutDialog {
  focus_handle: FocusHandle,
  checking_for_updates: bool,
  update_message: Option<String>,
  update_message_is_error: bool,
}

impl EventEmitter<AboutDialogEvent> for AboutDialog {}
impl EventEmitter<AboutDialogCloseEvent> for AboutDialog {}

impl AboutDialog {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let focus_handle = cx.focus_handle();
    window.focus(&focus_handle);
    Self {
      focus_handle,
      checking_for_updates: false,
      update_message: None,
      update_message_is_error: false,
    }
  }

  fn close(&mut self, cx: &mut Context<Self>) {
    cx.emit(AboutDialogCloseEvent);
  }

  fn check_for_updates(&mut self, cx: &mut Context<Self>) {
    if self.checking_for_updates {
      return;
    }

    self.checking_for_updates = true;
    self.update_message = Some("Checking for updates...".to_string());
    self.update_message_is_error = false;
    cx.notify();
    cx.emit(AboutDialogEvent::CheckForUpdates);
  }

  pub(crate) fn finish_update_check(
    &mut self,
    message: String,
    is_error: bool,
    cx: &mut Context<Self>,
  ) {
    self.checking_for_updates = false;
    self.update_message = Some(message);
    self.update_message_is_error = is_error;
    cx.notify();
  }

  fn get_license() -> &'static str {
    "GPL-3.0"
  }

  fn get_author() -> &'static str {
    "fallenwood"
  }

  fn get_repo() -> &'static str {
    "https://github.com/fallenwood/kazeterm"
  }
}

impl Focusable for AboutDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for AboutDialog {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let settings = cx.global::<SettingsStore>();
    let active_theme = settings.theme();
    let colors = active_theme.colors();

    let short_hash = crate::build_info::short_commit_hash();
    let version = crate::build_info::app_version();
    let build_source = crate::build_info::build_source();
    let license = Self::get_license();
    let author = Self::get_author();
    let repo = Self::get_repo();
    let config_path = ::config::Config::get_config_path();
    let config_path_str = config_path.display().to_string();

    // Get theme display with mode
    let mode_str = if settings.is_dark { "Dark" } else { "Light" };
    let theme_display = if settings.is_system {
      format!("{} {} (System)", active_theme.name.clone(), mode_str)
    } else {
      format!("{} {}", active_theme.name.clone(), mode_str)
    };

    div()
      .absolute()
      .inset_0()
      .flex()
      .items_center()
      .justify_center()
      .bg(colors.overlay_background)
      .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
        cx.stop_propagation();
      })
      .on_key_down(cx.listener(|this, e: &KeyDownEvent, _window, cx| {
        if e.keystroke.key == "escape" || e.keystroke.key == "enter" {
          this.close(cx);
        }
      }))
      .child(
        div()
          .track_focus(&self.focus_handle)
          .bg(theme.popover)
          .text_color(theme.popover_foreground)
          .rounded_md()
          .shadow_lg()
          .border_1()
          .border_color(theme.border)
          .p_4()
          .w(px(400.0))
          .child(
            div()
              .flex()
              .flex_col()
              .gap_3()
              .w_full()
              // Title
              .child(
                div()
                  .text_lg()
                  .font_weight(FontWeight::BOLD)
                  .child("About Kazeterm"),
              )
              // Version and commit
              .child(
                div()
                  .flex()
                  .flex_col()
                  .gap_2()
                  .text_sm()
                  .child(self.info_row("Theme", &theme_display, theme))
                  .child(self.info_row("Version", version, theme))
                  .child(self.info_row("Build", build_source, theme))
                  .child(self.info_row("Commit", short_hash, theme))
                  .child(self.info_row("License", license, theme))
                  .child(self.info_row("Author", author, theme))
                  .child(self.info_row_with_wrap("Repository", repo, theme))
                  .child(self.info_row_with_wrap("Config Location", &config_path_str, theme)),
              )
              .when(self.update_message.is_some(), |this: Div| {
                if let Some(message) = self.update_message.as_ref() {
                  let color = if self.update_message_is_error {
                    theme.red
                  } else {
                    theme.muted_foreground
                  };

                  this.child(div().text_xs().text_color(color).child(message.clone()))
                } else {
                  this
                }
              })
              // Buttons
              .child(
                gpui_component::h_flex()
                  .mt_2()
                  .justify_end()
                  .gap_2()
                  .child(
                    Button::new("check-for-updates")
                      .disabled(self.checking_for_updates)
                      .label(if self.checking_for_updates {
                        "Checking..."
                      } else {
                        "Check for Updates"
                      })
                      .on_click(cx.listener(|this, _, _window, cx| {
                        this.check_for_updates(cx);
                      })),
                  )
                  .child(
                    Button::new("close")
                      .primary()
                      .label("Close")
                      .on_click(cx.listener(|this, _, _window, cx| {
                        this.close(cx);
                      })),
                  ),
              ),
          ),
      )
  }
}

impl AboutDialog {
  fn info_row(&self, label: &str, value: &str, theme: &gpui_component::Theme) -> impl IntoElement {
    div()
      .flex()
      .gap_2()
      .child(
        div()
          .w(px(120.0))
          .flex_shrink_0()
          .text_color(theme.muted_foreground)
          .child(format!("{}:", label)),
      )
      .child(
        div()
          .flex_1()
          .font_weight(FontWeight::MEDIUM)
          .child(value.to_string()),
      )
  }

  fn info_row_with_wrap(
    &self,
    label: &str,
    value: &str,
    theme: &gpui_component::Theme,
  ) -> impl IntoElement {
    div()
      .flex()
      .flex_col()
      .gap_1()
      .child(
        div()
          .text_color(theme.muted_foreground)
          .child(format!("{}:", label)),
      )
      .child(
        div()
          .text_xs()
          .font_weight(FontWeight::MEDIUM)
          .overflow_hidden()
          .text_ellipsis()
          .child(value.to_string()),
      )
  }
}

#[cfg(test)]
mod tests {
  use super::{AboutDialog, AboutDialogCloseEvent, AboutDialogEvent};
  use gpui::TestAppContext;
  use std::{cell::RefCell, rc::Rc};

  #[test]
  fn metadata_accessors_are_nonempty() {
    assert_eq!(AboutDialog::get_license(), "GPL-3.0");
    assert!(!AboutDialog::get_author().is_empty());
    assert!(AboutDialog::get_repo().starts_with("https://"));
  }

  #[gpui::test]
  fn close_emits_event(cx: &mut TestAppContext) {
    crate::test_support::init_test_app(cx);
    let window = cx.add_window(|window, cx| AboutDialog::new(window, cx));
    cx.run_until_parked();

    let count: Rc<RefCell<u32>> = Default::default();
    let count_clone = count.clone();
    cx.update(|cx| {
      let dialog = window.root(cx).unwrap();
      cx.subscribe(&dialog, move |_, _event: &AboutDialogCloseEvent, _cx| {
        *count_clone.borrow_mut() += 1;
      })
      .detach();
    });

    window.update(cx, |this, _, cx| this.close(cx)).unwrap();
    cx.run_until_parked();
    assert_eq!(*count.borrow(), 1);
  }

  #[gpui::test]
  fn check_for_updates_keeps_dialog_open_and_sets_status(cx: &mut TestAppContext) {
    crate::test_support::init_test_app(cx);
    let window = cx.add_window(|window, cx| AboutDialog::new(window, cx));
    cx.run_until_parked();

    let close_count: Rc<RefCell<u32>> = Default::default();
    let close_count_clone = close_count.clone();
    let action_count: Rc<RefCell<u32>> = Default::default();
    let action_count_clone = action_count.clone();

    cx.update(|cx| {
      let dialog = window.root(cx).unwrap();
      cx.subscribe(&dialog, move |_, _event: &AboutDialogCloseEvent, _cx| {
        *close_count_clone.borrow_mut() += 1;
      })
      .detach();
      cx.subscribe(&dialog, move |_, _event: &AboutDialogEvent, _cx| {
        *action_count_clone.borrow_mut() += 1;
      })
      .detach();
    });

    window
      .update(cx, |this, _, cx| this.check_for_updates(cx))
      .unwrap();
    cx.run_until_parked();

    let dialog = window.root(cx).unwrap();
    dialog.read_with(cx, |dialog, _| {
      assert!(dialog.checking_for_updates);
      assert_eq!(
        dialog.update_message.as_deref(),
        Some("Checking for updates...")
      );
      assert!(!dialog.update_message_is_error);
    });

    assert_eq!(*close_count.borrow(), 0);
    assert_eq!(*action_count.borrow(), 1);
  }
}
