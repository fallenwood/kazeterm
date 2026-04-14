use config::{Config, ThemeMode};
use gpui::Hsla;

pub(crate) fn apply_background_opacity(color: Hsla, opacity: f32) -> Hsla {
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
  let is_system = matches!(config.colors.theme_mode, ThemeMode::System);
  let is_dark = match config.colors.theme_mode {
    ThemeMode::Light => false,
    ThemeMode::Dark => true,
    ThemeMode::System => system_is_dark,
  };

  // Load theme from assets by name
  let (theme_name, mut palette) = config::load_theme(&config.colors.theme, is_dark);

  let opacity = config.appearance.get_background_opacity();
  if opacity < 1.0 {
    palette.border = apply_background_opacity(palette.border, opacity);
    palette.border_variant = apply_background_opacity(palette.border_variant, opacity);
    palette.border_disabled = apply_background_opacity(palette.border_disabled, opacity);
    palette.surface_background = apply_background_opacity(palette.surface_background, opacity);
    palette.background = apply_background_opacity(palette.background, opacity);
    palette.element_background = apply_background_opacity(palette.element_background, opacity);
    palette.element_hover = apply_background_opacity(palette.element_hover, opacity);
    palette.element_active = apply_background_opacity(palette.element_active, opacity);
    palette.element_selected = apply_background_opacity(palette.element_selected, opacity);
    palette.element_selection_background =
      apply_background_opacity(palette.element_selection_background, opacity);
    palette.element_disabled = apply_background_opacity(palette.element_disabled, opacity);
    palette.title_bar_background = apply_background_opacity(palette.title_bar_background, opacity);
    palette.title_bar_inactive_background =
      apply_background_opacity(palette.title_bar_inactive_background, opacity);
    palette.tab_inactive_background =
      apply_background_opacity(palette.tab_inactive_background, opacity);
    palette.tab_active_background =
      apply_background_opacity(palette.tab_active_background, opacity);
    palette.search_match_background =
      apply_background_opacity(palette.search_match_background, opacity);
    palette.search_highlight_background =
      apply_background_opacity(palette.search_highlight_background, opacity);
    palette.terminal_background = apply_background_opacity(palette.terminal_background, opacity);
    palette.terminal_ansi_background =
      apply_background_opacity(palette.terminal_ansi_background, opacity);
    palette.scrollbar_track_background =
      apply_background_opacity(palette.scrollbar_track_background, opacity);
    palette.scrollbar_thumb_background =
      apply_background_opacity(palette.scrollbar_thumb_background, opacity);
  }

  themeing::SettingsStore {
    active_theme: Arc::new(themeing::Theme {
      id: config.colors.theme.clone(),
      name: SharedString::from(theme_name),
      styles: themeing::ThemeStyles { colors: palette },
    }),
    is_dark,
    is_system,
  }
}
