use std::{ops::Range, time::Duration};

use crate::mappings::keys::KnownKeys;
use crate::{TerminalBounds, hover_target::HoverTarget, ime_state::ImeState};
use alacritty_terminal::{grid::Scroll as AlacScroll, term::TermMode};
use gpui::{
  App, Context, Entity, EventEmitter, FocusHandle, InteractiveElement, IntoElement, KeyContext,
  ParentElement, Pixels, Render, Styled, Task, UpdateGlobal, Window, actions, div,
};
use smol::Timer;

actions!(
  terminal,
  [
    SendTab,
    SendTabPrev,
    Copy,
    Paste,
    ScrollPageUp,
    ScrollPageDown,
    SendPageUp,
    SendPageDown,
    ZoomIn,
    ZoomOut,
    ZoomReset
  ]
);

use super::terminal::Terminal;
use super::terminal_element::TerminalElement;

const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Debug)]
pub enum TerminalEvent {
  UpdateTab,
  Wakeup,
  CloseTerminal(usize),
}

pub struct TerminalView {
  pub terminal: Entity<Terminal>,
  pub focus_handle: FocusHandle,
  pub has_bell: bool,
  // context_menu: Option<(Entity<ContextMenu>, gpui::Point<Pixels>, Subscription)>,
  // cursor_shape: CursorShape,
  pub blink_state: bool,
  // mode: TerminalMode,
  // blinking_terminal_enabled: bool,
  // cwd_serialized: bool,
  pub blinking_paused: bool,
  pub blink_epoch: usize,
  pub hover: Option<HoverTarget>,
  pub index: usize,
  pub hover_tooltip_update: Task<()>,
  // workspace_id: Option<WorkspaceId>,
  // show_breadcrumbs: bool,
  // block_below_cursor: Option<Rc<BlockProperties>>,
  pub scroll_top: Pixels,
  // scroll_handle: TerminalScrollHandle,
  pub ime_state: Option<ImeState>,
  _subscriptions: Vec<gpui::Subscription>,
  _terminal_subscriptions: Vec<gpui::Subscription>,
}

impl EventEmitter<TerminalEvent> for TerminalView {}

impl gpui::Focusable for TerminalView {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for TerminalView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let terminal_handle = self.terminal.clone();
    let terminal_view_handle = cx.entity();

    let focused = self.focus_handle.is_focused(window);

    div()
      .id("terminal-view")
      .size_full()
      .relative()
      .track_focus(&self.focus_handle)
      .key_context(self.dispatch_context(cx))
      .on_action(cx.listener(Self::send_tab))
      .on_action(cx.listener(Self::send_tab_prev))
      .on_action(cx.listener(Self::copy))
      .on_action(cx.listener(Self::paste))
      .on_action(cx.listener(Self::scroll_page_up))
      .on_action(cx.listener(Self::scroll_page_down))
      .on_action(cx.listener(Self::send_page_up))
      .on_action(cx.listener(Self::send_page_down))
      .on_action(cx.listener(Self::zoom_in))
      .on_action(cx.listener(Self::zoom_out))
      .on_action(cx.listener(Self::zoom_reset))
      .on_key_down(cx.listener(Self::key_down))
      .child(
        div()
          .id("terminal-view-container")
          .size_full()
          .child(TerminalElement::new(
            terminal_handle,
            terminal_view_handle,
            self.focus_handle.clone(),
            focused,
            self.should_show_cursor(focused, cx),
            Default::default(),
          )),
      )
  }
}

impl TerminalView {
  pub fn new(
    terminal: Entity<Terminal>,
    window: &mut Window,
    index: usize,
    cx: &mut Context<Self>,
  ) -> Self {
    let terminal_subscriptions = subscribe_for_terminal_events(&terminal, window, cx);

    let focus_handle = cx.focus_handle();
    let focus_in = cx.on_focus_in(&focus_handle, window, |terminal_view, window, cx| {
      terminal_view.focus_in(window, cx);
    });
    let focus_out = cx.on_focus_out(
      &focus_handle,
      window,
      |terminal_view, _event, window, cx| {
        terminal_view.focus_out(window, cx);
      },
    );

    Self {
      terminal,
      focus_handle: focus_handle,
      has_bell: false,
      blink_state: true,
      blinking_paused: false,
      blink_epoch: 0,
      hover: None,
      hover_tooltip_update: Task::ready(()),
      scroll_top: Pixels::ZERO,
      ime_state: None,
      index: index,
      _subscriptions: vec![focus_in, focus_out],
      _terminal_subscriptions: terminal_subscriptions,
    }
  }

