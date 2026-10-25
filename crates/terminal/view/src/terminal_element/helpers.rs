use std::collections::HashMap;
use std::ops::RangeInclusive;

use gpui::{Pixels, Point};
use terminal_kernel::index::Point as AlacPoint;

use crate::{
  background_region::BackgroundRegion, highlighted_range_line::HighlightedRangeLine,
  indexed_cell::IndexedCell,
};

use super::LayoutState;

/// Merge background regions to minimize the number of rectangles.
pub(super) fn merge_background_regions(regions: Vec<BackgroundRegion>) -> Vec<BackgroundRegion> {
  // The renderer emits regions in row-major order. Merge horizontal runs in
  // one pass before indexing the previous row for vertical extension.
  let mut horizontal = Vec::<BackgroundRegion>::with_capacity(regions.len());
  for region in regions {
    if let Some(previous) = horizontal.last_mut()
      && previous.start_line == region.start_line
      && previous.end_line == region.end_line
      && previous.end_col + 1 == region.start_col
      && previous.color == region.color
    {
      previous.end_col = region.end_col;
    } else {
      horizontal.push(region);
    }
  }

  let mut merged = Vec::<BackgroundRegion>::with_capacity(horizontal.len());
  let mut previous_row = HashMap::<(i32, i32, gpui::Hsla), usize>::new();
  let mut current_row = HashMap::<(i32, i32, gpui::Hsla), usize>::new();
  let mut current_line = None;

  for region in horizontal {
    if current_line != Some(region.start_line) {
      if let Some(previous_line) = current_line {
        previous_row = std::mem::take(&mut current_row);
        if region.start_line != previous_line + 1 {
          previous_row.clear();
        }
      }
      current_line = Some(region.start_line);
    }

    let key = (region.start_col, region.end_col, region.color);
    if let Some(&merged_index) = previous_row.get(&key) {
      merged[merged_index].end_line = region.end_line;
      current_row.insert(key, merged_index);
    } else {
      let merged_index = merged.len();
      merged.push(region);
      current_row.insert(key, merged_index);
    }
  }

  merged
}

pub(crate) fn is_blank(cell: &IndexedCell) -> bool {
  if cell.c != ' ' {
    return false;
  }

  if !terminal_kernel::is_default_background(&cell.bg) {
    return false;
  }

  if cell.hyperlink().is_some() {
    return false;
  }

  if cell.flags.intersects(
    terminal_kernel::term::cell::Flags::ALL_UNDERLINES
      | terminal_kernel::term::cell::Flags::INVERSE
      | terminal_kernel::term::cell::Flags::STRIKEOUT,
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

#[cfg(test)]
mod tests {
  use gpui::Hsla;

  use super::merge_background_regions;
  use crate::background_region::BackgroundRegion;

  #[test]
  fn merges_row_major_regions_horizontally_then_vertically() {
    let color = Hsla::black();
    let regions = vec![
      BackgroundRegion::new(0, 0, color),
      BackgroundRegion::new(0, 1, color),
      BackgroundRegion::new(1, 0, color),
      BackgroundRegion::new(1, 1, color),
    ];

    let merged = merge_background_regions(regions);

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].start_line, 0);
    assert_eq!(merged[0].end_line, 1);
    assert_eq!(merged[0].start_col, 0);
    assert_eq!(merged[0].end_col, 1);
  }

  #[test]
  fn preserves_color_boundaries_and_line_gaps() {
    let black = Hsla::black();
    let white = Hsla::white();
    let regions = vec![
      BackgroundRegion::new(0, 0, black),
      BackgroundRegion::new(0, 1, white),
      BackgroundRegion::new(2, 0, black),
    ];

    let merged = merge_background_regions(regions);

    assert_eq!(merged.len(), 3);
  }

  #[test]
  fn merges_long_vertical_runs_without_repeated_scans() {
    let color = Hsla::black();
    let regions = (0..1_000)
      .map(|line| BackgroundRegion::new(line, 4, color))
      .collect();

    let merged = merge_background_regions(regions);

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].start_line, 0);
    assert_eq!(merged[0].end_line, 999);
  }
}
