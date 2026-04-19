//! Core ghostty terminal state and `TerminalBackend` adapter.
//!
//! `GhosttyTermInner` stores the grid and metadata using alacritty-compatible
//! types.  It is shared via `Arc<Mutex<…>>` between the event-loop thread
//! (writer) and the UI thread (reader through `GhosttyBackend`).

use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::Mutex;
use terminal_kernel::grid::Scroll;
use terminal_kernel::index::{Boundary, Column, Direction, Line, Point as AlacPoint, Side};
use terminal_kernel::selection::{Selection, SelectionRange, SelectionType};
use terminal_kernel::term::cell::{Cell, Flags as CellFlags};
use terminal_kernel::term::{RenderableCursor, TermMode};
use terminal_kernel::vte::ansi::{CursorShape, CursorStyle, Rgb};
use terminal_kernel::{RenderableSnapshot, SelectionDisplay, TerminalBackend};

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

struct CursorState {
  point: AlacPoint,
  style: CursorStyle,
}

#[derive(Clone, Copy)]
struct SelectionAnchor {
  point: AlacPoint,
  side: Side,
}

pub struct GhosttyTermInner {
  // Primary screen buffer (num_lines rows × num_cols cols).
  pub(crate) rows: Vec<Vec<Cell>>,
  // Scrollback buffer (most recent at the back).
  pub(crate) scrollback: VecDeque<Vec<Cell>>,

  pub(crate) num_lines: usize,
  pub(crate) num_cols: usize,
  pub(crate) max_scrollback: usize,

  cursor: CursorState,
  mode: TermMode,
  pub(crate) display_offset: usize,
  selection: Option<Selection>,
  selection_display: Option<SelectionDisplay>,

  pub(crate) colors: [Option<Rgb>; 256],
}

// ---------------------------------------------------------------------------
// Construction helpers
// ---------------------------------------------------------------------------

fn blank_row(cols: usize) -> Vec<Cell> {
  vec![Cell::default(); cols]
}

fn blank_grid(lines: usize, cols: usize) -> Vec<Vec<Cell>> {
  (0..lines).map(|_| blank_row(cols)).collect()
}

impl GhosttyTermInner {
  pub fn new(
    lines: usize,
    cols: usize,
    max_scrollback: usize,
    event_tx: futures::channel::mpsc::UnboundedSender<terminal_kernel::event::Event>,
  ) -> Self {
    let _ = event_tx; // stored elsewhere; events sent via the event loop
    Self {
      rows: blank_grid(lines, cols),
      scrollback: VecDeque::new(),
      num_lines: lines,
      num_cols: cols,
      max_scrollback,
      cursor: CursorState {
        point: AlacPoint::new(Line(0), Column(0)),
        style: CursorStyle {
          shape: CursorShape::Block,
          blinking: true,
        },
      },
      mode: TermMode::SHOW_CURSOR | TermMode::LINE_WRAP,
      display_offset: 0,
      selection: None,
      selection_display: None,
      colors: [None; 256],
    }
  }

  /// Resize the grid (called from the UI thread via the backend trait).
  pub fn do_resize(&mut self, new_lines: usize, new_cols: usize) {
    if new_lines == 0 || new_cols == 0 {
      return;
    }

    for row in &mut self.rows {
      row.resize(new_cols, Cell::default());
    }
    while self.rows.len() < new_lines {
      self.rows.push(blank_row(new_cols));
    }
    while self.rows.len() > new_lines {
      let removed = self.rows.remove(0);
      self.scrollback.push_back(removed);
      while self.scrollback.len() > self.max_scrollback {
        self.scrollback.pop_front();
      }
    }

    for row in &mut self.scrollback {
      row.resize(new_cols, Cell::default());
    }

    self.num_lines = new_lines;
    self.num_cols = new_cols;

    self.cursor.point.line.0 = self.cursor.point.line.0.min(new_lines as i32 - 1).max(0);
    self.cursor.point.column.0 = self.cursor.point.column.0.min(new_cols.saturating_sub(1));
    self.display_offset = self.display_offset.min(self.scrollback.len());
  }

