use gpui::{Hsla, Rgba};

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
  pub overlay_background: Hsla,

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

#[derive(Clone, Copy, Debug)]
pub(crate) struct ThemeSeed {
  pub background: Hsla,
  pub foreground: Hsla,
  pub accent: Hsla,
  pub border: Hsla,
  pub black: Hsla,
  pub red: Hsla,
  pub green: Hsla,
  pub yellow: Hsla,
  pub blue: Hsla,
  pub magenta: Hsla,
  pub cyan: Hsla,
  pub white: Hsla,
  pub bright_black: Option<Hsla>,
  pub bright_red: Option<Hsla>,
  pub bright_green: Option<Hsla>,
  pub bright_yellow: Option<Hsla>,
  pub bright_blue: Option<Hsla>,
  pub bright_magenta: Option<Hsla>,
  pub bright_cyan: Option<Hsla>,
  pub bright_white: Option<Hsla>,
  pub cursor: Option<Hsla>,
  pub overlay_background: Option<Hsla>,
  pub selection_background: Option<Hsla>,
  pub search_match_background: Option<Hsla>,
  pub search_highlight_background: Option<Hsla>,
}

impl Default for Palette {
  fn default() -> Self {
    Self::builtin(true)
  }
}

impl Palette {
  pub(crate) fn builtin(is_dark: bool) -> Self {
    crate::theme::default_theme_variant(is_dark).to_palette(is_dark)
  }

  pub(crate) fn from_seed(seed: ThemeSeed) -> Self {
    let background_is_dark = is_dark(seed.background);
    let bright_pole = if background_is_dark {
      seed.bright_white.unwrap_or(seed.white)
    } else {
      seed.black
    };

    let terminal_ansi_bright_black =
      derive_bright_variant(seed.black, seed.bright_black, bright_pole);
    let terminal_ansi_bright_red = derive_bright_variant(seed.red, seed.bright_red, bright_pole);
    let terminal_ansi_bright_green =
      derive_bright_variant(seed.green, seed.bright_green, bright_pole);
    let terminal_ansi_bright_yellow =
      derive_bright_variant(seed.yellow, seed.bright_yellow, bright_pole);
    let terminal_ansi_bright_blue = derive_bright_variant(seed.blue, seed.bright_blue, bright_pole);
    let terminal_ansi_bright_magenta =
      derive_bright_variant(seed.magenta, seed.bright_magenta, bright_pole);
    let terminal_ansi_bright_cyan = derive_bright_variant(seed.cyan, seed.bright_cyan, bright_pole);
    let terminal_ansi_bright_white =
      derive_bright_variant(seed.white, seed.bright_white, bright_pole);

    let surface_background = blend(seed.background, seed.border, 0.18);
    let elevated_surface_background = blend(seed.background, seed.border, 0.30);
    let element_background = blend(seed.background, seed.border, 0.22);
    let element_hover = blend(element_background, seed.accent, 0.08);
    let element_active = blend(element_background, seed.accent, 0.18);
    let element_selected = blend(seed.background, seed.accent, 0.18);
    let text_muted = blend(seed.foreground, seed.background, 0.35);
    let text_placeholder = blend(seed.foreground, seed.background, 0.55);
    let text_disabled = blend(seed.foreground, seed.background, 0.70);
    let title_bar_background = blend(seed.background, seed.border, 0.28);
    let title_bar_inactive_background = blend(seed.background, seed.border, 0.14);
    let tab_inactive_background = blend(seed.background, seed.border, 0.24);
    let overlay_background =
      seed
        .overlay_background
        .unwrap_or(
          seed
            .black
            .opacity(if background_is_dark { 0.55 } else { 0.45 }),
        );

    Palette {
      border: seed.border,
      border_variant: blend(seed.border, seed.background, 0.45),
      border_focused: blend(seed.accent, bright_pole, 0.18),
      border_selected: blend(seed.border, seed.accent, 0.72),
      border_transparent: seed.background.opacity(0.0),
      border_disabled: blend(seed.border, seed.background, 0.70),
      elevated_surface_background,
      surface_background,
      background: seed.background,
      element_background,
      element_hover,
      element_active,
      element_selected,
      element_selection_background: seed
        .selection_background
        .unwrap_or(seed.accent.opacity(0.28)),
      element_disabled: blend(element_background, seed.background, 0.55),
      text: seed.foreground,
      text_muted,
      text_placeholder,
      text_disabled,
      text_accent: seed.accent,
      title_bar_background,
      title_bar_inactive_background,
      tab_inactive_background,
      tab_active_background: seed.background,
      search_match_background: seed
        .search_match_background
        .unwrap_or(terminal_ansi_bright_yellow.opacity(0.52)),
      search_highlight_background: seed
        .search_highlight_background
        .unwrap_or(blend(seed.accent, seed.background, 0.15).opacity(0.42)),
      overlay_background,
      terminal_background: seed.background,
      terminal_foreground: seed.foreground,
      terminal_bright_foreground: blend(seed.foreground, bright_pole, 0.22),
      terminal_dim_foreground: derive_dim_variant(seed.foreground, seed.background),
      terminal_ansi_background: seed.background,
      terminal_ansi_black: seed.black,
      terminal_ansi_bright_black,
      terminal_ansi_dim_black: derive_dim_variant(seed.black, seed.background),
      terminal_ansi_red: seed.red,
      terminal_ansi_bright_red,
      terminal_ansi_dim_red: derive_dim_variant(seed.red, seed.background),
      terminal_ansi_green: seed.green,
      terminal_ansi_bright_green,
      terminal_ansi_dim_green: derive_dim_variant(seed.green, seed.background),
      terminal_ansi_yellow: seed.yellow,
      terminal_ansi_bright_yellow,
      terminal_ansi_dim_yellow: derive_dim_variant(seed.yellow, seed.background),
      terminal_ansi_blue: seed.blue,
      terminal_ansi_bright_blue,
      terminal_ansi_dim_blue: derive_dim_variant(seed.blue, seed.background),
      terminal_ansi_magenta: seed.magenta,
      terminal_ansi_bright_magenta,
      terminal_ansi_dim_magenta: derive_dim_variant(seed.magenta, seed.background),
      terminal_ansi_cyan: seed.cyan,
      terminal_ansi_bright_cyan,
      terminal_ansi_dim_cyan: derive_dim_variant(seed.cyan, seed.background),
      terminal_ansi_white: seed.white,
      terminal_ansi_bright_white,
      terminal_ansi_dim_white: derive_dim_variant(seed.white, seed.background),
      terminal_cursor: seed.cursor.unwrap_or(seed.accent),
      scrollbar_track_background: blend(seed.background, seed.border, 0.34),
      scrollbar_thumb_background: blend(seed.border, bright_pole, 0.18),
      link_text_hover: blend(seed.accent, bright_pole, 0.16),
    }
  }
}

