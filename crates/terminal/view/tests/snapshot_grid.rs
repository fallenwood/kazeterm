//! Snapshot-based visual regression tests for the ANSI rendering pipeline.
//!
//! Renders a series of canned ANSI scenarios through an in-memory
//! `alacritty_terminal::Term` and snapshots the resulting grid as plain text.
//! Run `cargo insta review --workspace` to accept intentional changes.

use terminal_kernel::event::VoidListener;
use terminal_kernel::grid::Dimensions;
use terminal_kernel::index::{Column, Line, Point as AlacPoint};
use terminal_kernel::term::{Config, Term};
use terminal_kernel::vte::ansi::Processor;

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

fn render(cols: usize, lines: usize, bytes: &[u8]) -> String {
  let mut term = Term::new(Config::default(), &Dims { cols, lines }, VoidListener);
  let mut parser: Processor = Processor::new();
  parser.advance(&mut term, bytes);

  let mut out = String::new();
  for line in 0..term.screen_lines() {
    let mut row = String::new();
    for col in 0..term.columns() {
      let point = AlacPoint::new(Line(line as i32), Column(col));
      let cell = &term.grid()[point];
      row.push(if cell.c == '\0' { ' ' } else { cell.c });
    }
    out.push_str(row.trim_end());
    out.push('\n');
  }
  out
}

#[test]
fn snapshot_shell_like_prompt() {
  let frame = render(40, 4, b"user@host:~$ echo hello\r\nhello\r\nuser@host:~$ ");
  insta::assert_snapshot!(frame);
}

#[test]
fn snapshot_ls_output() {
  let frame = render(30, 6, b"$ ls\r\nCargo.toml  README.md  src\r\n$ ");
  insta::assert_snapshot!(frame);
}

#[test]
fn snapshot_cursor_movement_overwrites() {
  // Print "abcdef", then CSI 1;1H (home) and overwrite with "XY".
  let frame = render(10, 2, b"abcdef\x1b[1;1HXY");
  insta::assert_snapshot!(frame);
}

#[test]
fn snapshot_clear_screen() {
  let frame = render(12, 3, b"junk\r\nmore junk\x1b[2J\x1b[1;1Hfresh");
  insta::assert_snapshot!(frame);
}