  /// Replace the visible grid, cursor, colors, and scrollback from a ghostty
  /// render-state snapshot.  Called by the event-loop thread after each
  /// `vt_write` + render-state update cycle.
  pub fn sync_from_ghostty(
    &mut self,
    visible_rows: Vec<Vec<Cell>>,
    cursor_point: AlacPoint,
    cursor_style: CursorStyle,
    mode: TermMode,
    palette: [Option<Rgb>; 256],
    scrollback_delta: Vec<Vec<Cell>>,
  ) {
    // Push newly-scrolled-off rows into our scrollback.
    for row in scrollback_delta {
      self.scrollback.push_back(row);
      while self.scrollback.len() > self.max_scrollback {
        self.scrollback.pop_front();
      }
    }

    self.rows = visible_rows;
    if !self.rows.is_empty() {
      self.num_lines = self.rows.len();
      self.num_cols = self.rows[0].len();
    }
    self.cursor.point = cursor_point;
    self.cursor.style = cursor_style;
    self.mode = mode;
    self.colors = palette;
  }
}

fn cell_at_state(s: &GhosttyTermInner, point: AlacPoint) -> Cell {
  let line = point.line.0;
  let col = point.column.0;

  if line < 0 {
    let sb_idx = s.scrollback.len() as i32 + line;
    if sb_idx >= 0 && (sb_idx as usize) < s.scrollback.len() {
      return s.scrollback[sb_idx as usize]
        .get(col)
        .cloned()
        .unwrap_or_default();
    }
  } else {
    let row_idx = line as usize;
    if row_idx < s.num_lines && col < s.num_cols {
      return s.rows[row_idx][col].clone();
    }
  }

  Cell::default()
}

fn clamp_point_to_grid(s: &GhosttyTermInner, point: AlacPoint) -> AlacPoint {
  let min_line = -(s.scrollback.len() as i32);
  let max_line = s.num_lines as i32 - 1;
  let line = point.line.0.clamp(min_line, max_line);
  let col = point.column.0.min(s.num_cols.saturating_sub(1));
  AlacPoint::new(Line(line), Column(col))
}

fn line_search_left_state(s: &GhosttyTermInner, point: AlacPoint) -> AlacPoint {
  let mut target_line = point.line.0;
  loop {
    let prev = target_line - 1;
    let is_wrapped = if prev < 0 {
      let sb_idx = s.scrollback.len() as i32 + prev;
      if sb_idx >= 0 && (sb_idx as usize) < s.scrollback.len() {
        let row = &s.scrollback[sb_idx as usize];
        row
          .last()
          .is_some_and(|c| c.flags.contains(CellFlags::WRAPLINE))
      } else {
        false
      }
    } else {
      let ri = prev as usize;
      if ri < s.num_lines {
        s.rows[ri]
          .last()
          .is_some_and(|c| c.flags.contains(CellFlags::WRAPLINE))
      } else {
        false
      }
    };

    if is_wrapped {
      target_line = prev;
    } else {
      break;
    }
  }

  AlacPoint::new(Line(target_line), Column(0))
}

fn line_search_right_state(s: &GhosttyTermInner, point: AlacPoint) -> AlacPoint {
  let mut target_line = point.line.0;
  loop {
    let is_wrapped = if target_line < 0 {
      let sb_idx = s.scrollback.len() as i32 + target_line;
      if sb_idx >= 0 && (sb_idx as usize) < s.scrollback.len() {
        let row = &s.scrollback[sb_idx as usize];
        row
          .last()
          .is_some_and(|c| c.flags.contains(CellFlags::WRAPLINE))
      } else {
        false
      }
    } else {
      let ri = target_line as usize;
      if ri < s.num_lines {
        s.rows[ri]
          .last()
          .is_some_and(|c| c.flags.contains(CellFlags::WRAPLINE))
      } else {
        false
      }
    };

    if is_wrapped {
      target_line += 1;
    } else {
      break;
    }
  }

  AlacPoint::new(Line(target_line), Column(s.num_cols.saturating_sub(1)))
}

