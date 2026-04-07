use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, Sizable};

/// Event emitted when the import dialog completes
#[derive(Clone)]
pub enum ImportAlacrittyEvent {
  /// User confirmed import with the given path
  Import(String),
  /// User cancelled the dialog
  Cancel,
}

pub struct ImportAlacrittyDialog {
  input_state: Entity<InputState>,
  error_message: Option<String>,
  _subscription: Subscription,
}

impl EventEmitter<ImportAlacrittyEvent> for ImportAlacrittyDialog {}

impl ImportAlacrittyDialog {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let default_path = config::alacritty_import::default_alacritty_config_path()
      .map(|p| p.display().to_string())
      .unwrap_or_default();

    let input_state = cx.new(|cx| InputState::new(window, cx).default_value(default_path));

    let subscription = cx.subscribe_in(&input_state, window, |view, _state, event, _window, cx| {
      if let gpui_component::input::InputEvent::PressEnter { .. } = event {
        view.confirm(cx);
      }
    });

    Self {
      input_state,
      error_message: None,
      _subscription: subscription,
    }
  }

  pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
    let focus_handle = self.input_state.focus_handle(cx);
    window.focus(&focus_handle);
  }

  fn confirm(&mut self, cx: &mut Context<Self>) {
    let path_str = self.input_state.read(cx).value().to_string();
    let path = std::path::Path::new(&path_str);

    if !path.exists() {
      self.error_message = Some("File does not exist".to_string());
      cx.notify();
      return;
    }

    // Try a quick parse to validate before closing the dialog
    match config::alacritty_import::import_alacritty_config(path) {
      Ok(_) => {
        self.error_message = None;
        cx.emit(ImportAlacrittyEvent::Import(path_str));
      }
      Err(e) => {
        self.error_message = Some(format!("Failed to parse: {e}"));
        cx.notify();
      }
    }
  }

  fn cancel(&mut self, cx: &mut Context<Self>) {
    cx.emit(ImportAlacrittyEvent::Cancel);
  }
}

impl Focusable for ImportAlacrittyDialog {
  fn focus_handle(&self, cx: &App) -> FocusHandle {
    self.input_state.focus_handle(cx)
  }
}

impl Render for ImportAlacrittyDialog {
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
          .w(px(480.0))
          .on_key_down(cx.listener(|this, e: &KeyDownEvent, _window, cx| {
            if e.keystroke.key == "escape" {
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
                  .child("Import Alacritty Config"),
              )
              .child(
                div()
                  .text_xs()
                  .text_color(theme.muted_foreground)
                  .child("Path to your alacritty.toml configuration file"),
              )
              .child(
                div()
                  .w_full()
                  .child(Input::new(&self.input_state).w_full().cursor_text()),
              )
              .when(self.error_message.is_some(), |this: Div| {
                if let Some(ref msg) = self.error_message {
                  this.child(div().text_xs().text_color(gpui::red()).child(msg.clone()))
                } else {
                  this
                }
              })
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
                    Button::new("import")
                      .primary()
                      .small()
                      .label("Import")
                      .on_click(cx.listener(|this, _, _window, cx| {
                        this.confirm(cx);
                      })),
                  ),
              ),
          ),
      )
  }
}
