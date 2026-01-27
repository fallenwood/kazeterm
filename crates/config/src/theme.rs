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
  /// Convert ThemeColors to a Palette, deriving missing colors from base colors
  pub fn to_palette(&self) -> Palette {
    let mut palette = Palette::default();

    // Parse core colors
    let bg = self.background.as_ref().and_then(|s| parse_hex_color(s));
    let fg = self.foreground.as_ref().and_then(|s| parse_hex_color(s));
    let accent = self.accent.as_ref().and_then(|s| parse_hex_color(s));
    let border_color = self.border.as_ref().and_then(|s| parse_hex_color(s));

    // Apply core colors
    if let Some(c) = bg {
      palette.background = c;
      palette.terminal_background = c;
      palette.terminal_ansi_background = c;
      palette.tab_active_background = c;
    }
    if let Some(c) = fg {
      palette.text = c;
      palette.text_muted = c;
      palette.terminal_foreground = c;
      palette.terminal_ansi_white = c;
    }
    if let Some(c) = accent {
      palette.text_accent = c;
      palette.terminal_cursor = c;
      palette.border_focused = c;
      palette.border_selected = c;
    }
    if let Some(c) = border_color {
      palette.border = c;
    }

    // Parse ANSI base colors
    let black = self.black.as_ref().and_then(|s| parse_hex_color(s));
    let red = self.red.as_ref().and_then(|s| parse_hex_color(s));
    let green = self.green.as_ref().and_then(|s| parse_hex_color(s));
    let yellow = self.yellow.as_ref().and_then(|s| parse_hex_color(s));
    let blue = self.blue.as_ref().and_then(|s| parse_hex_color(s));
    let magenta = self.magenta.as_ref().and_then(|s| parse_hex_color(s));
    let cyan = self.cyan.as_ref().and_then(|s| parse_hex_color(s));
    let white = self.white.as_ref().and_then(|s| parse_hex_color(s));

    // Apply ANSI colors with auto-derived bright/dim variants
    if let Some(c) = black {
      palette.terminal_ansi_black = c;
      palette.terminal_ansi_bright_black = self.bright_black.as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_black = dim(c);
    }
    if let Some(c) = red {
      palette.terminal_ansi_red = c;
      palette.terminal_ansi_bright_red = self.bright_red.as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_red = dim(c);
    }
    if let Some(c) = green {
      palette.terminal_ansi_green = c;
      palette.terminal_ansi_bright_green = self.bright_green.as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_green = dim(c);
    }
    if let Some(c) = yellow {
      palette.terminal_ansi_yellow = c;
      palette.terminal_ansi_bright_yellow = self.bright_yellow.as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_yellow = dim(c);
    }
    if let Some(c) = blue {
      palette.terminal_ansi_blue = c;
      palette.terminal_ansi_bright_blue = self.bright_blue.as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_blue = dim(c);
    }
    if let Some(c) = magenta {
      palette.terminal_ansi_magenta = c;
      palette.terminal_ansi_bright_magenta = self.bright_magenta.as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_magenta = dim(c);
    }
    if let Some(c) = cyan {
      palette.terminal_ansi_cyan = c;
      palette.terminal_ansi_bright_cyan = self.bright_cyan.as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_cyan = dim(c);
    }
    if let Some(c) = white {
      palette.terminal_ansi_white = c;
      palette.terminal_ansi_bright_white = self.bright_white.as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_white = dim(c);
    }

    // Apply cursor if specified (otherwise uses accent)
    if let Some(ref s) = self.cursor {
      if let Some(c) = parse_hex_color(s) {
        palette.terminal_cursor = c;
      }
    }

    // Derive UI colors from background
    if let Some(bg) = bg {
      palette.surface_background = dim(bg);
      palette.elevated_surface_background = brighten(bg);
      palette.element_background = dim(bg);
      palette.element_hover = brighten(bg);
      palette.element_active = brighten(brighten(bg));
      palette.element_selected = brighten(brighten(bg));
      palette.title_bar_background = brighten(bg);
      palette.title_bar_inactive_background = dim(bg);
      palette.tab_inactive_background = brighten(bg);
    }

    palette
  }
}

/// Brighten a color by increasing lightness
fn brighten(color: Hsla) -> Hsla {
  Hsla {
    h: color.h,
    s: color.s,
    l: (color.l + 0.1).min(1.0),
    a: color.a,
  }
}

/// Dim a color by decreasing lightness
fn dim(color: Hsla) -> Hsla {
  Hsla {
    h: color.h,
    s: color.s,
    l: (color.l - 0.1).max(0.0),
    a: color.a,
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

    let palette = colors.to_palette();

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

    let palette = colors.to_palette();

    // Bright red should be lighter than base red
    assert!(palette.terminal_ansi_bright_red.l > palette.terminal_ansi_red.l);
    // Dim red should be darker than base red
    assert!(palette.terminal_ansi_dim_red.l < palette.terminal_ansi_red.l);
  }
}
