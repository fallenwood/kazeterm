use config::{Config, ThemeMode};
use gpui::Hsla;

/// Apply background opacity to a color.
/// Only modifies the alpha channel if opacity is less than 1.0.
fn apply_opacity(color: Hsla, opacity: f32) -> Hsla {
  if opacity < 1.0 {
    color.opacity(opacity)
  } else {
    color
  }
}

/// Creates a SettingsStore from the config, loading the theme from assets
pub fn create_settings_store(config: &Config, system_is_dark: bool) -> themeing::SettingsStore {
  use gpui::SharedString;
  use std::sync::Arc;

  // Determine if we should use dark mode
  let is_system = matches!(config.theme_mode, ThemeMode::System);
  let is_dark = match config.theme_mode {
    ThemeMode::Light => false,
    ThemeMode::Dark => true,
    ThemeMode::System => system_is_dark,
  };

  // Load theme from assets by name
  let (theme_name, mut palette) = config::load_theme(&config.theme, is_dark);

  // Apply background opacity to relevant background colors
  let opacity = config.get_background_opacity();
  if opacity < 1.0 {
    palette.background = apply_opacity(palette.background, opacity);
    palette.surface_background = apply_opacity(palette.surface_background, opacity);
    palette.elevated_surface_background =
      apply_opacity(palette.elevated_surface_background, opacity);
    palette.title_bar_background = apply_opacity(palette.title_bar_background, opacity);
    palette.title_bar_inactive_background =
      apply_opacity(palette.title_bar_inactive_background, opacity);
    palette.tab_inactive_background = apply_opacity(palette.tab_inactive_background, opacity);
    palette.tab_active_background = apply_opacity(palette.tab_active_background, opacity);
    palette.terminal_background = apply_opacity(palette.terminal_background, opacity);
    palette.terminal_ansi_background = apply_opacity(palette.terminal_ansi_background, opacity);
  }

  themeing::SettingsStore {
    active_theme: Arc::new(themeing::Theme {
      id: config.theme.clone(),
      name: SharedString::from(theme_name),
      styles: themeing::ThemeStyles { colors: palette },
    }),
    is_dark,
    is_system,
  }
}
