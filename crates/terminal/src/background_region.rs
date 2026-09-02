use gpui::Hsla;

/// Represents a rectangular region with a specific background color
#[derive(Debug, Clone)]
pub struct BackgroundRegion {
  pub start_line: i32,
  pub start_col: i32,
  pub end_line: i32,
  pub end_col: i32,
  pub color: Hsla,
}

impl BackgroundRegion {
  pub fn new(line: i32, col: i32, color: Hsla) -> Self {
    BackgroundRegion {
      start_line: line,
      start_col: col,
      end_line: line,
      end_col: col,
      color,
    }
  }
}
