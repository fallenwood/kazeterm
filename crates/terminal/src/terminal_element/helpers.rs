use std::ops::RangeInclusive;

use alacritty_terminal::{
  index::Point as AlacPoint,
  vte::ansi::{Color, NamedColor},
};
use gpui::{Pixels, Point};

use crate::{
  background_region::BackgroundRegion, highlighted_range_line::HighlightedRangeLine,
  indexed_cell::IndexedCell,
};

use super::LayoutState;

/// Merge background regions to minimize the number of rectangles.
pub(super) fn merge_background_regions(regions: Vec<BackgroundRegion>) -> Vec<BackgroundRegion> {
  if regions.is_empty() {
    return regions;
  }

  let mut merged = regions;
  let mut changed = true;

  while changed {
    changed = false;
    let mut i = 0;

    while i < merged.len() {
      let mut j = i + 1;
      while j < merged.len() {
        if merged[i].can_merge_with(&merged[j]) {
          let other = merged.remove(j);
          merged[i].merge_with(&other);
          changed = true;
        } else {
          j += 1;
        }
      }
      i += 1;
    }
  }

  merged
}

pub(crate) fn is_blank(cell: &IndexedCell) -> bool {
  if cell.c != ' ' {
    return false;
  }

  if cell.bg != Color::Named(NamedColor::Background) {
    return false;
  }

  if cell.hyperlink().is_some() {
    return false;
  }

  if cell.flags.intersects(
    alacritty_terminal::term::cell::Flags::ALL_UNDERLINES
      | alacritty_terminal::term::cell::Flags::INVERSE
      | alacritty_terminal::term::cell::Flags::STRIKEOUT,
  ) {
    return false;
  }

  true
}

/// Helper struct for converting data between Alacritty's cursor points, and displayed cursor points.
pub(super) struct DisplayCursor {
  line: i32,
  col: usize,
}

impl DisplayCursor {
  pub fn from(cursor_point: AlacPoint, display_offset: usize) -> Self {
    Self {
      line: cursor_point.line.0 + display_offset as i32,
      col: cursor_point.column.0,
    }
  }

  pub fn line(&self) -> i32 {
    self.line
  }

  pub fn col(&self) -> usize {
    self.col
  }
}

pub(super) fn to_highlighted_range_lines(
  range: &RangeInclusive<AlacPoint>,
  layout: &LayoutState,
  origin: Point<Pixels>,
) -> Option<(Pixels, Vec<HighlightedRangeLine>)> {
  let unclamped_start = AlacPoint::new(
    range.start().line + layout.display_offset,
    range.start().column,
  );
  let unclamped_end = AlacPoint::new(range.end().line + layout.display_offset, range.end().column);

  if unclamped_end.line.0 < 0 || unclamped_start.line.0 > layout.dimensions.num_lines() as i32 {
    return None;
  }

  let clamped_start_line = unclamped_start.line.0.max(0) as usize;
  let clamped_end_line = unclamped_end
    .line
    .0
    .min(layout.dimensions.num_lines() as i32) as usize;
  let start_y = origin.y + clamped_start_line as f32 * layout.dimensions.line_height;

  let mut highlighted_range_lines = Vec::new();
  for line in clamped_start_line..=clamped_end_line {
    let mut line_start = 0;
    let mut line_end = layout.dimensions.num_columns();

    if line == clamped_start_line {
      line_start = unclamped_start.column.0;
    }
    if line == clamped_end_line {
      line_end = unclamped_end.column.0 + 1;
    }

    highlighted_range_lines.push(HighlightedRangeLine {
      start_x: origin.x + line_start as f32 * layout.dimensions.cell_width,
      end_x: origin.x + line_end as f32 * layout.dimensions.cell_width,
    });
  }

  Some((start_y, highlighted_range_lines))
}

pub(super) fn is_decorative_character(ch: char) -> bool {
  matches!(
      ch as u32,
      0x2500..=0x257F
      | 0x2580..=0x259F
      | 0x25A0..=0x25FF
      | 0xE0B0..=0xE0B7
      | 0xE0B8..=0xE0BF
      | 0xE0C0..=0xE0CA
      | 0xE0CC..=0xE0D1
      | 0xE0D2..=0xE0D7
  )
}
