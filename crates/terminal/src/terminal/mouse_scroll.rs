use alacritty_terminal::{
  grid::Scroll,
  index::{Direction, Point as AlacPoint},
  selection::{Selection, SelectionType},
  term::TermMode,
};
use gpui::{Context, Pixels, TouchPhase, px};

use crate::mouse::grid_point_and_side;

use super::{
  Event, InternalEvent, SelectionPhase, Terminal, TouchMode, TouchState, content_index_for_mouse,
};

impl Terminal {
  pub fn mouse_mode(&self, shift: bool) -> bool {
    self.last_content.mode.intersects(TermMode::MOUSE_MODE) && !shift
  }

  pub fn mouse_changed(&mut self, point: AlacPoint, side: Direction) -> bool {
    match self.last_mouse {
      Some((old_point, old_side)) => {
        if old_point == point && old_side == side {
          false
        } else {
          self.last_mouse = Some((point, side));
          true
        }
      }
      None => {
        self.last_mouse = Some((point, side));
        true
      }
    }
  }

  pub fn mouse_move(&mut self, e: &gpui::MouseMoveEvent, cx: &mut Context<Self>) {
    let position = e.position - self.last_content.terminal_bounds.bounds.origin;
    let mut should_clear_last_hovered_word = false;

    if self.mouse_mode(e.modifiers.shift) {
      let (point, side) = grid_point_and_side(
        position,
        self.last_content.terminal_bounds,
        self.last_content.display_offset,
      );

      if self.mouse_changed(point, side)
        && let Some(bytes) = crate::mappings::mouse::mouse_moved_report(
          point,
          e.pressed_button,
          e.modifiers,
          self.last_content.mode,
        )
      {
        self.write_to_pty(bytes);
      }
      should_clear_last_hovered_word = true;
    } else if e.modifiers.secondary() {
      self.word_from_position(e.position);
    } else {
      should_clear_last_hovered_word = true;
    }

    if should_clear_last_hovered_word {
      self.last_content.last_hovered_word = None;
    }

    cx.notify();
  }

  fn word_from_position(&mut self, position: gpui::Point<Pixels>) {
    if self.selection_phase == SelectionPhase::Selecting {
      self.last_content.last_hovered_word = None;
    } else if self.last_content.terminal_bounds.bounds.contains(&position) {
      let now = std::time::Instant::now();
      let should_search = if let Some(last_pos) = self.last_hyperlink_search_position {
        let distance_moved =
          ((position.x - last_pos.x).abs() + (position.y - last_pos.y).abs()) > px(5.0);
        let time_elapsed = now.duration_since(self.last_mouse_move_time).as_millis() > 100;
        distance_moved || time_elapsed
      } else {
        true
      };

      if should_search {
        self.last_mouse_move_time = now;
        self.last_hyperlink_search_position = Some(position);
        self.events.push_back(InternalEvent::FindHyperlink(
          position - self.last_content.terminal_bounds.bounds.origin,
          false,
        ));
      }
    } else {
      self.last_content.last_hovered_word = None;
    }
  }

  pub fn select_word_at_event_position(&mut self, e: &gpui::MouseDownEvent) {
    let position = e.position - self.last_content.terminal_bounds.bounds.origin;
    let (point, side) = grid_point_and_side(
      position,
      self.last_content.terminal_bounds,
      self.last_content.display_offset,
    );
    let selection = Selection::new(SelectionType::Semantic, point, side);
    self
      .events
      .push_back(InternalEvent::SetSelection(Some((selection, point))));
  }

  pub fn mouse_down(&mut self, e: &gpui::MouseDownEvent, _cx: &mut Context<Self>) {
    let position = e.position - self.last_content.terminal_bounds.bounds.origin;
    let point = crate::mappings::mouse::grid_point(
      position,
      self.last_content.terminal_bounds,
      self.last_content.display_offset,
    );

    if self.mouse_mode(e.modifiers.shift) {
      if let Some(bytes) = crate::mappings::mouse::mouse_button_report(
        point,
        e.button,
        e.modifiers,
        true,
        self.last_content.mode,
      ) {
        self.write_to_pty(bytes);
      }
    } else {
      match e.button {
        gpui::MouseButton::Left => {
          let (point, side) = grid_point_and_side(
            position,
            self.last_content.terminal_bounds,
            self.last_content.display_offset,
          );

          let selection_type = match e.click_count {
            0 => return,
            1 => Some(SelectionType::Simple),
            2 => Some(SelectionType::Semantic),
            3 => Some(SelectionType::Lines),
            _ => None,
          };

          if selection_type == Some(SelectionType::Simple) && e.modifiers.shift {
            self
              .events
              .push_back(InternalEvent::UpdateSelection(position));
            return;
          }

          let selection =
            selection_type.map(|selection_type| Selection::new(selection_type, point, side));

          if let Some(sel) = selection {
            self
              .events
              .push_back(InternalEvent::SetSelection(Some((sel, point))));
          }
        }
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        gpui::MouseButton::Middle => {
          if let Some(item) = _cx.read_from_primary() {
            let text = item.text().unwrap_or_default();
            self.input(text.into_bytes());
          }
        }
        _ => {}
      }
    }
  }

