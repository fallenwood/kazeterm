use super::ThemeColors;
use super::default_theme_variant;
use super::parse_hex_color;
use crate::Palette;
use crate::palette::{ThemeSeed, blend};

impl ThemeColors {
  /// Convert ThemeColors to a Palette, deriving most colors from theme seeds.
  pub fn to_palette(&self, is_dark: bool) -> Palette {
    let fallback = default_theme_variant(is_dark);

    let background = resolve_required_color(&self.background, &fallback.background, "background");
    let foreground = resolve_required_color(&self.foreground, &fallback.foreground, "foreground");

    let black = resolve_required_color(&self.black, &fallback.black, "black");
    let red = resolve_required_color(&self.red, &fallback.red, "red");
    let green = resolve_required_color(&self.green, &fallback.green, "green");
    let yellow = resolve_required_color(&self.yellow, &fallback.yellow, "yellow");
    let blue = resolve_required_color(&self.blue, &fallback.blue, "blue");
    let magenta = resolve_required_color(&self.magenta, &fallback.magenta, "magenta");
    let cyan = resolve_required_color(&self.cyan, &fallback.cyan, "cyan");
    let white = resolve_required_color(&self.white, &fallback.white, "white");

    let seed = ThemeSeed {
      background,
      foreground,
      accent: parse_color(&self.accent)
        .or_else(|| parse_color(&self.blue))
        .or_else(|| resolve_color(&fallback.accent, &fallback.blue))
        .unwrap_or(blue),
      border: parse_color(&self.border).unwrap_or_else(|| blend(background, foreground, 0.18)),
      black,
      red,
      green,
      yellow,
      blue,
      magenta,
      cyan,
      white,
      bright_black: resolve_bright_variant(&self.black, &self.bright_black, &fallback.bright_black),
      bright_red: resolve_bright_variant(&self.red, &self.bright_red, &fallback.bright_red),
      bright_green: resolve_bright_variant(&self.green, &self.bright_green, &fallback.bright_green),
      bright_yellow: resolve_bright_variant(
        &self.yellow,
        &self.bright_yellow,
        &fallback.bright_yellow,
      ),
      bright_blue: resolve_bright_variant(&self.blue, &self.bright_blue, &fallback.bright_blue),
      bright_magenta: resolve_bright_variant(
        &self.magenta,
        &self.bright_magenta,
        &fallback.bright_magenta,
      ),
      bright_cyan: resolve_bright_variant(&self.cyan, &self.bright_cyan, &fallback.bright_cyan),
      bright_white: resolve_bright_variant(&self.white, &self.bright_white, &fallback.bright_white),
      cursor: resolve_color(&self.cursor, &fallback.cursor),
      overlay_background: resolve_color(&self.overlay, &fallback.overlay),
      selection_background: resolve_color(&self.selection, &fallback.selection),
      search_match_background: resolve_color(&self.search_match, &fallback.search_match),
      search_highlight_background: resolve_color(
        &self.search_highlight,
        &fallback.search_highlight,
      ),
    };

    Palette::from_seed(seed)
  }
}

fn parse_color(value: &Option<String>) -> Option<gpui::Hsla> {
  value.as_ref().and_then(|color| parse_hex_color(color))
}

fn resolve_color(primary: &Option<String>, fallback: &Option<String>) -> Option<gpui::Hsla> {
  parse_color(primary).or_else(|| parse_color(fallback))
}

fn resolve_required_color(
  primary: &Option<String>,
  fallback: &Option<String>,
  name: &str,
) -> gpui::Hsla {
  resolve_color(primary, fallback)
    .unwrap_or_else(|| panic!("default theme must provide a valid '{name}' color"))
}

fn resolve_bright_variant(
  base_override: &Option<String>,
  explicit_override: &Option<String>,
  fallback: &Option<String>,
) -> Option<gpui::Hsla> {
  if parse_color(base_override).is_some() {
    parse_color(explicit_override)
  } else {
    parse_color(explicit_override).or_else(|| parse_color(fallback))
  }
}
