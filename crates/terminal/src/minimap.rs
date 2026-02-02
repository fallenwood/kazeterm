use alacritty_terminal::vte::ansi::{Color, NamedColor};
use gpui::{BorderStyle, Bounds, Hsla, Pixels, Point, Window, fill, px};
use themeing::convert_color;

use crate::indexed_cell::IndexedCell;

/// Minimap width in pixels
pub const MINIMAP_WIDTH: f32 = 80.0;

/// Height per line in the minimap (in pixels)
const MINIMAP_LINE_HEIGHT: f32 = 2.0;

/// Character width in the minimap (in pixels)
const MINIMAP_CHAR_WIDTH: f32 = 1.0;

/// Minimap state for rendering
#[derive(Clone, Debug)]
pub struct MinimapState {
  /// Total number of lines in the terminal (visible + history)
  pub total_lines: usize,
  /// Number of visible lines
  pub visible_lines: usize,
  /// Current scroll offset (0 = bottom, history_size = top)
  pub display_offset: usize,
  /// History size (max scroll offset)
  pub history_size: usize,
}

impl MinimapState {
  pub fn new(
    total_lines: usize,
    visible_lines: usize,
    display_offset: usize,
    history_size: usize,
  ) -> Self {
    Self {
      total_lines,
      visible_lines,
      display_offset,
      history_size,
    }
  }

  /// Calculate the viewport indicator position and size
  pub fn viewport_metrics(&self, minimap_height: Pixels) -> (Pixels, Pixels) {
    if self.total_lines == 0 {
      return (px(0.0), minimap_height);
    }

    let total = self.total_lines as f32;
    let visible = self.visible_lines as f32;

    // Viewport size is the ratio of visible lines to total lines
    let viewport_height_ratio = (visible / total).min(1.0);
    let viewport_height = (minimap_height * viewport_height_ratio).max(px(10.0));

    // Viewport position: display_offset is 0 at bottom, history_size at top
    // We need to invert it for the minimap (0 at top)
    let scroll_ratio = if self.history_size > 0 {
      1.0 - (self.display_offset as f32 / self.history_size as f32)
    } else {
      1.0
    };

    // Position should be adjusted so the viewport stays within bounds
    let viewport_top = scroll_ratio * (minimap_height - viewport_height);

    (viewport_top, viewport_height)
  }

  /// Convert a Y position to a scroll offset
  pub fn position_to_offset(&self, position_ratio: f32) -> usize {
    if self.history_size == 0 {
      return 0;
    }

    // Invert the position (top = history_size, bottom = 0)
    let inverted = 1.0 - position_ratio;
    (inverted * self.history_size as f32).round() as usize
  }
}

/// Render the minimap content
/// This creates a simplified visualization of the terminal content
pub fn paint_minimap(
  bounds: Bounds<Pixels>,
  cells: &[IndexedCell],
  _visible_lines: usize,
  columns: usize,
  state: &MinimapState,
  theme: &themeing::Theme,
  background_color: Hsla,
  viewport_color: Hsla,
  window: &mut Window,
) {
  // Paint background
  window.paint_quad(fill(bounds, background_color));

  // Calculate scale factors
  let minimap_line_height = px(MINIMAP_LINE_HEIGHT);
  let minimap_char_width = px(MINIMAP_CHAR_WIDTH);
  let max_chars_per_line = (bounds.size.width / minimap_char_width).floor() as usize;
  let scale_x = if columns > max_chars_per_line {
    max_chars_per_line as f32 / columns as f32
  } else {
    1.0
  };

  // Group cells by line and paint them
  let mut current_line = i32::MIN;
  let mut line_y = bounds.origin.y;

  for cell in cells {
    let line = cell.point.line.0;

    // Skip if we've exceeded the minimap height
    if line_y >= bounds.origin.y + bounds.size.height {
      break;
    }

    // Move to next line if needed
    if line != current_line {
      current_line = line;
      line_y = bounds.origin.y + px(line as f32 * MINIMAP_LINE_HEIGHT);
    }

    // Skip whitespace and cells outside bounds
    if cell.c == ' ' || cell.c == '\t' {
      continue;
    }

    // Get the cell color
    let fg = if cell
      .flags
      .contains(alacritty_terminal::term::cell::Flags::INVERSE)
    {
      cell.bg
    } else {
      cell.fg
    };

    let color = match fg {
      Color::Named(NamedColor::Foreground) => theme.colors().terminal_foreground,
      _ => convert_color(&fg, theme),
    };

    // Calculate position in minimap
    let col = cell.point.column.0 as f32;
    let x = bounds.origin.x + px(col * scale_x * MINIMAP_CHAR_WIDTH);

    // Paint a small rectangle for the character
    let char_bounds = Bounds {
      origin: Point { x, y: line_y },
      size: gpui::Size {
        width: minimap_char_width,
        height: minimap_line_height,
      },
    };

    // Simplify color for minimap (reduce opacity for less visual noise)
    let minimap_color = color.opacity(0.6);
    window.paint_quad(fill(char_bounds, minimap_color));
  }

  // Paint viewport indicator
  let (viewport_top, viewport_height) = state.viewport_metrics(bounds.size.height);
  let viewport_bounds = Bounds {
    origin: Point {
      x: bounds.origin.x,
      y: bounds.origin.y + viewport_top,
    },
    size: gpui::Size {
      width: bounds.size.width,
      height: viewport_height,
    },
  };

  // Draw viewport border
  window.paint_quad(gpui::quad(
    viewport_bounds,
    px(2.0), // corner radius
    viewport_color.opacity(0.15),
    gpui::Edges::all(px(1.0)),
    viewport_color.opacity(0.5),
    BorderStyle::default(),
  ));
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_minimap_viewport_metrics() {
    // At bottom of scroll (display_offset = 0)
    let state = MinimapState::new(100, 20, 0, 80);
    let (top, height) = state.viewport_metrics(px(200.0));

    // Height should be 20% of minimap height (20/100)
    assert!((height - px(40.0)).abs() < px(1.0));
    // Top should be near the bottom (1.0 - 0.2 = 0.8 * available_space)
    assert!(top > px(100.0));

    // At top of scroll (display_offset = history_size)
    let state = MinimapState::new(100, 20, 80, 80);
    let (top, _height) = state.viewport_metrics(px(200.0));
    assert!(top < px(1.0)); // viewport at top
  }
}
