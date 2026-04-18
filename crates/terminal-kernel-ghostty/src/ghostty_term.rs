//! Core ghostty terminal state and `TerminalBackend` adapter.
//!
//! `GhosttyTermInner` stores the grid and metadata using alacritty-compatible
//! types.  It is shared via `Arc<Mutex<…>>` between the event-loop thread
//! (writer) and the UI thread (reader through `GhosttyBackend`).

use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::Mutex;
use terminal_kernel::grid::Scroll;
use terminal_kernel::index::{Boundary, Column, Direction, Line, Point as AlacPoint};
use terminal_kernel::selection::Selection;
use terminal_kernel::term::cell::{Cell, Flags as CellFlags};
use terminal_kernel::term::{RenderableCursor, TermMode};
use terminal_kernel::vte::ansi::{CursorShape, CursorStyle, Rgb};
use terminal_kernel::{RenderableSnapshot, TerminalBackend};

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

struct CursorState {
  point: AlacPoint,
  style: CursorStyle,
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
    let line = point.line.0;
    let col = point.column.0;

    if line < 0 {
      let sb_idx = s.scrollback.len() as i32 + line;
      if sb_idx >= 0 && (sb_idx as usize) < s.scrollback.len() {
        let row = &s.scrollback[sb_idx as usize];
        if col < row.len() {
          return row[col].clone();
        }
      }
    } else {
      let row_idx = line as usize;
      if row_idx < s.num_lines && col < s.num_cols {
        return s.rows[row_idx][col].clone();
      }
    }
    Cell::default()
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
      .selection
      .as_ref()
      .map(|_| {
        // Selection range approximation is not precise without Term —
        // return None for now.
        None::<terminal_kernel::selection::SelectionRange>
      })
      .flatten();

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
    let _sel = s.selection.as_ref()?;
    None
  }

  fn bounds_to_string(&self, start: AlacPoint, end: AlacPoint) -> String {
    let s = self.state.lock();
    let mut result = String::new();
    let start_line = start.line.0;
    let end_line = end.line.0;

    for line in start_line..=end_line {
      let start_col = if line == start_line {
        start.column.0
      } else {
        0
      };
      let end_col = if line == end_line {
        end.column.0
      } else {
        s.num_cols.saturating_sub(1)
      };

      for col in start_col..=end_col {
        let cell = if line < 0 {
          let sb_idx = s.scrollback.len() as i32 + line;
          if sb_idx >= 0 && (sb_idx as usize) < s.scrollback.len() {
            s.scrollback[sb_idx as usize]
              .get(col)
              .cloned()
              .unwrap_or_default()
          } else {
            Cell::default()
          }
        } else {
          let row_idx = line as usize;
          if row_idx < s.num_lines && col < s.num_cols {
            s.rows[row_idx][col].clone()
          } else {
            Cell::default()
          }
        };

        if !cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
          result.push(cell.c);
        }
      }

      if line != end_line {
        let trimmed = result.trim_end_matches(' ');
        let trimmed_len = trimmed.len();
        result.truncate(trimmed_len);
        result.push('\n');
      }
    }
    result
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

  fn line_search_right(&self, point: AlacPoint) -> AlacPoint {
    let s = self.state.lock();
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

  fn find_hyperlink_at(
    &self,
    _point: AlacPoint,
    _url_regex_pattern: &str,
  ) -> Option<(String, bool, std::ops::RangeInclusive<AlacPoint>)> {
    None
  }
}
