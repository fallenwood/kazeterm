use std::path::{Path, PathBuf};

use config::{Config, ThemeMode};
use gpui::Hsla;

/// Keep startup and hot reload on the same custom theme directory.
pub(crate) fn configure_theme_path(config: &Config) {
  match resolve_themes_path(
    config.appearance.themes_path.as_deref(),
    &Config::get_config_path(),
  ) {
    Some(path) => {
      tracing::debug!("Using themes path: {}", path.display());
      config::set_custom_themes_path(path);
    }
    None => {
      config::clear_custom_themes_path();
      if let Some(path) = &config.appearance.themes_path {
        tracing::warn!("Custom themes path does not exist or is not a directory: {path}");
      }
    }
  }
}

fn resolve_themes_path(custom_path: Option<&str>, config_dir: &Path) -> Option<PathBuf> {
  let path = custom_path
    .map(PathBuf::from)
    .unwrap_or_else(|| config_dir.join("themes"));
  path.is_dir().then_some(path)
}

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

#[cfg(test)]
mod tests {
  use super::resolve_themes_path;

  #[test]
  fn theme_directory_resolution_preserves_defaults_and_explicit_overrides() {
    struct TestDirectory(std::path::PathBuf);
    impl Drop for TestDirectory {
      fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
      }
    }
    let unique = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    let root = std::env::temp_dir().join(format!(
      "kazeterm-theme-path-{}-{unique}",
      std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    let _directory = TestDirectory(root.clone());
    assert_eq!(resolve_themes_path(None, &root), None);

    let default = root.join("themes");
    let custom = root.join("custom themes");
    std::fs::create_dir(&default).unwrap();
    std::fs::create_dir(&custom).unwrap();
    assert_eq!(resolve_themes_path(None, &root), Some(default.clone()));
    assert_eq!(resolve_themes_path(custom.to_str(), &root), Some(custom),);
    // Removing an explicit override must restore the default, not disable custom themes.
    let restored = resolve_themes_path(None, &root).unwrap();
    assert_eq!(restored, default);
    let theme = restored.join("two or three.toml");
    std::fs::write(&theme, "name = 'Custom'\n[dark]\nbackground = '#123456'").unwrap();
    assert!(config::parse_theme_content(&std::fs::read_to_string(&theme).unwrap()).is_some());
    for invalid in [root.join("missing"), theme] {
      assert_eq!(resolve_themes_path(invalid.to_str(), &root), None);
    }
  }
}
