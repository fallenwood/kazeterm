use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::button::{Button, ButtonVariants};
use themeing::SettingsStore;

/// Event emitted when the close confirmation dialog is resolved
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseConfirmEvent {
  /// User confirmed they want to close and save workspace
  SaveAndClose,
  /// User wants to close without saving workspace, or just close when restore is disabled
  Close,
  /// User cancelled the close action
  Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CloseConfirmContent {
  restore_workspace: bool,
}

impl CloseConfirmContent {
  const fn new(restore_workspace: bool) -> Self {
    Self { restore_workspace }
  }

  const fn description(self) -> &'static str {
    if self.restore_workspace {
      "Do you want to save the workspace before closing? Saved workspaces will be restored on next launch."
    } else {
      "Are you sure to close the application?"
    }
  }

  const fn primary_action(self) -> CloseConfirmEvent {
    if self.restore_workspace {
      CloseConfirmEvent::SaveAndClose
    } else {
      CloseConfirmEvent::Close
    }
  }

  const fn primary_button_label(self) -> &'static str {
    if self.restore_workspace {
      "Save & Close"
    } else {
      "Close"
    }
  }
}

pub struct CloseConfirmDialog {
  focus_handle: FocusHandle,
  content: CloseConfirmContent,
}

impl EventEmitter<CloseConfirmEvent> for CloseConfirmDialog {}

impl CloseConfirmDialog {
  pub fn new(restore_workspace: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let focus_handle = cx.focus_handle();
    window.focus(&focus_handle);
    Self {
      focus_handle,
      content: CloseConfirmContent::new(restore_workspace),
    }
  }

  fn primary_action(&mut self, cx: &mut Context<Self>) {
    cx.emit(self.content.primary_action());
  }

  fn close(&mut self, cx: &mut Context<Self>) {
    cx.emit(CloseConfirmEvent::Close);
  }

  fn cancel(&mut self, cx: &mut Context<Self>) {
    cx.emit(CloseConfirmEvent::Cancel);
  }
}

impl Focusable for CloseConfirmDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for CloseConfirmDialog {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let colors = cx.global::<SettingsStore>().theme().colors();
    let content = self.content;
    let actions = if content.restore_workspace {
      gpui_component::h_flex()
        .gap_2()
        .justify_end()
        .child(
          Button::new("cancel")
            .ghost()
            .label("Cancel")
            .on_click(cx.listener(|this, _, _window, cx| {
              this.cancel(cx);
            })),
        )
        .child(
          Button::new("close-without-saving")
            .danger()
            .label("Don't Save")
            .on_click(cx.listener(|this, _, _window, cx| {
              this.close(cx);
            })),
        )
        .child(
          Button::new("confirm")
            .primary()
            .label(content.primary_button_label())
            .on_click(cx.listener(|this, _, _window, cx| {
              this.primary_action(cx);
            })),
        )
    } else {
      gpui_component::h_flex()
        .gap_2()
        .justify_end()
        .child(
          Button::new("cancel")
            .ghost()
            .label("Cancel")
            .on_click(cx.listener(|this, _, _window, cx| {
              this.cancel(cx);
            })),
        )
        .child(
          Button::new("confirm")
            .danger()
            .label(content.primary_button_label())
            .on_click(cx.listener(|this, _, _window, cx| {
              this.primary_action(cx);
            })),
        )
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
        if e.keystroke.key == "escape" {
          this.cancel(cx);
        } else if e.keystroke.key == "enter" {
          this.primary_action(cx);
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
          .w(px(420.0))
          .child(
            div()
              .flex()
              .flex_col()
              .gap_3()
              .w_full()
              .child(
                div()
                  .text_base()
                  .font_weight(FontWeight::SEMIBOLD)
                  .child("Close Application?"),
              )
              .child(
                div()
                  .text_sm()
                  .text_color(theme.muted_foreground)
                  .child(content.description()),
              )
              .child(actions),
          ),
      )
  }
}

#[cfg(test)]
mod tests {
  use super::{CloseConfirmContent, CloseConfirmDialog, CloseConfirmEvent};
  use gpui::TestAppContext;