  /// Sets the marked (pre-edit) text from the IME.
  pub(crate) fn set_marked_text(
    &mut self,
    text: String,
    range: Option<Range<usize>>,
    cx: &mut Context<Self>,
  ) {
    self.ime_state = Some(ImeState {
      marked_text: text,
      marked_range_utf16: range,
    });
    cx.notify();
  }

  /// Gets the current marked range (UTF-16).
  pub(crate) fn marked_text_range(&self) -> Option<Range<usize>> {
    self
      .ime_state
      .as_ref()
      .and_then(|state| state.marked_range_utf16.clone())
  }

  /// Clears the marked (pre-edit) text state.
  pub(crate) fn clear_marked_text(&mut self, cx: &mut Context<Self>) {
    if self.ime_state.is_some() {
      self.ime_state = None;
      cx.notify();
    }
  }

  /// Commits (sends) the given text to the PTY. Called by InputHandler::replace_text_in_range.
  pub(crate) fn commit_text(&mut self, text: &str, cx: &mut Context<Self>) {
    if !text.is_empty() {
      self.terminal.update(cx, |term, _| {
        term.input(text.to_string().into_bytes());
      });
    }
  }

  pub(crate) fn terminal_bounds(&self, cx: &App) -> TerminalBounds {
    self.terminal.read(cx).last_content().terminal_bounds
  }

  pub fn entity(&self) -> &Entity<Terminal> {
    &self.terminal
  }

  pub fn has_bell(&self) -> bool {
    self.has_bell
  }

  pub fn clear_bell(&mut self, _cx: &mut Context<TerminalView>) {
    self.has_bell = false;
  }

  fn focus_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.terminal.update(cx, |terminal, _| {
      terminal.focus_in();
    });

    self.blinking_paused = false;
    self.blink_state = true;

