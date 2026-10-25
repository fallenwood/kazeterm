use gpui::{Hsla, Rgba};
use terminal_kernel::{
  ANSI_COLOR_COUNT,
  vte::ansi::{Color as AlacColor, Rgb as AlacRgb},
};

//Convenience method to convert from a GPUI color to an alacritty Rgb
pub fn to_alac_rgb(color: impl Into<Rgba>) -> AlacRgb {
  let color = color.into();
  let r = ((color.r * color.a) * 255.) as u8;
  let g = ((color.g * color.a) * 255.) as u8;
  let b = ((color.b * color.a) * 255.) as u8;
  AlacRgb { r, g, b }
}

pub fn resolve_palette_index(
  index: usize,
  theme: &themeing::Theme,
  color_table: &[Option<AlacRgb>; ANSI_COLOR_COUNT],
) -> Hsla {
  color_table[index]
    .map(|rgb| themeing::rgba_color(rgb.r, rgb.g, rgb.b))
    .unwrap_or_else(|| themeing::get_color_at_index(index, theme))
}

pub fn resolve_terminal_color(
  color: &AlacColor,
  theme: &themeing::Theme,
  color_table: &[Option<AlacRgb>; ANSI_COLOR_COUNT],
) -> Hsla {
  match color {
    AlacColor::Named(named) => resolve_palette_index(*named as usize, theme, color_table),
    AlacColor::Indexed(index) => resolve_palette_index(*index as usize, theme, color_table),
    AlacColor::Spec(rgb) => themeing::rgba_color(rgb.r, rgb.g, rgb.b),
  }
}

#[cfg(test)]
mod tests {
  use config::Palette;
  use gpui::SharedString;
  use terminal_kernel::{
    FOREGROUND_COLOR_INDEX,
    vte::ansi::{Color as AlacColor, NamedColor, Rgb as AlacRgb},
  };

  use super::{resolve_palette_index, resolve_terminal_color};

  fn test_theme() -> themeing::Theme {
    themeing::Theme {
      id: "test".to_string(),
      name: SharedString::from("Test"),
      styles: themeing::ThemeStyles {
        colors: Palette::default(),
      },
    }
  }

  #[test]
  fn resolve_palette_index_prefers_backend_override() {
    let theme = test_theme();
    let mut color_table = [None; terminal_kernel::ANSI_COLOR_COUNT];
    color_table[FOREGROUND_COLOR_INDEX] = Some(AlacRgb { r: 1, g: 2, b: 3 });

    assert_eq!(
      resolve_palette_index(FOREGROUND_COLOR_INDEX, &theme, &color_table),
      themeing::rgba_color(1, 2, 3),
    );
  }

  #[test]
  fn resolve_terminal_color_uses_dynamic_named_and_indexed_colors() {
    let theme = test_theme();
    let mut color_table = [None; terminal_kernel::ANSI_COLOR_COUNT];
    color_table[4] = Some(AlacRgb {
      r: 0x11,
      g: 0x22,
      b: 0x33,
    });
    color_table[FOREGROUND_COLOR_INDEX] = Some(AlacRgb {
      r: 0xaa,
      g: 0xbb,
      b: 0xcc,
    });

    assert_eq!(
      resolve_terminal_color(&AlacColor::Indexed(4), &theme, &color_table),
      themeing::rgba_color(0x11, 0x22, 0x33),
    );
    assert_eq!(
      resolve_terminal_color(
        &AlacColor::Named(NamedColor::Foreground),
        &theme,
        &color_table,
      ),
      themeing::rgba_color(0xaa, 0xbb, 0xcc),
    );
  }
}
