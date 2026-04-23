use std::ops::RangeInclusive;

use gpui::{
  Entity, FocusHandle, Hsla, InteractiveElement, IntoElement, Pixels, Point, ShapedLine,
  StatefulInteractiveElement, TextStyle, point,
};
use terminal_kernel::{ANSI_COLOR_COUNT, grid::Dimensions, index::Point as AlacPoint, term::TermMode, vte::ansi::Rgb};

use crate::{cursor_layout::CursorLayout, minimap::MinimapState, scrollbar::ScrollbarState};

use super::batched_text_run::BatchedTextRun;
use super::layout_rect::LayoutRect;
use super::terminal::Terminal;
use super::terminal_bounds::TerminalBounds;
use super::terminal_content::TerminalContent;
use super::terminal_view::TerminalView;

mod element_impl;
mod grid_layout;
mod helpers;
mod mouse_handlers;

/// Check if the current mouse event originated from a touch screen on Windows.
#[cfg(target_os = "windows")]
fn is_mouse_from_touch() -> bool {
  use windows::Win32::UI::WindowsAndMessaging::GetMessageExtraInfo;
  const MI_WP_SIGNATURE: isize = 0xFF515700;
  const SIGNATURE_MASK: isize = 0xFFFFFF00u32 as isize;
  let extra = unsafe { GetMessageExtraInfo() };
  (extra.0 & SIGNATURE_MASK) == MI_WP_SIGNATURE
}

#[cfg(not(target_os = "windows"))]
fn is_mouse_from_touch() -> bool {
  false
}

pub struct TerminalElement {
  terminal: Entity<Terminal>,
  terminal_view: Entity<TerminalView>,
  focus: FocusHandle,
  focused: bool,
  cursor_visible: bool,
  /// Whether this terminal is in an inactive split pane (colors will be desaturated).
  inactive: bool,
  interactivity: gpui::Interactivity,
}

impl TerminalElement {
  pub fn new(
    terminal: Entity<Terminal>,
    terminal_view: Entity<TerminalView>,
    focus: FocusHandle,
    focused: bool,
    cursor_visible: bool,
    inactive: bool,
    interactivity: gpui::Interactivity,
  ) -> Self {
    Self {
      terminal,
      terminal_view,
      focus: focus.clone(),
      focused,
      cursor_visible,
      inactive,
      interactivity,
    }
    .track_focus(&focus)
  }

  fn shape_cursor(
    cursor_point: helpers::DisplayCursor,
    size: TerminalBounds,
    text_fragment: &ShapedLine,
  ) -> Option<(Point<Pixels>, Pixels)> {
    if cursor_point.line() < size.total_lines() as i32 {
      let cursor_width = if text_fragment.width == Pixels::ZERO {
        size.cell_width()
      } else {
        text_fragment.width
      };

      Some((
        point(
          (cursor_point.col() as f32 * size.cell_width()).floor(),
          (cursor_point.line() as f32 * size.line_height()).floor(),
        ),
        cursor_width.ceil(),
      ))
    } else {
      None
    }
  }
}

impl InteractiveElement for TerminalElement {
  fn interactivity(&mut self) -> &mut gpui::Interactivity {
    &mut self.interactivity
  }
}

impl StatefulInteractiveElement for TerminalElement {}

impl IntoElement for TerminalElement {
  type Element = Self;

  fn into_element(self) -> Self::Element {
    self
  }
}

/// The information generated during layout that is necessary for painting.
pub struct LayoutState {
  hitbox: gpui::Hitbox,
  batched_text_runs: Vec<BatchedTextRun>,
  rects: Vec<LayoutRect>,
  relative_highlighted_ranges: Vec<(RangeInclusive<AlacPoint>, Hsla)>,
  cursor: Option<CursorLayout>,
  background_color: Hsla,
  dimensions: TerminalBounds,
  mode: TermMode,
  display_offset: usize,
  gutter: Pixels,
  base_text_style: TextStyle,
  scrollbar_state: Option<ScrollbarState>,
  scrollbar_bounds: Option<gpui::Bounds<Pixels>>,
  minimap_state: Option<MinimapState>,
  minimap_bounds: Option<gpui::Bounds<Pixels>>,
  color_table: [Option<Rgb>; ANSI_COLOR_COUNT],
  minimap_cells: Vec<crate::indexed_cell::IndexedCell>,
  image_placements: Vec<crate::kitty_graphics::VisiblePlacement>,
}
