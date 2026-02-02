use std::{borrow::Cow, cmp, collections::VecDeque, process::ExitStatus, sync::Arc};

use crate::{
  TerminalBounds, indexed_cell::IndexedCell, mouse::grid_point_and_side, pty_info::PtyProcessInfo,
  terminal_content::TerminalContent, terminal_hyperlinks::RegexSearches,
};
use alacritty_terminal::{
  Term,
  event::{Event as AlacTermEvent, EventListener, Notify},
  event_loop::{EventLoopSender, Msg, Notifier},
  grid::{Dimensions as _, Scroll},
  index::{Column, Direction, Point as AlacPoint},
  selection::{Selection, SelectionType},
  sync::FairMutex,
  term::TermMode,
};
use futures::channel::mpsc::UnboundedSender;
use gpui::{Context, EventEmitter, Keystroke, Pixels, Window, px};
use themeing::ActiveTheme;

#[derive(Clone)]
pub enum InternalEvent {
  Resize(TerminalBounds),
  Clear,
  // FocusNextMatch,
  Scroll(Scroll),
  ScrollToAlacPoint(AlacPoint),
  SetSelection(Option<(Selection, AlacPoint)>),
  UpdateSelection(gpui::Point<Pixels>),
  // Adjusted mouse position, should open
  FindHyperlink(gpui::Point<Pixels>, bool),
  ProcessHyperlink(
    (String, bool, alacritty_terminal::term::search::Match),
    bool,
  ),
  // Whether keep selection when copy
  Copy(Option<bool>),
}

///Upward flowing events, for changing the title and such
#[derive(Clone, Debug)]
pub enum Event {
  TitleChanged,
  CloseTerminal,
  Bell,
  Wakeup,
  BlinkChanged(bool),
  SelectionsChanged,
  NewNavigationTarget(Option<String>),
  Open(String),
}

#[derive(Clone)]
pub struct TerminalEventListener(pub UnboundedSender<AlacTermEvent>);

impl EventListener for TerminalEventListener {
  fn send_event(&self, event: AlacTermEvent) {
    self.0.unbounded_send(event).ok();
  }
}

pub struct Terminal {
  pub pty_tx: Notifier,
  // pub completion_tx: Option<Sender<Option<ExitStatus>>>,
  pub events: VecDeque<InternalEvent>,
  pub term: Arc<FairMutex<Term<TerminalEventListener>>>,
  pub last_content: TerminalContent,
  pub selection_head: Option<AlacPoint>,
  pub pty_info: PtyProcessInfo,
  pub selection_phase: SelectionPhase,
  pub last_mouse: Option<(AlacPoint, Direction)>,
  pub last_mouse_move_time: std::time::Instant,
  pub hyperlink_regex_searches: RegexSearches,
  pub last_hyperlink_search_position: Option<gpui::Point<Pixels>>,
  pub child_exited: Option<ExitStatus>,
  pub scroll_px: Pixels,
  pub title_text: String,
  pub next_link_id: usize,
  /// Timestamp of last process change, used to ignore shell title updates immediately after
  pub process_changed_at: Option<std::time::Instant>,
}

impl Terminal {
  pub fn new(
    pty_tx: EventLoopSender,
    term: Arc<FairMutex<Term<TerminalEventListener>>>,
    pty_info: PtyProcessInfo,
  ) -> Self {
    return Self {
      pty_tx: Notifier(pty_tx),
      events: VecDeque::with_capacity(10), //Should never get this high.
      term,
      last_content: Default::default(),
      pty_info,
      selection_head: None,
      selection_phase: SelectionPhase::Ended,
      last_mouse: None,
      last_mouse_move_time: std::time::Instant::now(),
      hyperlink_regex_searches: RegexSearches::default(),
      last_hyperlink_search_position: None,
      child_exited: None,
      scroll_px: px(0.),
      title_text: "".to_string(),
      next_link_id: 0,
      process_changed_at: None,
    };
  }
  pub fn last_content(&self) -> &TerminalContent {
    &self.last_content
  }

  pub fn set_size(&mut self, new_bounds: TerminalBounds) {
    if self.last_content.terminal_bounds != new_bounds {
      self.events.push_back(InternalEvent::Resize(new_bounds))
    }
  }

  pub fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let term = self.term.clone();
    let mut terminal = term.lock_unfair();
    //Note that the ordering of events matters for event processing
    while let Some(e) = self.events.pop_front() {
      self.process_terminal_event(&e, &mut terminal, window, cx)
    }

