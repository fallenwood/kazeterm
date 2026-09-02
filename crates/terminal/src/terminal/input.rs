use std::borrow::Cow;
use std::process::ExitStatus;
use std::sync::atomic::Ordering;

use gpui::{Context, Keystroke};
use terminal_kernel::{
  grid::Scroll,
  index::{Column, Line, Point as AlacPoint},
};

use super::{Event, InternalEvent, Terminal};

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
      let after_ok =
        match_end >= line.len() || !is_word_char(line[match_end..].chars().next().unwrap_or(' '));
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

fn append_line_matches(
  line_number: Line,
  current_line_text: &str,
  search_state: &super::SearchState,
  match_ranges: &mut Vec<std::ops::RangeInclusive<AlacPoint>>,
) {
  let trimmed_len = current_line_text.trim_end().len();
  if trimmed_len == 0 {
    return;
  }

  let line_matches: Vec<(usize, usize)> = if let Some(ref regex) = search_state.compiled_regex {
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
    // Convert byte offsets to cell indices. Each terminal cell stores one
    // char, including wide-character spacer cells.
    let start_column = current_line_text[..byte_start].chars().count();
    let end_column = current_line_text[..byte_end]
      .chars()
      .count()
      .saturating_sub(1);
    match_ranges.push(
      AlacPoint::new(line_number, Column(start_column))
        ..=AlacPoint::new(line_number, Column(end_column)),
    );
  }
}

impl Terminal {
  pub fn get_content(&self) -> String {
    let start = AlacPoint::new(self.term.topmost_line(), Column(0));
    let end = AlacPoint::new(self.term.bottommost_line(), self.term.last_column());
    self.term.bounds_to_string(start, end)
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
    if let Some(txt) = self.term.selection_to_string() {
      cx.write_to_clipboard(gpui::ClipboardItem::new_string(txt));
    }
    self.term.set_selection(None);
    self.term.sync_selection_display(None);
    self.selection_display = None;
    self.selection_head = None;
    self.last_content.selection = None;
    self.last_content.selection_text = None;
    cx.emit(Event::SelectionsChanged);
    cx.notify();
  }

  pub fn try_keystroke(&mut self, keystroke: &Keystroke, alt_is_meta: bool) -> bool {
    let keyboard_protocol_flags = self.keyboard_protocol_flags.load(Ordering::Relaxed);
    let input = crate::mappings::keys::to_input_bytes(
      keystroke,
      &self.last_content.mode,
      alt_is_meta,
      keyboard_protocol_flags,
    );
    if let Some(input) = input {
      self.input(input);
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

      let display_offset = self.term.display_offset();
      let screen_lines = self.term.screen_lines() as i32;

      let match_line_i32 = match_line.0;
      let visible_top_line = -(display_offset as i32);
      let visual_line = match_line_i32 - visible_top_line;

      let target_line = 5;

      if visual_line < 0 || visual_line >= screen_lines || visual_line > 10 {
        let scroll_delta = visual_line - target_line;
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
        self.last_search_revision = self.content_revision;
        self.last_content.search_matches =
          Self::execute_search(&*self.term, self.search_state.as_ref().unwrap());
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
    term: &dyn terminal_kernel::TerminalBackend,
    search_state: &super::SearchState,
  ) -> Vec<std::ops::RangeInclusive<AlacPoint>> {
    let topmost_line = term.topmost_line();
    let columns = term.columns();

    let mut match_ranges = Vec::new();
    let mut current_line_number = topmost_line;
    let mut current_line_text = String::with_capacity(columns);

    term.iter_from(
      AlacPoint::new(topmost_line, Column(0)),
      &mut |point, cell| {
        if point.line != current_line_number {
          append_line_matches(
            current_line_number,
            &current_line_text,
            search_state,
            &mut match_ranges,
          );
          current_line_text.clear();
          current_line_number = point.line;
        }
        current_line_text.push(cell.c);
        true
      },
    );
    append_line_matches(
      current_line_number,
      &current_line_text,
      search_state,
      &mut match_ranges,
    );

    match_ranges
  }

  pub(super) fn register_task_finished(
    &mut self,
    exit_status: Option<ExitStatus>,
    cx: &mut Context<Terminal>,
  ) {
    if let Some(status) = exit_status {
      self.child_exited = Some(status);
    }

    if self.child_exited.is_none_or(|e| e.code() == Some(0)) {
      cx.emit(Event::CloseTerminal);
    }
  }

  /// Write the Input payload to the tty.
  pub(super) fn write_to_pty(&self, input: impl Into<Cow<'static, [u8]>>) {
    self.pty_tx.send_input(input.into());
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn line_search_maps_utf8_offsets_to_terminal_columns() {
    let state = super::super::SearchState::new("→".to_string(), true, false, false).unwrap();
    let mut matches = Vec::new();

    append_line_matches(Line(-2), "a→b ", &state, &mut matches);

    assert_eq!(
      matches,
      vec![AlacPoint::new(Line(-2), Column(1))..=AlacPoint::new(Line(-2), Column(1))]
    );
  }

  #[test]
  fn whole_word_search_rejects_embedded_matches() {
    let state = super::super::SearchState::new("cat".to_string(), true, true, false).unwrap();
    let mut matches = Vec::new();

    append_line_matches(Line(0), "cat scatter cat", &state, &mut matches);

    assert_eq!(
      matches,
      vec![
        AlacPoint::new(Line(0), Column(0))..=AlacPoint::new(Line(0), Column(2)),
        AlacPoint::new(Line(0), Column(12))..=AlacPoint::new(Line(0), Column(14))
      ]
    );
  }

  #[test]
  fn content_revision_invalidates_search_without_cursor_or_history_changes() {
    let (mut terminal, _events, _writes, _resizes) =
      crate::test_support::fake_terminal_session(8, 2);

    assert!(terminal.set_search_query("prompt".to_string(), true, false, false));
    assert_eq!(terminal.last_search_revision, terminal.content_revision);

    terminal.mark_content_changed();

    assert_ne!(terminal.last_search_revision, terminal.content_revision);
  }
}
