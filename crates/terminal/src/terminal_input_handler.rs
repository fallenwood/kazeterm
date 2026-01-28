use alacritty_terminal::term::TermMode;
use gpui::{App, Bounds, Entity, InputHandler, Pixels, Point, UTF16Selection, Window};

use crate::{Terminal, TerminalView};

pub struct TerminalInputHandler {
  pub terminal: Entity<Terminal>,
  pub terminal_view: Entity<TerminalView>,
  pub cursor_bounds: Option<Bounds<Pixels>>,
}

impl InputHandler for TerminalInputHandler {
  fn selected_text_range(
    &mut self,
    _ignore_disabled_input: bool,
    _: &mut Window,
    cx: &mut App,
  ) -> Option<UTF16Selection> {
    if self
      .terminal
      .read(cx)
      .last_content
      .mode
      .contains(TermMode::ALT_SCREEN)
    {
      None
    } else {
      Some(UTF16Selection {
        range: 0..0,
        reversed: false,
      })
    }
  }

  fn marked_text_range(
    &mut self,
    _window: &mut Window,
    cx: &mut App,
  ) -> Option<std::ops::Range<usize>> {
    self.terminal_view.read(cx).marked_text_range()
  }

  fn text_for_range(
    &mut self,
    _: std::ops::Range<usize>,
    _: &mut Option<std::ops::Range<usize>>,
    _: &mut Window,
    _: &mut App,
  ) -> Option<String> {
    None
  }

  fn replace_text_in_range(
    &mut self,
    _replacement_range: Option<std::ops::Range<usize>>,
    text: &str,
    _window: &mut Window,
    cx: &mut App,
  ) {
    self.terminal_view.update(cx, |view, view_cx| {
      view.clear_marked_text(view_cx);
      view.commit_text(text, view_cx);
    });
  }

  fn replace_and_mark_text_in_range(
    &mut self,
    _range_utf16: Option<std::ops::Range<usize>>,
    new_text: &str,
    new_marked_range: Option<std::ops::Range<usize>>,
    _window: &mut Window,
    cx: &mut App,
  ) {
    self.terminal_view.update(cx, |view, view_cx| {
      view.set_marked_text(new_text.to_string(), new_marked_range, view_cx);
    });
  }

  fn unmark_text(&mut self, _window: &mut Window, cx: &mut App) {
    self.terminal_view.update(cx, |view, view_cx| {
      view.clear_marked_text(view_cx);
    });
  }

  fn bounds_for_range(
    &mut self,
    range_utf16: std::ops::Range<usize>,
    _window: &mut Window,
    cx: &mut App,
  ) -> Option<Bounds<Pixels>> {
    let term_bounds = self.terminal_view.read(cx).terminal_bounds(cx);

    let mut bounds = self.cursor_bounds?;
    let offset_x = term_bounds.cell_width * range_utf16.start as f32;
    bounds.origin.x += offset_x;

    Some(bounds)
  }

  fn apple_press_and_hold_enabled(&mut self) -> bool {
    false
  }

  fn character_index_for_point(
    &mut self,
    _point: Point<Pixels>,
    _window: &mut Window,
    _cx: &mut App,
  ) -> Option<usize> {
    None
  }
}
