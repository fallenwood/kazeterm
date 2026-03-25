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
    let term = self.term.lock_unfair();
    self.last_content.search_history_size = term.history_size();
    drop(term);
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
