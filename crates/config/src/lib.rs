use gpui::Rgba;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub mod palette;
pub use palette::Palette;

mod ssh;
pub use ssh::get_ssh_hosts;

mod shell;
pub use shell::{DetectedShell, detect_shells, get_default_shell};

mod theme;
pub use theme::{
  EmbeddedThemeLister, EmbeddedThemeLoader, ThemeColors, ThemeFile, ThemeMode,
  get_custom_themes_path, list_available_themes, load_theme, load_theme_from_assets,
  parse_hex_color, parse_theme_content, register_embedded_theme_lister,
  register_embedded_theme_loader, set_custom_themes_path,
};

pub mod migration;
pub use migration::CURRENT_CONFIG_VERSION;

mod keybinding;
pub use keybinding::{KeybindingConfig, ParsedKeybinding};

pub mod alacritty_import;

mod profiles;
pub use profiles::Profile;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
  /// Config file version in YYYYMMDD.Rev format (e.g., "20260220.1")
  pub version: String,
  pub theme: String,
  pub theme_mode: ThemeMode,
  /// Custom themes directory path
  /// Themes in this directory take priority over embedded themes
  pub themes_path: Option<String>,
  pub default_profile: Option<String>,
  #[serde(default)]
  pub profiles: Vec<Profile>,
  pub font_size: f32,
  pub font_family: String,
  pub ui_font_family: String,
  pub ui_font_size: f32,
  pub window_width: f32,
  pub window_height: f32,
  #[serde(skip)]
  pub container_profiles: Vec<Profile>,
  /// Enable the terminal minimap (shows a zoomed-out preview of scrollback)
  pub minimap_enabled: bool,
  /// Render tabs vertically in a left sidebar instead of horizontally at the top
  pub vertical_tabs: bool,
  /// Close the application when the last tab is closed
  /// When false (default), a new tab is created instead
  pub close_on_last_tab: bool,
  /// Show a tab switcher popup when using Ctrl+Tab / Ctrl+Shift+Tab
  /// When false, tabs switch directly without showing a popup
  pub tab_switcher_popup: bool,
  /// Window background opacity (0.0 = fully transparent, 1.0 = fully opaque)
  /// Values between 0.0 and 1.0 allow seeing through the terminal window
  pub background_opacity: f32,
  /// Blur the desktop background behind the window instead of plain transparency.
  /// Only takes effect when background_opacity < 1.0. Not supported on all platforms.
  pub background_blur: bool,
  /// Custom keyboard shortcuts
  pub keybindings: KeybindingConfig,
  /// Minimum idle time (in seconds) since last input before a command completion
  /// triggers a native OS notification. Set to 0 to notify on every prompt return.
  pub long_running_threshold_secs: u64,
  /// Minimum interval (in seconds) between consecutive OS notifications.
  /// Prevents notification spam from rapid command completions.
  /// Set to 0 to allow every notification. Default is 5 seconds.
  pub notification_interval_secs: u64,
  /// Maximum number of lines in the scrollback buffer.
  /// Higher values use more memory. Default is 10000.
  pub scrollback_lines: u32,
  /// Default cursor shape: "block", "underline", or "beam"
  pub cursor_shape: String,
  /// Whether the cursor blinks
  pub cursor_blink: bool,
  /// Cursor blink interval in milliseconds
  pub cursor_blink_interval: u64,
  /// OSC 52 clipboard access mode: "disabled", "copy_only", "paste_only", "copy_paste"
  pub osc52: String,
  /// Automatically copy selected text to the clipboard
  pub copy_on_select: bool,
  /// Show a context menu on right-click instead of the default copy/paste behavior
  pub right_click_context_menu: bool,
  /// Additional environment variables to set for the terminal shell
  #[serde(default)]
  pub env: HashMap<String, String>,
  /// Default working directory for new terminals.
  /// Per-profile working_directory takes priority over this setting.
  pub working_directory: Option<String>,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      version: CURRENT_CONFIG_VERSION.to_string(),
      theme: "one".to_string(),
      theme_mode: ThemeMode::default(),
      themes_path: None,
      default_profile: None,
      profiles: profiles::default_profiles(),
      font_size: 18.0,
      font_family: "Cascadia Code NF".to_string(),
      #[cfg(target_os = "windows")]
      ui_font_family: "Segoe UI".to_string(),
      #[cfg(not(target_os = "windows"))]
      ui_font_family: "Noto Sans".to_string(),
      ui_font_size: 18.0,
      window_width: 800.0,
      window_height: 600.0,
      container_profiles: profiles::detect_container_profiles(),
      minimap_enabled: false,
      vertical_tabs: false,
      close_on_last_tab: true,
      tab_switcher_popup: true,
      background_opacity: 1.0,
      background_blur: false,
      keybindings: KeybindingConfig::default(),
      long_running_threshold_secs: 10,
      notification_interval_secs: 0,
      scrollback_lines: 10_000,
      cursor_shape: "block".to_string(),
      cursor_blink: true,
      cursor_blink_interval: 750,
      osc52: "copy_only".to_string(),
      copy_on_select: false,
      right_click_context_menu: true,
      env: HashMap::new(),
      working_directory: None,
    }
  }
}