  pub fn mouse_up(&mut self, e: &gpui::MouseUpEvent, cx: &Context<Self>) {
    let position = e.position - self.last_content.terminal_bounds.bounds.origin;
    if self.mouse_mode(e.modifiers.shift) {
      let point = crate::mappings::mouse::grid_point(
        position,
        self.last_content.terminal_bounds,
        self.last_content.display_offset,
      );

      if let Some(bytes) = crate::mappings::mouse::mouse_button_report(
        point,
        e.button,
        e.modifiers,
        false,
        self.last_content.mode,
      ) {
        self.write_to_pty(bytes);
      }
    } else if self.selection_phase == SelectionPhase::Ended {
      let mouse_cell_index = content_index_for_mouse(position, &self.last_content.terminal_bounds);
      if let Some(link) = self.last_content.cells[mouse_cell_index].hyperlink() {
        cx.open_url(link.uri());
      } else if e.modifiers.secondary() {
        self
          .events
          .push_back(InternalEvent::FindHyperlink(position, true));
      }
    }

    self.selection_phase = SelectionPhase::Ended;
    self.last_mouse = None;
  }

  fn determine_scroll_lines(
    &mut self,
    e: &gpui::ScrollWheelEvent,
    scroll_multiplier: f32,
  ) -> Option<i32> {
    let line_height = self.last_content.terminal_bounds.line_height;
    if line_height == px(0.) {
      return None;
    }

    let delta_y = e.delta.pixel_delta(line_height).y * scroll_multiplier;
    if delta_y.abs() < px(0.1) {
      return None;
    }

    let scroll_lines = (delta_y / line_height) as i32;
    if scroll_lines != 0 {
      Some(scroll_lines)
    } else {
      self.scroll_px += delta_y;
      let accumulated_lines = (self.scroll_px / line_height) as i32;
      if accumulated_lines != 0 {
        self.scroll_px -= line_height * accumulated_lines as f32;
        Some(accumulated_lines)
      } else {
        None
      }
    }
  }

  pub fn scroll_to_bottom(&mut self) {
    self.scroll(Scroll::Bottom);
  }

  pub fn scroll(&mut self, scroll: Scroll) {
    self.events.push_back(InternalEvent::Scroll(scroll));
  }

  /// Scroll the terminal, returns true if momentum scrolling should start.
  pub fn scroll_wheel(&mut self, e: &gpui::ScrollWheelEvent, scroll_multiplier: f32) -> bool {
    let mouse_mode = self.mouse_mode(e.shift);
    let scroll_multiplier = if mouse_mode { 1. } else { scroll_multiplier };

    let line_height = self.last_content.terminal_bounds.line_height;
    if line_height > px(0.) {
      let delta_y = e.delta.pixel_delta(line_height).y;
      let now = std::time::Instant::now();

      match e.touch_phase {
        TouchPhase::Started => {
          self.scroll_velocity = 0.0;
          self.last_scroll_time = Some(now);
        }
        TouchPhase::Moved => {
          if let Some(last_time) = self.last_scroll_time {
            let dt = now.duration_since(last_time).as_secs_f32();
            if dt > 0.0 && dt < 0.1 {
              let instant_velocity = f32::from(delta_y) / dt;
              self.scroll_velocity = self.scroll_velocity * 0.3 + instant_velocity * 0.7;
            }
          }
          self.last_scroll_time = Some(now);
        }
        TouchPhase::Ended => {
          self.last_scroll_time = None;
        }
      }
    }

    if let Some(scroll_lines) = self.determine_scroll_lines(e, scroll_multiplier) {
      if mouse_mode {
        let point = crate::mappings::mouse::grid_point(
          e.position - self.last_content.terminal_bounds.bounds.origin,
          self.last_content.terminal_bounds,
          self.last_content.display_offset,
        );

        if let Some(scrolls) =
          crate::mappings::mouse::scroll_report(point, scroll_lines, e, self.last_content.mode)
        {
          for scroll in scrolls {
            self.write_to_pty(scroll);
          }
        };
      } else if self
        .last_content
        .mode
        .contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL)
        && !e.shift
      {
        self.write_to_pty(crate::mappings::mouse::alt_scroll(scroll_lines));
      } else if scroll_lines != 0 {
        let scroll = Scroll::Delta(scroll_lines);
        self.events.push_back(InternalEvent::Scroll(scroll));
      }
    }

