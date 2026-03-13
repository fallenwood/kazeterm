use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::button::{Button, ButtonVariants};

/// Event emitted when the session restore error dialog is resolved
#[derive(Clone)]
pub enum SessionRestoreErrorEvent {
  /// User chose to remove the saved session and start fresh
  RemoveAndStartFresh,
  /// User chose to keep the saved session file and exit the app
  KeepAndExit,
}

pub struct SessionRestoreErrorDialog {
  focus_handle: FocusHandle,
  error_message: String,
}

impl EventEmitter<SessionRestoreErrorEvent> for SessionRestoreErrorDialog {}

impl SessionRestoreErrorDialog {
  pub fn new(error_message: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let focus_handle = cx.focus_handle();
    window.focus(&focus_handle);
    Self {
      focus_handle,
      error_message,
    }
  }

  fn remove_and_start_fresh(&mut self, cx: &mut Context<Self>) {
    cx.emit(SessionRestoreErrorEvent::RemoveAndStartFresh);
  }

  fn keep_and_exit(&mut self, cx: &mut Context<Self>) {
    cx.emit(SessionRestoreErrorEvent::KeepAndExit);
  }
}

impl Focusable for SessionRestoreErrorDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for SessionRestoreErrorDialog {
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
          this.keep_and_exit(cx);
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
          .w(px(450.0))
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
                  .child("Session Restore Failed"),
              )
              .child(
                div()
                  .text_sm()
                  .text_color(theme.muted_foreground)
                  .child("Failed to restore your previous session:"),
              )
              .child(
                div()
                  .text_sm()
                  .text_color(theme.danger)
                  .overflow_x_hidden()
                  .child(self.error_message.clone()),
              )
              .child(
                gpui_component::h_flex()
                  .gap_2()
                  .justify_end()
                  .child(
                    Button::new("keep-exit")
                      .ghost()
                      .label("Keep & Exit")
                      .on_click(cx.listener(|this, _, _window, cx| {
                        this.keep_and_exit(cx);
                      })),
                  )
                  .child(
                    Button::new("remove-start-fresh")
                      .primary()
                      .label("Remove Session & Start Fresh")
                      .on_click(cx.listener(|this, _, _window, cx| {
                        this.remove_and_start_fresh(cx);
                      })),
                  ),
              ),
          ),
      )
  }
}