    window.invalidate_character_coordinates();
    cx.notify();
  }

  fn focus_out(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    self.blink_state = true;
    self.blinking_paused = true;

    self.terminal.update(cx, |terminal, _| {
      terminal.focus_out();
    });
    cx.notify();
  }

  pub fn scroll_wheel(&mut self, event: &gpui::ScrollWheelEvent, cx: &mut Context<Self>) {
    // Ctrl+Scroll for zooming
    if event.modifiers.control {
      let delta = event.delta.pixel_delta(gpui::px(1.0)).y;
      if delta > gpui::px(0.0) {
        themeing::ZoomState::update_global(cx, |zoom: &mut themeing::ZoomState, _| {
          zoom.zoom_in();
        });
      } else if delta < gpui::px(0.0) {
        themeing::ZoomState::update_global(cx, |zoom: &mut themeing::ZoomState, _| {
          zoom.zoom_out();
        });
      }
      cx.notify();
      return;
    }

    self
      .terminal
      .update(cx, |term, _| term.scroll_wheel(event, 1.0));
    cx.notify();
  }

  pub fn should_show_cursor(&self, focused: bool, cx: &mut Context<Self>) -> bool {
    if !focused
      || self.blinking_paused
      || self
        .terminal
        .read(cx)
        .last_content
        .mode
        .contains(TermMode::ALT_SCREEN)
    {
      return true;
    }

    self.blink_state
  }

  fn blink_cursors(&mut self, epoch: usize, window: &mut Window, cx: &mut Context<Self>) {
    if epoch == self.blink_epoch && !self.blinking_paused {
      self.blink_state = !self.blink_state;
      cx.notify();

      let epoch = self.next_blink_epoch();
      cx.spawn_in(window, async move |this, cx| {
        Timer::after(CURSOR_BLINK_INTERVAL).await;
        this
          .update_in(cx, |this, window, cx| this.blink_cursors(epoch, window, cx))
          .ok();
      })
      .detach();
    }
  }

  pub fn pause_cursor_blinking(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.blink_state = true;
    cx.notify();

    let epoch = self.next_blink_epoch();
    cx.spawn_in(window, async move |this, cx| {
      Timer::after(CURSOR_BLINK_INTERVAL).await;
      this
        .update_in(cx, |this, window, cx| {
          this.resume_cursor_blinking(epoch, window, cx)
        })
        .ok();
    })
    .detach();
  }

  pub fn terminal(&self) -> &Entity<Terminal> {
    &self.terminal
  }

  fn next_blink_epoch(&mut self) -> usize {
    self.blink_epoch += 1;
    self.blink_epoch
  }

  fn resume_cursor_blinking(&mut self, epoch: usize, window: &mut Window, cx: &mut Context<Self>) {
    if epoch == self.blink_epoch {
      self.blinking_paused = false;
      self.blink_cursors(epoch, window, cx);
    }
  }

  fn dispatch_context(&self, cx: &App) -> KeyContext {
    let mut dispatch_context = KeyContext::new_with_defaults();
    dispatch_context.add("Terminal");

    let mode = self.terminal.read(cx).last_content.mode;
    dispatch_context.set(
      "screen",
      if mode.contains(TermMode::ALT_SCREEN) {
        "alt"
      } else {
        "normal"
      },
    );

    if mode.contains(TermMode::APP_CURSOR) {
      dispatch_context.add("DECCKM");
    }
    if mode.contains(TermMode::APP_KEYPAD) {
      dispatch_context.add("DECPAM");
    } else {
      dispatch_context.add("DECPNM");
    }
    if mode.contains(TermMode::SHOW_CURSOR) {
      dispatch_context.add("DECTCEM");
    }
    if mode.contains(TermMode::LINE_WRAP) {
      dispatch_context.add("DECAWM");
    }
    if mode.contains(TermMode::ORIGIN) {
      dispatch_context.add("DECOM");
    }
    if mode.contains(TermMode::INSERT) {
      dispatch_context.add("IRM");
    }
    //LNM is apparently the name for this. https://vt100.net/docs/vt510-rm/LNM.html
    if mode.contains(TermMode::LINE_FEED_NEW_LINE) {
      dispatch_context.add("LNM");
    }
    if mode.contains(TermMode::FOCUS_IN_OUT) {
      dispatch_context.add("report_focus");
    }
    if mode.contains(TermMode::ALTERNATE_SCROLL) {
      dispatch_context.add("alternate_scroll");
    }
    if mode.contains(TermMode::BRACKETED_PASTE) {
      dispatch_context.add("bracketed_paste");
    }
    if mode.intersects(TermMode::MOUSE_MODE) {
      dispatch_context.add("any_mouse_reporting");
    }
    {
      let mouse_reporting = if mode.contains(TermMode::MOUSE_REPORT_CLICK) {
        "click"
      } else if mode.contains(TermMode::MOUSE_DRAG) {
        "drag"
      } else if mode.contains(TermMode::MOUSE_MOTION) {
        "motion"
      } else {
        "off"
      };
      dispatch_context.set("mouse_reporting", mouse_reporting);
    }
    {
      let format = if mode.contains(TermMode::SGR_MOUSE) {
        "sgr"
      } else if mode.contains(TermMode::UTF8_MOUSE) {
        "utf8"
      } else {
        "normal"
      };
      dispatch_context.set("mouse_format", format);
    };

    if self.terminal.read(cx).last_content.selection.is_some() {
      dispatch_context.add("selection");
    }

    dispatch_context
  }

  fn key_down(&mut self, event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
    self.clear_bell(cx);
    self.pause_cursor_blinking(window, cx);

    let handled = self.terminal.update(cx, |term, _cx| {
      let handled = term.try_keystroke(&event.keystroke, false);
      tracing::trace!("key {:?} handled: {}", event.keystroke, handled);
      handled
    });

    if handled {
      cx.stop_propagation();
    }
  }

  fn send_tab(&mut self, _: &SendTab, window: &mut Window, cx: &mut Context<Self>) {
    self.clear_bell(cx);
    self.pause_cursor_blinking(window, cx);
    self.terminal.update(cx, |term, _cx| {
      term.input(KnownKeys::Tab.as_slice());
    });
  }

  fn send_tab_prev(&mut self, _: &SendTabPrev, window: &mut Window, cx: &mut Context<Self>) {
    self.clear_bell(cx);
    self.pause_cursor_blinking(window, cx);
    self.terminal.update(cx, |term, _cx| {
      term.input(KnownKeys::ShiftTab.as_slice());
    });
  }

  fn copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
    self.terminal.update(cx, |term, cx| {
      term.copy_and_clear_selection(cx);
    });
  }

  fn paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
      self.terminal.update(cx, |term, _cx| {
        term.input(text.into_bytes());
      });
    }
  }

  fn scroll_page_up(&mut self, _: &ScrollPageUp, _window: &mut Window, cx: &mut Context<Self>) {
    self.terminal.update(cx, |term, _cx| {
      term.scroll(AlacScroll::PageUp);
    });
  }

  fn scroll_page_down(&mut self, _: &ScrollPageDown, _window: &mut Window, cx: &mut Context<Self>) {
    self.terminal.update(cx, |term, _cx| {
      term.scroll(AlacScroll::PageDown);
    });
  }

  fn send_page_up(&mut self, _: &SendPageUp, window: &mut Window, cx: &mut Context<Self>) {
    self.clear_bell(cx);
    self.pause_cursor_blinking(window, cx);
    self.terminal.update(cx, |term, _cx| {
      term.input(KnownKeys::PageUp.as_slice()); // Page Up escape sequence
    });
  }

  fn send_page_down(&mut self, _: &SendPageDown, window: &mut Window, cx: &mut Context<Self>) {
    self.clear_bell(cx);
    self.pause_cursor_blinking(window, cx);
    self.terminal.update(cx, |term, _cx| {
      term.input(KnownKeys::PageDown.as_slice()); // Page Down escape sequence
    });
  }

  fn zoom_in(&mut self, _: &ZoomIn, _window: &mut Window, cx: &mut Context<Self>) {
    themeing::ZoomState::update_global(cx, |zoom: &mut themeing::ZoomState, _| {
      zoom.zoom_in();
    });
    cx.notify();
  }

  fn zoom_out(&mut self, _: &ZoomOut, _window: &mut Window, cx: &mut Context<Self>) {
    themeing::ZoomState::update_global(cx, |zoom: &mut themeing::ZoomState, _| {
      zoom.zoom_out();
    });
    cx.notify();
  }

  fn zoom_reset(&mut self, _: &ZoomReset, _window: &mut Window, cx: &mut Context<Self>) {
    themeing::ZoomState::update_global(cx, |zoom: &mut themeing::ZoomState, _| {
      zoom.reset();
    });
    cx.notify();
  }
}

