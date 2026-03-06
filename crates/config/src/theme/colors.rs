use gpui::Hsla;

use super::ThemeColors;
use super::parse_hex_color;
use crate::Palette;

impl ThemeColors {
  /// Convert ThemeColors to a Palette, deriving missing colors from base colors
  ///
  /// The `is_dark` parameter affects how UI colors are derived from background.
  pub fn to_palette(&self, is_dark: bool) -> Palette {
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
      palette.terminal_ansi_bright_black = self
        .bright_black
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_black = dim(c);
    }
    if let Some(c) = red {
      palette.terminal_ansi_red = c;
      palette.terminal_ansi_bright_red = self
        .bright_red
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_red = dim(c);
    }
    if let Some(c) = green {
      palette.terminal_ansi_green = c;
      palette.terminal_ansi_bright_green = self
        .bright_green
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_green = dim(c);
    }
    if let Some(c) = yellow {
      palette.terminal_ansi_yellow = c;
      palette.terminal_ansi_bright_yellow = self
        .bright_yellow
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_yellow = dim(c);
    }
    if let Some(c) = blue {
      palette.terminal_ansi_blue = c;
      palette.terminal_ansi_bright_blue = self
        .bright_blue
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_blue = dim(c);
    }
    if let Some(c) = magenta {
      palette.terminal_ansi_magenta = c;
      palette.terminal_ansi_bright_magenta = self
        .bright_magenta
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_magenta = dim(c);
    }
    if let Some(c) = cyan {
      palette.terminal_ansi_cyan = c;
      palette.terminal_ansi_bright_cyan = self
        .bright_cyan
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_cyan = dim(c);
    }
    if let Some(c) = white {
      palette.terminal_ansi_white = c;
      palette.terminal_ansi_bright_white = self
        .bright_white
        .as_ref()
        .and_then(|s| parse_hex_color(s))
        .unwrap_or_else(|| brighten(c));
      palette.terminal_ansi_dim_white = dim(c);
    }

    // Apply cursor if specified (otherwise uses accent)
    if let Some(ref s) = self.cursor
      && let Some(c) = parse_hex_color(s)
    {
      palette.terminal_cursor = c;
    }

    // Derive UI colors from background
    // For dark themes: brighten for elevated, dim for recessed
    // For light themes: dim for elevated, brighten for recessed
    if let Some(bg) = bg {
      if is_dark {
        palette.surface_background = dim(bg);
        palette.elevated_surface_background = brighten(bg);
        palette.element_background = dim(bg);
        palette.element_hover = brighten(bg);
        palette.element_active = brighten(brighten(bg));
        palette.element_selected = brighten(brighten(bg));
        palette.title_bar_background = brighten(bg);
        palette.title_bar_inactive_background = dim(bg);
        palette.tab_inactive_background = slightly_brighten(bg);
        palette.scrollbar_track_background = brighten(bg);
        palette.scrollbar_thumb_background = brighten(brighten(brighten(bg)));
      } else {
        palette.surface_background = brighten(bg);
        palette.elevated_surface_background = dim(bg);
        palette.element_background = brighten(bg);
        palette.element_hover = dim(bg);
        palette.element_active = dim(dim(bg));
        palette.element_selected = dim(dim(bg));
        palette.title_bar_background = dim(bg);
        palette.title_bar_inactive_background = brighten(bg);
        palette.tab_inactive_background = slightly_dim(bg);
        palette.scrollbar_track_background = dim(bg);
        palette.scrollbar_thumb_background = dim(dim(dim(bg)));
      }
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

/// Slightly brighten a color by increasing lightness (half of brighten)
fn slightly_brighten(color: Hsla) -> Hsla {
  Hsla {
    h: color.h,
    s: color.s,
    l: (color.l + 0.05).min(1.0),
    a: color.a,
  }
}

/// Slightly dim a color by decreasing lightness (half of dim)
fn slightly_dim(color: Hsla) -> Hsla {
  Hsla {
    h: color.h,
    s: color.s,
    l: (color.l - 0.05).max(0.0),
    a: color.a,
  }
}
