//! `TerminalBackend` trait and `AlacrittyBackend` implementation.

use std::sync::Arc;

use crate::event::EventListener;
use crate::grid::Dimensions;
use crate::index::{Boundary, Column, Direction, Line, Point as AlacPoint};
use crate::selection::{Selection, SelectionRange};
use crate::sync::FairMutex;
use crate::term::cell::Cell;
use crate::term::{RenderableCursor, TermMode};
use crate::vte::ansi::{CursorStyle, Rgb};
use crate::{Term, grid};

/// A snapshot of the renderable terminal content, independent of backend.
pub struct RenderableSnapshot {
  pub cells: Vec<(AlacPoint, Cell)>,
  pub mode: TermMode,
  pub display_offset: usize,
  pub cursor: RenderableCursor,
  pub selection: Option<SelectionRange>,
}

/// Trait abstracting the terminal emulator backend.
///
/// Implementations wrap a specific terminal library (alacritty, vte, etc.)
/// and handle their own internal locking. All methods take `&self`.
///
/// No references to internal state are returned — everything is by value.
///
/// Object-safe: can be used as `dyn TerminalBackend`.
pub trait TerminalBackend: Send + Sync {
  // --- Dimensions ---

  fn history_size(&self) -> usize;
  fn screen_lines(&self) -> usize;
  fn columns(&self) -> usize;
  fn total_lines(&self) -> usize {
    self.history_size() + self.screen_lines()
  }
  fn topmost_line(&self) -> Line;
  fn bottommost_line(&self) -> Line;
  fn last_column(&self) -> Column;

  // --- Grid cell access (by value) ---

  fn cell_at(&self, point: AlacPoint) -> Cell;
  fn display_offset(&self) -> usize;

  // --- Cursor ---

  fn cursor_point(&self) -> AlacPoint;
  fn cursor_style(&self) -> CursorStyle;

  // --- Rendering ---

  fn renderable_snapshot(&self) -> RenderableSnapshot;

  // --- Colors ---

  fn color_at(&self, index: usize) -> Option<Rgb>;

  // --- Text extraction ---

  fn selection_to_string(&self) -> Option<String>;
  fn bounds_to_string(&self, start: AlacPoint, end: AlacPoint) -> String;

  // --- Selection state ---

  fn get_selection(&self) -> Option<Selection>;
  fn set_selection(&self, sel: Option<Selection>);
  fn take_selection(&self) -> Option<Selection>;
  /// Atomic read-modify-write on the selection.
  fn update_selection(&self, f: &mut dyn FnMut(&mut Option<Selection>));

  // --- Mutations (lock internally) ---

  fn resize(&self, lines: usize, cols: usize);
  fn scroll_display(&self, scroll: grid::Scroll);
  fn scroll_to_point(&self, point: AlacPoint);

  // --- Point arithmetic (grid-aware) ---

  fn point_add(&self, point: AlacPoint, boundary: Boundary, n: usize) -> AlacPoint;
  fn point_sub(&self, point: AlacPoint, boundary: Boundary, n: usize) -> AlacPoint;
  fn grid_clamp(&self, point: AlacPoint, boundary: Boundary) -> AlacPoint;
  fn expand_wide(&self, point: AlacPoint, direction: Direction) -> AlacPoint;

  // --- Grid iteration ---

  /// Iterate cells starting from `point`, calling `f` for each.
  /// Return `false` from `f` to stop iteration.
  /// The lock is held for the duration of the callback.
  fn iter_from(&self, point: AlacPoint, f: &mut dyn FnMut(AlacPoint, &Cell) -> bool);

  fn line_search_left(&self, point: AlacPoint) -> AlacPoint;
  fn line_search_right(&self, point: AlacPoint) -> AlacPoint;

  // --- Hyperlink support ---

  /// Find a hyperlink (explicit OSC 8 or regex-matched URL) at the given grid point.
  /// Returns `(url, is_url, match_range)` or `None`.
  fn find_hyperlink_at(
    &self,
    point: AlacPoint,
    url_regex_pattern: &str,
  ) -> Option<(String, bool, std::ops::RangeInclusive<AlacPoint>)>;
}

// ---------------------------------------------------------------------------
// AlacrittyBackend
// ---------------------------------------------------------------------------

/// Backend wrapping `alacritty_terminal::Term<L>` behind an `Arc<FairMutex<…>>`.
///
/// The `Arc` is shared with the alacritty `EventLoop` so that both sides can
/// access the same `Term`. All trait methods lock internally.
pub struct AlacrittyBackend<L: EventListener + Send> {
  term: Arc<FairMutex<Term<L>>>,
}

impl<L: EventListener + Send> AlacrittyBackend<L> {
  pub fn new(term: Arc<FairMutex<Term<L>>>) -> Self {
    Self { term }
  }

  /// Get the shared term Arc for the EventLoop.
  pub fn term_arc(&self) -> Arc<FairMutex<Term<L>>> {
    self.term.clone()
  }
}

impl<L: EventListener + Send> TerminalBackend for AlacrittyBackend<L> {
  fn history_size(&self) -> usize {
    self.term.lock().history_size()
  }

  fn screen_lines(&self) -> usize {
    self.term.lock().screen_lines()
  }

  fn columns(&self) -> usize {
    self.term.lock().columns()
  }

