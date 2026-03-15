use std::{cmp, collections::VecDeque, process::ExitStatus, sync::Arc};

use crate::{
  TerminalBounds, indexed_cell::IndexedCell,
  kitty_graphics::{
    ImagePlacement, KittyAction, KittyCommand, KittyDelete, KittyImageStorage, KittyParser,
    KittyResponse, PlacementManager, RawGraphicsCommand,
  },
  mouse::grid_point_and_side,
  pty_info::PtyProcessInfo,
  terminal_content::TerminalContent,
  terminal_hyperlinks::RegexSearches,
};
use alacritty_terminal::{
  Term,
  event::Event as AlacTermEvent,
  event_loop::{EventLoopSender, Msg, Notifier},
  grid::{Dimensions as _, Scroll},
  index::{Column as AlacColumn, Direction, Line as AlacLine, Point as AlacPoint},
  selection::Selection,
  sync::FairMutex,
};
use gpui::{Context, EventEmitter, Pixels, Window, px};
use themeing::ActiveTheme;

mod events;
mod input;
mod mouse_scroll;

pub use events::TerminalEventListener;

#[derive(Clone)]
pub enum InternalEvent {
  Resize(TerminalBounds),
  Clear,
  Scroll(Scroll),
  ScrollToAlacPoint(AlacPoint),
  SetSelection(Option<(Selection, AlacPoint)>),
  UpdateSelection(gpui::Point<Pixels>),
  FindHyperlink(gpui::Point<Pixels>, bool),
  ProcessHyperlink(
    (String, bool, alacritty_terminal::term::search::Match),
    bool,
  ),
  Copy(Option<bool>),
}

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

pub struct Terminal {
  pub pty_tx: Notifier,
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
  pub process_changed_at: Option<std::time::Instant>,
  pub scroll_velocity: f32,
  pub last_scroll_time: Option<std::time::Instant>,
  pub touch_state: Option<TouchState>,
  /// Tracks the last time the user sent input (keystrokes/paste) to the terminal.
  /// Used to determine if a bell follows a long-running command.
  pub last_input_time: std::time::Instant,
  /// Kitty graphics protocol state.
  graphics_rx: Option<std::sync::mpsc::Receiver<RawGraphicsCommand>>,
  graphics_parser: KittyParser,
  pub image_storage: KittyImageStorage,
  pub placement_manager: PlacementManager,
  /// Shared atomic for signaling cursor advancement to the PTY filter.
  pending_cnl: Option<Arc<std::sync::atomic::AtomicU32>>,
}

impl Terminal {
  pub fn new(
    pty_tx: EventLoopSender,
    term: Arc<FairMutex<Term<TerminalEventListener>>>,
    pty_info: PtyProcessInfo,
    graphics_rx: Option<std::sync::mpsc::Receiver<RawGraphicsCommand>>,
    pending_cnl: Option<Arc<std::sync::atomic::AtomicU32>>,
  ) -> Self {
    Self {
      pty_tx: Notifier(pty_tx),
      events: VecDeque::with_capacity(10),
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
      scroll_velocity: 0.0,
      last_scroll_time: None,
      touch_state: None,
      last_input_time: std::time::Instant::now(),
      graphics_rx,
      graphics_parser: KittyParser::new(),
      image_storage: KittyImageStorage::new(),
      placement_manager: PlacementManager::new(),
      pending_cnl,
    }
  }

  pub fn last_content(&self) -> &TerminalContent {
    &self.last_content
  }

  /// Collect all grid cells (history + visible) for minimap rendering.
  /// Returns cells with 0-based line numbers (0 = oldest history line).
  pub fn collect_minimap_cells(&self) -> Vec<IndexedCell> {
    let term = self.term.lock_unfair();
    let history_size = term.history_size();
    let screen_lines = term.screen_lines();
    let columns = term.columns();
    let total_lines = history_size + screen_lines;

    let mut cells = Vec::new();
    for line_idx in 0..total_lines {
      let original_line = line_idx as i32 - history_size as i32;
      let row = &term.grid()[AlacLine(original_line)];
      for col_idx in 0..columns {
        let cell = &row[AlacColumn(col_idx)];
        if cell.c != ' ' && cell.c != '\t' && cell.c != '\0' {
          cells.push(IndexedCell {
            point: AlacPoint::new(AlacLine(line_idx as i32), AlacColumn(col_idx)),
            cell: cell.clone(),
          });
        }
      }
    }

    cells
  }

  pub fn set_size(&mut self, new_bounds: TerminalBounds) {
    if self.last_content.terminal_bounds != new_bounds {
      self.events.push_back(InternalEvent::Resize(new_bounds))
    }
  }

