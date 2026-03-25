use std::{cmp, collections::VecDeque, process::ExitStatus, sync::Arc};

use crate::{
  TerminalBounds, indexed_cell::IndexedCell,
  kitty_graphics::{
    ImagePlacement, KittyAction, KittyCommand, KittyDelete, KittyImageStorage, KittyParser,
    PlacementManager, RawGraphicsCommand,
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

/// Active search parameters stored in Terminal so the search can be
/// automatically re-executed whenever terminal content changes.
pub struct SearchState {
  pub query: String,
  pub match_case: bool,
  pub match_whole: bool,
  pub use_regex: bool,
  /// Pre-compiled regex (only set when `use_regex` is true and the pattern is valid).
  compiled_regex: Option<regex::Regex>,
}

impl SearchState {
  pub fn new(query: String, match_case: bool, match_whole: bool, use_regex: bool) -> Option<Self> {
    if query.is_empty() {
      return None;
    }

    let compiled_regex = if use_regex {
      let pattern = if match_whole {
        format!(r"\b{}\b", query)
      } else {
        query.clone()
      };
      let result = if match_case {
        regex::Regex::new(&pattern)
      } else {
        regex::Regex::new(&format!("(?i){}", pattern))
      };
      match result {
        Ok(re) => Some(re),
        Err(_) => return None, // invalid regex
      }
    } else {
      None
    };

    Some(Self {
      query,
      match_case,
      match_whole,
      use_regex,
      compiled_regex,
    })
  }
}

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
  /// Auto-copy selection to clipboard (triggered by copy_on_select config)
  CopySelectionToClipboard,
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
  /// Emitted when the shell prompt returns (detected via OSC 7 or cwd_file change).
  /// Used to trigger notifications for long-running command completion.
  PromptReturned,
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
  /// The title before the most recent title change.
  /// Used to show which process just finished in notifications.
  pub previous_title_text: String,
  pub next_link_id: usize,
  pub process_changed_at: Option<std::time::Instant>,
  pub scroll_velocity: f32,
  pub last_scroll_time: Option<std::time::Instant>,
  pub touch_state: Option<TouchState>,
  /// Tracks the last time the user sent input (keystrokes/paste) to the terminal.
  /// Used to determine if a command ran long enough to warrant a notification.
  pub last_input_time: std::time::Instant,
  /// Kitty graphics protocol state.
  graphics_rx: Option<std::sync::mpsc::Receiver<RawGraphicsCommand>>,
  graphics_parser: KittyParser,
  pub image_storage: KittyImageStorage,
  pub placement_manager: PlacementManager,
  /// Shared atomic for signaling cursor advancement to the PTY filter.
  pending_cnl: Option<Arc<std::sync::atomic::AtomicU32>>,
  /// Receives CWD paths extracted from OSC 7 sequences in PTY output.
  osc7_rx: Option<std::sync::mpsc::Receiver<std::path::PathBuf>>,
  /// Last CWD reported via OSC 7 (takes priority over sysinfo).
  pub osc7_cwd: Option<std::path::PathBuf>,
  /// Path to a temp file where the shell writes its CWD on each prompt.
  cwd_file: Option<std::path::PathBuf>,
  /// Last known modification time of `cwd_file`, used to detect prompt returns on Windows.
  cwd_file_mtime: Option<std::time::SystemTime>,
  /// Throttle for cwd_file polling (only check every ~500ms).
  last_cwd_file_check: Option<std::time::Instant>,
  /// Active search state. When set, search is re-run every sync to keep matches current.
  pub search_state: Option<SearchState>,
}

impl Terminal {
  pub fn new(
    pty_tx: EventLoopSender,
    term: Arc<FairMutex<Term<TerminalEventListener>>>,
    pty_info: PtyProcessInfo,
    graphics_rx: Option<std::sync::mpsc::Receiver<RawGraphicsCommand>>,
    pending_cnl: Option<Arc<std::sync::atomic::AtomicU32>>,
    osc7_rx: Option<std::sync::mpsc::Receiver<std::path::PathBuf>>,
    cwd_file: Option<std::path::PathBuf>,
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
      previous_title_text: "".to_string(),
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
      osc7_rx,
      osc7_cwd: None,
      cwd_file,
      cwd_file_mtime: None,
      last_cwd_file_check: None,
      search_state: None,
    }
  }

  /// Force-refresh and return the current working directory of the foreground process.
  pub fn current_working_directory(&mut self) -> Option<String> {
    // Prefer OSC 7 (shell-reported, most reliable on Unix).
    if let Some(osc7) = &self.osc7_cwd {
      tracing::debug!("CWD from OSC 7: {:?}", osc7);
      return Some(osc7.to_string_lossy().to_string());
    }

    // Try direct /proc/<pid>/cwd readlink (bypasses sysinfo, most reliable on Linux).
    #[cfg(target_os = "linux")]
    if let Some(pid) = self.pty_info.pid() {
      let proc_path = format!("/proc/{}/cwd", pid.as_u32());
      match std::fs::read_link(&proc_path) {
        Ok(cwd) if cwd.is_dir() => {
          tracing::debug!("CWD from /proc readlink: {:?}, pid: {}", cwd, pid);
          return Some(cwd.to_string_lossy().to_string());
        }
        Ok(cwd) => {
          tracing::debug!("CWD from /proc readlink not a dir: {:?}", cwd);
        }
        Err(e) => {
          tracing::debug!("Failed to readlink {}: {}", proc_path, e);
        }
      }
    }

    // Read CWD from the shell-written temp file (cross-platform, works on Windows).
    if let Some(cwd_file) = &self.cwd_file {
      match std::fs::read_to_string(cwd_file) {
        Ok(contents) => {
          let cwd = contents.trim().to_string();
          if !cwd.is_empty() && std::path::Path::new(&cwd).is_dir() {
            tracing::debug!("CWD from cwd_file: {:?}", cwd);
            return Some(cwd);
          }
        }
        Err(e) => {
          tracing::debug!("Failed to read cwd_file {:?}: {}", cwd_file, e);
        }
      }
    }

    // Fallback: sysinfo refresh.
    self.pty_info.has_changed();
    let cwd = self
      .pty_info
      .current
      .as_ref()
      .map(|info| info.cwd.to_string_lossy().to_string());
    tracing::debug!(
      "CWD from sysinfo: {:?}, pid: {:?}",
      cwd,
      self.pty_info.pid()
    );
    cwd
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

    // Re-run search if there's an active search query.
    // This keeps matches current as content changes (new output, clear, etc.).
    if let Some(search_state) = &self.search_state {
      let old_count = self.last_content.search_matches.len();
      self.last_content.search_matches = Self::execute_search(&terminal, search_state);
      let new_count = self.last_content.search_matches.len();
      // Clamp current match index if matches were removed.
      if self.last_content.current_search_match_index > new_count {
        self.last_content.current_search_match_index = if new_count > 0 { new_count } else { 0 };
      }
      // If match count changed from 0 to >0 (or vice versa), and no match was selected, select first.
      if old_count == 0 && new_count > 0 && self.last_content.current_search_match_index == 0 {
        self.last_content.current_search_match_index = 1;
      }
    }

    let history_size = terminal.history_size() as i32;
    let display_offset = self.last_content.display_offset as i32;
    drop(terminal);

    // Process graphics commands AFTER terminal events so terminal_bounds is up to date.
    self.process_graphics_commands();

    // Detect shell prompt returns and update CWD (non-blocking).
    self.process_prompt_detection(cx);

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

    for raw_cmd in raw_commands {
      if raw_cmd.clear_all {
        self.placement_manager.clear();
        self.image_storage.clear();
        continue;
      }
      let cursor_line = raw_cmd.cursor_line;
      let cursor_column = raw_cmd.cursor_column;
      if let Some(cmd) = self.graphics_parser.parse(&raw_cmd.data) {
        self.execute_graphics_command(&cmd, cursor_line, cursor_column);
      }
    }
    // Note: Kitty protocol responses are intentionally NOT sent back.
    // Our architecture intercepts APC on the read side, so write_to_pty
    // would send responses to the shell's stdin (appearing as typed text).
    // Tools like kitten icat use q=2 (suppress all) and handle timeouts.
  }

  /// Detect shell prompt returns and update CWD.
  ///
  /// On Unix, the PTY filter extracts OSC 7 sequences and sends them via `osc7_rx`.
  /// On Windows (where the PTY filter is not used), we poll the `cwd_file` modification
  /// time to detect when the shell writes a new CWD on prompt display.
  ///
  /// Emits `Event::PromptReturned` whenever a prompt is detected, which is used
  /// to trigger notifications for long-running command completion.
  fn process_prompt_detection(&mut self, cx: &mut Context<Self>) {
    let mut prompt_returned = false;

    // OSC 7 channel (Unix: extracted by PTY filter)
    if let Some(rx) = &self.osc7_rx {
      let mut new_cwd = None;
      while let Ok(path) = rx.try_recv() {
        new_cwd = Some(path);
        prompt_returned = true;
      }
      if let Some(cwd) = new_cwd {
        self.update_cwd(cwd, cx);
      }
    } else if let Some(cwd_file) = self.cwd_file.clone() {
      // Fallback: poll cwd_file modification time (Windows, or when PTY filter is not used).
      // Throttled to every ~500ms to avoid excessive stat() calls.
      let should_check = self
        .last_cwd_file_check
        .map_or(true, |t| t.elapsed() >= std::time::Duration::from_millis(500));
      if should_check {
        self.last_cwd_file_check = Some(std::time::Instant::now());
        if let Ok(mtime) = std::fs::metadata(&cwd_file).and_then(|m| m.modified()) {
          // Only treat as prompt return if mtime changed (not the initial read).
          if self.cwd_file_mtime.is_some_and(|prev| prev < mtime) {
            prompt_returned = true;
            if let Ok(contents) = std::fs::read_to_string(&cwd_file) {
              let cwd_str = contents.trim().to_string();
              if !cwd_str.is_empty() {
                self.update_cwd(std::path::PathBuf::from(&cwd_str), cx);
              }
            }
          }
          self.cwd_file_mtime = Some(mtime);
        }
      }
    }

    if prompt_returned {
      cx.emit(Event::PromptReturned);
    }
  }

  /// Update the tracked CWD if it changed.
  fn update_cwd(&mut self, cwd: std::path::PathBuf, cx: &mut Context<Self>) {
    let changed = self.osc7_cwd.as_ref() != Some(&cwd);
    if changed {
      if let Some(info) = &mut self.pty_info.current {
        info.cwd = cwd.clone();
      }
      self.osc7_cwd = Some(cwd);
      cx.emit(Event::TitleChanged);
    }
  }

  fn execute_graphics_command(
    &mut self,
    cmd: &KittyCommand,
    cursor_line: i32,
    cursor_column: i32,
  ) {
    match cmd.action {
      KittyAction::Transmit => {
        let _ = self.image_storage.store(cmd);
      }
      KittyAction::TransmitAndDisplay => {
        if let Ok(id) = self.image_storage.store(cmd) {
          self.place_image(id, cmd, cursor_line, cursor_column);
        }
      }
      KittyAction::Display => {
        let image_id = cmd.image_id;
        if self.image_storage.get(image_id).is_some() {
          self.place_image(image_id, cmd, cursor_line, cursor_column);
        }
      }
      KittyAction::Delete => {
        self.handle_delete(cmd);
      }
      KittyAction::Query => {
        // We support the protocol but can't send responses back
        // without them leaking to the shell's stdin.
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

    // Adjust search match coordinates when content has shifted.
    // When new output pushes content into scrollback, history_size increases
    // and all grid coordinates shift by the delta.
    let current_history_size = term.history_size();

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
      history_size: current_history_size,
      scrolled_to_top: content.display_offset == current_history_size,
      scrolled_to_bottom: content.display_offset == 0,
      // Search matches are preserved from last_content; they will be
      // re-computed in sync() if there is an active search query.
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
            self.previous_title_text = std::mem::replace(&mut self.title_text, name);
          }
        } else {
          self.previous_title_text = std::mem::replace(&mut self.title_text, title);
        }
        cx.emit(Event::TitleChanged);
      }
      AlacTermEvent::ResetTitle => {
        if let Some(name) = self.pty_info.current_process_name() {
          self.previous_title_text = std::mem::replace(&mut self.title_text, name);
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
      InternalEvent::CopySelectionToClipboard => {
        if let Some(txt) = term.selection_to_string() {
          cx.write_to_clipboard(gpui::ClipboardItem::new_string(txt));
        }
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
