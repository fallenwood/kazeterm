use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::button::{Button, ButtonVariants};
use themeing::SettingsStore;

/// Event emitted when the about dialog is closed
#[derive(Clone)]
pub struct AboutDialogCloseEvent;

pub struct AboutDialog {
  focus_handle: FocusHandle,
}

impl EventEmitter<AboutDialogCloseEvent> for AboutDialog {}

impl AboutDialog {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let focus_handle = cx.focus_handle();
    window.focus(&focus_handle);
    Self { focus_handle }
  }

  fn close(&mut self, cx: &mut Context<Self>) {
    cx.emit(AboutDialogCloseEvent);
  }

  fn get_commit_hash() -> &'static str {
    option_env!("KAZETERM_COMMIT_SHA").unwrap_or("unknown")
  }

  fn get_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
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

    let commit_hash = Self::get_commit_hash();
    let short_hash = if commit_hash.len() > 7 {
      &commit_hash[..7]
    } else {
      commit_hash
    };
    let version = Self::get_version();
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
                  .child(self.info_row("Theme", &theme_display, &theme))
                  .child(self.info_row("Version", version, &theme))
                  .child(self.info_row("Commit", short_hash, &theme))
                  .child(self.info_row("License", license, &theme))
                  .child(self.info_row("Author", author, &theme))
                  .child(self.info_row_with_wrap("Repository", repo, &theme))
                  .child(self.info_row_with_wrap("Config Location", &config_path_str, &theme)),
              )
              // Close button
              .child(
                gpui_component::h_flex()
                  .mt_2()
                  .justify_end()
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
  fn info_row(
    &self,
    label: &str,
    value: &str,
    theme: &gpui_component::Theme,
  ) -> impl IntoElement {
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