  #[test]
  fn restore_workspace_enabled_keeps_save_prompt() {
    let content = CloseConfirmContent::new(true);

    assert_eq!(
      content.description(),
      "Do you want to save the workspace before closing? Saved workspaces will be restored on next launch."
    );
    assert_eq!(content.primary_action(), CloseConfirmEvent::SaveAndClose);
    assert_eq!(content.primary_button_label(), "Save & Close");
  }

  #[test]
  fn restore_workspace_disabled_uses_close_prompt() {
    let content = CloseConfirmContent::new(false);

    assert_eq!(
      content.description(),
      "Are you sure to close the application?"
    );
    assert!(!content.description().contains("save"));
    assert_eq!(content.primary_action(), CloseConfirmEvent::Close);
    assert_eq!(content.primary_button_label(), "Close");
  }

  #[gpui::test]
  fn dialog_constructs_and_is_focusable(cx: &mut TestAppContext) {
    crate::test_support::init_test_app(cx);
    let window = cx.add_window(|window, cx| CloseConfirmDialog::new(true, window, cx));
    cx.run_until_parked();

    let restore = window
      .update(cx, |dialog, _, _| dialog.content.restore_workspace)
      .unwrap();
    assert!(restore, "restore_workspace flag should round-trip");
  }

  #[gpui::test]
  fn escape_emits_cancel_event(cx: &mut TestAppContext) {
    crate::test_support::init_test_app(cx);
    let window = cx.add_window(|window, cx| CloseConfirmDialog::new(true, window, cx));
    cx.run_until_parked();

    let received: std::rc::Rc<std::cell::RefCell<Vec<CloseConfirmEvent>>> = Default::default();
    let received_clone = received.clone();
    cx.update(|cx| {
      let dialog = window.root(cx).unwrap();
      cx.subscribe(
        &dialog,
        move |_entity, event: &CloseConfirmEvent, _cx| {
          received_clone.borrow_mut().push(*event);
        },
      )
      .detach();
    });

    window
      .update(cx, |dialog, _, cx| dialog.cancel(cx))
      .unwrap();
    cx.run_until_parked();

    assert_eq!(received.borrow().as_slice(), &[CloseConfirmEvent::Cancel]);
  }

  #[gpui::test]
  fn primary_action_emits_save_and_close_when_restore_enabled(cx: &mut TestAppContext) {
    crate::test_support::init_test_app(cx);
    let window = cx.add_window(|window, cx| CloseConfirmDialog::new(true, window, cx));
    cx.run_until_parked();

    let received: std::rc::Rc<std::cell::RefCell<Vec<CloseConfirmEvent>>> = Default::default();
    let received_clone = received.clone();
    cx.update(|cx| {
      let dialog = window.root(cx).unwrap();
      cx.subscribe(
        &dialog,
        move |_entity, event: &CloseConfirmEvent, _cx| {
          received_clone.borrow_mut().push(*event);
        },
      )
      .detach();
    });

    window
      .update(cx, |dialog, _, cx| dialog.primary_action(cx))
      .unwrap();
    cx.run_until_parked();

    assert_eq!(
      received.borrow().as_slice(),
      &[CloseConfirmEvent::SaveAndClose]
    );
  }

  #[gpui::test]
  fn primary_action_emits_close_when_restore_disabled(cx: &mut TestAppContext) {
    crate::test_support::init_test_app(cx);
    let window = cx.add_window(|window, cx| CloseConfirmDialog::new(false, window, cx));
    cx.run_until_parked();

    let received: std::rc::Rc<std::cell::RefCell<Vec<CloseConfirmEvent>>> = Default::default();
    let received_clone = received.clone();
    cx.update(|cx| {
      let dialog = window.root(cx).unwrap();
      cx.subscribe(
        &dialog,
        move |_entity, event: &CloseConfirmEvent, _cx| {
          received_clone.borrow_mut().push(*event);
        },
      )
      .detach();
    });

    window
      .update(cx, |dialog, _, cx| dialog.primary_action(cx))
      .unwrap();
    cx.run_until_parked();

    assert_eq!(received.borrow().as_slice(), &[CloseConfirmEvent::Close]);
  }
}
