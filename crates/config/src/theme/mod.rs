//! Theme file loading and parsing
//!
//! Themes are loaded from TOML files in the assets/themes/ directory.
//! Each theme can have light and dark variants.
//!
//! Theme loading priority:
//! 1. Custom theme path (if specified in config)
//! 2. Embedded binary themes (provided by main crate)
//! 3. Fallback to default palette

use gpui::{Hsla, Rgba};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

use crate::Palette;

mod colors;

/// Type alias for embedded theme loader function
/// This function takes a theme name and returns the raw TOML bytes if found
pub type EmbeddedThemeLoader = fn(&str) -> Option<Vec<u8>>;

/// Type alias for embedded theme lister function
/// This function returns a list of all available embedded theme names
pub type EmbeddedThemeLister = fn() -> Vec<String>;

/// Global holder for embedded theme loader function
static EMBEDDED_THEME_LOADER: OnceLock<EmbeddedThemeLoader> = OnceLock::new();

/// Global holder for embedded theme lister function
static EMBEDDED_THEME_LISTER: OnceLock<EmbeddedThemeLister> = OnceLock::new();

/// Global holder for custom themes path
static CUSTOM_THEMES_PATH: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Register the embedded theme loader function
///
/// This should be called once at startup from the main crate.
/// The loader function should return the raw TOML bytes for a theme by name.
pub fn register_embedded_theme_loader(loader: EmbeddedThemeLoader) {
  let _ = EMBEDDED_THEME_LOADER.set(loader);
}

/// Register the embedded theme lister function
///
/// This should be called once at startup from the main crate.
/// The lister function should return all available embedded theme names.
pub fn register_embedded_theme_lister(lister: EmbeddedThemeLister) {
  let _ = EMBEDDED_THEME_LISTER.set(lister);
}

/// Set the custom themes directory path
///
/// Themes in this directory take priority over embedded themes.
/// Path should be a directory containing `{theme_name}.toml` files.
pub fn set_custom_themes_path(path: PathBuf) {
  if let Ok(mut guard) = CUSTOM_THEMES_PATH.write() {
    *guard = Some(path);
  }
}

/// Get the custom themes directory path
pub fn get_custom_themes_path() -> Option<PathBuf> {
  CUSTOM_THEMES_PATH.read().ok().and_then(|g| g.clone())
}

/// Parse a hex color string to Hsla
pub fn parse_hex_color(hex: &str) -> Option<Hsla> {
  let hex = hex.trim_start_matches('#');
  if hex.len() == 6 {
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(
      Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
      }
      .into(),
    )
  } else if hex.len() == 8 {
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
    Some(
      Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: a as f32 / 255.0,
      }
      .into(),
    )
  } else {
    None
  }
}

/// Theme mode selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
  /// Always use light theme
  Light,
  /// Always use dark theme
  #[default]
  Dark,
  /// Follow system dark mode setting
  System,
}

/// Theme file structure for loading from TOML
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThemeFile {
  /// Display name of the theme
  pub name: String,
  /// Dark theme colors (required)
  pub dark: ThemeColors,
  /// Light theme colors (optional, falls back to dark if not specified)
  pub light: Option<ThemeColors>,
}

/// Theme colors - simplified structure with auto-derivation of variants
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ThemeColors {
  // Core colors - most other colors derive from these
  pub background: Option<String>,
  pub foreground: Option<String>,
  pub accent: Option<String>,
  pub border: Option<String>,

  // ANSI colors (8 base colors - bright/dim auto-derived if not specified)
  pub black: Option<String>,
  pub red: Option<String>,
  pub green: Option<String>,
  pub yellow: Option<String>,
  pub blue: Option<String>,
  pub magenta: Option<String>,
  pub cyan: Option<String>,
  pub white: Option<String>,

  // Optional bright variants (auto-derived from base if not specified)
  pub bright_black: Option<String>,
  pub bright_red: Option<String>,
  pub bright_green: Option<String>,
  pub bright_yellow: Option<String>,
  pub bright_blue: Option<String>,
  pub bright_magenta: Option<String>,
  pub bright_cyan: Option<String>,
  pub bright_white: Option<String>,

  // Optional: cursor color (defaults to accent)
  pub cursor: Option<String>,
}

/// Parse a theme from TOML content
pub fn parse_theme_content(content: &str) -> Option<ThemeFile> {
  toml::from_str::<ThemeFile>(content).ok()
}

/// Load a theme from the custom themes directory
fn load_theme_from_custom_path(name: &str) -> Option<ThemeFile> {
  let custom_path = get_custom_themes_path()?;
  let theme_path = custom_path.join(format!("{}.toml", name));

  if theme_path.exists() {
    tracing::debug!("Loading theme from custom path: {}", theme_path.display());
    if let Ok(content) = std::fs::read_to_string(&theme_path) {
      if let Some(theme) = parse_theme_content(&content) {
        return Some(theme);
      } else {
        tracing::warn!("Failed to parse theme file: {}", theme_path.display());
      }
    }
  }
  None
}

/// Load a theme from embedded binary assets
fn load_theme_from_embedded(name: &str) -> Option<ThemeFile> {
  let loader = EMBEDDED_THEME_LOADER.get()?;
  let bytes = loader(name)?;

  let content = String::from_utf8(bytes).ok()?;
  tracing::debug!("Loading theme '{}' from embedded assets", name);
  parse_theme_content(&content)
}

