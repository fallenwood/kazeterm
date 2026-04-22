//! Headless terminal-behaviour integration tests.
//!
//! These tests drive an in-memory `alacritty_terminal::Term` through the ANSI
//! `vte::ansi::Processor` — no PTY, no child process, no GPUI window. They
//! assert on the grid state that our renderer would eventually display, which
//! is a good proxy for "what the user sees" across a broad set of escape
//! sequences.
//!
//! The snapshot helpers here form the foundation for future visual-regression
//! tests (see `snapshot_grid.rs`).

use terminal_kernel::event::VoidListener;
use terminal_kernel::grid::Dimensions;
use terminal_kernel::index::{Column, Line, Point as AlacPoint};
use terminal_kernel::term::{Config, Term};
use terminal_kernel::vte::ansi::Processor;
use terminal::TerminalBounds;

fn make_term(cols: usize, lines: usize) -> Term<VoidListener> {
  struct Dims {
    cols: usize,
    lines: usize,
  }
  impl Dimensions for Dims {
    fn total_lines(&self) -> usize {
      self.lines
    }
    fn screen_lines(&self) -> usize {
      self.lines
    }
    fn columns(&self) -> usize {
      self.cols
    }
  }
  Term::new(Config::default(), &Dims { cols, lines }, VoidListener)
}

fn advance(term: &mut Term<VoidListener>, bytes: &[u8]) {
  let mut parser: Processor = Processor::new();
  parser.advance(term, bytes);
}

/// Render the visible grid as a `Vec<String>`, trimming trailing whitespace.
fn grid_to_lines(term: &Term<VoidListener>) -> Vec<String> {
  let mut lines = Vec::with_capacity(term.screen_lines());
  for line in 0..term.screen_lines() {
    let mut row = String::new();
    for col in 0..term.columns() {
      let point = AlacPoint::new(Line(line as i32), Column(col));
      let cell = &term.grid()[point];
      row.push(if cell.c == '\0' { ' ' } else { cell.c });
    }
    lines.push(row.trim_end().to_string());
  }
  lines
}

#[test]
fn plain_text_lands_in_the_grid() {
  let mut term = make_term(40, 5);
  advance(&mut term, b"hello, world");

  let lines = grid_to_lines(&term);
  assert_eq!(lines[0], "hello, world");
  for line in &lines[1..] {
    assert!(line.is_empty(), "unexpected content on blank line: {line:?}");
  }
}

#[test]
fn crlf_moves_to_next_line() {
  let mut term = make_term(20, 4);
  advance(&mut term, b"line-a\r\nline-b\r\nline-c");

  let lines = grid_to_lines(&term);
  assert_eq!(lines[0], "line-a");
  assert_eq!(lines[1], "line-b");
  assert_eq!(lines[2], "line-c");
}

#[test]
fn erase_in_line_clears_the_current_row() {
  let mut term = make_term(20, 3);
  advance(&mut term, b"to-be-erased");
  // Move cursor to column 1, then `ESC [ 2 K` erases the whole line.
  advance(&mut term, b"\r\x1b[2K");

  let lines = grid_to_lines(&term);
  assert!(
    lines[0].is_empty(),
    "expected line 0 to be erased, got {:?}",
    lines[0]
  );
}

#[test]
fn sgr_color_sequences_do_not_corrupt_text() {
  let mut term = make_term(40, 2);
  // Red "ERR" then reset then plain "ok".
  advance(&mut term, b"\x1b[31mERR\x1b[0m ok");

  let lines = grid_to_lines(&term);
  assert_eq!(lines[0], "ERR ok");
}

#[test]
fn cursor_position_tracks_input() {
  let mut term = make_term(10, 3);
  advance(&mut term, b"abc\r\nde");

  let cursor = term.grid().cursor.point;
  assert_eq!(cursor.line.0, 1);
  assert_eq!(cursor.column.0, 2);
}

#[test]
fn default_terminal_bounds_is_sane() {
  // The helper used throughout the crate should produce a 1-row, 1-col terminal
  // (sentinel value) that's still large enough to construct without panicking.
  let b = TerminalBounds::default();
  // Guard against zero cell size which would NaN out layout math.
  assert!(f32::from(b.cell_width) > 0.0);
  assert!(f32::from(b.line_height) > 0.0);
}