fn selection_is_empty(ty: SelectionType, start: SelectionAnchor, end: SelectionAnchor) -> bool {
  match ty {
    SelectionType::Simple => {
      (start.point == end.point && start.side == end.side)
        || (start.side == Side::Right
          && end.side == Side::Left
          && start.point.line == end.point.line
          && start.point.column + 1 == end.point.column)
    }
    SelectionType::Block => {
      (start.point.column == end.point.column && start.side == end.side)
        || (start.point.column + 1 == end.point.column
          && start.side == Side::Right
          && end.side == Side::Left)
        || (end.point.column + 1 == start.point.column
          && start.side == Side::Left
          && end.side == Side::Right)
    }
    SelectionType::Semantic | SelectionType::Lines => false,
  }
}

fn selection_range(s: &GhosttyTermInner, selection: SelectionDisplay) -> Option<SelectionRange> {
  let mut start = SelectionAnchor {
    point: selection.start,
    side: selection.start_side,
  };
  let mut end = SelectionAnchor {
    point: selection.end,
    side: selection.end_side,
  };

  if start.point > end.point {
    std::mem::swap(&mut start, &mut end);
  }

  if end.point.line < Line(-(s.scrollback.len() as i32)) {
    return None;
  }

  start.point = clamp_point_to_grid(s, start.point);
  end.point = clamp_point_to_grid(s, end.point);

  match selection.ty {
    SelectionType::Simple | SelectionType::Semantic => {
      if selection_is_empty(SelectionType::Simple, start, end) {
        return None;
      }

      if end.side == Side::Left && start.point != end.point {
        if end.point.column == Column(0) {
          end.point.column = Column(s.num_cols.saturating_sub(1));
          end.point.line -= 1;
        } else {
          end.point.column -= 1;
        }
      }

      if start.side == Side::Right && start.point != end.point {
        start.point.column += 1;
        if start.point.column.0 == s.num_cols {
          start.point.column = Column(0);
          start.point.line += 1;
        }
      }

      (start.point <= end.point).then_some(SelectionRange {
        start: start.point,
        end: end.point,
        is_block: false,
      })
    }
    SelectionType::Block => {
      if selection_is_empty(SelectionType::Block, start, end) {
        return None;
      }

      if start.point.column > end.point.column {
        std::mem::swap(&mut start.side, &mut end.side);
        std::mem::swap(&mut start.point.column, &mut end.point.column);
      }

      if end.side == Side::Left && start.point != end.point && end.point.column.0 > 0 {
        end.point.column -= 1;
      }

      if start.side == Side::Right && start.point != end.point {
        start.point.column += 1;
      }

      Some(SelectionRange {
        start: start.point,
        end: end.point,
        is_block: true,
      })
    }
    SelectionType::Lines => Some(SelectionRange {
      start: line_search_left_state(s, start.point),
      end: line_search_right_state(s, end.point),
      is_block: false,
    }),
  }
}

fn bounds_to_string_state(s: &GhosttyTermInner, start: AlacPoint, end: AlacPoint) -> String {
  let mut result = String::new();

  for line in start.line.0..=end.line.0 {
    let start_col = if line == start.line.0 {
      start.column.0
    } else {
      0
    };
    let end_col = if line == end.line.0 {
      end.column.0
    } else {
      s.num_cols.saturating_sub(1)
    };

    for col in start_col..=end_col {
      let cell = cell_at_state(s, AlacPoint::new(Line(line), Column(col)));
      if !cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
        result.push(cell.c);
      }
    }

    if line != end.line.0 {
      let trimmed = result.trim_end_matches(' ');
      let trimmed_len = trimmed.len();
      result.truncate(trimmed_len);
      result.push('\n');
    }
  }

  result
}

// ---------------------------------------------------------------------------
// GhosttyBackend — TerminalBackend implementation
// ---------------------------------------------------------------------------

pub struct GhosttyBackend {
  state: Arc<Mutex<GhosttyTermInner>>,
}

impl GhosttyBackend {
  pub fn new(state: Arc<Mutex<GhosttyTermInner>>) -> Self {
    Self { state }
  }
}