/// Load a theme from assets by name
///
/// Looks for a theme file at `assets/themes/{name}.toml` relative to the executable.
/// Priority: Custom path > Embedded > Filesystem fallback
pub fn load_theme_from_assets(name: &str) -> Option<ThemeFile> {
  // 1. Try custom themes path first
  if let Some(theme) = load_theme_from_custom_path(name) {
    return Some(theme);
  }

  // 2. Try embedded themes
  if let Some(theme) = load_theme_from_embedded(name) {
    return Some(theme);
  }

  // 3. Fallback: Try to find the assets directory relative to the executable
  //    This is mainly for development mode
  let exe_path = std::env::current_exe().ok()?;
  let exe_dir = exe_path.parent()?;

  // Try multiple possible locations for assets
  let possible_paths = [
    exe_dir
      .join("assets")
      .join("themes")
      .join(format!("{}.toml", name)),
    exe_dir
      .join("..")
      .join("assets")
      .join("themes")
      .join(format!("{}.toml", name)),
    exe_dir
      .join("..")
      .join("..")
      .join("assets")
      .join("themes")
      .join(format!("{}.toml", name)),
    // For development: relative to current working directory
    std::env::current_dir()
      .ok()?
      .join("assets")
      .join("themes")
      .join(format!("{}.toml", name)),
  ];

  for path in &possible_paths {
    if path.exists() {
      tracing::debug!("Loading theme from filesystem: {}", path.display());
      if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(theme) = toml::from_str::<ThemeFile>(&content) {
          return Some(theme);
        }
      }
    }
  }

  tracing::warn!("Theme '{}' not found in assets", name);
  None
}

/// List all available themes
///
/// Returns theme names from both embedded themes and custom themes path.
/// Custom themes are listed first.
pub fn list_available_themes() -> Vec<String> {
  let mut themes = Vec::new();

  // 1. List themes from custom path
  if let Some(custom_path) = get_custom_themes_path() {
    if let Ok(entries) = std::fs::read_dir(&custom_path) {
      for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
          if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
            themes.push(name.to_string());
          }
        }
      }
    }
  }

  // 2. List embedded themes
  if let Some(lister) = EMBEDDED_THEME_LISTER.get() {
    for name in lister() {
      if !themes.contains(&name) {
        themes.push(name);
      }
    }
  }

  themes.sort();
  themes
}

/// Load a theme and convert it to a Palette
///
/// The `is_dark` parameter determines which variant to use.
/// For ThemeMode::System, the caller should detect system preference and pass it here.
pub fn load_theme(name: &str, is_dark: bool) -> (String, Palette) {
  if let Some(theme_file) = load_theme_from_assets(name) {
    let colors = if is_dark {
      &theme_file.dark
    } else {
      theme_file.light.as_ref().unwrap_or(&theme_file.dark)
    };
    let palette = colors.to_palette(is_dark);
    let variant = if is_dark { "" } else { " Light" };
    (format!("{}{}", theme_file.name, variant), palette)
  } else {
    tracing::info!("Using default palette for theme '{}'", name);
    ("One".to_string(), Palette::default())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_hex_color_works() {
    // 6-digit hex
    let color = parse_hex_color("#FF0000").unwrap();
    let rgba = color.to_rgb();
    assert!((rgba.r - 1.0).abs() < 0.01);
    assert!((rgba.g - 0.0).abs() < 0.01);
    assert!((rgba.b - 0.0).abs() < 0.01);

    // 8-digit hex with alpha
    let color = parse_hex_color("#00FF0080").unwrap();
    let rgba = color.to_rgb();
    assert!((rgba.g - 1.0).abs() < 0.01);
    assert!((rgba.a - 0.5).abs() < 0.02);

    // Invalid hex
    assert!(parse_hex_color("invalid").is_none());
    assert!(parse_hex_color("#FFF").is_none());
  }

  #[test]
  fn theme_colors_to_palette_applies_core_colors() {
    let mut colors = ThemeColors::default();
    colors.background = Some("#000000".to_string());
    colors.foreground = Some("#FFFFFF".to_string());
    colors.red = Some("#FF0000".to_string());

    let palette = colors.to_palette(true); // dark mode

    // Check background was applied
    let bg = palette.background.to_rgb();
    assert!((bg.r - 0.0).abs() < 0.01);

    // Check foreground was applied to text
    let text = palette.text.to_rgb();
    assert!((text.r - 1.0).abs() < 0.01);

    // Check ANSI red was applied
    let red = palette.terminal_ansi_red.to_rgb();
    assert!((red.r - 1.0).abs() < 0.01);
    assert!((red.g - 0.0).abs() < 0.01);
  }

  #[test]
  fn bright_colors_auto_derived() {
    let mut colors = ThemeColors::default();
    colors.red = Some("#800000".to_string()); // Dark red

    let palette = colors.to_palette(true);

    // Bright red should be lighter than base red
    assert!(palette.terminal_ansi_bright_red.l > palette.terminal_ansi_red.l);
    // Dim red should be darker than base red
    assert!(palette.terminal_ansi_dim_red.l < palette.terminal_ansi_red.l);
  }

  #[test]
  fn theme_mode_serialization() {
    assert_eq!(serde_json::to_string(&ThemeMode::Dark).unwrap(), "\"dark\"");
    assert_eq!(
      serde_json::to_string(&ThemeMode::Light).unwrap(),
      "\"light\""
    );
    assert_eq!(
      serde_json::to_string(&ThemeMode::System).unwrap(),
      "\"system\""
    );
  }
}
