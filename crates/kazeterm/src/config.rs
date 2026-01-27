use config::{Config, ThemeMode};

/// Creates a SettingsStore from the config, loading the theme from assets
pub fn create_settings_store(config: &Config, system_is_dark: bool) -> themeing::SettingsStore {
  use gpui::SharedString;
  use std::sync::Arc;

  // Determine if we should use dark mode
  let is_dark = match config.theme_mode {
    ThemeMode::Light => false,
    ThemeMode::Dark => true,
    ThemeMode::System => system_is_dark,
  };

  // Load theme from assets by name
  let (theme_name, palette) = config::load_theme(&config.theme, is_dark);

  let settings = themeing::SettingsStore {
    active_theme: Arc::new(themeing::Theme {
      id: config.theme.clone(),
      name: SharedString::from(theme_name),
      styles: themeing::ThemeStyles { colors: palette },
    }),
  };

  settings
}
