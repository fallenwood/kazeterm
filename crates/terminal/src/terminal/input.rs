use std::borrow::Cow;
use std::process::ExitStatus;

use alacritty_terminal::{
  event::Notify,
  grid::{Dimensions as _, Scroll},
  index::{Column, Point as AlacPoint},
};
use gpui::{Context, Keystroke};

use super::{Event, InternalEvent, Terminal};

impl Terminal {
  pub fn get_content(&self) -> String {
    let term = self.term.lock_unfair();
    let start = AlacPoint::new(term.topmost_line(), Column(0));
    let end = AlacPoint::new(term.bottommost_line(), term.last_column());
    term.bounds_to_string(start, end)
  }

  pub fn input(&mut self, input: impl Into<Cow<'static, [u8]>>) {
    self.last_input_time = std::time::Instant::now();
    self.events.push_back(InternalEvent::Scroll(Scroll::Bottom));
    self.events.push_back(InternalEvent::SetSelection(None));
    self.write_to_pty(input);
  }

  pub fn copy(&mut self, _cx: &mut Context<Self>) {
    self.events.push_back(InternalEvent::Copy(Some(true)));
  }

  /// Copy selection to clipboard and immediately clear the selection.
  pub fn copy_and_clear_selection(&mut self, cx: &mut Context<Self>) {
    let mut term = self.term.lock_unfair();
    if let Some(txt) = term.selection_to_string() {
      cx.write_to_clipboard(gpui::ClipboardItem::new_string(txt));
    }
    term.selection = None;
    self.last_content.selection = None;
    self.last_content.selection_text = None;
    cx.emit(Event::SelectionsChanged);
    cx.notify();
  }

  pub fn try_keystroke(&mut self, keystroke: &Keystroke, alt_is_meta: bool) -> bool {
    let esc = crate::mappings::keys::to_esc_str(keystroke, &self.last_content.mode, alt_is_meta);
    if let Some(esc) = esc {
      match esc {
        Cow::Borrowed(string) => self.input(string.as_bytes()),
        Cow::Owned(string) => self.input(string.into_bytes()),
      };
      true
    } else {
      false
    }
  }

  pub fn set_search_matches(&mut self, matches: Vec<std::ops::RangeInclusive<AlacPoint>>) {
    self.last_content.search_matches = matches;
  }

  pub fn set_current_search_match(&mut self, index: usize) {
    self.last_content.current_search_match_index = index;

    if index > 0 && index <= self.last_content.search_matches.len() {
      let match_range = &self.last_content.search_matches[index - 1];
      let match_line = match_range.start().line;

      let term = self.term.lock_unfair();
      let display_offset = term.grid().display_offset();
      let screen_lines = term.screen_lines() as i32;

      let match_line_i32 = match_line.0;
      let visible_top_line = -(display_offset as i32);
      let visual_line = match_line_i32 - visible_top_line;

      let target_line = 5;

      if visual_line < 0 || visual_line >= screen_lines || visual_line > 10 {
        let scroll_delta = visual_line - target_line;
        drop(term);
        if scroll_delta != 0 {
          self
            .events
            .push_back(InternalEvent::Scroll(Scroll::Delta(-scroll_delta)));
        }
      }
    }
  }

  /// Set the active search query. The search will be automatically re-run
  /// on every sync to keep matches current as terminal content changes.
  /// Returns `false` if `use_regex` was true but the pattern was invalid.
  pub fn set_search_query(
    &mut self,
    query: String,
    match_case: bool,
    match_whole: bool,
    use_regex: bool,
  ) -> bool {
    if query.is_empty() {
      self.clear_search_query();
      return true;
    }

    match super::SearchState::new(query, match_case, match_whole, use_regex) {
      Some(state) => {
        self.search_state = Some(state);
        // Run the search immediately so results are available this frame.
        let term = self.term.lock_unfair();
        self.search_fingerprint = (term.history_size(), self.last_content.cursor.point);
        self.last_content.search_matches =
          Self::execute_search(&term, self.search_state.as_ref().unwrap());
        let match_count = self.last_content.search_matches.len();
        self.last_content.current_search_match_index = if match_count > 0 { 1 } else { 0 };
        true
      }
      None => {
        // Invalid regex — clear matches but keep no active search.
        self.search_state = None;
        self.last_content.search_matches.clear();
        self.last_content.current_search_match_index = 0;
        false
      }
    }
  }

