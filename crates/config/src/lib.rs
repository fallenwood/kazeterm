use gpui::Rgba;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const GENERATED_CONFIG_HEADER: &str =
  "# Kazeterm Configuration\n# Generated automatically\n\n";

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
pub use keybinding::{KeybindingConfig, KeybindingList, ParsedKeybinding};

pub mod alacritty_import;

mod profiles;
pub use profiles::Profile;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ColorsConfig {
  pub theme: String,
  pub theme_mode: ThemeMode,
  /// Use bright ANSI colors for bold text instead of only increasing font weight.
  pub bold_as_bright: bool,
  /// Minimum APCA contrast between foreground and background colors.
  /// Set to 0 to disable contrast enforcement. Default is 45.
  pub minimum_contrast: f32,
}

impl Default for ColorsConfig {
  fn default() -> Self {
    Self {
      theme: "one".to_string(),
      theme_mode: ThemeMode::default(),
      bold_as_bright: false,
      minimum_contrast: 45.0,
    }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AppearanceConfig {
  /// Custom themes directory path.
  /// Themes in this directory take priority over embedded themes.
  pub themes_path: Option<String>,
  /// Window background opacity (0.0 = fully transparent, 1.0 = fully opaque).
  /// Values between 0.0 and 1.0 allow seeing through the terminal window.
  pub background_opacity: f32,
  /// Blur the desktop background behind the window instead of plain transparency.
  /// Only takes effect when background_opacity < 1.0. Not supported on all platforms.
  pub background_blur: bool,
}

impl Default for AppearanceConfig {
  fn default() -> Self {
    Self {
      themes_path: None,
      background_opacity: 1.0,
      background_blur: false,
    }
  }
}

impl AppearanceConfig {
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
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct FontConfig {
  pub size: f32,
  pub family: String,
  pub ui_family: String,
  pub ui_size: f32,
}

impl Default for FontConfig {
  fn default() -> Self {
    Self {
      size: 18.0,
      family: "Cascadia Code NF".to_string(),
      #[cfg(target_os = "windows")]
      ui_family: "Segoe UI".to_string(),
      #[cfg(not(target_os = "windows"))]
      ui_family: "Noto Sans".to_string(),
      ui_size: 18.0,
    }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct WindowConfig {
  pub width: f32,
  pub height: f32,
  /// Open the main window maximized on application startup.
  pub start_maximized: bool,
  /// Automatically restore the previous workspace (tabs, splits, working directories)
  /// on application launch. Default: true.
  pub restore_workspace: bool,
}

impl Default for WindowConfig {
  fn default() -> Self {
    Self {
      width: 800.0,
      height: 600.0,
      start_maximized: false,
      restore_workspace: true,
    }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct TabConfig {
  /// Render tabs vertically in a left sidebar instead of horizontally at the top.
  pub vertical: bool,
  /// Close the application when the last tab is closed.
  /// When false, a new tab is created instead.
  pub close_on_last: bool,
  /// Show a tab switcher popup when using Ctrl+Tab / Ctrl+Shift+Tab.
  /// When false, tabs switch directly without showing a popup.
  pub switcher_popup: bool,
  /// Delay before applying terminal-driven tab title changes, in milliseconds.
  /// Helps avoid rapid title churn from shells or apps that update the title frequently.
  pub title_change_delay_ms: u64,
}

impl Default for TabConfig {
  fn default() -> Self {
    Self {
      vertical: false,
      close_on_last: true,
      switcher_popup: true,
      title_change_delay_ms: 200,
    }
  }
}

impl TabConfig {
  /// Get the tab title change delay as Duration, clamped to [0, 5000] ms.
  pub fn get_title_change_delay(&self) -> std::time::Duration {
    std::time::Duration::from_millis(self.title_change_delay_ms.clamp(0, 5_000))
  }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PaneConfig {
  /// Width of split pane divider drag handles in pixels.
  pub divider_width: f32,
  /// Opacity applied to inactive (unfocused) split panes to visually distinguish them.
  /// 0.0 = fully transparent, 1.0 = no dimming. Default is 0.6.
  pub inactive_opacity: f32,
}

impl Default for PaneConfig {
  fn default() -> Self {
    Self {
      divider_width: 6.0,
      inactive_opacity: 0.6,
    }
  }
}

impl PaneConfig {
  /// Get split pane divider width clamped to a reasonable range in pixels.
  pub fn get_divider_width(&self) -> f32 {
    self.divider_width.clamp(1.0, 32.0)
  }

  /// Get inactive pane opacity clamped to [0.0, 1.0].
  pub fn get_inactive_opacity(&self) -> f32 {
    self.inactive_opacity.clamp(0.0, 1.0)
  }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct TerminalConfig {
  /// Maximum number of lines in the scrollback buffer.
  /// Higher values use more memory. Default is 10000.
  pub scrollback_lines: u32,
  /// OSC 52 clipboard access mode: "disabled", "copy_only", "paste_only", "copy_paste"
  pub osc52: String,
  /// Automatically copy selected text to the clipboard.
  pub copy_on_select: bool,
  /// Show a context menu on right-click instead of the default copy/paste behavior.
  pub right_click_context_menu: bool,
  /// Enable Ctrl+Scroll to zoom (change font size). Default is true.
  pub ctrl_scroll_zoom: bool,
  /// Enable the terminal minimap (shows a zoomed-out preview of scrollback).
  pub minimap_enabled: bool,
  /// Default working directory for new terminals.
  /// Per-profile working_directory takes priority over this setting.
  pub working_directory: Option<String>,
  /// Default profile name for new terminals.
  pub default_profile: Option<String>,
  /// Additional environment variables to set for the terminal shell.
  #[serde(default)]
  pub env: HashMap<String, String>,
}

impl Default for TerminalConfig {
  fn default() -> Self {
    Self {
      scrollback_lines: 10_000,
      osc52: "copy_only".to_string(),
      copy_on_select: false,
      right_click_context_menu: true,
      ctrl_scroll_zoom: true,
      minimap_enabled: false,
      working_directory: None,
      default_profile: None,
      env: HashMap::new(),
    }
  }
}

impl TerminalConfig {
  /// Get scrollback lines clamped to [0, 100_000]
  pub fn get_scrollback_lines(&self) -> usize {
    (self.scrollback_lines as usize).min(100_000)
  }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct CursorConfig {
  /// Default cursor shape: "block", "underline", or "beam"
  pub shape: String,
  /// Whether the cursor blinks.
  pub blink: bool,
  /// Cursor blink interval in milliseconds.
  pub blink_interval: u64,
}

impl Default for CursorConfig {
  fn default() -> Self {
    Self {
      shape: "block".to_string(),
      blink: true,
      blink_interval: 750,
    }
  }
}

impl CursorConfig {
  /// Get cursor blink interval as Duration, clamped to [10, 10000] ms
  pub fn get_blink_interval(&self) -> std::time::Duration {
    std::time::Duration::from_millis(self.blink_interval.clamp(10, 10_000))
  }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct NotificationConfig {
  /// Minimum idle time (in seconds) since last input before a command completion
  /// triggers a native OS notification. Set to 0 to notify on every prompt return.
  pub long_running_threshold_secs: u64,
  /// Minimum interval (in seconds) between consecutive OS notifications.
  /// Prevents notification spam from rapid command completions.
  /// Set to 0 to allow every notification.
  pub interval_secs: u64,
}

impl Default for NotificationConfig {
  fn default() -> Self {
    Self {
      long_running_threshold_secs: 10,
      interval_secs: 0,
    }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
  /// Config file version in YYYYMMDD.Rev format (e.g., "20260220.1")
  pub version: String,
  /// Additional config files to merge after the main `kazeterm.toml`.
  /// Imported files override the base config, and later imports override earlier ones.
  #[serde(default)]
  pub imports: Vec<String>,
  pub colors: ColorsConfig,
  pub appearance: AppearanceConfig,
  pub font: FontConfig,
  pub window: WindowConfig,
  pub tab: TabConfig,
  pub pane: PaneConfig,
  pub terminal: TerminalConfig,
  pub cursor: CursorConfig,
  pub notification: NotificationConfig,
  #[serde(default)]
  pub profiles: Vec<Profile>,
  /// Custom keyboard shortcuts
  pub keybindings: KeybindingConfig,
  #[serde(skip)]
  pub container_profiles: Vec<Profile>,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      version: CURRENT_CONFIG_VERSION.to_string(),
      imports: Vec::new(),
      colors: ColorsConfig::default(),
      appearance: AppearanceConfig::default(),
      font: FontConfig::default(),
      window: WindowConfig::default(),
      tab: TabConfig::default(),
      pane: PaneConfig::default(),
      terminal: TerminalConfig::default(),
      cursor: CursorConfig::default(),
      notification: NotificationConfig::default(),
      profiles: profiles::default_profiles(),
      keybindings: KeybindingConfig::default(),
      container_profiles: profiles::detect_container_profiles(),
    }
  }
}

impl Config {

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
    let content = format!("{}{}", GENERATED_CONFIG_HEADER, config_str);

    std::fs::write(path, content)?;
    Ok(())
  }

  fn load_from_path(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
    let (original_content, mut raw) = Self::read_raw_config_with_content(path)?;

    let migrated = migration::apply_migrations(&mut raw);
    let mut merged = raw.clone();
    let mut visited = HashSet::from([Self::normalize_path(path)]);
    Self::apply_imports(&mut merged, path, &Self::extract_imports(&raw), &mut visited);

    let mut config: Config = merged.try_into()?;
    config.container_profiles = profiles::detect_container_profiles();

    if migrated {
      tracing::info!("Config migrated to version {}", config.version);
      match Self::create_migration_backup(path, &original_content) {
        Ok(backup_path) => {
          tracing::info!("Created migrated config backup at: {}", backup_path.display());
        }
        Err(error) => {
          tracing::error!(
            "Failed to create migrated config backup for {}: {}",
            path.display(),
            error
          );
          return Ok(config);
        }
      }

      if let Err(e) = Self::save_raw_to_path(path, &raw) {
        tracing::error!("Failed to save migrated config: {}", e);
      }
    }

    Ok(config)
  }

  fn read_raw_config_with_content(
    path: &Path,
  ) -> Result<(String, toml::Value), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let raw = toml::from_str(&content)?;
    Ok((content, raw))
  }

  fn read_raw_config(path: &Path) -> Result<toml::Value, Box<dyn std::error::Error>> {
    let (_, raw) = Self::read_raw_config_with_content(path)?;
    Ok(raw)
  }

  fn save_raw_to_path(
    path: &Path,
    config: &toml::Value,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let config_str = toml::to_string_pretty(config)?;
    let content = format!("{}{}", GENERATED_CONFIG_HEADER, config_str);
    std::fs::write(path, content)?;
    Ok(())
  }

  fn create_migration_backup(
    path: &Path,
    original_content: &str,
  ) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let parent = path.parent().ok_or_else(|| {
      std::io::Error::other("Could not create config backup because the config has no parent directory")
    })?;
    let stem = path
      .file_stem()
      .and_then(|stem| stem.to_str())
      .unwrap_or("kazeterm");
    let extension = path
      .extension()
      .and_then(|extension| extension.to_str())
      .unwrap_or("toml");
    let timestamp = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)?
      .as_millis();

    for collision_index in 0_u32.. {
      let file_name = if collision_index == 0 {
        format!("{stem}.backup.{timestamp}.{extension}")
      } else {
        format!("{stem}.backup.{timestamp}.{collision_index}.{extension}")
      };
      let backup_path = parent.join(file_name);

      if backup_path.exists() {
        continue;
      }

      std::fs::write(&backup_path, original_content)?;
      return Ok(backup_path);
    }

    unreachable!("unbounded collision search should always find a free backup filename")
  }

  fn extract_imports(raw: &toml::Value) -> Vec<String> {
    raw
      .get("imports")
      .and_then(toml::Value::as_array)
      .into_iter()
      .flatten()
      .filter_map(|value| value.as_str().map(ToOwned::to_owned))
      .collect()
  }

  fn apply_imports(
    merged: &mut toml::Value,
    current_path: &Path,
    imports: &[String],
    visited: &mut HashSet<PathBuf>,
  ) {
    for import in imports {
      let resolved_path = Self::resolve_import_path(current_path, import);
      let normalized_path = Self::normalize_path(&resolved_path);

      if !visited.insert(normalized_path) {
        tracing::warn!(
          "Skipping duplicate or recursive config import: {}",
          resolved_path.display()
        );
        continue;
      }

      let imported_raw = match Self::read_raw_config(&resolved_path) {
        Ok(raw) => raw,
        Err(error) => {
          tracing::warn!(
            "Skipping config import {}: {}",
            resolved_path.display(),
            error
          );
          continue;
        }
      };

      Self::merge_config_value(merged, imported_raw.clone());
      Self::apply_imports(
        merged,
        &resolved_path,
        &Self::extract_imports(&imported_raw),
        visited,
      );
    }
  }

  fn merge_config_value(target: &mut toml::Value, overlay: toml::Value) {
    match (target, overlay) {
      (toml::Value::Table(target_table), toml::Value::Table(overlay_table)) => {
        for (key, overlay_value) in overlay_table {
          if matches!(key.as_str(), "version" | "imports") {
            continue;
          }

          match target_table.get_mut(&key) {
            Some(target_value)
              if matches!(target_value, toml::Value::Table(_))
                && matches!(overlay_value, toml::Value::Table(_)) =>
            {
              Self::merge_config_value(target_value, overlay_value);
            }
            _ => {
              target_table.insert(key, overlay_value);
            }
          }
        }
      }
      (target_value, overlay_value) => {
        *target_value = overlay_value;
      }
    }
  }

  fn resolve_import_path(current_path: &Path, import_path: &str) -> PathBuf {
    let expanded = if import_path == "~" {
      dirs::home_dir().unwrap_or_else(|| PathBuf::from(import_path))
    } else if let Some(rest) = import_path
      .strip_prefix("~/")
      .or_else(|| import_path.strip_prefix("~\\"))
    {
      dirs::home_dir()
        .map(|home| home.join(rest))
        .unwrap_or_else(|| PathBuf::from(import_path))
    } else {
      PathBuf::from(import_path)
    };

    if expanded.is_absolute() {
      return expanded;
    }

    current_path
      .parent()
      .map(|parent| parent.join(&expanded))
      .unwrap_or(expanded)
  }

  fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
  }

  fn collect_import_paths(
    current_path: &Path,
    imports: &[String],
    visited: &mut HashSet<PathBuf>,
    paths: &mut Vec<PathBuf>,
  ) {
    for import in imports {
      let resolved_path = Self::resolve_import_path(current_path, import);
      let normalized_path = Self::normalize_path(&resolved_path);

      if !visited.insert(normalized_path) {
        continue;
      }

      paths.push(resolved_path.clone());

      let Ok(imported_raw) = Self::read_raw_config(&resolved_path) else {
        continue;
      };

      let nested_imports = Self::extract_imports(&imported_raw);
      if !nested_imports.is_empty() {
        Self::collect_import_paths(&resolved_path, &nested_imports, visited, paths);
      }
    }
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

  pub fn get_config_file_paths() -> Vec<PathBuf> {
    let Some(path) = Self::get_config_file_path() else {
      return Vec::new();
    };

    let mut paths = vec![path.clone()];
    let mut visited = HashSet::from([Self::normalize_path(&path)]);

    if let Ok(raw) = Self::read_raw_config(&path) {
      Self::collect_import_paths(
        &path,
        &Self::extract_imports(&raw),
        &mut visited,
        &mut paths,
      );
    }

    paths
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
  use std::time::{SystemTime, UNIX_EPOCH};

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

  fn test_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_nanos();
    let path = std::env::temp_dir().join(format!(
      "kazeterm-config-tests-{}-{}-{}",
      name,
      std::process::id(),
      unique,
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
  }

  #[test]
  fn load_from_path_merges_imports_with_higher_priority() {
    let dir = test_dir("imports-override");
    let base_path = dir.join("kazeterm.toml");
    let overlay_path = dir.join("kazeterm.windows.toml");

    std::fs::write(
      &base_path,
      format!(
        r#"version = "{}"
imports = ["./kazeterm.windows.toml"]

[appearance]
background_opacity = 1.0

[keybindings]
copy = "ctrl-shift-c"

[terminal.env]
BASE = "from-base"
"#,
        CURRENT_CONFIG_VERSION,
      ),
    )
    .unwrap();
    std::fs::write(
      &overlay_path,
      r#"[appearance]
background_opacity = 0.4

[keybindings]
paste = "ctrl-alt-v"

[terminal.env]
BASE = "from-overlay"
EXTRA = "present"
"#,
    )
    .unwrap();

    let config = Config::load_from_path(&base_path).unwrap();

    assert_eq!(config.appearance.background_opacity, 0.4);
    assert_eq!(config.keybindings.copy, "ctrl-shift-c");
    assert_eq!(config.keybindings.paste, "ctrl-alt-v");
    assert_eq!(config.terminal.env.get("BASE").unwrap(), "from-overlay");
    assert_eq!(config.terminal.env.get("EXTRA").unwrap(), "present");

    std::fs::remove_dir_all(dir).unwrap();
  }

  #[test]
  fn resolve_import_path_expands_home_directory() {
    let current_path = PathBuf::from("config/kazeterm.toml");
    let home_dir = dirs::home_dir().expect("home directory should exist");

    let resolved = Config::resolve_import_path(&current_path, "~/kazeterm.windows.toml");

    assert_eq!(resolved, home_dir.join("kazeterm.windows.toml"));
  }

  #[test]
  fn collect_import_paths_includes_nested_imports() {
    let dir = test_dir("nested-imports");
    let base_path = dir.join("kazeterm.toml");
    let overlay_path = dir.join("layer.toml");
    let nested_path = dir.join("layer.local.toml");

    std::fs::write(
      &base_path,
      format!(
        r#"version = "{}"
imports = ["./layer.toml"]
"#,
        CURRENT_CONFIG_VERSION,
      ),
    )
    .unwrap();
    std::fs::write(&overlay_path, "imports = [\"./layer.local.toml\"]\n").unwrap();
    std::fs::write(&nested_path, "[appearance]\nbackground_opacity = 0.7\n").unwrap();

    let mut paths = vec![base_path.clone()];
    let mut visited = HashSet::from([Config::normalize_path(&base_path)]);
    let raw = Config::read_raw_config(&base_path).unwrap();
    Config::collect_import_paths(
      &base_path,
      &Config::extract_imports(&raw),
      &mut visited,
      &mut paths,
    );

    assert_eq!(paths, vec![base_path.clone(), overlay_path, nested_path]);

    std::fs::remove_dir_all(dir).unwrap();
  }

  #[test]
  fn load_from_path_creates_backup_when_migrating() {
    let dir = test_dir("migration-backup");
    let base_path = dir.join("kazeterm.toml");
    let original_content = r#"version = "20260412.1"
theme = "one"
font_size = 18.0
inactive_pane_opacity = 0.6
"#;

    std::fs::write(&base_path, original_content).unwrap();

    let config = Config::load_from_path(&base_path).unwrap();

    assert_eq!(config.version, CURRENT_CONFIG_VERSION);

    let backup_paths = std::fs::read_dir(&dir)
      .unwrap()
      .map(|entry| entry.unwrap().path())
      .filter(|path| {
        path != &base_path
          && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
              name.starts_with("kazeterm.backup.") && name.ends_with(".toml")
            })
      })
      .collect::<Vec<_>>();

    assert_eq!(backup_paths.len(), 1);
    assert_eq!(std::fs::read_to_string(&backup_paths[0]).unwrap(), original_content);

    let updated_content = std::fs::read_to_string(&base_path).unwrap();
    assert!(updated_content.contains(&format!("version = \"{}\"", CURRENT_CONFIG_VERSION)));
    assert!(updated_content.contains("imports = []"));

    std::fs::remove_dir_all(dir).unwrap();
  }
}
