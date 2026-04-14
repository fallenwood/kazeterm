use gpui::{Hsla, Rgba, hsla};

#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
  /// Border color. Used for most borders, is usually a high contrast color.
  pub border: Hsla,
  /// Border color. Used for deemphasized borders, like a visual divider between two sections
  pub border_variant: Hsla,
  /// Border color. Used for focused elements, like keyboard focused list item.
  pub border_focused: Hsla,
  /// Border color. Used for selected elements, like an active search filter or selected checkbox.
  pub border_selected: Hsla,
  /// Border color. Used for transparent borders. Used for placeholder borders when an element gains a border on state change.
  pub border_transparent: Hsla,
  /// Border color. Used for disabled elements, like a disabled input or button.
  pub border_disabled: Hsla,
  /// Border color. Used for elevated surfaces, like a context menu, popup, or dialog.
  pub elevated_surface_background: Hsla,
  /// Background Color. Used for grounded surfaces like a panel or tab.
  pub surface_background: Hsla,
  /// Background Color. Used for the app background and blank panels or windows.
  pub background: Hsla,
  /// Background Color. Used for the background of an element that should have a different background than the surface it's on.
  ///
  /// Elements might include: Buttons, Inputs, Checkboxes, Radio Buttons...
  ///
  /// For an element that should have the same background as the surface it's on, use `ghost_element_background`.
  pub element_background: Hsla,
  /// Background Color. Used for the hover state of an element that should have a different background than the surface it's on.
  ///
  /// Hover states are triggered by the mouse entering an element, or a finger touching an element on a touch screen.
  pub element_hover: Hsla,
  /// Background Color. Used for the active state of an element that should have a different background than the surface it's on.
  ///
  /// Active states are triggered by the mouse button being pressed down on an element, or the Return button or other activator being pressed.
  pub element_active: Hsla,
  /// Background Color. Used for the selected state of an element that should have a different background than the surface it's on.
  ///
  /// Selected states are triggered by the element being selected (or "activated") by the user.
  ///
  /// This could include a selected checkbox, a toggleable button that is toggled on, etc.
  pub element_selected: Hsla,
  /// Background Color. Used for the background of selections in a UI element.
  pub element_selection_background: Hsla,
  /// Background Color. Used for the disabled state of an element that should have a different background than the surface it's on.
  ///
  /// Disabled states are shown when a user cannot interact with an element, like a disabled button or input.
  pub element_disabled: Hsla,

  /// Text Color. Default text color used for most text.
  pub text: Hsla,
  /// Text Color. Color of muted or deemphasized text. It is a subdued version of the standard text color.
  pub text_muted: Hsla,
  /// Text Color. Color of the placeholder text typically shown in input fields to guide the user to enter valid data.
  pub text_placeholder: Hsla,
  /// Text Color. Color used for text denoting disabled elements. Typically, the color is faded or grayed out to emphasize the disabled state.
  pub text_disabled: Hsla,
  /// Text Color. Color used for emphasis or highlighting certain text, like an active filter or a matched character in a search.
  pub text_accent: Hsla,

  // ===
  // UI Elements
  // ===
  pub title_bar_background: Hsla,
  pub title_bar_inactive_background: Hsla,
  pub tab_inactive_background: Hsla,
  pub tab_active_background: Hsla,
  pub search_match_background: Hsla,
  pub search_highlight_background: Hsla,

  // ===
  // Terminal
  // ===
  /// Terminal layout background color.
  pub terminal_background: Hsla,
  /// Terminal foreground color.
  pub terminal_foreground: Hsla,
  /// Bright terminal foreground color.
  pub terminal_bright_foreground: Hsla,
  /// Dim terminal foreground color.
  pub terminal_dim_foreground: Hsla,
  /// Terminal ANSI background color.
  pub terminal_ansi_background: Hsla,
  /// Black ANSI terminal color.
  pub terminal_ansi_black: Hsla,
  /// Bright black ANSI terminal color.
  pub terminal_ansi_bright_black: Hsla,
  /// Dim black ANSI terminal color.
  pub terminal_ansi_dim_black: Hsla,
  /// Red ANSI terminal color.
  pub terminal_ansi_red: Hsla,
  /// Bright red ANSI terminal color.
  pub terminal_ansi_bright_red: Hsla,
  /// Dim red ANSI terminal color.
  pub terminal_ansi_dim_red: Hsla,
  /// Green ANSI terminal color.
  pub terminal_ansi_green: Hsla,
  /// Bright green ANSI terminal color.
  pub terminal_ansi_bright_green: Hsla,
  /// Dim green ANSI terminal color.
  pub terminal_ansi_dim_green: Hsla,
  /// Yellow ANSI terminal color.
  pub terminal_ansi_yellow: Hsla,
  /// Bright yellow ANSI terminal color.
  pub terminal_ansi_bright_yellow: Hsla,
  /// Dim yellow ANSI terminal color.
  pub terminal_ansi_dim_yellow: Hsla,
  /// Blue ANSI terminal color.
  pub terminal_ansi_blue: Hsla,
  /// Bright blue ANSI terminal color.
  pub terminal_ansi_bright_blue: Hsla,
  /// Dim blue ANSI terminal color.
  pub terminal_ansi_dim_blue: Hsla,
  /// Magenta ANSI terminal color.
  pub terminal_ansi_magenta: Hsla,
  /// Bright magenta ANSI terminal color.
  pub terminal_ansi_bright_magenta: Hsla,
  /// Dim magenta ANSI terminal color.
  pub terminal_ansi_dim_magenta: Hsla,
  /// Cyan ANSI terminal color.
  pub terminal_ansi_cyan: Hsla,
  /// Bright cyan ANSI terminal color.
  pub terminal_ansi_bright_cyan: Hsla,
  /// Dim cyan ANSI terminal color.
  pub terminal_ansi_dim_cyan: Hsla,
  /// White ANSI terminal color.
  pub terminal_ansi_white: Hsla,
  /// Bright white ANSI terminal color.
  pub terminal_ansi_bright_white: Hsla,
  /// Dim white ANSI terminal color.
  pub terminal_ansi_dim_white: Hsla,
  /// Terminal cursor color.
  pub terminal_cursor: Hsla,

  // ===
  // Scrollbar
  // ===
  /// Scrollbar track background color.
  pub scrollbar_track_background: Hsla,
  /// Scrollbar thumb background color.
  pub scrollbar_thumb_background: Hsla,

  /// Represents a link text hover color.
  pub link_text_hover: Hsla,
}

