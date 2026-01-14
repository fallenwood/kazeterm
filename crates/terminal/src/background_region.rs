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

  /// Check if this region can be merged with another region
  pub fn can_merge_with(&self, other: &BackgroundRegion) -> bool {
    if self.color != other.color {
      return false;
    }

    // Check if regions are adjacent horizontally
    if self.start_line == other.start_line && self.end_line == other.end_line {
      return self.end_col + 1 == other.start_col || other.end_col + 1 == self.start_col;
    }

    // Check if regions are adjacent vertically with same column span
    if self.start_col == other.start_col && self.end_col == other.end_col {
      return self.end_line + 1 == other.start_line || other.end_line + 1 == self.start_line;
    }

    false
  }

  /// Merge this region with another region
  pub fn merge_with(&mut self, other: &BackgroundRegion) {
    self.start_line = self.start_line.min(other.start_line);
    self.start_col = self.start_col.min(other.start_col);
    self.end_line = self.end_line.max(other.end_line);
    self.end_col = self.end_col.max(other.end_col);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use gpui::Hsla;

  #[test]
  fn merge_horizontally_and_vertically_when_adjacent_with_same_color() {
    let color = Hsla::black();
    let mut r1 = BackgroundRegion::new(0, 0, color);
    let r2 = BackgroundRegion::new(0, 1, color);
    assert!(r1.can_merge_with(&r2));
    r1.merge_with(&r2);
    assert_eq!(r1.start_col, 0);
    assert_eq!(r1.end_col, 1);

    let mut r3 = BackgroundRegion::new(0, 0, color);
    let r4 = BackgroundRegion::new(1, 0, color);
    assert!(r3.can_merge_with(&r4));
    r3.merge_with(&r4);
    assert_eq!(r3.start_line, 0);
    assert_eq!(r3.end_line, 1);
  }

  #[test]
  fn cannot_merge_different_colors_or_non_adjacent() {
    let color = Hsla::black();
    let other_color = Hsla { a: color.a, ..Hsla::white() };
    let r1 = BackgroundRegion::new(0, 0, color);
    let r2 = BackgroundRegion::new(0, 2, color);
    let r3 = BackgroundRegion::new(0, 1, other_color);
    assert!(!r1.can_merge_with(&r2));
    assert!(!r1.can_merge_with(&r3));
  }
}