  /// Clear the active search query and all match highlights.
  pub fn clear_search_query(&mut self) {
    self.search_state = None;
    self.last_content.search_matches.clear();
    self.last_content.current_search_match_index = 0;
  }

  /// Execute the search against the current terminal grid content.
  pub(super) fn execute_search(
    term: &alacritty_terminal::Term<super::TerminalEventListener>,
    search_state: &super::SearchState,
  ) -> Vec<std::ops::RangeInclusive<AlacPoint>> {
    use alacritty_terminal::grid::Dimensions as _;

    fn is_word_char(c: char) -> bool {
      c.is_alphanumeric() || c == '_'
    }

    fn find_matches_simple(
      line: &str,
      query: &str,
      match_case: bool,
      match_whole: bool,
    ) -> Vec<(usize, usize)> {
      let mut matches = Vec::new();
      let (search_line, search_query) = if match_case {
        (line.to_string(), query.to_string())
      } else {
        (line.to_lowercase(), query.to_lowercase())
      };

      let mut start = 0;
      while let Some(pos) = search_line[start..].find(&search_query) {
        let match_start = start + pos;
        let match_end = match_start + query.len();

        if match_whole {
          let before_ok =
            match_start == 0 || !is_word_char(line[..match_start].chars().last().unwrap_or(' '));
          let after_ok = match_end >= line.len()
            || !is_word_char(line[match_end..].chars().next().unwrap_or(' '));
          if before_ok && after_ok {
            matches.push((match_start, match_end));
          }
        } else {
          matches.push((match_start, match_end));
        }
        start = match_start + 1;
      }
      matches
    }

    let topmost_line = term.topmost_line();
    let bottommost_line = term.bottommost_line();
    let columns = term.columns();

    let mut match_ranges = Vec::new();
    let mut line = topmost_line;
    while line <= bottommost_line {
      let mut current_line_text = String::new();
      let mut current_line_cells = Vec::new();

      for col in 0..columns {
        let point = AlacPoint::new(line, Column(col));
        let cell = &term.grid()[point];
        current_line_text.push(cell.c);
        current_line_cells.push(point);
      }

      let trimmed_len = current_line_text.trim_end().len();

      if trimmed_len > 0 {
        let line_matches: Vec<(usize, usize)> = if let Some(ref regex) = search_state.compiled_regex
        {
          regex
            .find_iter(&current_line_text[..trimmed_len])
            .map(|m| (m.start(), m.end()))
            .collect()
        } else {
          find_matches_simple(
            &current_line_text[..trimmed_len],
            &search_state.query,
            search_state.match_case,
            search_state.match_whole,
          )
        };

        for (byte_start, byte_end) in line_matches {
          // Convert byte offsets to character indices (= cell indices),
          // since multibyte chars (e.g. "→") occupy 1 cell but multiple bytes.
          let start_pos = current_line_text[..byte_start].chars().count();
          let end_pos = current_line_text[..byte_end]
            .chars()
            .count()
            .saturating_sub(1);
          if start_pos < current_line_cells.len() {
            let match_start = current_line_cells[start_pos];
            let match_end_idx = end_pos.min(current_line_cells.len().saturating_sub(1));
            let match_end = current_line_cells[match_end_idx];
            match_ranges.push(match_start..=match_end);
          }
        }
      }

      line += 1;
    }

    match_ranges
  }

  pub(super) fn register_task_finished(
    &mut self,
    error_code: Option<i32>,
    cx: &mut Context<Terminal>,
  ) {
    let e: Option<ExitStatus> = error_code.map(|code| {
      #[cfg(unix)]
      {
        std::os::unix::process::ExitStatusExt::from_raw(code)
      }
      #[cfg(windows)]
      {
        std::os::windows::process::ExitStatusExt::from_raw(code as u32)
      }
    });

    if let Some(e) = e {
      self.child_exited = Some(e);
    }

    if self.child_exited.is_none_or(|e| e.code() == Some(0)) {
      cx.emit(Event::CloseTerminal);
    }
  }

  /// Write the Input payload to the tty.
  pub(super) fn write_to_pty(&self, input: impl Into<Cow<'static, [u8]>>) {
    self.pty_tx.notify(input.into());
  }
}
