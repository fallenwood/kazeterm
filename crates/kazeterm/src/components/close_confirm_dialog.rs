use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::ActiveTheme;

/// Event emitted when the close confirmation dialog is resolved
#[derive(Clone)]
pub enum CloseConfirmEvent {
  /// User confirmed they want to close the app
  Confirm,
  /// User cancelled the close action
  Cancel,
}

pub struct CloseConfirmDialog {
  focus_handle: FocusHandle,
}

impl EventEmitter<CloseConfirmEvent> for CloseConfirmDialog {}

impl CloseConfirmDialog {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let focus_handle = cx.focus_handle();
    window.focus(&focus_handle);
    Self { focus_handle }
  }

  fn confirm(&mut self, cx: &mut Context<Self>) {
    cx.emit(CloseConfirmEvent::Confirm);
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

    div()
      .absolute()
      .inset_0()
      .flex()
      .items_center()
      .justify_center()
      .bg(gpui::black().opacity(0.5))
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
          .w(px(350.0))
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
                  .child("Are you sure you want to close the application? All terminal sessions will be terminated."),
              )
              .child(
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
                      .label("Close")
                      .on_click(cx.listener(|this, _, _window, cx| {
                        this.confirm(cx);
                      })),
                  ),
              ),
          ),
      )
  }
}