    self.last_content = Self::make_content(&terminal, &self.last_content);
  }

  fn make_content(
    term: &Term<TerminalEventListener>,
    last_content: &TerminalContent,
  ) -> TerminalContent {
    let content = term.renderable_content();

    // Pre-allocate with estimated size to reduce reallocations
    let estimated_size = content.display_iter.size_hint().0;
    let mut cells = Vec::with_capacity(estimated_size);

    cells.extend(content.display_iter.map(|ic| IndexedCell {
      point: ic.point,
      cell: ic.cell.clone(),
    }));

    let selection_text = if content.selection.is_some() {
      term.selection_to_string()
    } else {
      None
    };

    TerminalContent {
      cells,
      mode: content.mode,
      display_offset: content.display_offset,
      selection_text,
      selection: content.selection,
      cursor: content.cursor,
      cursor_char: term.grid()[content.cursor.point].c,
      terminal_bounds: last_content.terminal_bounds,
      last_hovered_word: last_content.last_hovered_word.clone(),
      history_size: term.history_size(),
      scrolled_to_top: content.display_offset == term.history_size(),
      scrolled_to_bottom: content.display_offset == 0,
      search_matches: last_content.search_matches.clone(),
      current_search_match_index: last_content.current_search_match_index,
    }
  }

  pub fn set_search_matches(&mut self, matches: Vec<std::ops::RangeInclusive<AlacPoint>>) {
    self.last_content.search_matches = matches;
  }

  pub fn set_current_search_match(&mut self, index: usize) {
    self.last_content.current_search_match_index = index;

    // Scroll to make the matched item visible in the first 10 lines
    if index > 0 && index <= self.last_content.search_matches.len() {
      let match_range = &self.last_content.search_matches[index - 1];
      let match_line = match_range.start().line;

      // Get the term to calculate scroll delta
      let term = self.term.lock_unfair();
      let display_offset = term.grid().display_offset();
      let screen_lines = term.screen_lines() as i32;

      // Calculate the visual line of the match (relative to the top of the visible area)
      // match_line is negative for history, 0 to screen_lines-1 for visible area
      let match_line_i32 = match_line.0;
      let visible_top_line = -(display_offset as i32);
      let visual_line = match_line_i32 - visible_top_line;

      // Target: bring the match to within the first 10 lines (indices 0-9)
      let target_line = 5; // Center-ish in the first 10 lines

      if visual_line < 0 || visual_line >= screen_lines {
        // Match is outside visible area, scroll to it
        let scroll_delta = visual_line - target_line;
        drop(term); // Release lock before pushing event
        if scroll_delta != 0 {
          self
            .events
            .push_back(InternalEvent::Scroll(Scroll::Delta(-scroll_delta)));
        }
      } else if visual_line > 10 {
        // Match is visible but below line 10, scroll up to bring it into first 10 lines
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

  pub fn process_event(&mut self, event: AlacTermEvent, cx: &mut Context<Self>) {
    match event {
      AlacTermEvent::Title(title) => {
        tracing::debug!(
          "Terminal title changed to: '{}', current pty_info: {:?}",
          title,
          self.pty_info.current
        );

        // Ignore shell title updates that come immediately after a process change
        // (e.g., bash's PROMPT_COMMAND setting title right after we switch back to bash)
        const PROCESS_CHANGE_GRACE_PERIOD: std::time::Duration =
          std::time::Duration::from_millis(100);
        if let Some(changed_at) = self.process_changed_at {
          if changed_at.elapsed() < PROCESS_CHANGE_GRACE_PERIOD {
            tracing::debug!(
              "Ignoring title update '{}' within grace period after process change",
              title
            );
            return;
          }
        }

        if title.is_empty() {
          // Fall back to current process name when title is empty
          if let Some(name) = self.pty_info.current_process_name() {
            tracing::debug!("Empty title, falling back to process name: '{}'", name);
            self.title_text = name;
          } else {
            tracing::debug!("Empty title, but no process name available");
          }
        } else {
          self.title_text = title;
        }
        cx.emit(Event::TitleChanged);
      }
      AlacTermEvent::ResetTitle => {
        tracing::debug!(
          "Terminal title reset, current pty_info: {:?}",
          self.pty_info.current
        );

        // Reset to current process name
        if let Some(name) = self.pty_info.current_process_name() {
          tracing::debug!("Reset title to process name: '{}'", name);
          self.title_text = name;
        } else {
          tracing::debug!("Reset title, but no process name available");
        }
        cx.emit(Event::TitleChanged);
      }
      AlacTermEvent::ClipboardStore(_, data) => {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(data))
      }
      AlacTermEvent::ClipboardLoad(_, format) => {
        self.write_to_pty(
          match &cx.read_from_clipboard().and_then(|item| item.text()) {
            // The terminal only supports pasting strings, not images.
            Some(text) => format(text),
            _ => format(""),
          }
          .into_bytes(),
        )
      }
      AlacTermEvent::PtyWrite(out) => self.write_to_pty(out.into_bytes()),
      AlacTermEvent::TextAreaSizeRequest(format) => {
        self.write_to_pty(format(self.last_content.terminal_bounds.into()).into_bytes())
      }
      AlacTermEvent::CursorBlinkingChange => {
        let terminal = self.term.lock();
        let blinking = terminal.cursor_style().blinking;
        cx.emit(Event::BlinkChanged(blinking));
      }
      AlacTermEvent::Bell => {
        cx.emit(Event::Bell);
      }
      AlacTermEvent::Exit => {
        tracing::info!("Terminal child exited");
        self.register_task_finished(None, cx);
      }
      AlacTermEvent::MouseCursorDirty => {
        //NOOP, Handled in render
      }
      AlacTermEvent::Wakeup => {
        cx.emit(Event::Wakeup);

        if self.pty_info.has_changed() {
          // Update title to current process name when foreground process changes
          if let Some(info) = &self.pty_info.current {
            tracing::debug!(
              "Process changed, updating title to: '{}' (was: '{}')",
              info.name,
              self.title_text
            );
            self.title_text = info.name.clone();
            // Record when process changed so we can ignore shell title updates
            // that arrive shortly after (e.g., bash's PROMPT_COMMAND)
            self.process_changed_at = Some(std::time::Instant::now());
            cx.emit(Event::TitleChanged);
          }
        }
      }
      AlacTermEvent::ColorRequest(index, format) => {
        // It's important that the color request is processed here to retain relative order
        // with other PTY writes. Otherwise applications might witness out-of-order
        // responses to requests. For example: An application sending `OSC 11 ; ? ST`
        // (color request) followed by `CSI c` (request device attributes) would receive
        // the response to `CSI c` first.
        // Instead of locking, we could store the colors in `self.last_content`. But then
        // we might respond with out of date value if a "set color" sequence is immediately
        // followed by a color request sequence.
        let color = self.term.lock().colors()[index].unwrap_or_else(|| {
          crate::mappings::colors::to_alac_rgb(themeing::get_color_at_index(
            index,
            cx.theme().as_ref(),
          ))
        });
        self.write_to_pty(format(color).into_bytes());
      }
      AlacTermEvent::ChildExit(error_code) => {
        self.register_task_finished(Some(error_code), cx);
      }
    }
  }

  pub fn selection_started(&self) -> bool {
    self.selection_phase == SelectionPhase::Selecting
  }

  fn process_terminal_event(
    &mut self,
    event: &InternalEvent,
    term: &mut Term<TerminalEventListener>,
    _window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    match event {
      &InternalEvent::Resize(mut new_bounds) => {
        new_bounds.bounds.size.height = cmp::max(new_bounds.line_height, new_bounds.height());
        new_bounds.bounds.size.width = cmp::max(new_bounds.cell_width, new_bounds.width());

        self.last_content.terminal_bounds = new_bounds;

        self.pty_tx.0.send(Msg::Resize(new_bounds.into())).ok();

        term.resize(new_bounds);
      }
      InternalEvent::Clear => {
        // Noop
      }
      InternalEvent::Scroll(scroll) => {
        term.scroll_display(*scroll);
      }
      InternalEvent::SetSelection(selection) => {
        term.selection = selection.as_ref().map(|(sel, _)| sel.clone());

        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        if let Some(selection_text) = term.selection_to_string() {
          cx.write_to_primary(gpui::ClipboardItem::new_string(selection_text));
        }

        if let Some((_, head)) = selection {
          self.selection_head = Some(*head);
        }
        cx.emit(Event::SelectionsChanged)
      }
      InternalEvent::UpdateSelection(position) => {
        if let Some(mut selection) = term.selection.take() {
          let (point, side) = grid_point_and_side(
            *position,
            self.last_content.terminal_bounds,
            term.grid().display_offset(),
          );

          selection.update(point, side);
          term.selection = Some(selection);

          #[cfg(any(target_os = "linux", target_os = "freebsd"))]
          if let Some(selection_text) = term.selection_to_string() {
            cx.write_to_primary(gpui::ClipboardItem::new_string(selection_text));
          }

          self.selection_head = Some(point);
          cx.emit(Event::SelectionsChanged)
        }
      }
      InternalEvent::Copy(_keep_selection) => {
        // TODO
      }
      InternalEvent::ScrollToAlacPoint(point) => {
        term.scroll_to_point(*point);
      }
      InternalEvent::FindHyperlink(position, open) => {
        let point = crate::mappings::mouse::grid_point(
          *position,
          self.last_content.terminal_bounds,
          term.grid().display_offset(),
        )
        .grid_clamp(term, alacritty_terminal::index::Boundary::Grid);

        match crate::terminal_hyperlinks::find_from_grid_point(
          term,
          point,
          &mut self.hyperlink_regex_searches,
        ) {
          Some(hyperlink) => {
            self.process_hyperlink(hyperlink, *open, cx);
          }
          None => {
            cx.emit(Event::NewNavigationTarget(None));
          }
        }
      }
      InternalEvent::ProcessHyperlink(hyperlink, open) => {
        self.process_hyperlink(hyperlink.clone(), *open, cx);
      }
    }
  }

  pub fn get_content(&self) -> String {
    let term = self.term.lock_unfair();
    let start = AlacPoint::new(term.topmost_line(), Column(0));
    let end = AlacPoint::new(term.bottommost_line(), term.last_column());
    term.bounds_to_string(start, end)
  }

  pub fn input(&mut self, input: impl Into<Cow<'static, [u8]>>) {
    self.events.push_back(InternalEvent::Scroll(Scroll::Bottom));
    self.events.push_back(InternalEvent::SetSelection(None));

    self.write_to_pty(input);
  }

  pub fn copy(&mut self, _cx: &mut Context<Self>) {
    self.events.push_back(InternalEvent::Copy(Some(true)));
  }

  /// Copy selection to clipboard and immediately clear the selection.
  /// This directly modifies the terminal state without going through the event queue.
  pub fn copy_and_clear_selection(&mut self, cx: &mut Context<Self>) {
    let mut term = self.term.lock_unfair();
    if let Some(txt) = term.selection_to_string() {
      cx.write_to_clipboard(gpui::ClipboardItem::new_string(txt));
    }
    // Clear selection immediately
    term.selection = None;
    self.last_content.selection = None;
    self.last_content.selection_text = None;
    cx.emit(Event::SelectionsChanged);
    cx.notify();
  }

  pub fn try_keystroke(&mut self, keystroke: &Keystroke, alt_is_meta: bool) -> bool {
    // Keep default terminal behavior
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

  fn register_task_finished(&mut self, error_code: Option<i32>, cx: &mut Context<Terminal>) {
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

  ///Write the Input payload to the tty.
  fn write_to_pty(&self, input: impl Into<Cow<'static, [u8]>>) {
    self.pty_tx.notify(input.into());
  }

  // Mouse
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
      // Throttle hyperlink searches to avoid excessive processing
      let now = std::time::Instant::now();
      let should_search = if let Some(last_pos) = self.last_hyperlink_search_position {
        // Only search if mouse moved significantly or enough time passed
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
            0 => return, //This is a release
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
    } else {
      //Hyperlinks
      if self.selection_phase == SelectionPhase::Ended {
        let mouse_cell_index =
          content_index_for_mouse(position, &self.last_content.terminal_bounds);
        if let Some(link) = self.last_content.cells[mouse_cell_index].hyperlink() {
          cx.open_url(link.uri());
        } else if e.modifiers.secondary() {
          self
            .events
            .push_back(InternalEvent::FindHyperlink(position, true));
        }
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

    // Handle scroll wheel events - compute scroll lines from pixel delta
    let delta_y = e.delta.pixel_delta(line_height).y * scroll_multiplier;
    if delta_y.abs() < px(0.1) {
      return None;
    }

    // Convert pixel delta to line delta
    let scroll_lines = (delta_y / line_height) as i32;
    if scroll_lines != 0 {
      Some(scroll_lines)
    } else {
      // Accumulate sub-line scrolling
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

  ///Scroll the terminal
  pub fn scroll_wheel(&mut self, e: &gpui::ScrollWheelEvent, scroll_multiplier: f32) {
    let mouse_mode = self.mouse_mode(e.shift);
    let scroll_multiplier = if mouse_mode { 1. } else { scroll_multiplier };

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
      // Alacritty has the same ordering, of first updating the selection
      // then scrolling 15ms later
      self
        .events
        .push_back(InternalEvent::UpdateSelection(position));

      // Doesn't make sense to scroll the alt screen
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

  fn process_hyperlink(
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

impl EventEmitter<Event> for Terminal {}

#[derive(PartialEq, Eq)]
pub enum SelectionPhase {
  Selecting,
  Ended,
}

fn content_index_for_mouse(pos: gpui::Point<Pixels>, terminal_bounds: &TerminalBounds) -> usize {
  let col = (pos.x / terminal_bounds.cell_width()).round() as usize;
  let clamped_col = cmp::min(col, terminal_bounds.columns() - 1);
  let row = (pos.y / terminal_bounds.line_height()).round() as usize;
  let clamped_row = cmp::min(row, terminal_bounds.screen_lines() - 1);
  clamped_row * terminal_bounds.columns() + clamped_col
}
