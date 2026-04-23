//! Core VTE terminal state, `vte::Perform` implementation, and `TerminalBackend` adapter.

use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::Mutex;
use terminal_kernel::grid::Scroll;
use terminal_kernel::index::{Boundary, Column, Direction, Line, Point as AlacPoint, Side};
use terminal_kernel::selection::{Selection, SelectionRange, SelectionType};
use terminal_kernel::term::cell::{Cell, Flags as CellFlags};
use terminal_kernel::term::{RenderableCursor, TermMode};
use terminal_kernel::vte::ansi::{Color, CursorShape, CursorStyle, NamedColor, Rgb};
use terminal_kernel::{
  ANSI_COLOR_COUNT, BACKGROUND_COLOR_INDEX, FOREGROUND_COLOR_INDEX, RenderableSnapshot,
  SelectionDisplay, TerminalBackend,
};

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

struct SavedCursor {
  point: AlacPoint,
  template_cell: Cell,
}

pub struct VteTermInner {
  // Primary screen buffer (num_lines rows × num_cols cols).
  rows: Vec<Vec<Cell>>,
  // Scrollback buffer (most recent at the back).
  scrollback: VecDeque<Vec<Cell>>,
  // Alternate screen buffer.
  alt_rows: Vec<Vec<Cell>>,

  num_lines: usize,
  num_cols: usize,
  max_scrollback: usize,

  cursor: CursorState,
  saved_cursor: Option<SavedCursor>,

  mode: TermMode,
  display_offset: usize,
  selection: Option<Selection>,
  selection_display: Option<SelectionDisplay>,

  // Scroll region (0-indexed, inclusive).
  scroll_top: usize,
  scroll_bottom: usize,

  // Attributes applied to newly-written cells.
  template_cell: Cell,

  tab_stops: Vec<bool>,
  title: String,
  colors: [Option<Rgb>; ANSI_COLOR_COUNT],

  using_alt_screen: bool,
  pending_wrap: bool,

  // Channel for events consumed by the Terminal UI layer.
  event_tx: futures::channel::mpsc::UnboundedSender<terminal_kernel::event::Event>,

  // Channel for OSC 7 working directory updates.
  osc7_tx: Option<std::sync::mpsc::Sender<std::path::PathBuf>>,
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

fn default_tab_stops(cols: usize) -> Vec<bool> {
  (0..cols).map(|c| c % 8 == 0).collect()
}

impl VteTermInner {
  pub fn new(
    lines: usize,
    cols: usize,
    max_scrollback: usize,
    event_tx: futures::channel::mpsc::UnboundedSender<terminal_kernel::event::Event>,
    osc7_tx: Option<std::sync::mpsc::Sender<std::path::PathBuf>>,
    initial_cursor_blink: bool,
  ) -> Self {
    Self {
      rows: blank_grid(lines, cols),
      scrollback: VecDeque::new(),
      alt_rows: blank_grid(lines, cols),
      num_lines: lines,
      num_cols: cols,
      max_scrollback,
      cursor: CursorState {
        point: AlacPoint::new(Line(0), Column(0)),
        style: CursorStyle {
          shape: CursorShape::Block,
          blinking: initial_cursor_blink,
        },
      },
      saved_cursor: None,
      mode: TermMode::SHOW_CURSOR | TermMode::LINE_WRAP,
      display_offset: 0,
      selection: None,
      selection_display: None,
      scroll_top: 0,
      scroll_bottom: lines.saturating_sub(1),
      template_cell: Cell::default(),
      tab_stops: default_tab_stops(cols),
      title: String::new(),
      colors: [None; ANSI_COLOR_COUNT],
      using_alt_screen: false,
      pending_wrap: false,
      event_tx,
      osc7_tx,
    }
  }

  // -- Grid helpers -------------------------------------------------------

  /// Scroll the scroll-region up by one line (content moves up, blank line at bottom).
  fn scroll_up_in_region(&mut self) {
    let top = self.scroll_top;
    let bottom = self.scroll_bottom;
    let removed = self.rows.remove(top);
    // If the scroll region is the full screen, push to scrollback.
    if top == 0 && bottom == self.num_lines - 1 && !self.using_alt_screen {
      self.scrollback.push_back(removed);
      while self.scrollback.len() > self.max_scrollback {
        self.scrollback.pop_front();
      }
    }
    self.rows.insert(bottom, blank_row(self.num_cols));
  }

  /// Scroll the scroll-region down by one line (content moves down, blank line at top).
  fn scroll_down_in_region(&mut self) {
    let top = self.scroll_top;
    let bottom = self.scroll_bottom;
    self.rows.remove(bottom);
    self.rows.insert(top, blank_row(self.num_cols));
  }

  fn linefeed(&mut self) {
    let row = self.cursor.point.line.0 as usize;
    if row == self.scroll_bottom {
      self.scroll_up_in_region();
    } else if row + 1 < self.num_lines {
      self.cursor.point.line.0 += 1;
    }
  }

