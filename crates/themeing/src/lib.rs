use std::sync::Arc;

use alacritty_terminal::vte::ansi::{Color, NamedColor};
use config::Palette;
use gpui::{App, Global, Hsla, SharedString, UpdateGlobal};

/// Zoom state for terminal font size adjustment
#[derive(Clone, Debug)]
pub struct ZoomState {
  /// The current zoom level as a multiplier (1.0 = 100%, 1.1 = 110%, etc.)
  pub zoom_level: f32,
}

impl Default for ZoomState {
  fn default() -> Self {
    Self { zoom_level: 1.0 }
  }
}

impl ZoomState {
  const MIN_ZOOM: f32 = 0.5;
  const MAX_ZOOM: f32 = 3.0;
  const ZOOM_STEP: f32 = 0.1;

  pub fn zoom_in(&mut self) {
    self.zoom_level = (self.zoom_level + Self::ZOOM_STEP).min(Self::MAX_ZOOM);
  }

  pub fn zoom_out(&mut self) {
    self.zoom_level = (self.zoom_level - Self::ZOOM_STEP).max(Self::MIN_ZOOM);
  }

  pub fn reset(&mut self) {
    self.zoom_level = 1.0;
  }

  pub fn effective_font_size(&self, base_font_size: f32) -> f32 {
    base_font_size * self.zoom_level
  }
}

impl Global for ZoomState {}

mod defaults;
pub use defaults::*;

/// A theme is the primary mechanism for defining the appearance of the UI.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
  /// The unique identifier for the theme.
  pub id: String,
  /// The name of the theme.
  pub name: SharedString,
  /// The colors and other styles for the theme.
  pub styles: ThemeStyles,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThemeStyles {
  pub colors: Palette,
}

impl Theme {
  #[inline(always)]
  pub fn colors(&self) -> &Palette {
    &self.styles.colors
  }
}

/// Converts a 2, 8, or 24 bit color ANSI color to the GPUI equivalent.
pub fn convert_color(fg: &Color, theme: &Theme) -> Hsla {
  let colors = theme.colors();
  match fg {
    // Named and theme defined colors
    Color::Named(n) => match n {
      NamedColor::Black => colors.terminal_ansi_black,
      NamedColor::Red => colors.terminal_ansi_red,
      NamedColor::Green => colors.terminal_ansi_green,
      NamedColor::Yellow => colors.terminal_ansi_yellow,
      NamedColor::Blue => colors.terminal_ansi_blue,
      NamedColor::Magenta => colors.terminal_ansi_magenta,
      NamedColor::Cyan => colors.terminal_ansi_cyan,
      NamedColor::White => colors.terminal_ansi_white,
      NamedColor::BrightBlack => colors.terminal_ansi_bright_black,
      NamedColor::BrightRed => colors.terminal_ansi_bright_red,
      NamedColor::BrightGreen => colors.terminal_ansi_bright_green,
      NamedColor::BrightYellow => colors.terminal_ansi_bright_yellow,
      NamedColor::BrightBlue => colors.terminal_ansi_bright_blue,
      NamedColor::BrightMagenta => colors.terminal_ansi_bright_magenta,
      NamedColor::BrightCyan => colors.terminal_ansi_bright_cyan,
      NamedColor::BrightWhite => colors.terminal_ansi_bright_white,
      NamedColor::Foreground => colors.terminal_foreground,
      NamedColor::Background => colors.terminal_ansi_background,
      NamedColor::Cursor => colors.terminal_cursor,
      NamedColor::DimBlack => colors.terminal_ansi_dim_black,
      NamedColor::DimRed => colors.terminal_ansi_dim_red,
      NamedColor::DimGreen => colors.terminal_ansi_dim_green,
      NamedColor::DimYellow => colors.terminal_ansi_dim_yellow,
      NamedColor::DimBlue => colors.terminal_ansi_dim_blue,
      NamedColor::DimMagenta => colors.terminal_ansi_dim_magenta,
      NamedColor::DimCyan => colors.terminal_ansi_dim_cyan,
      NamedColor::DimWhite => colors.terminal_ansi_dim_white,
      NamedColor::BrightForeground => colors.terminal_bright_foreground,
      NamedColor::DimForeground => colors.terminal_dim_foreground,
    },
    // 'True' colors
    Color::Spec(rgb) => rgba_color(rgb.r, rgb.g, rgb.b),
    // 8 bit, indexed colors
    Color::Indexed(i) => get_color_at_index(*i as usize, theme),
  }
}

pub fn rgba_color(r: u8, g: u8, b: u8) -> Hsla {
  gpui::Rgba {
    r: (r as f32 / 255.),
    g: (g as f32 / 255.),
    b: (b as f32 / 255.),
    a: 1.,
  }
  .into()
}