// SAFETY: parking_lot::Mutex is Send + Sync.
unsafe impl Send for GhosttyBackend {}
unsafe impl Sync for GhosttyBackend {}

impl TerminalBackend for GhosttyBackend {
  fn history_size(&self) -> usize {
    self.state.lock().scrollback.len()
  }

  fn screen_lines(&self) -> usize {
    self.state.lock().num_lines
  }

  fn columns(&self) -> usize {
    self.state.lock().num_cols
  }

  fn topmost_line(&self) -> Line {
    let s = self.state.lock();
    Line(-(s.scrollback.len() as i32))
  }

  fn bottommost_line(&self) -> Line {
    let s = self.state.lock();
    Line(s.num_lines as i32 - 1)
  }

  fn last_column(&self) -> Column {
    let s = self.state.lock();
    Column(s.num_cols.saturating_sub(1))
  }

  fn cell_at(&self, point: AlacPoint) -> Cell {
    let s = self.state.lock();
    cell_at_state(&s, point)
  }

  fn display_offset(&self) -> usize {
    self.state.lock().display_offset
  }

  fn cursor_point(&self) -> AlacPoint {
    self.state.lock().cursor.point
  }

  fn cursor_style(&self) -> CursorStyle {
    self.state.lock().cursor.style
  }

  fn renderable_snapshot(&self) -> RenderableSnapshot {
    let s = self.state.lock();
    let offset = s.display_offset;
    let mut cells = Vec::new();

    for vis_row in 0..s.num_lines {
      let abs_line = vis_row as i32 - offset as i32;
      for col in 0..s.num_cols {
        let cell = if abs_line < 0 {
          let sb_idx = s.scrollback.len() as i32 + abs_line;
          if sb_idx >= 0 && (sb_idx as usize) < s.scrollback.len() {
            s.scrollback[sb_idx as usize]
              .get(col)
              .cloned()
              .unwrap_or_default()
          } else {
            Cell::default()
          }
        } else {
          let row_idx = abs_line as usize;
          if row_idx < s.num_lines {
            s.rows[row_idx].get(col).cloned().unwrap_or_default()
          } else {
            Cell::default()
          }
        };

        let point = AlacPoint::new(Line(abs_line), Column(col));
        cells.push((point, cell));
      }
    }

    let cursor = RenderableCursor {
      shape: s.cursor.style.shape,
      point: s.cursor.point,
    };

    let selection = s
      .selection_display
      .and_then(|selection| selection_range(&s, selection));

    RenderableSnapshot {
      cells,
      mode: s.mode,
      display_offset: offset,
      cursor,
      selection,
    }
  }

  fn color_at(&self, index: usize) -> Option<Rgb> {
    let s = self.state.lock();
    if index < 256 { s.colors[index] } else { None }
  }

  fn selection_to_string(&self) -> Option<String> {
    let s = self.state.lock();
    let selection = s.selection_display?;
    let SelectionRange { start, end, .. } = selection_range(&s, selection)?;

    match selection.ty {
      SelectionType::Block => {
        let mut result = String::new();
        for line in start.line.0..end.line.0 {
          result.push_str(
            bounds_to_string_state(
              &s,
              AlacPoint::new(Line(line), start.column),
              AlacPoint::new(Line(line), end.column),
            )
            .trim_end(),
          );
          result.push('\n');
        }

        result.push_str(
          bounds_to_string_state(&s, AlacPoint::new(end.line, start.column), end).trim_end(),
        );

        Some(result)
      }
      SelectionType::Lines => Some(format!("{}\n", bounds_to_string_state(&s, start, end))),
      SelectionType::Simple | SelectionType::Semantic => {
        Some(bounds_to_string_state(&s, start, end))
      }
    }
  }

  fn bounds_to_string(&self, start: AlacPoint, end: AlacPoint) -> String {
    let s = self.state.lock();
    bounds_to_string_state(&s, start, end)
  }

  fn get_selection(&self) -> Option<Selection> {
    self.state.lock().selection.clone()
  }

