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

impl Default for Palette {
  fn default() -> Self {
    let background = rgb_u8(40, 44, 51);
    let surface_background = rgb_u8(36, 40, 59);
    let elevated_surface_background = rgb_u8(47, 51, 77);
    let element_background = rgb_u8(36, 40, 59);
    let element_hover = rgb_u8(47, 51, 77);
    let element_active = rgb_u8(59, 66, 97);
    let element_selected = rgb_u8(59, 66, 97);
    let element_selection_background = rgba_u8(122, 162, 247, 110);
    let element_disabled = rgb_u8(22, 23, 33);
    let accent = rgb_u8(116, 173, 232);
    let accent_bright = rgb_u8(142, 178, 255);
    let border = rgb_u8(70, 75, 87);
    let border_variant = rgb_u8(36, 40, 59);
    let border_disabled = rgb_u8(31, 35, 52);
    let border_transparent = rgba_u8(0, 0, 0, 0);
    let text = rgb_u8(220, 224, 229);
    let text_muted = rgb_u8(220, 224, 229);
    let text_placeholder = rgb_u8(86, 95, 137);
    let text_disabled = rgb_u8(65, 72, 104);
    let text_accent = accent;
    let title_bar_background = rgb_u8(47, 52, 62);
    let title_bar_inactive_background = rgb_u8(22, 25, 37);
    let tab_inactive_background = rgb_u8(43, 48, 57); // Between title_bar_background and background
    let tab_active_background = background;
    let search_match_background = hsla(30.0 / 360.0, 1.0, 0.5, 0.8);
    let search_highlight_background = hsla(60.0 / 360.0, 1.0, 0.5, 0.6);
    let terminal_background = background;
    let terminal_foreground = text;
    let terminal_bright_foreground = rgb_u8(250, 250, 250);
    let terminal_dim_foreground = text_muted;
    let terminal_ansi_background = background;
    let terminal_ansi_black = rgb_u8(40, 44, 51);
    let terminal_ansi_bright_black = rgb_u8(82, 85, 97);
    let terminal_ansi_dim_black = rgb_u8(15, 16, 22);
    let terminal_ansi_red = rgb_u8(208, 114, 119);
    let terminal_ansi_bright_red = rgb_u8(103, 58, 60);
    let terminal_ansi_dim_red = rgb_u8(179, 86, 103);
    let terminal_ansi_green = rgb_u8(161, 193, 129);
    let terminal_ansi_bright_green = rgb_u8(79, 100, 65);
    let terminal_ansi_dim_green = rgb_u8(114, 149, 78);
    let terminal_ansi_yellow = rgb_u8(222, 193, 132);
    let terminal_ansi_bright_yellow = rgb_u8(229, 192, 123);
    let terminal_ansi_dim_yellow = rgb_u8(163, 127, 75);
    let terminal_ansi_blue = rgb_u8(116, 173, 232);
    let terminal_ansi_bright_blue = rgb_u8(56, 83, 120);
    let terminal_ansi_dim_blue = rgb_u8(89, 118, 179);
    let terminal_ansi_magenta = rgb_u8(180, 119, 207);
    let terminal_ansi_bright_magenta = rgb_u8(214, 180, 228);
    let terminal_ansi_dim_magenta = rgb_u8(136, 112, 179);
    let terminal_ansi_cyan = rgb_u8(110, 180, 191);
    let terminal_ansi_bright_cyan = rgb_u8(58, 86, 91);
    let terminal_ansi_dim_cyan = rgb_u8(90, 149, 184);
    let terminal_ansi_white = rgb_u8(220, 224, 229);
    let terminal_ansi_bright_white = rgb_u8(250, 250, 250);
    let terminal_ansi_dim_white = text_muted;
    let terminal_cursor = accent;
    let scrollbar_track_background = rgb_u8(47, 52, 62);
    let scrollbar_thumb_background = rgb_u8(100, 110, 130);
    let link_text_hover = accent_bright;

    Palette {
      border,
      border_variant,
      border_focused: accent,
      border_selected: accent,
      border_transparent,
      border_disabled,
      elevated_surface_background,
      surface_background,
      background,
      element_background,
      element_hover,
      element_active,
      element_selected,
      element_selection_background,
      element_disabled,
      text,
      text_muted,
      text_placeholder,
      text_disabled,
      text_accent,
      title_bar_background,
      title_bar_inactive_background,
      tab_inactive_background,
      tab_active_background,
      search_match_background,
      search_highlight_background,
      terminal_background,
      terminal_foreground,
      terminal_bright_foreground,
      terminal_dim_foreground,
      terminal_ansi_background,
      terminal_ansi_black,
      terminal_ansi_bright_black,
      terminal_ansi_dim_black,
      terminal_ansi_red,
      terminal_ansi_bright_red,
      terminal_ansi_dim_red,
      terminal_ansi_green,
      terminal_ansi_bright_green,
      terminal_ansi_dim_green,
      terminal_ansi_yellow,
      terminal_ansi_bright_yellow,
      terminal_ansi_dim_yellow,
      terminal_ansi_blue,
      terminal_ansi_bright_blue,
      terminal_ansi_dim_blue,
      terminal_ansi_magenta,
      terminal_ansi_bright_magenta,
      terminal_ansi_dim_magenta,
      terminal_ansi_cyan,
      terminal_ansi_bright_cyan,
      terminal_ansi_dim_cyan,
      terminal_ansi_white,
      terminal_ansi_bright_white,
      terminal_ansi_dim_white,
      terminal_cursor,
      scrollbar_track_background,
      scrollbar_thumb_background,
      link_text_hover,
    }
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