impl Palette {
  /// Derive all computed UI colors from the primary colors already set.
  ///
  /// Primary colors that should be set before calling:
  /// - `background`, `text`, `text_accent`, `border` (4 core colors)
  /// - All `terminal_ansi_*` base + bright + dim colors
  ///
  /// This method computes: border variants, surface/element backgrounds,
  /// text variants, search highlights, terminal fg variants, scrollbar colors,
  /// link hover, and terminal cursor.
  pub(crate) fn derive_ui_colors(&mut self, is_dark: bool) {
    let bg = self.background;
    let fg = self.text;
    let accent = self.text_accent;

    // Border variants
    self.border_variant = if is_dark {
      dim(self.border)
    } else {
      brighten(self.border)
    };
    self.border_focused = {
      let mut c = accent;
      c.s *= 0.5;
      c.l = c.l * 0.8 + 0.5 * 0.2;
      c
    };
    self.border_selected = accent;
    self.border_transparent = hsla(0.0, 0.0, 0.0, 0.0);
    self.border_disabled = if is_dark {
      dim(dim(bg))
    } else {
      brighten(brighten(bg))
    };

    // Surface and element backgrounds
    if is_dark {
      self.surface_background = dim(bg);
      self.elevated_surface_background = brighten(bg);
      self.element_background = dim(bg);
      self.element_hover = brighten(bg);
      self.element_active = brighten(brighten(bg));
      self.element_selected = brighten(brighten(bg));
      self.element_disabled = dim(dim(bg));
      self.title_bar_background = brighten(bg);
      self.title_bar_inactive_background = dim(bg);
      self.tab_inactive_background = slightly_brighten(bg);
      self.scrollbar_track_background = brighten(bg);
      self.scrollbar_thumb_background = brighten(brighten(brighten(bg)));
    } else {
      self.surface_background = brighten(bg);
      self.elevated_surface_background = dim(bg);
      self.element_background = brighten(bg);
      self.element_hover = dim(bg);
      self.element_active = dim(dim(bg));
      self.element_selected = dim(dim(bg));
      self.element_disabled = brighten(brighten(bg));
      self.title_bar_background = dim(bg);
      self.title_bar_inactive_background = brighten(bg);
      self.tab_inactive_background = slightly_dim(bg);
      self.scrollbar_track_background = dim(bg);
      self.scrollbar_thumb_background = dim(dim(dim(bg)));
    }
    self.tab_active_background = bg;

    // Selection background from accent with alpha
    self.element_selection_background = {
      let mut c = accent;
      c.a = 0.43;
      c
    };

    // Text variants
    self.text_muted = fg;
    self.text_placeholder = blend(fg, bg, 0.5);
    self.text_disabled = blend(fg, bg, 0.65);

    // Search colors derived from theme colors
    self.search_match_background = {
      let mut c = self.terminal_ansi_yellow;
      c.a = 0.6;
      c
    };
    self.search_highlight_background = {
      let mut c = accent;
      c.a = 0.4;
      c
    };

    // Terminal base colors
    self.terminal_background = bg;
    self.terminal_foreground = fg;
    self.terminal_ansi_background = bg;
    self.terminal_bright_foreground = brighten(fg);
    self.terminal_dim_foreground = dim(fg);
    self.terminal_cursor = accent;

    // Link hover
    self.link_text_hover = brighten(accent);
  }
}

