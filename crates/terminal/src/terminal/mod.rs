use std::{cmp, collections::VecDeque, process::ExitStatus, sync::Arc};

use crate::{
  TerminalBounds, indexed_cell::IndexedCell, mouse::grid_point_and_side, pty_info::PtyProcessInfo,
  terminal_content::TerminalContent, terminal_hyperlinks::RegexSearches,
};
use alacritty_terminal::{
  Term,
  event::Event as AlacTermEvent,
  event_loop::{EventLoopSender, Msg, Notifier},
  grid::{Dimensions as _, Scroll},
  index::{Direction, Point as AlacPoint},
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
}

impl Terminal {
  pub fn new(
    pty_tx: EventLoopSender,
    term: Arc<FairMutex<Term<TerminalEventListener>>>,
    pty_info: PtyProcessInfo,
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
    }
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