  fn set_selection(&self, sel: Option<Selection>) {
    self.state.lock().selection = sel;
  }

  fn take_selection(&self) -> Option<Selection> {
    self.state.lock().selection.take()
  }

  fn update_selection(&self, f: &mut dyn FnMut(&mut Option<Selection>)) {
    let mut s = self.state.lock();
    f(&mut s.selection);
  }

  fn sync_selection_display(&self, selection: Option<SelectionDisplay>) {
    self.state.lock().selection_display = selection;
  }

  fn resize(&self, lines: usize, cols: usize) {
    self.state.lock().do_resize(lines, cols);
  }

  fn scroll_display(&self, scroll: Scroll) {
    let mut s = self.state.lock();
    let max = s.scrollback.len();
    match scroll {
      Scroll::Delta(delta) => {
        let new_offset = s.display_offset as i32 + delta;
        s.display_offset = (new_offset.max(0) as usize).min(max);
      }
      Scroll::PageUp => {
        let page = s.num_lines;
        s.display_offset = (s.display_offset + page).min(max);
      }
      Scroll::PageDown => {
        let page = s.num_lines;
        s.display_offset = s.display_offset.saturating_sub(page);
      }
      Scroll::Top => {
        s.display_offset = max;
      }
      Scroll::Bottom => {
        s.display_offset = 0;
      }
    }
  }

  fn scroll_to_point(&self, point: AlacPoint) {
    let mut s = self.state.lock();
    let line = point.line.0;
    if line < 0 {
      let target_offset = (-line) as usize;
      s.display_offset = target_offset.min(s.scrollback.len());
    } else {
      s.display_offset = 0;
    }
  }

  fn point_add(&self, point: AlacPoint, boundary: Boundary, n: usize) -> AlacPoint {
    let s = self.state.lock();
    let num_cols = s.num_cols;
    let num_lines = s.num_lines;
    let history = s.scrollback.len();

    let (min_line, max_line) = match boundary {
      Boundary::Cursor => (0i32, num_lines as i32 - 1),
      Boundary::Grid => (-(history as i32), num_lines as i32 - 1),
      Boundary::None => (i32::MIN, i32::MAX),
    };

    let mut line = point.line.0;
    let mut col = point.column.0;
    let mut remaining = n;

    while remaining > 0 {
      let cols_left = num_cols.saturating_sub(1) - col;
      if remaining <= cols_left {
        col += remaining;
        remaining = 0;
      } else {
        remaining -= cols_left + 1;
        col = 0;
        line += 1;
        if line > max_line {
          line = max_line;
          col = num_cols.saturating_sub(1);
          break;
        }
      }
    }

    line = line.max(min_line).min(max_line);
    col = col.min(num_cols.saturating_sub(1));
    AlacPoint::new(Line(line), Column(col))
  }

  fn point_sub(&self, point: AlacPoint, boundary: Boundary, n: usize) -> AlacPoint {
    let s = self.state.lock();
    let num_cols = s.num_cols;
    let num_lines = s.num_lines;
    let history = s.scrollback.len();

    let (min_line, _max_line) = match boundary {
      Boundary::Cursor => (0i32, num_lines as i32 - 1),
      Boundary::Grid => (-(history as i32), num_lines as i32 - 1),
      Boundary::None => (i32::MIN, i32::MAX),
    };

    let mut line = point.line.0;
    let mut col = point.column.0;
    let mut remaining = n;

    while remaining > 0 {
      if remaining <= col {
        col -= remaining;
        remaining = 0;
      } else {
        remaining -= col + 1;
        col = num_cols.saturating_sub(1);
        line -= 1;
        if line < min_line {
          line = min_line;
          col = 0;
          break;
        }
      }
    }

    col = col.min(num_cols.saturating_sub(1));
    AlacPoint::new(Line(line), Column(col))
  }