  fn topmost_line(&self) -> Line {
    self.term.lock().topmost_line()
  }

  fn bottommost_line(&self) -> Line {
    self.term.lock().bottommost_line()
  }

  fn last_column(&self) -> Column {
    self.term.lock().last_column()
  }

  fn cell_at(&self, point: AlacPoint) -> Cell {
    self.term.lock().grid()[point].clone()
  }

  fn display_offset(&self) -> usize {
    self.term.lock().grid().display_offset()
  }

  fn cursor_point(&self) -> AlacPoint {
    self.term.lock().grid().cursor.point
  }

  fn cursor_style(&self) -> CursorStyle {
    self.term.lock().cursor_style()
  }

  fn renderable_snapshot(&self) -> RenderableSnapshot {
    let term = self.term.lock();
    let content = term.renderable_content();
    let cells: Vec<(AlacPoint, Cell)> = content
      .display_iter
      .map(|ic| (ic.point, ic.cell.clone()))
      .collect();
    RenderableSnapshot {
      cells,
      mode: content.mode,
      display_offset: content.display_offset,
      cursor: content.cursor,
      selection: content.selection,
    }
  }

  fn color_at(&self, index: usize) -> Option<Rgb> {
    self.term.lock().colors()[index]
  }

  fn selection_to_string(&self) -> Option<String> {
    self.term.lock().selection_to_string()
  }

  fn bounds_to_string(&self, start: AlacPoint, end: AlacPoint) -> String {
    self.term.lock().bounds_to_string(start, end)
  }

  fn get_selection(&self) -> Option<Selection> {
    self.term.lock().selection.clone()
  }

  fn set_selection(&self, sel: Option<Selection>) {
    self.term.lock().selection = sel;
  }

  fn take_selection(&self) -> Option<Selection> {
    self.term.lock().selection.take()
  }

  fn update_selection(&self, f: &mut dyn FnMut(&mut Option<Selection>)) {
    let mut term = self.term.lock();
    f(&mut term.selection);
  }

  fn resize(&self, lines: usize, cols: usize) {
    /// Minimal `Dimensions` adapter for `Term::resize`.
    struct ResizeDims {
      lines: usize,
      cols: usize,
    }
    impl Dimensions for ResizeDims {
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
    self.term.lock().resize(ResizeDims { lines, cols });
  }

  fn scroll_display(&self, scroll: grid::Scroll) {
    self.term.lock().scroll_display(scroll);
  }

  fn scroll_to_point(&self, point: AlacPoint) {
    self.term.lock().scroll_to_point(point);
  }

  fn point_add(&self, point: AlacPoint, boundary: Boundary, n: usize) -> AlacPoint {
    let term = self.term.lock();
    point.add(&*term, boundary, n)
  }

  fn point_sub(&self, point: AlacPoint, boundary: Boundary, n: usize) -> AlacPoint {
    let term = self.term.lock();
    point.sub(&*term, boundary, n)
  }

  fn grid_clamp(&self, point: AlacPoint, boundary: Boundary) -> AlacPoint {
    let term = self.term.lock();
    point.grid_clamp(&*term, boundary)
  }

  fn expand_wide(&self, point: AlacPoint, direction: Direction) -> AlacPoint {
    self.term.lock().expand_wide(point, direction)
  }

  fn iter_from(&self, start: AlacPoint, f: &mut dyn FnMut(AlacPoint, &Cell) -> bool) {
    let term = self.term.lock();
    for cell in term.grid().iter_from(start) {
      if !f(cell.point, &cell.cell) {
        break;
      }
    }
  }

  fn line_search_left(&self, point: AlacPoint) -> AlacPoint {
    self.term.lock().line_search_left(point)
  }

  fn line_search_right(&self, point: AlacPoint) -> AlacPoint {
    self.term.lock().line_search_right(point)
  }

  fn find_hyperlink_at(
    &self,
    point: AlacPoint,
    url_regex_pattern: &str,
  ) -> Option<(String, bool, std::ops::RangeInclusive<AlacPoint>)> {
    use crate::term::search::{RegexIter, RegexSearch};

    let term = self.term.lock();
    let grid = term.grid();
    let link = grid[point].hyperlink();

    // Check for explicit OSC 8 hyperlink first.
    if let Some(ref url) = link {
      let mut min_index = point;
      loop {
        let new_min = min_index.sub(&*term, Boundary::Cursor, 1);
        if new_min == min_index || grid[new_min].hyperlink() != link {
          break;
        }
        min_index = new_min;
      }
      let mut max_index = point;
      loop {
        let new_max = max_index.add(&*term, Boundary::Cursor, 1);
        if new_max == max_index || grid[new_max].hyperlink() != link {
          break;
        }
        max_index = new_max;
      }
      return Some((url.uri().to_owned(), true, min_index..=max_index));
    }

    // Fall back to regex-based URL search.
    let mut url_regex = RegexSearch::new(url_regex_pattern).ok()?;
    let (line_start, line_end) = (term.line_search_left(point), term.line_search_right(point));
    RegexIter::new(
      line_start,
      line_end,
      Direction::Right,
      &*term,
      &mut url_regex,
    )
    .find(|rm| rm.contains(&point))
    .map(|url_match| {
      let url = term.bounds_to_string(*url_match.start(), *url_match.end());
      (url, true, url_match)
    })
  }
}
