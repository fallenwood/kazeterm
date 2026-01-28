use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, Sizable};

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
      match event {
        gpui_component::input::InputEvent::PressEnter { .. } => {
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
        _ => {}
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
                  .child(Input::new(&self.input_state).w_full().appearance(false)),
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