  fn grid_clamp(&self, point: AlacPoint, boundary: Boundary) -> AlacPoint {
    let s = self.state.lock();
    let num_cols = s.num_cols;
    let num_lines = s.num_lines;
    let history = s.scrollback.len();

    let (min_line, max_line) = match boundary {
      Boundary::Cursor => (0i32, num_lines as i32 - 1),
      Boundary::Grid => (-(history as i32), num_lines as i32 - 1),
      Boundary::None => return point,
    };

    let line = point.line.0.max(min_line).min(max_line);
    let col = point.column.0.min(num_cols.saturating_sub(1));
    AlacPoint::new(Line(line), Column(col))
  }

  fn expand_wide(&self, point: AlacPoint, _direction: Direction) -> AlacPoint {
    point
  }

  fn iter_from(&self, start: AlacPoint, f: &mut dyn FnMut(AlacPoint, &Cell) -> bool) {
    let s = self.state.lock();
    let num_cols = s.num_cols;
    let num_lines = s.num_lines;

    let mut line = start.line.0;
    let mut col = start.column.0;

    loop {
      let cell = if line < 0 {
        let sb_idx = s.scrollback.len() as i32 + line;
        if sb_idx < 0 || sb_idx as usize >= s.scrollback.len() {
          break;
        }
        s.scrollback[sb_idx as usize]
          .get(col)
          .cloned()
          .unwrap_or_default()
      } else {
        let row_idx = line as usize;
        if row_idx >= num_lines {
          break;
        }
        if col >= num_cols {
          break;
        }
        s.rows[row_idx][col].clone()
      };

      let point = AlacPoint::new(Line(line), Column(col));
      if !f(point, &cell) {
        break;
      }

      col += 1;
      if col >= num_cols {
        col = 0;
        line += 1;
      }
    }
  }

  fn line_search_left(&self, point: AlacPoint) -> AlacPoint {
    let s = self.state.lock();
    line_search_left_state(&s, point)
  }

  fn line_search_right(&self, point: AlacPoint) -> AlacPoint {
    let s = self.state.lock();
    line_search_right_state(&s, point)
  }

  fn find_hyperlink_at(
    &self,
    _point: AlacPoint,
    _url_regex_pattern: &str,
  ) -> Option<(String, bool, std::ops::RangeInclusive<AlacPoint>)> {
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn populate_row(text: &str, width: usize) -> Vec<Cell> {
    let mut row = vec![Cell::default(); width];
    for (index, ch) in text.chars().enumerate() {
      row[index].c = ch;
    }
    row
  }

  #[test]
  fn renderable_snapshot_exposes_simple_selection() {
    let (event_tx, _event_rx) = futures::channel::mpsc::unbounded();
    let state = Arc::new(Mutex::new(GhosttyTermInner::new(2, 5, 100, event_tx)));

    {
      let mut inner = state.lock();
      inner.rows[0] = populate_row("hello", 5);
      inner.selection_display = Some(SelectionDisplay {
        ty: SelectionType::Simple,
        start: AlacPoint::new(Line(0), Column(1)),
        start_side: Side::Left,
        end: AlacPoint::new(Line(0), Column(4)),
        end_side: Side::Left,
      });
    }

    let backend = GhosttyBackend::new(state);
    let selection = backend.renderable_snapshot().selection.unwrap();

    assert_eq!(selection.start, AlacPoint::new(Line(0), Column(1)));
    assert_eq!(selection.end, AlacPoint::new(Line(0), Column(3)));
  }

  #[test]
  fn selection_to_string_uses_visible_range() {
    let (event_tx, _event_rx) = futures::channel::mpsc::unbounded();
    let state = Arc::new(Mutex::new(GhosttyTermInner::new(2, 5, 100, event_tx)));

    {
      let mut inner = state.lock();
      inner.rows[0] = populate_row("hello", 5);
      inner.selection_display = Some(SelectionDisplay {
        ty: SelectionType::Simple,
        start: AlacPoint::new(Line(0), Column(1)),
        start_side: Side::Left,
        end: AlacPoint::new(Line(0), Column(4)),
        end_side: Side::Left,
      });
    }

    let backend = GhosttyBackend::new(state);

    assert_eq!(backend.selection_to_string().as_deref(), Some("ell"));
  }
}
