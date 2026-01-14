use alacritty_terminal::{event::WindowSize, grid::Dimensions};
use gpui::{Bounds, Pixels, Point, Size, px};

const DEBUG_TERMINAL_WIDTH: Pixels = px(500.);
const DEBUG_TERMINAL_HEIGHT: Pixels = px(30.);
const DEBUG_CELL_WIDTH: Pixels = px(5.);
const DEBUG_LINE_HEIGHT: Pixels = px(5.);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalBounds {
  pub cell_width: Pixels,
  pub line_height: Pixels,
  pub bounds: Bounds<Pixels>,
}

impl TerminalBounds {
  pub fn new(line_height: Pixels, cell_width: Pixels, bounds: Bounds<Pixels>) -> Self {
    TerminalBounds {
      cell_width,
      line_height,
      bounds,
    }
  }

  pub fn num_lines(&self) -> usize {
    (self.bounds.size.height / self.line_height).floor() as usize
  }

  pub fn num_columns(&self) -> usize {
    (self.bounds.size.width / self.cell_width).floor() as usize
  }

  pub fn height(&self) -> Pixels {
    self.bounds.size.height
  }

  pub fn width(&self) -> Pixels {
    self.bounds.size.width
  }

  pub fn cell_width(&self) -> Pixels {
    self.cell_width
  }

  pub fn line_height(&self) -> Pixels {
    self.line_height
  }
}

impl Default for TerminalBounds {
  fn default() -> Self {
    TerminalBounds::new(
      DEBUG_LINE_HEIGHT,
      DEBUG_CELL_WIDTH,
      Bounds {
        origin: Point::default(),
        size: Size {
          width: DEBUG_TERMINAL_WIDTH,
          height: DEBUG_TERMINAL_HEIGHT,
        },
      },
    )
  }
}

impl From<TerminalBounds> for WindowSize {
  fn from(val: TerminalBounds) -> Self {
    WindowSize {
      num_lines: val.num_lines() as u16,
      num_cols: val.num_columns() as u16,
      cell_width: f32::from(val.cell_width()) as u16,
      cell_height: f32::from(val.line_height()) as u16,
    }
  }
}

impl Dimensions for TerminalBounds {
  /// Note: this is supposed to be for the back buffer's length,
  /// but we exclusively use it to resize the terminal, which does not
  /// use this method. We still have to implement it for the trait though,
  /// hence, this comment.
  fn total_lines(&self) -> usize {
    self.screen_lines()
  }

  fn screen_lines(&self) -> usize {
    self.num_lines()
  }

  fn columns(&self) -> usize {
    self.num_columns()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn computes_lines_columns_and_converts_to_window_size() {
    let tb = TerminalBounds::new(
      px(2.0),
      px(10.0),
      Bounds {
        origin: Point::default(),
        size: Size {
          width: px(100.0),
          height: px(30.0),
        },
      },
    );

    assert_eq!(tb.num_columns(), 10);
    assert_eq!(tb.num_lines(), 15);

    let ws: WindowSize = tb.into();
    assert_eq!(ws.num_cols, 10);
    assert_eq!(ws.num_lines, 15);
    assert_eq!(ws.cell_width, 10);
    assert_eq!(ws.cell_height, 2);

    assert_eq!(tb.total_lines(), 15);
    assert_eq!(tb.screen_lines(), 15);
    assert_eq!(tb.columns(), 10);
  }
}
