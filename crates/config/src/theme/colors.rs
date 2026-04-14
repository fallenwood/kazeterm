use super::ThemeColors;
use super::parse_hex_color;
use crate::Palette;
use crate::palette::{brighten, dim};

impl ThemeColors {
  /// Convert ThemeColors to a Palette, deriving missing colors from base colors
  ///
  /// The `is_dark` parameter affects how UI colors are derived from background.
  pub fn to_palette(&self, is_dark: bool) -> Palette {
    let mut palette = Palette::default();

    // Parse core colors
    let fg = self.foreground.as_ref().and_then(|s| parse_hex_color(s));
    let accent = self.accent.as_ref().and_then(|s| parse_hex_color(s));
    let border_color = self.border.as_ref().and_then(|s| parse_hex_color(s));

    // Apply core colors as primaries
    if let Some(c) = self.background.as_ref().and_then(|s| parse_hex_color(s)) {
      palette.background = c;
    }
    if let Some(c) = fg {
      palette.text = c;
      palette.terminal_ansi_white = c;
    }
    if let Some(c) = accent {
      palette.text_accent = c;
    }
    if let Some(c) = border_color {
      palette.border = c;
    }

    // Parse and apply ANSI colors with auto-derived bright/dim variants
    if let Some(c) = self.black.as_ref().and_then(|s| parse_hex_color(s)) {
      palette.terminal_ansi_black = c;
      palette.terminal_ansi_bright_black = self
        .bright_black
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_black = dim(c);
    }
    if let Some(c) = self.red.as_ref().and_then(|s| parse_hex_color(s)) {
      palette.terminal_ansi_red = c;
      palette.terminal_ansi_bright_red = self
        .bright_red
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_red = dim(c);
    }
    if let Some(c) = self.green.as_ref().and_then(|s| parse_hex_color(s)) {
      palette.terminal_ansi_green = c;
      palette.terminal_ansi_bright_green = self
        .bright_green
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_green = dim(c);
    }
    if let Some(c) = self.yellow.as_ref().and_then(|s| parse_hex_color(s)) {
      palette.terminal_ansi_yellow = c;
      palette.terminal_ansi_bright_yellow = self
        .bright_yellow
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_yellow = dim(c);
    }
    if let Some(c) = self.blue.as_ref().and_then(|s| parse_hex_color(s)) {
      palette.terminal_ansi_blue = c;
      palette.terminal_ansi_bright_blue = self
        .bright_blue
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_blue = dim(c);
    }
    if let Some(c) = self.magenta.as_ref().and_then(|s| parse_hex_color(s)) {
      palette.terminal_ansi_magenta = c;
      palette.terminal_ansi_bright_magenta = self
        .bright_magenta
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_magenta = dim(c);
    }
    if let Some(c) = self.cyan.as_ref().and_then(|s| parse_hex_color(s)) {
      palette.terminal_ansi_cyan = c;
      palette.terminal_ansi_bright_cyan = self
        .bright_cyan
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_cyan = dim(c);
    }
    if let Some(c) = self.white.as_ref().and_then(|s| parse_hex_color(s)) {
      palette.terminal_ansi_white = c;
      palette.terminal_ansi_bright_white = self
        .bright_white
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_white = dim(c);
    }

    // Derive all computed UI colors from the now-set primaries
    palette.derive_ui_colors(is_dark);

    // Override cursor if theme specifies one (after derive_ui_colors sets it to accent)
    if let Some(ref s) = self.cursor
      && let Some(c) = parse_hex_color(s)
    {
      palette.terminal_cursor = c;
    }

    palette
  }
}
