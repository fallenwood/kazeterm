use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::button::{Button, ButtonVariants};

#[derive(Clone)]
pub struct ShellErrorCloseEvent;

pub struct ShellErrorDialog {
  focus_handle: FocusHandle,
  error_message: String,
}

impl EventEmitter<ShellErrorCloseEvent> for ShellErrorDialog {}

impl ShellErrorDialog {
  pub fn new(error_message: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let focus_handle = cx.focus_handle();
    window.focus(&focus_handle);
    Self {
      focus_handle,
      error_message,
    }
  }

  fn close(&mut self, cx: &mut Context<Self>) {
    cx.emit(ShellErrorCloseEvent);
  }
}

impl Focusable for ShellErrorDialog {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for ShellErrorDialog {
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
          .w(px(460.0))
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
                  .child("Failed to Start Shell"),
              )
              .child(
                div()
                  .text_sm()
                  .text_color(theme.muted_foreground)
                  .child(self.error_message.clone()),
              )
              .child(
                gpui_component::h_flex().gap_2().justify_end().child(
                  Button::new("ok")
                    .primary()
                    .label("OK")
                    .on_click(cx.listener(|this, _, _window, cx| {
                      this.close(cx);
                    })),
                ),
              ),
          ),
      )
  }
}