/// Converts an 8 bit ANSI color to its GPUI equivalent.
/// Accepts `usize` for compatibility with the `alacritty::Colors` interface,
/// Other than that use case, should only be called with values in the `[0,255]` range
pub fn get_color_at_index(index: usize, theme: &Theme) -> Hsla {
  let colors = theme.colors();

  match index {
    // 0-15 are the same as the named colors above
    0 => colors.terminal_ansi_black,
    1 => colors.terminal_ansi_red,
    2 => colors.terminal_ansi_green,
    3 => colors.terminal_ansi_yellow,
    4 => colors.terminal_ansi_blue,
    5 => colors.terminal_ansi_magenta,
    6 => colors.terminal_ansi_cyan,
    7 => colors.terminal_ansi_white,
    8 => colors.terminal_ansi_bright_black,
    9 => colors.terminal_ansi_bright_red,
    10 => colors.terminal_ansi_bright_green,
    11 => colors.terminal_ansi_bright_yellow,
    12 => colors.terminal_ansi_bright_blue,
    13 => colors.terminal_ansi_bright_magenta,
    14 => colors.terminal_ansi_bright_cyan,
    15 => colors.terminal_ansi_bright_white,
    // 16-231 are a 6x6x6 RGB color cube, mapped to 0-255 using steps defined by XTerm.
    // See: https://github.com/xterm-x11/xterm-snapshots/blob/master/256colres.pl
    16..=231 => {
      let (r, g, b) = rgb_for_index(index as u8);
      rgba_color(
        if r == 0 { 0 } else { r * 40 + 55 },
        if g == 0 { 0 } else { g * 40 + 55 },
        if b == 0 { 0 } else { b * 40 + 55 },
      )
    }
    // 232-255 are a 24-step grayscale ramp from (8, 8, 8) to (238, 238, 238).
    232..=255 => {
      let i = index as u8 - 232; // Align index to 0..24
      let value = i * 10 + 8;
      rgba_color(value, value, value)
    }
    // For compatibility with the alacritty::Colors interface
    // See: https://github.com/alacritty/alacritty/blob/master/alacritty_terminal/src/term/color.rs
    256 => colors.terminal_foreground,
    257 => colors.terminal_background,
    258 => colors.terminal_cursor,
    259 => colors.terminal_ansi_dim_black,
    260 => colors.terminal_ansi_dim_red,
    261 => colors.terminal_ansi_dim_green,
    262 => colors.terminal_ansi_dim_yellow,
    263 => colors.terminal_ansi_dim_blue,
    264 => colors.terminal_ansi_dim_magenta,
    265 => colors.terminal_ansi_dim_cyan,
    266 => colors.terminal_ansi_dim_white,
    267 => colors.terminal_bright_foreground,
    268 => colors.terminal_ansi_black, // 'Dim Background', non-standard color

    _ => Hsla::black(),
  }
}

/// Generates the RGB channels in [0, 5] for a given index into the 6x6x6 ANSI color cube.
///
/// See: [8 bit ANSI color](https://en.wikipedia.org/wiki/ANSI_escape_code#8-bit).
///
/// Wikipedia gives a formula for calculating the index for a given color:
///
/// ```text
/// index = 16 + 36 × r + 6 × g + b (0 ≤ r, g, b ≤ 5)
/// ```
///
/// This function does the reverse, calculating the `r`, `g`, and `b` components from a given index.
pub fn rgb_for_index(i: u8) -> (u8, u8, u8) {
  debug_assert!((16..=231).contains(&i));
  let i = i - 16;
  let r = (i - (i % 36)) / 36;
  let g = ((i % 36) - (i % 6)) / 6;
  let b = (i % 36) % 6;
  (r, g, b)
}

/// Implementing this trait allows accessing the active theme.
pub trait ActiveTheme {
  /// Returns the active theme.
  fn theme(&self) -> &Arc<Theme>;
}

pub struct SettingsStore {
  pub active_theme: Arc<Theme>,
  /// Whether the current theme is using dark mode
  pub is_dark: bool,
  /// Whether the theme mode is set to System
  pub is_system: bool,
}

impl SettingsStore {
  pub fn theme(&self) -> &Arc<Theme> {
    &self.active_theme
  }