    matches!(e.touch_phase, TouchPhase::Ended) && self.scroll_velocity.abs() > 100.0
  }

  /// Apply momentum scroll step, returns true if momentum should continue.
  pub fn apply_momentum_scroll(&mut self) -> bool {
    const FRICTION: f32 = 0.92;
    const MIN_VELOCITY: f32 = 50.0;

    if self.scroll_velocity.abs() < MIN_VELOCITY {
      self.scroll_velocity = 0.0;
      return false;
    }

    let line_height = self.last_content.terminal_bounds.line_height;
    if line_height <= px(0.) {
      return false;
    }

    if self.last_content.mode.contains(TermMode::ALT_SCREEN) {
      self.scroll_velocity = 0.0;
      return false;
    }

    let frame_delta_px = self.scroll_velocity * 0.016;
    self.scroll_px += px(frame_delta_px);

    let scroll_lines = (self.scroll_px / line_height) as i32;
    if scroll_lines != 0 {
      self.scroll_px -= line_height * scroll_lines as f32;
      self
        .events
        .push_back(InternalEvent::Scroll(Scroll::Delta(scroll_lines)));
    }

    self.scroll_velocity *= FRICTION;
    self.scroll_velocity.abs() >= MIN_VELOCITY
  }

  pub fn stop_momentum(&mut self) {
    self.scroll_velocity = 0.0;
  }

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

  pub fn mouse_drag(
    &mut self,
    e: &gpui::MouseMoveEvent,
    region: gpui::Bounds<Pixels>,
    cx: &mut Context<Self>,
  ) {
    let position = e.position - self.last_content.terminal_bounds.bounds.origin;
    if !self.mouse_mode(e.modifiers.shift) {
      self.selection_phase = SelectionPhase::Selecting;
      self
        .events
        .push_back(InternalEvent::UpdateSelection(position));

      if !self.last_content.mode.contains(TermMode::ALT_SCREEN) {
        let scroll_lines = match self.drag_line_delta(e, region) {
          Some(value) => value,
          None => return,
        };

        self
          .events
          .push_back(InternalEvent::Scroll(Scroll::Delta(scroll_lines)));
      }

      cx.notify();
    }
  }

  fn drag_line_delta(&self, e: &gpui::MouseMoveEvent, region: gpui::Bounds<Pixels>) -> Option<i32> {
    let top = region.origin.y;
    let bottom = region.bottom_left().y;

    let scroll_lines = if e.position.y < top {
      let scroll_delta = (top - e.position.y).pow(1.1);
      (scroll_delta / self.last_content.terminal_bounds.line_height).ceil() as i32
    } else if e.position.y > bottom {
      let scroll_delta = -((e.position.y - bottom).pow(1.1));
      (scroll_delta / self.last_content.terminal_bounds.line_height).floor() as i32
    } else {
      return None;
    };

    Some(scroll_lines.clamp(-3, 3))
  }

  pub fn focus_in(&self) {
    if self.last_content.mode.contains(TermMode::FOCUS_IN_OUT) {
      self.write_to_pty("\x1b[I".as_bytes());
    }
  }

  pub fn focus_out(&mut self) {
    if self.last_content.mode.contains(TermMode::FOCUS_IN_OUT) {
      self.write_to_pty("\x1b[O".as_bytes());
    }
  }

  pub(super) fn process_hyperlink(
    &mut self,
    hyperlink: (String, bool, alacritty_terminal::term::search::Match),
    open: bool,
    cx: &mut Context<Self>,
  ) {
    let (maybe_url_or_path, _is_url, url_match) = hyperlink;
    let prev_hovered_word = self.last_content.last_hovered_word.take();

    if open {
      cx.emit(Event::Open(maybe_url_or_path));
    } else {
      self.update_selected_word(
        prev_hovered_word,
        url_match,
        maybe_url_or_path.clone(),
        maybe_url_or_path,
        cx,
      );
    }
  }

  fn update_selected_word(
    &mut self,
    prev_word: Option<crate::hover_target::HoveredWord>,
    word_match: std::ops::RangeInclusive<AlacPoint>,
    word: String,
    navigation_target: String,
    cx: &mut Context<Self>,
  ) {
    if let Some(prev_word) = prev_word
      && prev_word.word == word
      && prev_word.word_match == word_match
    {
      self.last_content.last_hovered_word = Some(crate::hover_target::HoveredWord {
        word,
        word_match,
        id: prev_word.id,
      });
      return;
    }

    self.last_content.last_hovered_word = Some(crate::hover_target::HoveredWord {
      word,
      word_match,
      id: self.next_link_id(),
    });
    cx.emit(Event::NewNavigationTarget(Some(navigation_target)));
    cx.notify()
  }

  fn next_link_id(&mut self) -> usize {
    let res = self.next_link_id;
    self.next_link_id = self.next_link_id.wrapping_add(1);
    res
  }
}