impl Config {
  /// Get the background opacity clamped to valid range [0.0, 1.0]
  pub fn get_background_opacity(&self) -> f32 {
    #[cfg(target_os = "linux")]
    {
      self.background_opacity.clamp(0.0, 1.0)
    }

    // Hack
    #[cfg(not(target_os = "linux"))]
    {
      (self.background_opacity / 2.0).clamp(0.0, 1.0)
    }
  }

  /// Get scrollback lines clamped to [0, 100_000]
  pub fn get_scrollback_lines(&self) -> usize {
    (self.scrollback_lines as usize).min(100_000)
  }

  /// Get cursor blink interval as Duration, clamped to [10, 10000] ms
  pub fn get_cursor_blink_interval(&self) -> std::time::Duration {
    std::time::Duration::from_millis(self.cursor_blink_interval.clamp(10, 10_000))
  }

  pub fn load() -> Self {
    let config_path = Self::get_config_file_path_impl();

    if !config_path.exists() {
      // #[cfg(not(debug_assertions))]
      {
        // Create default config file
        if let Err(e) = Self::create_default_config(&config_path) {
          tracing::error!("Failed to create default config: {}", e);
          return Self::default();
        } else {
          tracing::info!("Created default config at: {}", config_path.display());
        }
      }
    }

    match Self::load_from_path(&config_path) {
      Ok(config) => {
        tracing::info!("Loaded config from: {}", config_path.display());
        tracing::debug!("Config: {:?}", config);
        return config;
      }
      Err(e) => {
        tracing::error!(
          "Failed to load config from {}: {}",
          config_path.display(),
          e
        );
      }
    }

    tracing::info!("Using default config");
    Self::default()
  }

  fn get_config_path_impl() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
      if let Some(app_data) = dirs::data_dir() {
        return app_data.join("kazeterm");
      }
    }

    #[cfg(not(target_os = "windows"))]
    {
      if let Some(home_dir) = dirs::home_dir() {
        return home_dir.join(".config").join("kazeterm");
      }
    }

    unreachable!("Could not determine config file path because home/data directory is not found");
  }
  /// Get the config file path
  /// On Windows: ~/AppData/Roaming/kazeterm/kazeterm.toml
  /// On other platforms: ~/.config/kazeterm/kazeterm.toml
  fn get_config_file_path_impl() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
      if let Some(app_data) = dirs::data_dir() {
        return app_data.join("kazeterm").join("kazeterm.toml");
      }
    }

    #[cfg(not(target_os = "windows"))]
    {
      if let Some(home_dir) = dirs::home_dir() {
        return home_dir
          .join(".config")
          .join("kazeterm")
          .join("kazeterm.toml");
      }
    }

    unreachable!("Could not determine config file path because home/data directory is not found");
  }

  /// Create a default config file at the specified path
  #[allow(unused)]
  fn create_default_config(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)?;
    }

    // Generate config from default
    let default_config = Self::default();
    let config_str = toml::to_string_pretty(&default_config)?;

    // Add header comment
    let content = format!(
      "# Kazeterm Configuration\n# Generated automatically\n\n{}",
      config_str
    );

    std::fs::write(path, content)?;
    Ok(())
  }

  fn load_from_path(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let mut raw: toml::Value = toml::from_str(&content)?;

    let migrated = migration::apply_migrations(&mut raw);
    let mut config: Config = raw.try_into()?;
    config.container_profiles = profiles::detect_container_profiles();

    if migrated {
      tracing::info!("Config migrated to version {}", config.version);
      if let Err(e) = Self::save_to_path(path, &config) {
        tracing::error!("Failed to save migrated config: {}", e);
      }
    }

    Ok(config)
  }

  fn save_to_path(path: &PathBuf, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let config_str = toml::to_string_pretty(config)?;
    let content = format!(
      "# Kazeterm Configuration\n# Generated automatically\n\n{}",
      config_str
    );
    std::fs::write(path, content)?;
    Ok(())
  }

  pub fn get_ssh_hosts() -> Vec<String> {
    ssh::get_ssh_hosts()
  }

  pub fn get_config_path() -> PathBuf {
    Self::get_config_path_impl()
  }

  pub fn get_config_file_path() -> Option<PathBuf> {
    let path = Self::get_config_file_path_impl();
    if path.exists() { Some(path) } else { None }
  }
}

impl gpui::Global for Config {}

pub fn to_hex_string(rgba: &Rgba) -> String {
  format!(
    "#{:02X}{:02X}{:02X}{:02X}",
    (rgba.r * 255.0) as u8,
    (rgba.g * 255.0) as u8,
    (rgba.b * 255.0) as u8,
    (rgba.a * 255.0) as u8
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba {
      r: r as f32 / 255.0,
      g: g as f32 / 255.0,
      b: b as f32 / 255.0,
      a: a as f32 / 255.0,
    }
  }

  #[test]
  fn to_hex_string_formats_uppercase_rgba() {
    assert_eq!(to_hex_string(&rgba(255, 0, 0, 255)), "#FF0000FF");
    assert_eq!(to_hex_string(&rgba(0, 255, 0, 128)), "#00FF0080");
    assert_eq!(to_hex_string(&rgba(34, 85, 136, 255)), "#225588FF");
  }

}
