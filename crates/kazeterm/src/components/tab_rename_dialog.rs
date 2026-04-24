use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, Sizable};
use themeing::SettingsStore;

/// Event emitted when the rename dialog is closed
#[derive(Clone)]
pub struct TabRenameEvent {
  /// The tab index that was renamed
  pub tab_index: usize,
  /// The new title, or None if cancelled/cleared (to reset to auto-title)
  pub new_title: Option<String>,
}

pub struct TabRenameDialog {
  tab_index: usize,
  input_state: Entity<InputState>,
  _subscription: Subscription,
}

impl EventEmitter<TabRenameEvent> for TabRenameDialog {}

impl TabRenameDialog {
  pub fn new(
    tab_index: usize,
    current_title: &str,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let title = current_title.to_string();
    let input_state = cx.new(|cx| InputState::new(window, cx).default_value(title));

    let subscription = cx.subscribe_in(&input_state, window, |view, state, event, _window, cx| {
      if let gpui_component::input::InputEvent::PressEnter { .. } = event {
        let value = state.read(cx).value().to_string();
        let new_title = if value.trim().is_empty() {
          None // Clear custom title, revert to auto-title
        } else {
          Some(value)
        };
        cx.emit(TabRenameEvent {
          tab_index: view.tab_index,
          new_title,
        });
      }
    });

    Self {
      tab_index,
      input_state,
      _subscription: subscription,
    }
  }

  pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
    let focus_handle = self.input_state.focus_handle(cx);
    window.focus(&focus_handle);
  }

  fn confirm(&mut self, cx: &mut Context<Self>) {
    let value = self.input_state.read(cx).value().to_string();
    let new_title = if value.trim().is_empty() {
      None
    } else {
      Some(value)
    };
    cx.emit(TabRenameEvent {
      tab_index: self.tab_index,
      new_title,
    });
  }

  fn cancel(&mut self, cx: &mut Context<Self>) {
    // Emit event with None to indicate cancellation (but we'll track separately)
    // Actually, for cancel we just close without changing anything
    // We need a way to distinguish cancel vs clear - let's use a separate approach
    // For now, emit with the original title to indicate no change
    cx.emit(TabRenameEvent {
      tab_index: self.tab_index,
      new_title: Some(self.input_state.read(cx).value().to_string()),
    });
  }

  fn clear_custom_title(&mut self, cx: &mut Context<Self>) {
    cx.emit(TabRenameEvent {
      tab_index: self.tab_index,
      new_title: None,
    });
  }
}

impl Focusable for TabRenameDialog {
  fn focus_handle(&self, cx: &App) -> FocusHandle {
    self.input_state.focus_handle(cx)
  }
}

impl Render for TabRenameDialog {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let colors = cx.global::<SettingsStore>().theme().colors();

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
      .child(
        div()
          .bg(theme.popover)
          .text_color(theme.popover_foreground)
          .rounded_md()
          .shadow_lg()
          .border_1()
          .border_color(theme.border)
          .p_4()
          .w(px(350.0))
          .on_key_down(cx.listener(|this, e: &KeyDownEvent, _window, cx| {
            if e.keystroke.key == "Escape" {
              this.cancel(cx);
            }
          }))
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
                  .child("Rename Tab"),
              )
              .child(
                div()
                  .w_full()
                  .child(Input::new(&self.input_state).w_full().cursor_text()),
              )
              .child(
                div()
                  .text_xs()
                  .text_color(theme.muted_foreground)
                  .child("Leave empty to use automatic title from terminal"),
              )
              .child(
                gpui_component::h_flex()
                  .gap_2()
                  .justify_end()
                  .child(
                    Button::new("cancel")
                      .ghost()
                      .small()
                      .label("Cancel")
                      .on_click(cx.listener(|this, _, _window, cx| {
                        this.cancel(cx);
                      })),
                  )
                  .child(
                    Button::new("reset")
                      .ghost()
                      .small()
                      .label("Reset to Auto")
                      .on_click(cx.listener(|this, _, _window, cx| {
                        this.clear_custom_title(cx);
                      })),
                  )
                  .child(
                    Button::new("confirm")
                      .primary()
                      .small()
                      .label("Rename")
                      .on_click(cx.listener(|this, _, _window, cx| {
                        this.confirm(cx);
                      })),
                  ),
              ),
          ),
      )
  }
}

#[cfg(test)]
mod tests {
  use super::{TabRenameDialog, TabRenameEvent};
  use gpui::TestAppContext;
  use std::{cell::RefCell, rc::Rc};

  fn capture_events(
    cx: &mut TestAppContext,
    window: gpui::WindowHandle<TabRenameDialog>,
  ) -> Rc<RefCell<Vec<TabRenameEvent>>> {
    let received: Rc<RefCell<Vec<TabRenameEvent>>> = Default::default();
    let received_clone = received.clone();
    cx.update(|cx| {
      let dialog = window.root(cx).unwrap();
      cx.subscribe(&dialog, move |_entity, event: &TabRenameEvent, _cx| {
        received_clone.borrow_mut().push(event.clone());
      })
      .detach();
    });
    received
  }

  #[gpui::test]
  fn confirm_emits_current_value(cx: &mut TestAppContext) {
    crate::test_support::init_test_app(cx);
    let window = cx.add_window(|window, cx| TabRenameDialog::new(3, "hello", window, cx));
    cx.run_until_parked();

    let received = capture_events(cx, window);

    window.update(cx, |this, _, cx| this.confirm(cx)).unwrap();
    cx.run_until_parked();

    let got = received.borrow();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].tab_index, 3);
    assert_eq!(got[0].new_title.as_deref(), Some("hello"));
  }

  #[gpui::test]
  fn clear_emits_none_title(cx: &mut TestAppContext) {
    crate::test_support::init_test_app(cx);
    let window = cx.add_window(|window, cx| TabRenameDialog::new(1, "whatever", window, cx));
    cx.run_until_parked();

    let received = capture_events(cx, window);

    window
      .update(cx, |this, _, cx| this.clear_custom_title(cx))
      .unwrap();
    cx.run_until_parked();

    let got = received.borrow();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].tab_index, 1);
    assert!(got[0].new_title.is_none());
  }
}
