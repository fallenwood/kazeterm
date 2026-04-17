use gpui::{Pixels, px};
use terminal_kernel::{
  grid::Scroll,
  selection::{Selection, SelectionType},
};

use crate::mouse::grid_point_and_side;

use super::{InternalEvent, SelectionPhase, Terminal};

/// State of an active touch interaction (Windows touch-to-mouse).
pub enum TouchState {
  Pending {
    position: gpui::Point<Pixels>,
    start_time: std::time::Instant,
  },
  Scrolling {
    last_position: gpui::Point<Pixels>,
  },
  Selecting,
}

#[derive(PartialEq, Eq)]
pub enum TouchMode {
  Pending,
  Scrolling,
  Selecting,
}

impl Terminal {
  /// Begin a touch interaction (for Windows touch-to-mouse events).
  pub fn begin_touch(&mut self, position: gpui::Point<Pixels>) {
    self.touch_state = Some(TouchState::Pending {
      position,
      start_time: std::time::Instant::now(),
    });
    self.scroll_px = px(0.);
  }

  /// Handle touch move: returns the current touch mode.
  pub fn touch_move(&mut self, position: gpui::Point<Pixels>) -> Option<TouchMode> {
    let state = self.touch_state.take()?;
    match state {
      TouchState::Pending {
        position: start_pos,
        ..
      } => {
        let distance = (position.x - start_pos.x).abs() + (position.y - start_pos.y).abs();
        if distance > px(10.0) {
          self.touch_state = Some(TouchState::Scrolling {
            last_position: position,
          });
          let delta_y = position.y - start_pos.y;
          self.apply_touch_scroll_delta(delta_y);
          Some(TouchMode::Scrolling)
        } else {
          self.touch_state = Some(TouchState::Pending {
            position: start_pos,
            start_time: std::time::Instant::now(),
          });
          Some(TouchMode::Pending)
        }
      }
      TouchState::Scrolling { last_position } => {
        let delta_y = position.y - last_position.y;
        self.touch_state = Some(TouchState::Scrolling {
          last_position: position,
        });
        self.apply_touch_scroll_delta(delta_y);
        Some(TouchMode::Scrolling)
      }
      TouchState::Selecting => {
        let position = position - self.last_content.terminal_bounds.bounds.origin;
        self.selection_phase = SelectionPhase::Selecting;
        self
          .events
          .push_back(InternalEvent::UpdateSelection(position));
        self.touch_state = Some(TouchState::Selecting);
        Some(TouchMode::Selecting)
      }
    }
  }

  fn apply_touch_scroll_delta(&mut self, delta_y: Pixels) {
    let line_height = self.last_content.terminal_bounds.line_height;
    if line_height > px(0.) {
      self.scroll_px += delta_y;
      let scroll_lines = (self.scroll_px / line_height) as i32;
      if scroll_lines != 0 {
        self.scroll_px -= line_height * scroll_lines as f32;
        self
          .events
          .push_back(InternalEvent::Scroll(Scroll::Delta(scroll_lines)));
      }
    }
  }

  /// Promote a pending touch to selection mode (called by long-press timer).
  pub fn promote_touch_to_selection(&mut self) {
    if let Some(TouchState::Pending { position, .. }) = self.touch_state {
      self.start_touch_selection(position);
    }
  }

  /// Start touch selection at the given screen position.
  pub fn start_touch_selection(&mut self, position: gpui::Point<Pixels>) {
    let position = position - self.last_content.terminal_bounds.bounds.origin;
    let (point, side) = grid_point_and_side(
      position,
      self.last_content.terminal_bounds,
      self.last_content.display_offset,
    );
    let selection = Selection::new(SelectionType::Simple, point, side);
    self
      .events
      .push_back(InternalEvent::SetSelection(Some((selection, point))));
    self.touch_state = Some(TouchState::Selecting);
  }

  pub fn end_touch(&mut self) {
    if matches!(self.touch_state, Some(TouchState::Selecting)) {
      self.selection_phase = SelectionPhase::Ended;
    }
    self.touch_state = None;
  }

  pub fn is_touch_active(&self) -> bool {
    self.touch_state.is_some()
  }
}