pub(crate) fn blend(from: Hsla, to: Hsla, amount: f32) -> Hsla {
  let amount = amount.clamp(0.0, 1.0);
  let from = from.to_rgb();
  let to = to.to_rgb();
  Rgba {
    r: from.r + (to.r - from.r) * amount,
    g: from.g + (to.g - from.g) * amount,
    b: from.b + (to.b - from.b) * amount,
    a: from.a + (to.a - from.a) * amount,
  }
  .into()
}

fn derive_bright_variant(base: Hsla, explicit: Option<Hsla>, bright_pole: Hsla) -> Hsla {
  explicit.unwrap_or_else(|| blend(base, bright_pole, 0.22))
}

fn derive_dim_variant(base: Hsla, background: Hsla) -> Hsla {
  blend(base, background, 0.32)
}

fn is_dark(color: Hsla) -> bool {
  let rgb = color.to_rgb();
  let luminance = 0.2126 * rgb.r + 0.7152 * rgb.g + 0.0722 * rgb.b;
  luminance < 0.5
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::to_hex_string;
  use gpui::Rgba;

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

  fn sample_seed() -> ThemeSeed {
    ThemeSeed {
      background: rgb_u8(24, 28, 33),
      foreground: rgb_u8(226, 232, 240),
      accent: rgb_u8(96, 165, 250),
      border: rgb_u8(71, 85, 105),
      black: rgb_u8(15, 23, 42),
      red: rgb_u8(248, 113, 113),
      green: rgb_u8(74, 222, 128),
      yellow: rgb_u8(250, 204, 21),
      blue: rgb_u8(96, 165, 250),
      magenta: rgb_u8(217, 70, 239),
      cyan: rgb_u8(34, 211, 238),
      white: rgb_u8(226, 232, 240),
      bright_black: Some(rgb_u8(100, 116, 139)),
      bright_red: None,
      bright_green: None,
      bright_yellow: None,
      bright_blue: None,
      bright_magenta: None,
      bright_cyan: None,
      bright_white: Some(rgb_u8(248, 250, 252)),
      cursor: None,
      overlay_background: None,
      selection_background: None,
      search_match_background: None,
      search_highlight_background: None,
    }
  }

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
    assert_eq!(
      to_hex_string(&palette.overlay_background.to_rgb()),
      "#282C338C"
    );
  }

  #[test]
  fn light_builtin_palette_uses_light_seed() {
    let palette = Palette::builtin(false);
    assert_eq!(to_hex_string(&palette.background.to_rgb()), "#FAFAFAFF");
    assert_eq!(to_hex_string(&palette.text.to_rgb()), "#383A42FF");
    assert!(palette.surface_background.to_rgb().r < palette.background.to_rgb().r);
  }

  #[test]
  fn palette_seed_overrides_are_respected() {
    let overlay = rgba_u8(10, 20, 30, 120);
    let selection = rgba_u8(90, 100, 110, 130);
    let search_match = rgba_u8(120, 130, 140, 150);
    let search_highlight = rgba_u8(150, 160, 170, 180);
    let cursor = rgb_u8(200, 210, 220);

    let palette = Palette::from_seed(ThemeSeed {
      overlay_background: Some(overlay),
      selection_background: Some(selection),
      search_match_background: Some(search_match),
      search_highlight_background: Some(search_highlight),
      cursor: Some(cursor),
      ..sample_seed()
    });

    assert_eq!(palette.overlay_background, overlay);
    assert_eq!(palette.element_selection_background, selection);
    assert_eq!(palette.search_match_background, search_match);
    assert_eq!(palette.search_highlight_background, search_highlight);
    assert_eq!(palette.terminal_cursor, cursor);
  }
}