fn subscribe_for_terminal_events(
  terminal: &Entity<Terminal>,
  window: &mut Window,
  cx: &mut Context<TerminalView>,
) -> Vec<gpui::Subscription> {
  let terminal_subscription = cx.observe(terminal, |_, _, cx| cx.notify());
  let terminal_events_subscription = cx.subscribe_in(
    terminal,
    window,
    move |terminal_view, terminal, event, _window, cx| match event {
      crate::terminal::Event::TitleChanged => {
        cx.emit(TerminalEvent::UpdateTab);
      }
      crate::terminal::Event::CloseTerminal => {
        cx.emit(TerminalEvent::CloseTerminal(terminal_view.index));
      }
      crate::terminal::Event::Bell => {
        terminal_view.has_bell = true;
        cx.emit(TerminalEvent::Wakeup);
      }
      crate::terminal::Event::Wakeup => {
        cx.notify();
      }
      crate::terminal::Event::BlinkChanged(_) => {}
      crate::terminal::Event::SelectionsChanged => {}
      crate::terminal::Event::Open(url) => {
        cx.open_url(url);
        cx.notify();
      }
      crate::terminal::Event::NewNavigationTarget(target) => {
        match target
          .as_ref()
          .zip(terminal.read(cx).last_content.last_hovered_word.as_ref())
        {
          Some((url, hovered_word)) => {
            if Some(hovered_word)
              != terminal_view
                .hover
                .as_ref()
                .map(|hover| &hover.hovered_word)
            {
              terminal_view.hover = Some(HoverTarget {
                tooltip: url.clone(),
                hovered_word: hovered_word.clone(),
              });
              terminal_view.hover_tooltip_update = Task::ready(());
              cx.notify();
            }
          }
          None => {
            terminal_view.hover = None;
            terminal_view.hover_tooltip_update = Task::ready(());
            cx.notify();
          }
        }
      }
    },
  );
  vec![terminal_subscription, terminal_events_subscription]
}
