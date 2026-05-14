use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::button::{Button, ButtonVariants};
use themeing::SettingsStore;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateConfirmEvent {
  Confirm,
  Cancel,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UpdateConfirmContent {
  release_tag: String,
}

impl UpdateConfirmContent {
  fn new(release_tag: String) -> Self {
    Self { release_tag }
  }

  fn description(&self) -> String {
    format!(
      "Kazeterm {} is available. Update now? The app will close and install the update.",
      self.release_tag
    )
  }
}

pub struct UpdateConfirmDialog {
  focus_handle: FocusHandle,
  content: UpdateConfirmContent,
}

impl EventEmitter<UpdateConfirmEvent> for UpdateConfirmDialog {}

impl UpdateConfirmDialog {
  pub fn new(release_tag: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let focus_handle = cx.focus_handle();
    window.focus(&focus_handle);
    Self {
      focus_handle,
      content: UpdateConfirmContent::new(release_tag),
    }
  }

  fn confirm(&mut self, cx: &mut Context<Self>) {
    cx.emit(UpdateConfirmEvent::Confirm);
  }

  fn cancel(&mut self, cx: &mut Context<Self>) {
    cx.emit(UpdateConfirmEvent::Cancel);
  }
}

impl Focusable for UpdateConfirmDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for UpdateConfirmDialog {
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
      .on_key_down(cx.listener(|this, e: &KeyDownEvent, _window, cx| {
        if e.keystroke.key == "escape" {
          this.cancel(cx);
        } else if e.keystroke.key == "enter" {
          this.confirm(cx);
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
          .w(px(440.0))
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
                  .child("Update Available"),
              )
              .child(
                div()
                  .text_sm()
                  .text_color(theme.muted_foreground)
                  .child(self.content.description()),
              )
              .child(
                gpui_component::h_flex()
                  .gap_2()
                  .justify_end()
                  .child(
                    Button::new("cancel-update")
                      .ghost()
                      .label("Cancel")
                      .on_click(cx.listener(|this, _, _window, cx| {
                        this.cancel(cx);
                      })),
                  )
                  .child(
                    Button::new("confirm-update")
                      .primary()
                      .label("Update")
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
  use super::{UpdateConfirmContent, UpdateConfirmDialog, UpdateConfirmEvent};
  use gpui::TestAppContext;
  use std::{cell::RefCell, rc::Rc};

  #[test]
  fn description_includes_release_tag() {
    let content = UpdateConfirmContent::new("0.2.0".to_string());

    assert!(content.description().contains("0.2.0"));
    assert!(content.description().contains("Update now?"));
  }

  #[gpui::test]
  fn cancel_emits_cancel_event(cx: &mut TestAppContext) {
    crate::test_support::init_test_app(cx);
    let window =
      cx.add_window(|window, cx| UpdateConfirmDialog::new("0.2.0".to_string(), window, cx));
    cx.run_until_parked();

    let received: Rc<RefCell<Vec<UpdateConfirmEvent>>> = Default::default();
    let received_clone = received.clone();
    cx.update(|cx| {
      let dialog = window.root(cx).unwrap();
      cx.subscribe(&dialog, move |_entity, event: &UpdateConfirmEvent, _cx| {
        received_clone.borrow_mut().push(*event);
      })
      .detach();
    });

    window
      .update(cx, |dialog, _, cx| dialog.cancel(cx))
      .unwrap();
    cx.run_until_parked();

    assert_eq!(received.borrow().as_slice(), &[UpdateConfirmEvent::Cancel]);
  }
}
