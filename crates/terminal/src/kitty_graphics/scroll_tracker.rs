use terminal_kernel::vte::ansi::{
  ClearMode, Handler, Mode, NamedMode, NamedPrivateMode, PrivateMode, Processor,
};
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphicsScrollDirection {
  Up,
  Down,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphicsScroll {
  pub direction: GraphicsScrollDirection,
  pub top: u32,
  pub bottom: u32,
  pub lines: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphicsTerminalEvent {
  Scroll(GraphicsScroll),
  ClearAll,
}

pub struct GraphicsScrollTracker {
  parser: Processor,
  state: GraphicsScrollState,
}

impl GraphicsScrollTracker {
  pub fn new(screen_lines: u32, screen_columns: u32) -> Self {
    Self {
      parser: Processor::new(),
      state: GraphicsScrollState::new(screen_lines, screen_columns),
    }
  }

  pub fn advance(&mut self, bytes: &[u8]) {
    self.parser.advance(&mut self.state, bytes);
  }

  pub fn resize(&mut self, screen_lines: u32, screen_columns: u32) {
    self.state.resize(screen_lines, screen_columns);
  }

  pub fn drain(&mut self) -> impl Iterator<Item = GraphicsTerminalEvent> + '_ {
    self.state.events.drain(..)
  }
}

struct GraphicsScrollState {
  screen_lines: usize,
  screen_columns: usize,
  cursor_line: usize,
  cursor_column: usize,
  saved_cursor_line: usize,
  saved_cursor_column: usize,
  saved_input_needs_wrap: bool,
  scroll_top: usize,
  scroll_bottom: usize,
  origin_mode: bool,
  line_wrap: bool,
  line_feed_new_line: bool,
  input_needs_wrap: bool,
  alternate_screen: bool,
  primary_cursor: Option<(usize, usize, bool)>,
  alternate_saved_cursor: (usize, usize, bool),
  events: Vec<GraphicsTerminalEvent>,
}

impl GraphicsScrollState {
  fn new(screen_lines: u32, screen_columns: u32) -> Self {
    let screen_lines = screen_lines.max(1) as usize;
    Self {
      screen_lines,
      screen_columns: screen_columns.max(1) as usize,
      cursor_line: 0,
      cursor_column: 0,
      saved_cursor_line: 0,
      saved_cursor_column: 0,
      saved_input_needs_wrap: false,
      scroll_top: 0,
      scroll_bottom: screen_lines,
      origin_mode: false,
      line_wrap: true,
      line_feed_new_line: false,
      input_needs_wrap: false,
      alternate_screen: false,
      primary_cursor: None,
      alternate_saved_cursor: (0, 0, false),
      events: Vec::new(),
    }
  }

  fn resize(&mut self, screen_lines: u32, screen_columns: u32) {
    self.screen_lines = screen_lines.max(1) as usize;
    self.screen_columns = screen_columns.max(1) as usize;
    self.cursor_line = self.cursor_line.min(self.screen_lines - 1);
    self.cursor_column = self.cursor_column.min(self.screen_columns - 1);
    self.saved_cursor_line = self.saved_cursor_line.min(self.screen_lines - 1);
    self.saved_cursor_column = self.saved_cursor_column.min(self.screen_columns - 1);
    if let Some((line, column, input_needs_wrap)) = self.primary_cursor.as_mut() {
      *line = (*line).min(self.screen_lines - 1);
      *column = (*column).min(self.screen_columns - 1);
      *input_needs_wrap &= *column == self.screen_columns - 1;
    }
    self.alternate_saved_cursor.0 = self
      .alternate_saved_cursor
      .0
      .min(self.screen_lines - 1);
    self.alternate_saved_cursor.1 = self
      .alternate_saved_cursor
      .1
      .min(self.screen_columns - 1);
    self.alternate_saved_cursor.2 &=
      self.alternate_saved_cursor.1 == self.screen_columns - 1;
    self.scroll_top = 0;
    self.scroll_bottom = self.screen_lines;
  }

  fn set_cursor_line(&mut self, line: i32) {
    let (offset, last_line) = if self.origin_mode {
      (self.scroll_top, self.scroll_bottom - 1)
    } else {
      (0, self.screen_lines - 1)
    };
    self.cursor_line = (line.max(0) as usize)
      .saturating_add(offset)
      .min(last_line);
    self.input_needs_wrap = false;
  }

  fn wrapline(&mut self) {
    if !self.line_wrap {
      return;
    }

    if self.cursor_line + 1 >= self.scroll_bottom {
      self.linefeed();
    } else {
      self.cursor_line += 1;
    }
    self.cursor_column = 0;
    self.input_needs_wrap = false;
  }

  fn push_scroll(
    &mut self,
    direction: GraphicsScrollDirection,
    top: usize,
    bottom: usize,
    lines: usize,
  ) {
    let lines = lines.min(bottom.saturating_sub(top));
    if lines == 0 {
      return;
    }
    self.events.push(GraphicsTerminalEvent::Scroll(GraphicsScroll {
      direction,
      top: top as u32,
      bottom: bottom as u32,
      lines: lines as u32,
    }));
  }
}

impl Handler for GraphicsScrollState {
  fn input(&mut self, character: char) {
    let Some(width) = character.width() else {
      return;
    };
    if width == 0 {
      return;
    }

    if self.input_needs_wrap {
      self.wrapline();
    }
    if width > 1 && self.cursor_column + 1 >= self.screen_columns {
      if self.line_wrap {
        self.wrapline();
      } else {
        self.input_needs_wrap = true;
        return;
      }
    }

    if self.cursor_column + width < self.screen_columns {
      self.cursor_column += width;
    } else {
      self.cursor_column = self.screen_columns - 1;
      self.input_needs_wrap = true;
    }
  }

  fn goto(&mut self, line: i32, column: usize) {
    self.set_cursor_line(line);
    self.cursor_column = column.min(self.screen_columns - 1);
  }

  fn goto_line(&mut self, line: i32) {
    self.set_cursor_line(line);
  }

  fn goto_col(&mut self, column: usize) {
    self.cursor_column = column.min(self.screen_columns - 1);
    self.input_needs_wrap = false;
  }

  fn move_up(&mut self, lines: usize) {
    let first_line = if self.origin_mode { self.scroll_top } else { 0 };
    self.cursor_line = self.cursor_line.saturating_sub(lines).max(first_line);
    self.input_needs_wrap = false;
  }

  fn move_down(&mut self, lines: usize) {
    let last_line = if self.origin_mode {
      self.scroll_bottom - 1
    } else {
      self.screen_lines - 1
    };
    self.cursor_line = self
      .cursor_line
      .saturating_add(lines)
      .min(last_line);
    self.input_needs_wrap = false;
  }

  fn move_forward(&mut self, columns: usize) {
    self.cursor_column = self
      .cursor_column
      .saturating_add(columns)
      .min(self.screen_columns - 1);
    self.input_needs_wrap = false;
  }

  fn move_backward(&mut self, columns: usize) {
    self.cursor_column = self.cursor_column.saturating_sub(columns);
    self.input_needs_wrap = false;
  }

  fn move_down_and_cr(&mut self, lines: usize) {
    self.move_down(lines);
    self.cursor_column = 0;
  }

  fn move_up_and_cr(&mut self, lines: usize) {
    self.move_up(lines);
    self.cursor_column = 0;
  }

  fn put_tab(&mut self, count: u16) {
    if self.input_needs_wrap {
      self.wrapline();
      return;
    }

    for _ in 0..count {
      let next_tab = (self.cursor_column / 8 + 1) * 8;
      self.cursor_column = next_tab.min(self.screen_columns - 1);
    }
  }

  fn backspace(&mut self) {
    if self.cursor_column > 0 {
      self.cursor_column -= 1;
      self.input_needs_wrap = false;
    }
  }

  fn carriage_return(&mut self) {
    self.cursor_column = 0;
    self.input_needs_wrap = false;
  }

  fn linefeed(&mut self) {
    if self.cursor_line + 1 == self.scroll_bottom {
      self.push_scroll(
        GraphicsScrollDirection::Up,
        self.scroll_top,
        self.scroll_bottom,
        1,
      );
    } else if self.cursor_line + 1 < self.screen_lines {
      self.cursor_line += 1;
    }
  }

  fn newline(&mut self) {
    self.linefeed();
    if self.line_feed_new_line {
      self.carriage_return();
    }
  }

  fn scroll_up(&mut self, lines: usize) {
    self.push_scroll(
      GraphicsScrollDirection::Up,
      self.scroll_top,
      self.scroll_bottom,
      lines,
    );
  }

  fn scroll_down(&mut self, lines: usize) {
    self.push_scroll(
      GraphicsScrollDirection::Down,
      self.scroll_top,
      self.scroll_bottom,
      lines,
    );
  }

  fn insert_blank_lines(&mut self, lines: usize) {
    if self.cursor_line >= self.scroll_top && self.cursor_line < self.scroll_bottom {
      self.push_scroll(
        GraphicsScrollDirection::Down,
        self.cursor_line,
        self.scroll_bottom,
        lines,
      );
    }
  }

  fn delete_lines(&mut self, lines: usize) {
    if self.cursor_line >= self.scroll_top && self.cursor_line < self.scroll_bottom {
      self.push_scroll(
        GraphicsScrollDirection::Up,
        self.cursor_line,
        self.scroll_bottom,
        lines,
      );
    }
  }

  fn save_cursor_position(&mut self) {
    self.saved_cursor_line = self.cursor_line;
    self.saved_cursor_column = self.cursor_column;
    self.saved_input_needs_wrap = self.input_needs_wrap;
  }

  fn restore_cursor_position(&mut self) {
    self.cursor_line = self.saved_cursor_line.min(self.screen_lines - 1);
    self.cursor_column = self.saved_cursor_column.min(self.screen_columns - 1);
    self.input_needs_wrap = self.saved_input_needs_wrap;
  }

  fn reset_state(&mut self) {
    self.cursor_line = 0;
    self.cursor_column = 0;
    self.saved_cursor_line = 0;
    self.saved_cursor_column = 0;
    self.saved_input_needs_wrap = false;
    self.scroll_top = 0;
    self.scroll_bottom = self.screen_lines;
    self.origin_mode = false;
    self.line_wrap = true;
    self.line_feed_new_line = false;
    self.input_needs_wrap = false;
    self.alternate_screen = false;
    self.primary_cursor = None;
    self.alternate_saved_cursor = (0, 0, false);
    self.events.push(GraphicsTerminalEvent::ClearAll);
  }

  fn clear_screen(&mut self, mode: ClearMode) {
    if matches!(mode, ClearMode::All | ClearMode::Saved) {
      self.events.push(GraphicsTerminalEvent::ClearAll);
    }
  }

  fn reverse_index(&mut self) {
    if self.cursor_line == self.scroll_top {
      self.push_scroll(
        GraphicsScrollDirection::Down,
        self.scroll_top,
        self.scroll_bottom,
        1,
      );
    } else {
      self.cursor_line = self.cursor_line.saturating_sub(1);
    }
  }

  fn set_private_mode(&mut self, mode: PrivateMode) {
    match mode {
      PrivateMode::Named(NamedPrivateMode::Origin) => {
        self.origin_mode = true;
        self.cursor_line = self.scroll_top;
        self.cursor_column = 0;
        self.input_needs_wrap = false;
      }
      PrivateMode::Named(NamedPrivateMode::LineWrap) => self.line_wrap = true,
      PrivateMode::Named(NamedPrivateMode::SwapScreenAndSetRestoreCursor)
        if !self.alternate_screen =>
      {
        self.primary_cursor = Some((
          self.cursor_line,
          self.cursor_column,
          self.input_needs_wrap,
        ));
        (
          self.saved_cursor_line,
          self.saved_cursor_column,
          self.saved_input_needs_wrap,
        ) = self.alternate_saved_cursor;
        self.alternate_screen = true;
      }
      _ => {}
    }
  }

  fn unset_private_mode(&mut self, mode: PrivateMode) {
    match mode {
      PrivateMode::Named(NamedPrivateMode::Origin) => {
        self.origin_mode = false;
      }
      PrivateMode::Named(NamedPrivateMode::LineWrap) => self.line_wrap = false,
      PrivateMode::Named(NamedPrivateMode::SwapScreenAndSetRestoreCursor)
        if self.alternate_screen =>
      {
        self.alternate_saved_cursor = (
          self.saved_cursor_line,
          self.saved_cursor_column,
          self.saved_input_needs_wrap,
        );
        if let Some((line, column, input_needs_wrap)) = self.primary_cursor.take() {
          self.cursor_line = line;
          self.cursor_column = column;
          self.input_needs_wrap = input_needs_wrap;
          self.saved_cursor_line = line;
          self.saved_cursor_column = column;
          self.saved_input_needs_wrap = input_needs_wrap;
        }
        self.alternate_screen = false;
      }
      _ => {}
    }
  }

  fn set_mode(&mut self, mode: Mode) {
    if mode == Mode::Named(NamedMode::LineFeedNewLine) {
      self.line_feed_new_line = true;
    }
  }

  fn unset_mode(&mut self, mode: Mode) {
    if mode == Mode::Named(NamedMode::LineFeedNewLine) {
      self.line_feed_new_line = false;
    }
  }

  fn set_scrolling_region(&mut self, top: usize, bottom: Option<usize>) {
    let bottom = bottom.unwrap_or(self.screen_lines).min(self.screen_lines);
    if top == 0 || top >= bottom {
      return;
    }
    self.scroll_top = top - 1;
    self.scroll_bottom = bottom;
    self.cursor_line = if self.origin_mode { self.scroll_top } else { 0 };
    self.cursor_column = 0;
    self.input_needs_wrap = false;
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn events(screen_lines: u32, bytes: &[u8]) -> Vec<GraphicsScroll> {
    events_with_size(screen_lines, 80, bytes)
  }

  fn events_with_size(
    screen_lines: u32,
    screen_columns: u32,
    bytes: &[u8],
  ) -> Vec<GraphicsScroll> {
    let mut tracker = GraphicsScrollTracker::new(screen_lines, screen_columns);
    tracker.advance(bytes);
    tracker
      .drain()
      .filter_map(|event| match event {
        GraphicsTerminalEvent::Scroll(scroll) => Some(scroll),
        GraphicsTerminalEvent::ClearAll => None,
      })
      .collect()
  }

  #[test]
  fn clear_and_reset_events_preserve_stream_order() {
    let mut tracker = GraphicsScrollTracker::new(2, 3);
    tracker.advance(b"\x1b[2;1H\n\x1b[2J\n\x1bc");

    assert_eq!(
      tracker.drain().collect::<Vec<_>>(),
      [
        GraphicsTerminalEvent::Scroll(GraphicsScroll {
          direction: GraphicsScrollDirection::Up,
          top: 0,
          bottom: 2,
          lines: 1,
        }),
        GraphicsTerminalEvent::ClearAll,
        GraphicsTerminalEvent::Scroll(GraphicsScroll {
          direction: GraphicsScrollDirection::Up,
          top: 0,
          bottom: 2,
          lines: 1,
        }),
        GraphicsTerminalEvent::ClearAll,
      ]
    );
  }

  #[test]
  fn resize_resets_the_scroll_region_to_the_full_screen() {
    let mut tracker = GraphicsScrollTracker::new(4, 4);
    tracker.advance(b"\x1b[2;3r");
    tracker.resize(3, 4);
    tracker.advance(b"\x1b[3;1H\n");

    assert_eq!(
      tracker.drain().collect::<Vec<_>>(),
      [GraphicsTerminalEvent::Scroll(GraphicsScroll {
        direction: GraphicsScrollDirection::Up,
        top: 0,
        bottom: 3,
        lines: 1,
      })]
    );
  }

  #[test]
  fn printable_input_wraps_and_scrolls_only_after_the_last_column() {
    assert!(events_with_size(2, 3, b"\x1b[2;1Habc").is_empty());
    assert_eq!(
      events_with_size(2, 3, b"\x1b[2;1Habcd"),
      [GraphicsScroll {
        direction: GraphicsScrollDirection::Up,
        top: 0,
        bottom: 2,
        lines: 1,
      }]
    );
  }

  #[test]
  fn wide_input_wraps_using_display_width() {
    assert_eq!(
      events_with_size(2, 2, "\x1b[2;1H界x".as_bytes()),
      [GraphicsScroll {
        direction: GraphicsScrollDirection::Up,
        top: 0,
        bottom: 2,
        lines: 1,
      }]
    );
  }

  #[test]
  fn disabled_line_wrap_does_not_scroll_printable_input() {
    assert!(events_with_size(2, 3, b"\x1b[?7l\x1b[2;1Habcdef").is_empty());
  }

  #[test]
  fn wrapping_below_decstbm_does_not_scroll_the_margin() {
    assert!(events_with_size(8, 2, b"\x1b[2;5r\x1b[8;1Hab").is_empty());
  }

  #[test]
  fn origin_mode_cursor_movement_stays_inside_decstbm() {
    assert_eq!(
      events_with_size(8, 2, b"\x1b[3;6r\x1b[?6h\x1b[20B\n"),
      [GraphicsScroll {
        direction: GraphicsScrollDirection::Up,
        top: 2,
        bottom: 6,
        lines: 1,
      }]
    );
  }

  #[test]
  fn disabling_origin_mode_keeps_the_cursor_inside_decstbm() {
    assert_eq!(
      events_with_size(8, 2, b"\x1b[3;6r\x1b[?6h\x1b[4;1H\x1b[?6l\n"),
      [GraphicsScroll {
        direction: GraphicsScrollDirection::Up,
        top: 2,
        bottom: 6,
        lines: 1,
      }]
    );
  }

  #[test]
  fn leaving_alternate_screen_restores_primary_wrap_pending_cursor() {
    assert_eq!(
      events_with_size(
        3,
        2,
        b"\x1b[3;1Hab\x1b[?1049h\x1b[H\x1b[?1049lc",
      ),
      [GraphicsScroll {
        direction: GraphicsScrollDirection::Up,
        top: 0,
        bottom: 3,
        lines: 1,
      }]
    );
  }

  #[test]
  fn linefeed_at_screen_bottom_reports_full_screen_scroll() {
    assert_eq!(
      events(5, b"\x1b[5;1H\n"),
      [GraphicsScroll {
        direction: GraphicsScrollDirection::Up,
        top: 0,
        bottom: 5,
        lines: 1,
      }]
    );
  }

  #[test]
  fn repeated_full_screen_scrolls_do_not_depend_on_history_growth() {
    assert_eq!(
      events(3, b"\x1b[3;1H\n\n\n"),
      vec![
        GraphicsScroll {
          direction: GraphicsScrollDirection::Up,
          top: 0,
          bottom: 3,
          lines: 1,
        };
        3
      ]
    );
  }

  #[test]
  fn linefeed_and_reverse_index_respect_decstbm() {
    assert_eq!(
      events(8, b"\x1b[3;6r\x1b[6;1H\n\x1b[3;1H\x1bM"),
      [
        GraphicsScroll {
          direction: GraphicsScrollDirection::Up,
          top: 2,
          bottom: 6,
          lines: 1,
        },
        GraphicsScroll {
          direction: GraphicsScrollDirection::Down,
          top: 2,
          bottom: 6,
          lines: 1,
        },
      ]
    );
  }

  #[test]
  fn explicit_scroll_insert_and_delete_report_affected_regions() {
    assert_eq!(
      events(8, b"\x1b[2;7r\x1b[3S\x1b[2T\x1b[4;1H\x1b[2L\x1b[M"),
      [
        GraphicsScroll {
          direction: GraphicsScrollDirection::Up,
          top: 1,
          bottom: 7,
          lines: 3,
        },
        GraphicsScroll {
          direction: GraphicsScrollDirection::Down,
          top: 1,
          bottom: 7,
          lines: 2,
        },
        GraphicsScroll {
          direction: GraphicsScrollDirection::Down,
          top: 3,
          bottom: 7,
          lines: 2,
        },
        GraphicsScroll {
          direction: GraphicsScrollDirection::Up,
          top: 3,
          bottom: 7,
          lines: 1,
        },
      ]
    );
  }
}