  fn reverse_index(&mut self) {
    let row = self.cursor.point.line.0 as usize;
    if row == self.scroll_top {
      self.scroll_down_in_region();
    } else if row > 0 {
      self.cursor.point.line.0 -= 1;
    }
  }

  fn erase_cell(cell: &mut Cell) {
    *cell = Cell::default();
  }

  pub fn do_resize(&mut self, new_lines: usize, new_cols: usize) {
    if new_lines == 0 || new_cols == 0 {
      return;
    }

    // Resize each row in the primary screen.
    for row in &mut self.rows {
      row.resize(new_cols, Cell::default());
    }
    // Add or remove lines.
    while self.rows.len() < new_lines {
      self.rows.push(blank_row(new_cols));
    }
    while self.rows.len() > new_lines {
      let removed = self.rows.remove(0);
      if !self.using_alt_screen {
        self.scrollback.push_back(removed);
        while self.scrollback.len() > self.max_scrollback {
          self.scrollback.pop_front();
        }
      }
    }

    // Resize alt screen.
    for row in &mut self.alt_rows {
      row.resize(new_cols, Cell::default());
    }
    while self.alt_rows.len() < new_lines {
      self.alt_rows.push(blank_row(new_cols));
    }
    self.alt_rows.truncate(new_lines);

    // Resize scrollback rows.
    for row in &mut self.scrollback {
      row.resize(new_cols, Cell::default());
    }

    self.num_lines = new_lines;
    self.num_cols = new_cols;
    self.scroll_top = 0;
    self.scroll_bottom = new_lines.saturating_sub(1);
    self.tab_stops = default_tab_stops(new_cols);

    // Clamp cursor.
    self.cursor.point.line.0 = self.cursor.point.line.0.min(new_lines as i32 - 1).max(0);
    self.cursor.point.column.0 = self.cursor.point.column.0.min(new_cols.saturating_sub(1));
    self.pending_wrap = false;
    self.display_offset = self.display_offset.min(self.scrollback.len());
  }

  pub(crate) fn send_event(&self, event: terminal_kernel::event::Event) {
    let _ = self.event_tx.unbounded_send(event);
  }

  fn set_color_entry(&mut self, index: usize, color: Rgb) {
    if index < self.colors.len() {
      self.colors[index] = Some(color);
    }
  }

  fn request_color_entry(&self, index: usize, prefix: String, bell_terminated: bool) {
    let terminator = if bell_terminated { "\x07" } else { "\x1b\\" }.to_string();
    self.send_event(terminal_kernel::event::Event::ColorRequest(
      index,
      Arc::new(move |color| {
        format!(
          "\x1b]{};rgb:{:02x}{:02x}/{:02x}{:02x}/{:02x}{:02x}{}",
          prefix,
          color.r,
          color.r,
          color.g,
          color.g,
          color.b,
          color.b,
          terminator,
        )
      }),
    ));
  }

  // -- Alternate screen ---------------------------------------------------

  fn enter_alt_screen(&mut self) {
    if self.using_alt_screen {
      return;
    }
    self.using_alt_screen = true;
    std::mem::swap(&mut self.rows, &mut self.alt_rows);
    // Clear alt screen.
    for row in &mut self.rows {
      for cell in row.iter_mut() {
        Self::erase_cell(cell);
      }
    }
    self.mode.insert(TermMode::ALT_SCREEN);
  }