  pub fn init_gpui_component_theme(cx: &mut App) {
    gpui_component::Theme::update_global(cx, |theme, app| {
      let settings = app.global::<SettingsStore>();

      let colors = settings.theme().colors();

      theme.accent = colors.text_accent;
      theme.accent_foreground = colors.terminal_foreground;
      theme.accordion = colors.element_background;
      theme.accordion_hover = colors.element_hover;

      theme.blue = colors.terminal_ansi_blue;
      theme.blue_light = colors.terminal_ansi_bright_blue;
      theme.cyan = colors.terminal_ansi_cyan;
      theme.cyan_light = colors.terminal_ansi_bright_cyan;
      theme.green = colors.terminal_ansi_green;
      theme.green_light = colors.terminal_ansi_bright_green;
      theme.red = colors.terminal_ansi_red;
      theme.red_light = colors.terminal_ansi_bright_red;
      theme.yellow = colors.terminal_ansi_yellow;
      theme.yellow_light = colors.terminal_ansi_bright_yellow;
      theme.magenta = colors.terminal_ansi_magenta;
      theme.magenta_light = colors.terminal_ansi_bright_magenta;

      theme.primary = colors.text_accent;
      theme.primary_hover = colors.element_hover;
      theme.primary_active = colors.element_active;

      theme.title_bar = colors.title_bar_background;
      theme.title_bar_border = colors.border;

      theme.tab = colors.tab_inactive_background;
      theme.tab_active = colors.tab_active_background;
      theme.tab_active_foreground = colors.text;
      theme.tab_foreground = colors.text;

      // Input styling
      theme.input = colors.surface_background;

      // Border colors
      theme.border = colors.border;

      // Background and foreground
      theme.background = colors.background;
      theme.foreground = colors.text;
      theme.muted = colors.text_muted;
      theme.muted_foreground = colors.text_muted;

      // Caret color (same as text for consistency)
      theme.caret = colors.text;

      // Popup/dropdown menu styling
      theme.popover = colors.elevated_surface_background;
      theme.popover_foreground = colors.text;

      // List styling (for dropdown items, menus)
      theme.list = colors.elevated_surface_background;
      theme.list_active = colors.element_selected;
      theme.list_active_border = colors.border_focused;
      theme.list_hover = colors.element_hover;

      // Secondary colors
      theme.secondary = colors.element_background;
      theme.secondary_hover = colors.element_hover;
      theme.secondary_active = colors.element_active;
      theme.secondary_foreground = colors.text;

      // Selection
      theme.selection = colors.element_selection_background;

      let config = app.global::<config::Config>();
      theme.font_family = config.ui_font_family.clone().into();
      theme.font_size = gpui::px(config.font_size);
      theme.mono_font_family = config.font_family.clone().into();
      theme.mono_font_size = gpui::px(config.font_size);
    });
  }
}

impl Global for SettingsStore {}

impl ActiveTheme for App {
  fn theme(&self) -> &Arc<Theme> {
    self.global::<SettingsStore>().theme()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};

  fn assert_rgb(color: Hsla, r: u8, g: u8, b: u8) {
    let rgba = color.to_rgb();
    assert!((rgba.r - r as f32 / 255.0).abs() < 1e-6, "r = {}", rgba.r);
    assert!((rgba.g - g as f32 / 255.0).abs() < 1e-6, "g = {}", rgba.g);
    assert!((rgba.b - b as f32 / 255.0).abs() < 1e-6, "b = {}", rgba.b);
  }

  #[test]
  fn zoom_state_behaves_within_bounds() {
    let mut z = ZoomState::default();
    assert_eq!(z.zoom_level, 1.0);

    z.zoom_in();
    assert_eq!(z.zoom_level, 1.1);

    for _ in 0..50 {
      z.zoom_in();
    }
    assert_eq!(z.zoom_level, ZoomState::MAX_ZOOM);

    z.reset();
    assert_eq!(z.zoom_level, 1.0);

    z.zoom_out();
    assert_eq!(z.zoom_level, 0.9);

    for _ in 0..50 {
      z.zoom_out();
    }
    assert_eq!(z.zoom_level, ZoomState::MIN_ZOOM);

    assert_eq!(z.effective_font_size(12.0), 12.0 * ZoomState::MIN_ZOOM);
  }

  #[test]
  fn rgb_for_index_inverts_formula() {
    // r=1,g=2,b=3
    let index = 16 + 36 * 1 + 6 * 2 + 3;
    assert_eq!(rgb_for_index(index as u8), (1, 2, 3));

    // extremes
    assert_eq!(rgb_for_index(16), (0, 0, 0));
    assert_eq!(rgb_for_index(231), (5, 5, 5));
  }

  #[test]
  fn convert_color_maps_named_indexed_and_spec() {
    let settings = default_settings();
    let theme = settings.active_theme;
    let colors = theme.colors();

    // Named
    assert_eq!(
      convert_color(&Color::Named(NamedColor::Red), &theme),
      colors.terminal_ansi_red
    );
    assert_eq!(
      convert_color(&Color::Named(NamedColor::BrightBlue), &theme),
      colors.terminal_ansi_bright_blue
    );
    assert_eq!(
      convert_color(&Color::Named(NamedColor::Cursor), &theme),
      colors.terminal_cursor
    );

    // Indexed
    assert_eq!(
      convert_color(&Color::Indexed(0), &theme),
      colors.terminal_ansi_black
    );
    assert_eq!(
      convert_color(&Color::Indexed(9), &theme),
      colors.terminal_ansi_bright_red
    );

    // 6x6x6 cube
    let c = convert_color(&Color::Indexed(16 + 36 * 1 + 6 * 2 + 3), &theme);
    // r=1,g=2,b=3 -> (95, 135, 175)
    assert_rgb(c, 95, 135, 175);

    // Spec
    let spec = convert_color(
      &Color::Spec(Rgb {
        r: 10,
        g: 20,
        b: 30,
      }),
      &theme,
    );
    assert_rgb(spec, 10, 20, 30);
  }
}
