use std::ops::RangeInclusive;

use alacritty_terminal::{
  index::{Column, Line, Point as AlacPoint},
  selection::SelectionRange,
  term::{RenderableCursor, TermMode},
};

use crate::{indexed_cell::IndexedCell, terminal_bounds::TerminalBounds};

#[derive(Clone)]
pub struct TerminalContent {
  pub cells: Vec<IndexedCell>,
  pub mode: TermMode,
  pub display_offset: usize,
  pub selection_text: Option<String>,
  pub selection: Option<SelectionRange>,
  pub cursor: RenderableCursor,
  pub cursor_char: char,
  pub terminal_bounds: TerminalBounds,
  pub last_hovered_word: Option<crate::hover_target::HoveredWord>,
  pub history_size: usize,
  pub scrolled_to_top: bool,
  pub scrolled_to_bottom: bool,
  pub search_matches: Vec<RangeInclusive<AlacPoint>>,
  pub current_search_match_index: usize,
}

impl Default for TerminalContent {
  fn default() -> Self {
    TerminalContent {
      cells: Default::default(),
      mode: Default::default(),
      display_offset: Default::default(),
      selection_text: Default::default(),
      selection: Default::default(),
      cursor: RenderableCursor {
        shape: alacritty_terminal::vte::ansi::CursorShape::Block,
        point: AlacPoint::new(Line(0), Column(0)),
      },
      cursor_char: Default::default(),
      terminal_bounds: Default::default(),
      last_hovered_word: None,
      history_size: 0,
      scrolled_to_top: false,
      scrolled_to_bottom: false,
      search_matches: Vec::new(),
      current_search_match_index: 0,
    }
  }
}
