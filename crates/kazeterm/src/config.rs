use config::Config;

/// Creates a SettingsStore from the config, applying theme customizations
pub fn create_settings_store(config: &Config) -> themeing::SettingsStore {
  use gpui::{Hsla, Rgba, SharedString};
  use std::sync::Arc;

  // Start with default settings
  let mut settings = themeing::default_settings();

  // If there's a theme config with colors, apply them
  let colors_config = &config.theme_config.colors;

  // Helper function to parse hex color string to Hsla
  fn parse_hex_color(hex: &str) -> Option<Hsla> {
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

  // Apply theme customizations
  macro_rules! apply_color {
    ($field:ident, $config_field:expr) => {
      if let Some(ref color_str) = $config_field {
        if let Some(color) = parse_hex_color(color_str) {
          // We need to get mutable access to the theme colors
          let theme = Arc::make_mut(&mut settings.active_theme);
          theme.styles.colors.$field = color;
        }
      }
    };
  }

  // Terminal colors
  apply_color!(terminal_background, colors_config.terminal_background);
  apply_color!(terminal_foreground, colors_config.terminal_foreground);
  apply_color!(terminal_cursor, colors_config.terminal_cursor);
  apply_color!(terminal_ansi_black, colors_config.terminal_ansi_black);
  apply_color!(terminal_ansi_red, colors_config.terminal_ansi_red);
  apply_color!(terminal_ansi_green, colors_config.terminal_ansi_green);
  apply_color!(terminal_ansi_yellow, colors_config.terminal_ansi_yellow);
  apply_color!(terminal_ansi_blue, colors_config.terminal_ansi_blue);
  apply_color!(terminal_ansi_magenta, colors_config.terminal_ansi_magenta);
  apply_color!(terminal_ansi_cyan, colors_config.terminal_ansi_cyan);
  apply_color!(terminal_ansi_white, colors_config.terminal_ansi_white);
  apply_color!(
    terminal_ansi_bright_black,
    colors_config.terminal_ansi_bright_black
  );
  apply_color!(
    terminal_ansi_bright_red,
    colors_config.terminal_ansi_bright_red
  );
  apply_color!(
    terminal_ansi_bright_green,
    colors_config.terminal_ansi_bright_green
  );
  apply_color!(
    terminal_ansi_bright_yellow,
    colors_config.terminal_ansi_bright_yellow
  );
  apply_color!(
    terminal_ansi_bright_blue,
    colors_config.terminal_ansi_bright_blue
  );
  apply_color!(
    terminal_ansi_bright_magenta,
    colors_config.terminal_ansi_bright_magenta
  );
  apply_color!(
    terminal_ansi_bright_cyan,
    colors_config.terminal_ansi_bright_cyan
  );
  apply_color!(
    terminal_ansi_bright_white,
    colors_config.terminal_ansi_bright_white
  );

  // UI colors
  apply_color!(background, colors_config.background);
  apply_color!(surface_background, colors_config.surface_background);
  apply_color!(text, colors_config.text);
  apply_color!(text_muted, colors_config.text_muted);
  apply_color!(border, colors_config.border);
  apply_color!(tab_active_background, colors_config.tab_active_background);
  apply_color!(
    tab_inactive_background,
    colors_config.tab_inactive_background
  );
  apply_color!(title_bar_background, colors_config.title_bar_background);

  // Update theme name if provided
  if let Some(ref name) = config.theme_config.name {
    let theme = Arc::make_mut(&mut settings.active_theme);
    theme.name = SharedString::from(name.clone());
  }

  settings
}
