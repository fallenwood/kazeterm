use config::Config;

/// Creates a SettingsStore from the config, loading the theme from assets
pub fn create_settings_store(config: &Config) -> themeing::SettingsStore {
  use gpui::SharedString;
  use std::sync::Arc;

  // Load theme from assets by name
  let (theme_name, palette) = config::load_theme(&config.theme);

  let settings = themeing::SettingsStore {
    active_theme: Arc::new(themeing::Theme {
      id: config.theme.clone(),
      name: SharedString::from(theme_name),
      styles: themeing::ThemeStyles { colors: palette },
    }),
  };

  settings
}