  fn exit_alt_screen(&mut self) {
    if !self.using_alt_screen {
      return;
    }
    self.using_alt_screen = false;
    std::mem::swap(&mut self.rows, &mut self.alt_rows);
    self.mode.remove(TermMode::ALT_SCREEN);
  }
}

fn cell_at_state(s: &VteTermInner, point: AlacPoint) -> Cell {
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

fn clamp_point_to_grid(s: &VteTermInner, point: AlacPoint) -> AlacPoint {
  let min_line = -(s.scrollback.len() as i32);
  let max_line = s.num_lines as i32 - 1;
  let line = point.line.0.clamp(min_line, max_line);
  let col = point.column.0.min(s.num_cols.saturating_sub(1));
  AlacPoint::new(Line(line), Column(col))
}

fn line_search_left_state(s: &VteTermInner, point: AlacPoint) -> AlacPoint {
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

fn line_search_right_state(s: &VteTermInner, point: AlacPoint) -> AlacPoint {
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

fn selection_range(s: &VteTermInner, selection: SelectionDisplay) -> Option<SelectionRange> {
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

fn bounds_to_string_state(s: &VteTermInner, start: AlacPoint, end: AlacPoint) -> String {
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

fn parse_osc_palette_index(param: &[u8]) -> Option<usize> {
  let index = std::str::from_utf8(param).ok()?.parse::<usize>().ok()?;
  (index < 256).then_some(index)
}

fn parse_osc_color_component(component: &str) -> Option<u8> {
  if component.is_empty() || component.len() > 4 {
    return None;
  }

  let value = u32::from_str_radix(component, 16).ok()?;
  let max_value = (1_u32 << (component.len() * 4)) - 1;
  Some(((value * 255 + max_value / 2) / max_value) as u8)
}

fn parse_osc_color_spec(spec: &[u8]) -> Option<Rgb> {
  let spec = std::str::from_utf8(spec).ok()?;

  if let Some(rest) = spec.strip_prefix("rgb:") {
    let mut parts = rest.split('/');
    let r = parse_osc_color_component(parts.next()?)?;
    let g = parse_osc_color_component(parts.next()?)?;
    let b = parse_osc_color_component(parts.next()?)?;
    if parts.next().is_some() {
      return None;
    }
    return Some(Rgb { r, g, b });
  }

  let rest = spec.strip_prefix('#')?;
  if rest.is_empty() || rest.len() % 3 != 0 {
    return None;
  }

  let width = rest.len() / 3;
  if width == 0 || width > 4 {
    return None;
  }

  let r = parse_osc_color_component(&rest[0..width])?;
  let g = parse_osc_color_component(&rest[width..width * 2])?;
  let b = parse_osc_color_component(&rest[width * 2..])?;
  Some(Rgb { r, g, b })
}

// ---------------------------------------------------------------------------
// vte::Perform — escape sequence handling
// ---------------------------------------------------------------------------

impl vte::Perform for VteTermInner {
  fn print(&mut self, c: char) {
    if self.pending_wrap {
      // Mark the current cell as wrapped.
      let row = self.cursor.point.line.0 as usize;
      let col = self.cursor.point.column.0;
      if row < self.num_lines && col < self.num_cols {
        self.rows[row][col].flags.insert(CellFlags::WRAPLINE);
      }
      self.cursor.point.column.0 = 0;
      self.linefeed();
      self.pending_wrap = false;
    }

    let row = self.cursor.point.line.0 as usize;
    let col = self.cursor.point.column.0;
    if row < self.num_lines && col < self.num_cols {
      let cell = &mut self.rows[row][col];
      cell.c = c;
      cell.fg = self.template_cell.fg.clone();
      cell.bg = self.template_cell.bg.clone();
      cell.flags = self.template_cell.flags;
    }

    if col + 1 < self.num_cols {
      self.cursor.point.column.0 += 1;
    } else {
      self.pending_wrap = true;
    }
  }

  fn execute(&mut self, byte: u8) {
    match byte {
      // Backspace.
      0x08 => {
        self.pending_wrap = false;
        if self.cursor.point.column.0 > 0 {
          self.cursor.point.column.0 -= 1;
        }
      }
      // Horizontal tab.
      0x09 => {
        self.pending_wrap = false;
        let col = self.cursor.point.column.0;
        let next = self
          .tab_stops
          .iter()
          .enumerate()
          .skip(col + 1)
          .find(|(_, stop)| **stop)
          .map(|(i, _)| i)
          .unwrap_or(self.num_cols.saturating_sub(1));
        self.cursor.point.column.0 = next;
      }
      // LF / VT / FF — line feed.
      0x0A | 0x0B | 0x0C => {
        self.linefeed();
        self.pending_wrap = false;
      }
      // CR — carriage return.
      0x0D => {
        self.cursor.point.column.0 = 0;
        self.pending_wrap = false;
      }
      // BEL.
      0x07 => {
        self.send_event(terminal_kernel::event::Event::Bell);
      }
      // SO / SI — charset switching (ignored).
      0x0E | 0x0F => {}
      _ => {}
    }
  }

  fn csi_dispatch(
    &mut self,
    params: &vte::Params,
    intermediates: &[u8],
    _ignore: bool,
    action: char,
  ) {
    let params_vec: Vec<u16> = params.iter().flat_map(|sub| sub.iter().copied()).collect();
    let p = |idx: usize, default: u16| -> u16 {
      params_vec
        .get(idx)
        .copied()
        .filter(|&v| v != 0)
        .unwrap_or(default)
    };

    match action {
      // CUU — cursor up.
      'A' => {
        let n = p(0, 1) as i32;
        self.cursor.point.line.0 = (self.cursor.point.line.0 - n).max(self.scroll_top as i32);
        self.pending_wrap = false;
      }
      // CUD — cursor down.
      'B' => {
        let n = p(0, 1) as i32;
        self.cursor.point.line.0 = (self.cursor.point.line.0 + n).min(self.scroll_bottom as i32);
        self.pending_wrap = false;
      }
      // CUF — cursor forward (right).
      'C' => {
        let n = p(0, 1) as usize;
        self.cursor.point.column.0 =
          (self.cursor.point.column.0 + n).min(self.num_cols.saturating_sub(1));
        self.pending_wrap = false;
      }
      // CUB — cursor backward (left).
      'D' => {
        let n = p(0, 1) as usize;
        self.cursor.point.column.0 = self.cursor.point.column.0.saturating_sub(n);
        self.pending_wrap = false;
      }
      // CUP — cursor position.
      'H' | 'f' => {
        let row = p(0, 1) as usize;
        let col = p(1, 1) as usize;
        self.cursor.point.line.0 =
          (row.saturating_sub(1)).min(self.num_lines.saturating_sub(1)) as i32;
        self.cursor.point.column.0 = (col.saturating_sub(1)).min(self.num_cols.saturating_sub(1));
        self.pending_wrap = false;
      }
      // ED — erase in display.
      'J' => {
        let mode = p(0, 0);
        match mode {
          0 => {
            // Erase from cursor to end of screen.
            let row = self.cursor.point.line.0 as usize;
            let col = self.cursor.point.column.0;
            if row < self.num_lines {
              for c in col..self.num_cols {
                Self::erase_cell(&mut self.rows[row][c]);
              }
              for r in (row + 1)..self.num_lines {
                for c in 0..self.num_cols {
                  Self::erase_cell(&mut self.rows[r][c]);
                }
              }
            }
          }
          1 => {
            // Erase from start of screen to cursor.
            let row = self.cursor.point.line.0 as usize;
            let col = self.cursor.point.column.0;
            for r in 0..row {
              for c in 0..self.num_cols {
                Self::erase_cell(&mut self.rows[r][c]);
              }
            }
            if row < self.num_lines {
              for c in 0..=col.min(self.num_cols.saturating_sub(1)) {
                Self::erase_cell(&mut self.rows[row][c]);
              }
            }
          }
          2 => {
            // Erase entire screen.
            for r in 0..self.num_lines {
              for c in 0..self.num_cols {
                Self::erase_cell(&mut self.rows[r][c]);
              }
            }
          }
          3 => {
            // Erase screen + scrollback.
            for r in 0..self.num_lines {
              for c in 0..self.num_cols {
                Self::erase_cell(&mut self.rows[r][c]);
              }
            }
            self.scrollback.clear();
            self.display_offset = 0;
          }
          _ => {}
        }
      }
      // EL — erase in line.
      'K' => {
        let mode = p(0, 0);
        let row = self.cursor.point.line.0 as usize;
        let col = self.cursor.point.column.0;
        if row < self.num_lines {
          match mode {
            0 => {
              for c in col..self.num_cols {
                Self::erase_cell(&mut self.rows[row][c]);
              }
            }
            1 => {
              for c in 0..=col.min(self.num_cols.saturating_sub(1)) {
                Self::erase_cell(&mut self.rows[row][c]);
              }
            }
            2 => {
              for c in 0..self.num_cols {
                Self::erase_cell(&mut self.rows[row][c]);
              }
            }
            _ => {}
          }
        }
      }
      // IL — insert lines.
      'L' => {
        let n = p(0, 1) as usize;
        let row = self.cursor.point.line.0 as usize;
        if row >= self.scroll_top && row <= self.scroll_bottom {
          for _ in 0..n {
            if self.scroll_bottom < self.rows.len() {
              self.rows.remove(self.scroll_bottom);
            }
            self.rows.insert(row, blank_row(self.num_cols));
          }
        }
        self.pending_wrap = false;
      }
      // DL — delete lines.
      'M' => {
        let n = p(0, 1) as usize;
        let row = self.cursor.point.line.0 as usize;
        if row >= self.scroll_top && row <= self.scroll_bottom {
          for _ in 0..n {
            if row < self.rows.len() {
              self.rows.remove(row);
            }
            self
              .rows
              .insert(self.scroll_bottom, blank_row(self.num_cols));
          }
        }
        self.pending_wrap = false;
      }
      // DCH — delete characters.
      'P' => {
        let n = p(0, 1) as usize;
        let row = self.cursor.point.line.0 as usize;
        let col = self.cursor.point.column.0;
        if row < self.num_lines {
          for _ in 0..n.min(self.num_cols - col) {
            if col < self.rows[row].len() {
              self.rows[row].remove(col);
              self.rows[row].push(Cell::default());
            }
          }
        }
      }
      // ICH — insert characters.
      '@' => {
        let n = p(0, 1) as usize;
        let row = self.cursor.point.line.0 as usize;
        let col = self.cursor.point.column.0;
        if row < self.num_lines {
          for _ in 0..n.min(self.num_cols - col) {
            self.rows[row].insert(col, Cell::default());
            self.rows[row].truncate(self.num_cols);
          }
        }
      }
      // SU — scroll up.
      'S' => {
        let n = p(0, 1) as usize;
        for _ in 0..n {
          self.scroll_up_in_region();
        }
      }
      // SD — scroll down.
      'T' => {
        let n = p(0, 1) as usize;
        for _ in 0..n {
          self.scroll_down_in_region();
        }
      }
      // SGR — select graphic rendition.
      'm' => {
        self.handle_sgr(&params_vec);
      }
      // DECSTBM — set scroll region.
      'r' => {
        let top = p(0, 1) as usize;
        let bottom = p(1, self.num_lines as u16) as usize;
        self.scroll_top = top.saturating_sub(1).min(self.num_lines.saturating_sub(1));
        self.scroll_bottom = bottom
          .saturating_sub(1)
          .min(self.num_lines.saturating_sub(1));
        if self.scroll_top >= self.scroll_bottom {
          self.scroll_top = 0;
          self.scroll_bottom = self.num_lines.saturating_sub(1);
        }
        // Reset cursor to top-left.
        self.cursor.point.line.0 = 0;
        self.cursor.point.column.0 = 0;
        self.pending_wrap = false;
      }
      // SM/RM — set/reset mode.
      'h' | 'l' => {
        let set = action == 'h';
        let private = intermediates.first() == Some(&b'?');
        for &val in &params_vec {
          if private {
            self.handle_private_mode(val, set);
          } else {
            // Standard modes.
            match val {
              4 => {
                if set {
                  self.mode.insert(TermMode::INSERT);
                } else {
                  self.mode.remove(TermMode::INSERT);
                }
              }
              20 => {
                if set {
                  self.mode.insert(TermMode::LINE_FEED_NEW_LINE);
                } else {
                  self.mode.remove(TermMode::LINE_FEED_NEW_LINE);
                }
              }
              _ => {}
            }
          }
        }
      }
      // CHA — cursor character absolute.
      'G' | '`' => {
        let col = p(0, 1) as usize;
        self.cursor.point.column.0 = col.saturating_sub(1).min(self.num_cols.saturating_sub(1));
        self.pending_wrap = false;
      }
      // VPA — vertical position absolute.
      'd' => {
        let row = p(0, 1) as usize;
        self.cursor.point.line.0 =
          row.saturating_sub(1).min(self.num_lines.saturating_sub(1)) as i32;
        self.pending_wrap = false;
      }
      // ECH — erase characters.
      'X' => {
        let n = p(0, 1) as usize;
        let row = self.cursor.point.line.0 as usize;
        let col = self.cursor.point.column.0;
        if row < self.num_lines {
          for c in col..(col + n).min(self.num_cols) {
            Self::erase_cell(&mut self.rows[row][c]);
          }
        }
      }
      // DECSC (save cursor via CSI s).
      's' if intermediates.is_empty() => {
        self.saved_cursor = Some(SavedCursor {
          point: self.cursor.point,
          template_cell: self.template_cell.clone(),
        });
      }
      // DECRC (restore cursor via CSI u).
      'u' if intermediates.is_empty() => {
        if let Some(saved) = self.saved_cursor.take() {
          self.cursor.point = saved.point;
          self.template_cell = saved.template_cell;
          self.pending_wrap = false;
        }
      }
      // CNL — cursor next line.
      'E' => {
        let n = p(0, 1) as i32;
        self.cursor.point.line.0 = (self.cursor.point.line.0 + n).min(self.scroll_bottom as i32);
        self.cursor.point.column.0 = 0;
        self.pending_wrap = false;
      }
      // CPL — cursor preceding line.
      'F' => {
        let n = p(0, 1) as i32;
        self.cursor.point.line.0 = (self.cursor.point.line.0 - n).max(self.scroll_top as i32);
        self.cursor.point.column.0 = 0;
        self.pending_wrap = false;
      }
      // DA — device attributes (respond with VT100-compatible).
      'c' if intermediates.is_empty() || intermediates == [b'?'] => {
        // Ignored — DA responses are sent by the PTY filter or not needed.
      }
      // DSR — device status report.
      'n' => {
        // Ignored — DSR responses are handled externally.
      }
      // DECSCUSR — set cursor style.
      'q' if intermediates.first() == Some(&b' ') => {
        let style = p(0, 0);
        self.cursor.style = match style {
          0 | 1 => CursorStyle {
            shape: CursorShape::Block,
            blinking: true,
          },
          2 => CursorStyle {
            shape: CursorShape::Block,
            blinking: false,
          },
          3 => CursorStyle {
            shape: CursorShape::Underline,
            blinking: true,
          },
          4 => CursorStyle {
            shape: CursorShape::Underline,
            blinking: false,
          },
          5 => CursorStyle {
            shape: CursorShape::Beam,
            blinking: true,
          },
          6 => CursorStyle {
            shape: CursorShape::Beam,
            blinking: false,
          },
          _ => self.cursor.style,
        };
      }
      _ => {}
    }
  }

  fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
    match (byte, intermediates) {
      // RI — reverse index.
      (b'M', []) => {
        self.reverse_index();
        self.pending_wrap = false;
      }
      // IND — index (line feed).
      (b'D', []) => {
        self.linefeed();
        self.pending_wrap = false;
      }
      // NEL — next line.
      (b'E', []) => {
        self.linefeed();
        self.cursor.point.column.0 = 0;
        self.pending_wrap = false;
      }
      // DECSC — save cursor.
      (b'7', []) => {
        self.saved_cursor = Some(SavedCursor {
          point: self.cursor.point,
          template_cell: self.template_cell.clone(),
        });
      }
      // DECRC — restore cursor.
      (b'8', []) => {
        if let Some(saved) = self.saved_cursor.take() {
          self.cursor.point = saved.point;
          self.template_cell = saved.template_cell;
          self.pending_wrap = false;
        }
      }
      // HTS — horizontal tab set.
      (b'H', []) => {
        let col = self.cursor.point.column.0;
        if col < self.tab_stops.len() {
          self.tab_stops[col] = true;
        }
      }
      _ => {}
    }
  }

  fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
    if params.is_empty() {
      return;
    }
    let cmd = std::str::from_utf8(params[0]).unwrap_or("");
    match cmd {
      "0" | "2" => {
        // Set window title.
        if let Some(title) = params.get(1) {
          self.title = String::from_utf8_lossy(title).to_string();
          self.send_event(terminal_kernel::event::Event::Title(self.title.clone()));
        }
      }
      "4" => {
        for pair in params[1..].chunks(2) {
          let Some(index) = pair.first().and_then(|param| parse_osc_palette_index(param)) else {
            continue;
          };
          let Some(spec) = pair.get(1) else {
            continue;
          };

          if *spec == b"?" {
            self.request_color_entry(index, format!("4;{index}"), bell_terminated);
          } else if let Some(color) = parse_osc_color_spec(spec) {
            self.set_color_entry(index, color);
          }
        }
      }
      "10" | "11" => {
        let color_index = if cmd == "10" {
          FOREGROUND_COLOR_INDEX
        } else {
          BACKGROUND_COLOR_INDEX
        };
        if let Some(spec) = params.get(1) {
          if *spec == b"?" {
            self.request_color_entry(color_index, cmd.to_string(), bell_terminated);
          } else if let Some(color) = parse_osc_color_spec(spec) {
            self.set_color_entry(color_index, color);
          }
        }
      }
      "7" => {
        // Set working directory.
        if let Some(uri) = params.get(1) {
          let uri_str = String::from_utf8_lossy(uri);
          // Parse file:// URI.
          if let Some(path) = uri_str.strip_prefix("file://") {
            // Strip hostname if present.
            let path = if let Some(idx) = path.find('/') {
              &path[idx..]
            } else {
              path
            };
            if let Some(tx) = &self.osc7_tx {
              let _ = tx.send(std::path::PathBuf::from(path));
            }
          }
        }
      }
      _ => {}
    }
  }

  fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _action: char) {}
  fn put(&mut self, _byte: u8) {}
  fn unhook(&mut self) {}
}

// ---------------------------------------------------------------------------
// SGR (Select Graphic Rendition) handling
// ---------------------------------------------------------------------------

impl VteTermInner {
  fn handle_sgr(&mut self, params: &[u16]) {
    if params.is_empty() {
      self.reset_sgr();
      return;
    }
    let mut i = 0;
    while i < params.len() {
      match params[i] {
        0 => self.reset_sgr(),
        1 => self.template_cell.flags.insert(CellFlags::BOLD),
        2 => self.template_cell.flags.insert(CellFlags::DIM),
        3 => self.template_cell.flags.insert(CellFlags::ITALIC),
        4 => self.template_cell.flags.insert(CellFlags::UNDERLINE),
        7 => self.template_cell.flags.insert(CellFlags::INVERSE),
        8 => self.template_cell.flags.insert(CellFlags::HIDDEN),
        9 => self.template_cell.flags.insert(CellFlags::STRIKEOUT),
        22 => self
          .template_cell
          .flags
          .remove(CellFlags::BOLD | CellFlags::DIM),
        23 => self.template_cell.flags.remove(CellFlags::ITALIC),
        24 => self.template_cell.flags.remove(CellFlags::ALL_UNDERLINES),
        27 => self.template_cell.flags.remove(CellFlags::INVERSE),
        28 => self.template_cell.flags.remove(CellFlags::HIDDEN),
        29 => self.template_cell.flags.remove(CellFlags::STRIKEOUT),
        // Foreground colours.
        30..=37 => {
          self.template_cell.fg = Color::Indexed(params[i] as u8 - 30);
        }
        38 => {
          i += 1;
          self.parse_extended_color(params, &mut i, true);
          continue; // `parse_extended_color` advances `i`.
        }
        39 => {
          self.template_cell.fg = Color::Named(NamedColor::Foreground);
        }
        // Background colours.
        40..=47 => {
          self.template_cell.bg = Color::Indexed(params[i] as u8 - 40);
        }
        48 => {
          i += 1;
          self.parse_extended_color(params, &mut i, false);
          continue;
        }
        49 => {
          self.template_cell.bg = Color::Named(NamedColor::Background);
        }
        // Bright foreground.
        90..=97 => {
          self.template_cell.fg = Color::Indexed(params[i] as u8 - 90 + 8);
        }
        // Bright background.
        100..=107 => {
          self.template_cell.bg = Color::Indexed(params[i] as u8 - 100 + 8);
        }
        _ => {}
      }
      i += 1;
    }
  }

  fn reset_sgr(&mut self) {
    self.template_cell.fg = Color::Named(NamedColor::Foreground);
    self.template_cell.bg = Color::Named(NamedColor::Background);
    self.template_cell.flags = CellFlags::empty();
  }

  /// Parse `38;5;N` / `38;2;R;G;B` (and `48` equivalents).
  fn parse_extended_color(&mut self, params: &[u16], i: &mut usize, is_fg: bool) {
    if *i >= params.len() {
      return;
    }
    match params[*i] {
      // 256-colour.
      5 => {
        *i += 1;
        if *i < params.len() {
          let color = Color::Indexed(params[*i] as u8);
          if is_fg {
            self.template_cell.fg = color;
          } else {
            self.template_cell.bg = color;
          }
          *i += 1;
        }
      }
      // RGB.
      2 => {
        if *i + 3 < params.len() {
          let r = params[*i + 1] as u8;
          let g = params[*i + 2] as u8;
          let b = params[*i + 3] as u8;
          let color = Color::Spec(Rgb { r, g, b });
          if is_fg {
            self.template_cell.fg = color;
          } else {
            self.template_cell.bg = color;
          }
          *i += 4;
        }
      }
      _ => {
        *i += 1;
      }
    }
  }

  fn handle_private_mode(&mut self, mode: u16, set: bool) {
    match mode {
      // DECCKM — application cursor keys.
      1 => {
        if set {
          self.mode.insert(TermMode::APP_CURSOR);
        } else {
          self.mode.remove(TermMode::APP_CURSOR);
        }
      }
      // DECAWM — auto-wrap mode.
      7 => {
        if set {
          self.mode.insert(TermMode::LINE_WRAP);
        } else {
          self.mode.remove(TermMode::LINE_WRAP);
        }
      }
      // Cursor blink.
      12 => {
        self.cursor.style.blinking = set;
      }
      // DECTCEM — show cursor.
      25 => {
        if set {
          self.mode.insert(TermMode::SHOW_CURSOR);
        } else {
          self.mode.remove(TermMode::SHOW_CURSOR);
        }
      }
      // Alternate screen buffer.
      47 | 1047 => {
        if set {
          self.enter_alt_screen();
        } else {
          self.exit_alt_screen();
        }
      }
      // 1049 — alternate screen + save/restore cursor.
      1049 => {
        if set {
          self.saved_cursor = Some(SavedCursor {
            point: self.cursor.point,
            template_cell: self.template_cell.clone(),
          });
          self.enter_alt_screen();
        } else {
          self.exit_alt_screen();
          if let Some(saved) = self.saved_cursor.take() {
            self.cursor.point = saved.point;
            self.template_cell = saved.template_cell;
          }
        }
      }
      // Bracketed paste.
      2004 => {
        if set {
          self.mode.insert(TermMode::BRACKETED_PASTE);
        } else {
          self.mode.remove(TermMode::BRACKETED_PASTE);
        }
      }
      // Mouse modes.
      1000 => {
        if set {
          self.mode.insert(TermMode::MOUSE_REPORT_CLICK);
        } else {
          self.mode.remove(TermMode::MOUSE_REPORT_CLICK);
        }
      }
      1002 => {
        if set {
          self.mode.insert(TermMode::MOUSE_DRAG);
        } else {
          self.mode.remove(TermMode::MOUSE_DRAG);
        }
      }
      1003 => {
        if set {
          self.mode.insert(TermMode::MOUSE_MOTION);
        } else {
          self.mode.remove(TermMode::MOUSE_MOTION);
        }
      }
      1006 => {
        if set {
          self.mode.insert(TermMode::SGR_MOUSE);
        } else {
          self.mode.remove(TermMode::SGR_MOUSE);
        }
      }
      // Alternate scroll mode.
      1007 => {
        if set {
          self.mode.insert(TermMode::ALTERNATE_SCROLL);
        } else {
          self.mode.remove(TermMode::ALTERNATE_SCROLL);
        }
      }
      // Focus in/out events.
      1004 => {
        if set {
          self.mode.insert(TermMode::FOCUS_IN_OUT);
        } else {
          self.mode.remove(TermMode::FOCUS_IN_OUT);
        }
      }
      _ => {}
    }
  }
}

// ---------------------------------------------------------------------------
// VteBackend — TerminalBackend implementation
// ---------------------------------------------------------------------------

pub struct VteBackend {
  state: Arc<Mutex<VteTermInner>>,
}

impl VteBackend {
  pub fn new(state: Arc<Mutex<VteTermInner>>) -> Self {
    Self { state }
  }

  #[allow(dead_code)]
  pub fn state(&self) -> &Arc<Mutex<VteTermInner>> {
    &self.state
  }
}

// SAFETY: parking_lot::Mutex is Send + Sync.
unsafe impl Send for VteBackend {}
unsafe impl Sync for VteBackend {}

impl TerminalBackend for VteBackend {
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
      // Map visible row to absolute line.
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
    if index < s.colors.len() { s.colors[index] } else { None }
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
    // Wide character support is not handled in this minimal VTE backend.
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
    // Regex-based URL detection not yet implemented for the VTE backend.
    None
  }
}

#[cfg(test)]
mod tests {
  use futures::{StreamExt as _, executor::block_on};

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
    let state = Arc::new(Mutex::new(VteTermInner::new(
      2, 5, 100, event_tx, None, true,
    )));

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

    let backend = VteBackend::new(state);
    let selection = backend.renderable_snapshot().selection.unwrap();

    assert_eq!(selection.start, AlacPoint::new(Line(0), Column(1)));
    assert_eq!(selection.end, AlacPoint::new(Line(0), Column(3)));
  }

  #[test]
  fn selection_to_string_uses_visible_range() {
    let (event_tx, _event_rx) = futures::channel::mpsc::unbounded();
    let state = Arc::new(Mutex::new(VteTermInner::new(
      2, 5, 100, event_tx, None, true,
    )));

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

    let backend = VteBackend::new(state);

    assert_eq!(backend.selection_to_string().as_deref(), Some("ell"));
  }

  fn feed(inner: &mut VteTermInner, bytes: &[u8]) {
    let mut parser = vte::Parser::new();
    parser.advance(inner, bytes);
  }

  #[test]
  fn osc_4_sets_palette_entries_and_answers_queries() {
    let (event_tx, mut event_rx) = futures::channel::mpsc::unbounded();
    let mut inner = VteTermInner::new(2, 5, 100, event_tx, None, true);

    feed(&mut inner, b"\x1b]4;1;rgb:12/34/56\x07");
    assert_eq!(inner.colors[1], Some(Rgb { r: 0x12, g: 0x34, b: 0x56 }));

    feed(&mut inner, b"\x1b]4;1;?\x1b\\");
    match block_on(event_rx.next()) {
      Some(terminal_kernel::event::Event::ColorRequest(index, formatter)) => {
        assert_eq!(index, 1);
        assert_eq!(
          formatter(Rgb { r: 0x12, g: 0x34, b: 0x56 }),
          "\x1b]4;1;rgb:1212/3434/5656\x1b\\",
        );
      }
      other => panic!("expected OSC 4 color request, got {other:?}"),
    }
  }

  #[test]
  fn osc_10_sets_default_foreground_and_answers_queries() {
    let (event_tx, mut event_rx) = futures::channel::mpsc::unbounded();
    let mut inner = VteTermInner::new(2, 5, 100, event_tx, None, true);

    feed(&mut inner, b"\x1b]10;#aabbcc\x07");
    assert_eq!(
      inner.colors[FOREGROUND_COLOR_INDEX],
      Some(Rgb {
        r: 0xaa,
        g: 0xbb,
        b: 0xcc,
      }),
    );

    feed(&mut inner, b"\x1b]10;?\x07");
    match block_on(event_rx.next()) {
      Some(terminal_kernel::event::Event::ColorRequest(index, formatter)) => {
        assert_eq!(index, FOREGROUND_COLOR_INDEX);
        assert_eq!(
          formatter(Rgb {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc,
          }),
          "\x1b]10;rgb:aaaa/bbbb/cccc\x07",
        );
      }
      other => panic!("expected OSC 10 color request, got {other:?}"),
    }
  }

  #[test]
  fn osc_11_sets_default_background_and_answers_queries() {
    let (event_tx, mut event_rx) = futures::channel::mpsc::unbounded();
    let mut inner = VteTermInner::new(2, 5, 100, event_tx, None, true);

    feed(&mut inner, b"\x1b]11;#123456\x07");
    assert_eq!(
      inner.colors[BACKGROUND_COLOR_INDEX],
      Some(Rgb {
        r: 0x12,
        g: 0x34,
        b: 0x56,
      }),
    );

    feed(&mut inner, b"\x1b]11;?\x1b\\");
    match block_on(event_rx.next()) {
      Some(terminal_kernel::event::Event::ColorRequest(index, formatter)) => {
        assert_eq!(index, BACKGROUND_COLOR_INDEX);
        assert_eq!(
          formatter(Rgb {
            r: 0x12,
            g: 0x34,
            b: 0x56,
          }),
          "\x1b]11;rgb:1212/3434/5656\x1b\\",
        );
      }
      other => panic!("expected OSC 11 color request, got {other:?}"),
    }
  }
}
