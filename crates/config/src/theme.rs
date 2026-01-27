//! Theme file loading and parsing
//!
//! Themes are loaded from TOML files in the assets/themes/ directory.

use gpui::{Hsla, Rgba};
use serde::{Deserialize, Serialize};

use crate::Palette;

/// Theme file structure for loading from TOML
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThemeFile {
  /// Display name of the theme
  pub name: String,
  /// Theme colors
  #[serde(default)]
  pub colors: ThemeColors,
}

/// Theme colors from a theme file (all optional to allow partial overrides)
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ThemeColors {
  // UI colors
  pub background: Option<String>,
  pub surface_background: Option<String>,
  pub elevated_surface_background: Option<String>,
  pub border: Option<String>,
  pub border_variant: Option<String>,
  pub text: Option<String>,
  pub text_muted: Option<String>,
  pub text_placeholder: Option<String>,
  pub text_disabled: Option<String>,
  pub text_accent: Option<String>,

  // Title bar and tabs
  pub title_bar_background: Option<String>,
  pub title_bar_inactive_background: Option<String>,
  pub tab_inactive_background: Option<String>,
  pub tab_active_background: Option<String>,

  // Element colors
  pub element_background: Option<String>,
  pub element_hover: Option<String>,
  pub element_active: Option<String>,
  pub element_selected: Option<String>,

  // Terminal colors
  pub terminal_background: Option<String>,
  pub terminal_foreground: Option<String>,
  pub terminal_bright_foreground: Option<String>,
  pub terminal_dim_foreground: Option<String>,
  pub terminal_ansi_background: Option<String>,
  pub terminal_cursor: Option<String>,

  // ANSI colors
  pub terminal_ansi_black: Option<String>,
  pub terminal_ansi_bright_black: Option<String>,
  pub terminal_ansi_dim_black: Option<String>,
  pub terminal_ansi_red: Option<String>,
  pub terminal_ansi_bright_red: Option<String>,
  pub terminal_ansi_dim_red: Option<String>,
  pub terminal_ansi_green: Option<String>,
  pub terminal_ansi_bright_green: Option<String>,
  pub terminal_ansi_dim_green: Option<String>,
  pub terminal_ansi_yellow: Option<String>,
  pub terminal_ansi_bright_yellow: Option<String>,
  pub terminal_ansi_dim_yellow: Option<String>,
  pub terminal_ansi_blue: Option<String>,
  pub terminal_ansi_bright_blue: Option<String>,
  pub terminal_ansi_dim_blue: Option<String>,
  pub terminal_ansi_magenta: Option<String>,
  pub terminal_ansi_bright_magenta: Option<String>,
  pub terminal_ansi_dim_magenta: Option<String>,
  pub terminal_ansi_cyan: Option<String>,
  pub terminal_ansi_bright_cyan: Option<String>,
  pub terminal_ansi_dim_cyan: Option<String>,
  pub terminal_ansi_white: Option<String>,
  pub terminal_ansi_bright_white: Option<String>,
  pub terminal_ansi_dim_white: Option<String>,
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

/// Load a theme from assets by name
///
/// Looks for a theme file at `assets/themes/{name}.toml` relative to the executable.
pub fn load_theme_from_assets(name: &str) -> Option<ThemeFile> {
  // Try to find the assets directory relative to the executable
  let exe_path = std::env::current_exe().ok()?;
  let exe_dir = exe_path.parent()?;

  // Try multiple possible locations for assets
  let possible_paths = [
    exe_dir.join("assets").join("themes").join(format!("{}.toml", name)),
    exe_dir.join("..").join("assets").join("themes").join(format!("{}.toml", name)),
    exe_dir.join("..").join("..").join("assets").join("themes").join(format!("{}.toml", name)),
    // For development: relative to current working directory
    std::env::current_dir().ok()?.join("assets").join("themes").join(format!("{}.toml", name)),
  ];

  for path in &possible_paths {
    if path.exists() {
      tracing::debug!("Loading theme from: {}", path.display());
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

/// Load a theme and convert it to a Palette
pub fn load_theme(name: &str) -> (String, Palette) {
  if let Some(theme_file) = load_theme_from_assets(name) {
    let palette = theme_file.colors.to_palette();
    (theme_file.name, palette)
  } else {
    tracing::info!("Using default palette for theme '{}'", name);
    ("One Dark".to_string(), Palette::default())
  }
}

impl ThemeColors {
  /// Convert ThemeColors to a Palette, using defaults for unspecified colors
  pub fn to_palette(&self) -> Palette {
    let mut palette = Palette::default();

    macro_rules! apply_color {
      ($field:ident) => {
        if let Some(ref color_str) = self.$field {
          if let Some(color) = parse_hex_color(color_str) {
            palette.$field = color;
          }
        }
      };
    }

    // UI colors
    apply_color!(background);
    apply_color!(surface_background);
    apply_color!(elevated_surface_background);
    apply_color!(border);
    apply_color!(border_variant);
    apply_color!(text);
    apply_color!(text_muted);
    apply_color!(text_placeholder);
    apply_color!(text_disabled);
    apply_color!(text_accent);

    // Title bar and tabs
    apply_color!(title_bar_background);
    apply_color!(title_bar_inactive_background);
    apply_color!(tab_inactive_background);
    apply_color!(tab_active_background);

    // Element colors
    apply_color!(element_background);
    apply_color!(element_hover);
    apply_color!(element_active);
    apply_color!(element_selected);

    // Terminal colors
    apply_color!(terminal_background);
    apply_color!(terminal_foreground);
    apply_color!(terminal_bright_foreground);
    apply_color!(terminal_dim_foreground);
    apply_color!(terminal_ansi_background);
    apply_color!(terminal_cursor);

    // ANSI colors
    apply_color!(terminal_ansi_black);
    apply_color!(terminal_ansi_bright_black);
    apply_color!(terminal_ansi_dim_black);
    apply_color!(terminal_ansi_red);
    apply_color!(terminal_ansi_bright_red);
    apply_color!(terminal_ansi_dim_red);
    apply_color!(terminal_ansi_green);
    apply_color!(terminal_ansi_bright_green);
    apply_color!(terminal_ansi_dim_green);
    apply_color!(terminal_ansi_yellow);
    apply_color!(terminal_ansi_bright_yellow);
    apply_color!(terminal_ansi_dim_yellow);
    apply_color!(terminal_ansi_blue);
    apply_color!(terminal_ansi_bright_blue);
    apply_color!(terminal_ansi_dim_blue);
    apply_color!(terminal_ansi_magenta);
    apply_color!(terminal_ansi_bright_magenta);
    apply_color!(terminal_ansi_dim_magenta);
    apply_color!(terminal_ansi_cyan);
    apply_color!(terminal_ansi_bright_cyan);
    apply_color!(terminal_ansi_dim_cyan);
    apply_color!(terminal_ansi_white);
    apply_color!(terminal_ansi_bright_white);
    apply_color!(terminal_ansi_dim_white);

    palette
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
  fn theme_colors_to_palette_applies_overrides() {
    let mut colors = ThemeColors::default();
    colors.background = Some("#000000".to_string());
    colors.terminal_ansi_red = Some("#FF0000".to_string());

    let palette = colors.to_palette();

    // Check the override was applied
    let bg = palette.background.to_rgb();
    assert!((bg.r - 0.0).abs() < 0.01);
    assert!((bg.g - 0.0).abs() < 0.01);
    assert!((bg.b - 0.0).abs() < 0.01);

    let red = palette.terminal_ansi_red.to_rgb();
    assert!((red.r - 1.0).abs() < 0.01);
    assert!((red.g - 0.0).abs() < 0.01);
    assert!((red.b - 0.0).abs() < 0.01);
  }
}