impl Default for Palette {
  fn default() -> Self {
    // One theme base colors
    let background = rgb_u8(40, 44, 51); // #282C33
    let foreground = rgb_u8(220, 224, 229); // #DCE0E5
    let accent = rgb_u8(116, 173, 232); // #74ADE8
    let border_color = rgb_u8(70, 75, 87); // #464B57

    // One theme ANSI colors
    let ansi_black = background;
    let ansi_red = rgb_u8(208, 114, 119);
    let ansi_green = rgb_u8(161, 193, 129);
    let ansi_yellow = rgb_u8(222, 193, 132);
    let ansi_blue = accent;
    let ansi_magenta = rgb_u8(180, 119, 207);
    let ansi_cyan = rgb_u8(110, 180, 191);
    let ansi_white = foreground;

    // One theme explicit bright overrides (only black and white)
    let bright_black = rgb_u8(82, 85, 97);
    let bright_white = rgb_u8(250, 250, 250);

    // Build palette with primary + ANSI colors, then derive all computed colors
    let mut palette = Palette {
      // 4 core colors
      background,
      text: foreground,
      text_accent: accent,
      border: border_color,

      // Placeholder values — derive_ui_colors() will overwrite these
      border_variant: border_color,
      border_focused: accent,
      border_selected: accent,
      border_transparent: hsla(0.0, 0.0, 0.0, 0.0),
      border_disabled: background,
      elevated_surface_background: background,
      surface_background: background,
      element_background: background,
      element_hover: background,
      element_active: background,
      element_selected: background,
      element_selection_background: accent,
      element_disabled: background,
      text_muted: foreground,
      text_placeholder: foreground,
      text_disabled: foreground,
      title_bar_background: background,
      title_bar_inactive_background: background,
      tab_inactive_background: background,
      tab_active_background: background,
      search_match_background: hsla(0.0, 0.0, 0.0, 0.0),
      search_highlight_background: hsla(0.0, 0.0, 0.0, 0.0),
      terminal_background: background,
      terminal_foreground: foreground,
      terminal_bright_foreground: foreground,
      terminal_dim_foreground: foreground,
      terminal_ansi_background: background,
      scrollbar_track_background: background,
      scrollbar_thumb_background: background,
      link_text_hover: accent,

      // ANSI terminal colors with derived bright/dim variants
      terminal_ansi_black: ansi_black,
      terminal_ansi_bright_black: bright_black,
      terminal_ansi_dim_black: dim(ansi_black),
      terminal_ansi_red: ansi_red,
      terminal_ansi_bright_red: brighten(ansi_red),
      terminal_ansi_dim_red: dim(ansi_red),
      terminal_ansi_green: ansi_green,
      terminal_ansi_bright_green: brighten(ansi_green),
      terminal_ansi_dim_green: dim(ansi_green),
      terminal_ansi_yellow: ansi_yellow,
      terminal_ansi_bright_yellow: brighten(ansi_yellow),
      terminal_ansi_dim_yellow: dim(ansi_yellow),
      terminal_ansi_blue: ansi_blue,
      terminal_ansi_bright_blue: brighten(ansi_blue),
      terminal_ansi_dim_blue: dim(ansi_blue),
      terminal_ansi_magenta: ansi_magenta,
      terminal_ansi_bright_magenta: brighten(ansi_magenta),
      terminal_ansi_dim_magenta: dim(ansi_magenta),
      terminal_ansi_cyan: ansi_cyan,
      terminal_ansi_bright_cyan: brighten(ansi_cyan),
      terminal_ansi_dim_cyan: dim(ansi_cyan),
      terminal_ansi_white: ansi_white,
      terminal_ansi_bright_white: bright_white,
      terminal_ansi_dim_white: dim(ansi_white),
      terminal_cursor: accent,
    };

    // Derive all computed UI colors from base colors
    palette.derive_ui_colors(true);
    palette
  }
}