  pub fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let term = self.term.clone();
    let mut terminal = term.lock_unfair();
    while let Some(e) = self.events.pop_front() {
      self.process_terminal_event(&e, &mut terminal, window, cx)
    }
    self.last_content = Self::make_content(&terminal, &self.last_content);

    let history_size = terminal.history_size() as i32;
    let display_offset = self.last_content.display_offset as i32;
    drop(terminal);

    // Process graphics commands AFTER terminal events so terminal_bounds is up to date.
    self.process_graphics_commands();

    // Collect visible image placements for rendering.
    let viewport_top = history_size - display_offset;
    let viewport_lines = self.last_content.terminal_bounds.screen_lines() as u32;

    self.last_content.image_placements = self.placement_manager.visible_placements(
      &self.image_storage,
      viewport_top,
      viewport_lines,
    );
  }

  fn process_graphics_commands(&mut self) {
    // Drain all available graphics commands (non-blocking).
    let mut raw_commands: Vec<RawGraphicsCommand> = Vec::new();
    if let Some(rx) = &self.graphics_rx {
      while let Ok(raw_cmd) = rx.try_recv() {
        raw_commands.push(raw_cmd);
      }
    }

    let mut responses = Vec::new();
    for raw_cmd in raw_commands {
      if raw_cmd.clear_all {
        self.placement_manager.clear();
        self.image_storage.clear();
        continue;
      }
      let cursor_line = raw_cmd.cursor_line;
      let cursor_column = raw_cmd.cursor_column;
      if let Some(cmd) = self.graphics_parser.parse(&raw_cmd.data) {
        let response = self.execute_graphics_command(&cmd, cursor_line, cursor_column);
        if let Some(resp) = response {
          if cmd.quiet == 0 || (cmd.quiet == 1 && !resp.ok) {
            responses.push(resp);
          }
        }
      }
    }

    // Send responses back through the PTY.
    for resp in responses {
      self.write_to_pty(resp.encode());
    }
  }

  fn execute_graphics_command(
    &mut self,
    cmd: &KittyCommand,
    cursor_line: i32,
    cursor_column: i32,
  ) -> Option<KittyResponse> {
    match cmd.action {
      KittyAction::Transmit => {
        match self.image_storage.store(cmd) {
          Ok(id) => Some(KittyResponse::ok(id)),
          Err(msg) => Some(KittyResponse::error(cmd.image_id, msg)),
        }
      }
      KittyAction::TransmitAndDisplay => {
        match self.image_storage.store(cmd) {
          Ok(id) => {
            self.place_image(id, cmd, cursor_line, cursor_column);
            Some(KittyResponse::ok(id))
          }
          Err(msg) => Some(KittyResponse::error(cmd.image_id, msg)),
        }
      }
      KittyAction::Display => {
        let image_id = cmd.image_id;
        if self.image_storage.get(image_id).is_some() {
          self.place_image(image_id, cmd, cursor_line, cursor_column);
          Some(KittyResponse::ok_with_placement(
            image_id,
            cmd.placement_id,
          ))
        } else {
          Some(KittyResponse::error(image_id, "Image not found"))
        }
      }
      KittyAction::Delete => {
        self.handle_delete(cmd);
        None
      }
      KittyAction::Query => {
        // Respond with OK to indicate we support the Kitty graphics protocol.
        Some(KittyResponse::ok(cmd.image_id))
      }
    }
  }

  fn place_image(
    &mut self,
    image_id: u32,
    cmd: &KittyCommand,
    cursor_line: i32,
    cursor_column: i32,
  ) {
    // Use cursor position captured at APC intercept time (not current cursor).
    let line = cursor_line;
    let column = cursor_column;

    // Determine display size in cells.
    let (width_cells, height_cells) = if cmd.display_columns > 0 && cmd.display_rows > 0 {
      (cmd.display_columns, cmd.display_rows)
    } else if let Some(img) = self.image_storage.peek(image_id) {
      // Scale to fit terminal width, preserving aspect ratio.
      let bounds = &self.last_content.terminal_bounds;
      let cw = f32::from(bounds.cell_width().max(gpui::px(1.0))) as u32;
      let lh = f32::from(bounds.line_height().max(gpui::px(1.0))) as u32;
      let terminal_cols = bounds.num_columns() as u32;
      let terminal_width_px = terminal_cols.saturating_mul(cw).max(1);

      if img.width > terminal_width_px {
        // Image wider than terminal — scale down to fit.
        let h_px =
          ((img.height as u64 * terminal_width_px as u64) / img.width.max(1) as u64) as u32;
        let h_cells = (h_px + lh - 1) / lh;
        (terminal_cols.max(1), h_cells.max(1))
      } else {
        // Image fits — use native pixel size.
        let w = (img.width + cw - 1) / cw;
        let h = (img.height + lh - 1) / lh;
        (w.max(1), h.max(1))
      }
    } else {
      (1, 1)
    };

    self.placement_manager.add(ImagePlacement {
      image_id,
      placement_id: cmd.placement_id,
      line,
      column,
      width_cells,
      height_cells,
      crop: (cmd.crop_x, cmd.crop_y, cmd.crop_width, cmd.crop_height),
      z_index: cmd.z_index,
      x_offset: cmd.x_offset,
      y_offset: cmd.y_offset,
    });

    // Signal the PTY filter to inject cursor advancement on next read.
    // This is the fallback mechanism for when the filter couldn't compute
    // the height from APC params alone (e.g., PNG without r=/v=).
    if let Some(cnl) = &self.pending_cnl {
      cnl.store(height_cells, std::sync::atomic::Ordering::Release);
    }
  }

  fn handle_delete(&mut self, cmd: &KittyCommand) {
    let delete = cmd.delete.as_ref().cloned().unwrap_or(KittyDelete::All);
    match delete {
      KittyDelete::All => {
        self.placement_manager.clear();
        self.image_storage.clear();
      }
      KittyDelete::ById {
        image_id: _,
        placement_id,
      } => {
        let id = cmd.image_id;
        self.placement_manager.remove_by_id(id, placement_id);
        if placement_id.is_none() {
          self.image_storage.remove(id);
        }
      }
      KittyDelete::AtCursor => {
        let term = self.term.lock_unfair();
        let cursor = term.renderable_content().cursor;
        let history_size = term.history_size() as i32;
        let line = history_size + cursor.point.line.0;
        let col = cursor.point.column.0 as i32;
        self.placement_manager.remove_at_cursor(line, col);
      }
      KittyDelete::ByZIndex(_) | KittyDelete::AtColumn(_) | KittyDelete::AtRow(_) => {
        // Simplified: just remove all for these advanced cases.
        self.placement_manager.clear();
      }
      KittyDelete::AnimationFrames => {
        // Not supported in MVP.
      }
    }
  }

  fn make_content(
    term: &Term<TerminalEventListener>,
    last_content: &TerminalContent,
  ) -> TerminalContent {
    let content = term.renderable_content();

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
      image_placements: Vec::new(),
    }
  }

  pub fn process_event(&mut self, event: AlacTermEvent, cx: &mut Context<Self>) {
    match event {
      AlacTermEvent::Title(title) => {
        const PROCESS_CHANGE_GRACE_PERIOD: std::time::Duration =
          std::time::Duration::from_millis(100);
        if let Some(changed_at) = self.process_changed_at
          && changed_at.elapsed() < PROCESS_CHANGE_GRACE_PERIOD
        {
          return;
        }

        if title.is_empty() {
          if let Some(name) = self.pty_info.current_process_name() {
            self.title_text = name;
          }
        } else {
          self.title_text = title;
        }
        cx.emit(Event::TitleChanged);
      }
      AlacTermEvent::ResetTitle => {
        if let Some(name) = self.pty_info.current_process_name() {
          self.title_text = name;
        }
        cx.emit(Event::TitleChanged);
      }
      AlacTermEvent::ClipboardStore(_, data) => {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(data))
      }
      AlacTermEvent::ClipboardLoad(_, format) => self.write_to_pty(
        match &cx.read_from_clipboard().and_then(|item| item.text()) {
          Some(text) => format(text),
          _ => format(""),
        }
        .into_bytes(),
      ),
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
      AlacTermEvent::MouseCursorDirty => {}
      AlacTermEvent::Wakeup => {
        cx.emit(Event::Wakeup);

        if self.pty_info.has_changed()
          && let Some(info) = &self.pty_info.current
        {
          self.title_text = info.name.clone();
          self.process_changed_at = Some(std::time::Instant::now());
          cx.emit(Event::TitleChanged);
        }
      }
      AlacTermEvent::ColorRequest(index, format) => {
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
      InternalEvent::Clear => {}
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
      InternalEvent::Copy(_keep_selection) => {}
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
}

impl EventEmitter<Event> for Terminal {}

#[derive(PartialEq, Eq)]
pub enum SelectionPhase {
  Selecting,
  Ended,
}

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

fn content_index_for_mouse(pos: gpui::Point<Pixels>, terminal_bounds: &TerminalBounds) -> usize {
  let col = (pos.x / terminal_bounds.cell_width()).round() as usize;
  let clamped_col = cmp::min(col, terminal_bounds.columns() - 1);
  let row = (pos.y / terminal_bounds.line_height()).round() as usize;
  let clamped_row = cmp::min(row, terminal_bounds.screen_lines() - 1);
  clamped_row * terminal_bounds.columns() + clamped_col
}