fn rgba_u8(r: u8, g: u8, b: u8, a: u8) -> Hsla {
  Rgba {
    r: r as f32 / 255.0,
    g: g as f32 / 255.0,
    b: b as f32 / 255.0,
    a: a as f32 / 255.0,
  }
  .into()
}

fn rgb_u8(r: u8, g: u8, b: u8) -> Hsla {
  rgba_u8(r, g, b, 255)
}

/// Brighten a color by increasing lightness
pub(crate) fn brighten(color: Hsla) -> Hsla {
  Hsla {
    h: color.h,
    s: color.s,
    l: (color.l + 0.1).min(1.0),
    a: color.a,
  }
}

/// Dim a color by decreasing lightness
pub(crate) fn dim(color: Hsla) -> Hsla {
  Hsla {
    h: color.h,
    s: color.s,
    l: (color.l - 0.1).max(0.0),
    a: color.a,
  }
}

/// Slightly brighten a color (half of brighten)
pub(crate) fn slightly_brighten(color: Hsla) -> Hsla {
  Hsla {
    h: color.h,
    s: color.s,
    l: (color.l + 0.05).min(1.0),
    a: color.a,
  }
}

/// Slightly dim a color (half of dim)
pub(crate) fn slightly_dim(color: Hsla) -> Hsla {
  Hsla {
    h: color.h,
    s: color.s,
    l: (color.l - 0.05).max(0.0),
    a: color.a,
  }
}

/// Blend two colors by linear interpolation in HSL space
pub(crate) fn blend(a: Hsla, b: Hsla, t: f32) -> Hsla {
  Hsla {
    h: a.h + (b.h - a.h) * t,
    s: a.s + (b.s - a.s) * t,
    l: a.l + (b.l - a.l) * t,
    a: a.a + (b.a - a.a) * t,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::to_hex_string;

  #[test]
  fn rgb_u8_round_trips_to_hex() {
    let hsla = rgb_u8(40, 44, 51);
    let hex = to_hex_string(&hsla.to_rgb());
    assert_eq!(hex, "#282C33FF");

    let hsla = rgb_u8(220, 224, 229);
    let hex = to_hex_string(&hsla.to_rgb());
    assert_eq!(hex, "#DCE0E5FF");
  }

  #[test]
  fn palette_defaults_match_expected_hex() {
    let palette = Palette::default();
    assert_eq!(
      to_hex_string(&palette.terminal_background.to_rgb()),
      "#282C33FF"
    );
    assert_eq!(
      to_hex_string(&palette.terminal_foreground.to_rgb()),
      "#DCE0E5FF"
    );
    assert_eq!(
      to_hex_string(&palette.terminal_cursor.to_rgb()),
      "#74ADE8FF"
    );
  }
}
