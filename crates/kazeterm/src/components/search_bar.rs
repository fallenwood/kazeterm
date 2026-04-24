use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Escape as InputEscape;
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, IconName, Sizable};
use terminal::TerminalView;

const DEFAULT_FONT_SIZE: f32 = 14.0;

#[derive(Clone)]
struct DragSearchBar(EntityId);

impl Render for DragSearchBar {
  fn render(&mut self, _window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    Empty
  }
}

#[derive(Clone)]
pub struct SearchBarCloseEvent;

/// Saveable search bar state, stored per-tab so each terminal has independent search.
#[derive(Clone)]
pub struct SearchBarState {
  pub query: SharedString,
  pub match_case: bool,
  pub match_whole: bool,
  pub use_regex: bool,
  pub visible: bool,
  pub position: Point<Pixels>,
}

impl Default for SearchBarState {
  fn default() -> Self {
    Self {
      query: SharedString::from(""),
      match_case: false,
      match_whole: false,
      use_regex: false,
      visible: false,
      position: Point::new(px(0.), px(0.)),
    }
  }
}

pub struct SearchBar {
  query: SharedString,
  terminal_view: Option<Entity<TerminalView>>,
  match_case: bool,
  match_whole: bool,
  use_regex: bool,
  search_input_state: Entity<InputState>,
  _subscription: Subscription,
  drag_offset: Option<Point<Pixels>>,
  position: Point<Pixels>,
  font_size: f32,
}

impl EventEmitter<SearchBarCloseEvent> for SearchBar {}

impl SearchBar {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let search_input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search..."));

    let subscription = cx.subscribe_in(
      &search_input_state,
      window,
      |view, state, event, _window, cx| match event {
        gpui_component::input::InputEvent::Focus => {}
        gpui_component::input::InputEvent::PressEnter { secondary } => {
          _ = secondary;
          view.query = state.read(cx).value().clone();
          view.perform_search(cx);
        }
        _ => {}
      },
    );

    Self {
      query: SharedString::from(""),
      terminal_view: None,
      match_case: false,
      match_whole: false,
      use_regex: false,
      search_input_state,
      _subscription: subscription,
      drag_offset: None,
      position: Point::new(px(0.), px(0.)),
      font_size: DEFAULT_FONT_SIZE,
    }
  }

  pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
    let focus_handle = self.search_input_state.focus_handle(cx);
    window.focus(&focus_handle);
  }

  pub fn set_terminal_view(&mut self, terminal_view: Entity<TerminalView>) {
    self.terminal_view = Some(terminal_view);
  }

  pub fn set_font_size(&mut self, font_size: f32) {
    self.font_size = font_size;
  }

  /// Save the current search bar state for later restoration.
  /// Reads the live input text so unsaved edits (typed but not yet Enter'd) are preserved.
  pub fn save_state(&self, visible: bool, cx: &App) -> SearchBarState {
    let input_text = self.search_input_state.read(cx).value().clone();
    SearchBarState {
      query: input_text,
      match_case: self.match_case,
      match_whole: self.match_whole,
      use_regex: self.use_regex,
      visible,
      position: self.position,
    }
  }

  /// Restore search bar state from a previously saved state.
  pub fn restore_state(
    &mut self,
    state: &SearchBarState,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.query = state.query.clone();
    self.match_case = state.match_case;
    self.match_whole = state.match_whole;
    self.use_regex = state.use_regex;
    self.position = state.position;

    // Update the input field text to match the restored query
    self.search_input_state.update(cx, |input_state, cx| {
      input_state.set_value(state.query.to_string(), window, cx);
    });

    // Re-execute the search on the (now-active) terminal so highlights appear
    if !state.query.is_empty() {
      self.perform_search(cx);
    }
  }

  pub fn clear_search(&mut self, cx: &mut Context<Self>) {
    self.query = SharedString::from("");

    if let Some(terminal_view) = &self.terminal_view {
      let terminal_view = terminal_view.clone();
      cx.update_entity(&terminal_view, |term_view, cx| {
        term_view.terminal.update(cx, |terminal, _cx| {
          terminal.clear_search_query();
        });
      });
    }

    cx.notify();
  }

  fn perform_search(&mut self, cx: &mut Context<Self>) {
    let query = self.query.to_string();
    if query.is_empty() {
      return;
    }

    let Some(terminal_view) = &self.terminal_view else {
      return;
    };

    let terminal_view = terminal_view.clone();
    let match_case = self.match_case;
    let match_whole = self.match_whole;
    let use_regex = self.use_regex;

    cx.update_entity(&terminal_view, |term_view, cx| {
      term_view.terminal.update(cx, |terminal, _cx| {
        terminal.set_search_query(query, match_case, match_whole, use_regex);
      });
    });

    cx.notify();
  }

  fn toggle_match_case(&mut self, cx: &mut Context<Self>) {
    self.match_case = !self.match_case;
    self.perform_search(cx);
  }

  fn toggle_match_whole(&mut self, cx: &mut Context<Self>) {
    self.match_whole = !self.match_whole;
    self.perform_search(cx);
  }

  fn toggle_use_regex(&mut self, cx: &mut Context<Self>) {
    self.use_regex = !self.use_regex;
    self.perform_search(cx);
  }

  /// Read the current match count and index from the terminal.
  fn read_match_state(&self, cx: &App) -> (usize, usize) {
    if let Some(tv) = &self.terminal_view {
      let tv = tv.read(cx);
      let term = tv.terminal.read(cx);
      let count = term.last_content.search_matches.len();
      let current = term.last_content.current_search_match_index;
      (count, current)
    } else {
      (0, 0)
    }
  }

  fn go_to_previous_match(&mut self, cx: &mut Context<Self>) {
    let (match_count, current_match) = self.read_match_state(cx);
    if match_count == 0 {
      return;
    }
    let new_match = if current_match > 1 {
      current_match - 1
    } else {
      match_count
    };
    self.update_terminal_match(new_match, cx);
    cx.notify();
  }

  fn go_to_next_match(&mut self, cx: &mut Context<Self>) {
    let (match_count, current_match) = self.read_match_state(cx);
    if match_count == 0 {
      return;
    }
    let new_match = if current_match < match_count {
      current_match + 1
    } else {
      1
    };
    self.update_terminal_match(new_match, cx);
    cx.notify();
  }

  fn update_terminal_match(&self, index: usize, cx: &mut Context<Self>) {
    let Some(terminal_view) = &self.terminal_view else {
      return;
    };

    let terminal_view = terminal_view.clone();
    cx.update_entity(&terminal_view, |term_view, cx| {
      term_view.terminal.update(cx, |terminal, _cx| {
        terminal.set_current_search_match(index);
      });
    });
  }

  fn close(&mut self, cx: &mut Context<Self>) {
    cx.emit(SearchBarCloseEvent);
  }

  pub fn reset_position(&mut self) {
    self.position = Point::new(px(0.), px(0.));
  }
}

impl Focusable for SearchBar {
  fn focus_handle(&self, cx: &App) -> FocusHandle {
    self.search_input_state.focus_handle(cx)
  }
}

#[cfg(test)]
mod tests {
  use super::{SearchBar, SearchBarCloseEvent, SearchBarState};
  use gpui::{SharedString, TestAppContext};
  use std::{cell::RefCell, rc::Rc};

  #[gpui::test]
  fn new_search_bar_has_default_state(cx: &mut TestAppContext) {
    crate::test_support::init_test_app(cx);
    let window = cx.add_window(|window, cx| SearchBar::new(window, cx));
    cx.run_until_parked();

    let (query, match_case, use_regex) = window
      .update(cx, |bar, _, _| {
        (bar.query.clone(), bar.match_case, bar.use_regex)
      })
      .unwrap();
    assert_eq!(query.as_ref(), "");
    assert!(!match_case);
    assert!(!use_regex);
  }

  #[gpui::test]
  fn toggle_flags_flip_without_terminal(cx: &mut TestAppContext) {
    crate::test_support::init_test_app(cx);
    let window = cx.add_window(|window, cx| SearchBar::new(window, cx));
    cx.run_until_parked();

    window
      .update(cx, |bar, _, cx| {
        bar.toggle_match_case(cx);
        bar.toggle_match_whole(cx);
        bar.toggle_use_regex(cx);
      })
      .unwrap();
    cx.run_until_parked();

    let (mc, mw, re) = window
      .update(cx, |bar, _, _| {
        (bar.match_case, bar.match_whole, bar.use_regex)
      })
      .unwrap();
    assert!(mc && mw && re);
  }

  #[gpui::test]
  fn close_emits_event(cx: &mut TestAppContext) {
    crate::test_support::init_test_app(cx);
    let window = cx.add_window(|window, cx| SearchBar::new(window, cx));
    cx.run_until_parked();

    let received: Rc<RefCell<u32>> = Default::default();
    let received_clone = received.clone();
    cx.update(|cx| {
      let bar = window.root(cx).unwrap();
      cx.subscribe(&bar, move |_, _event: &SearchBarCloseEvent, _cx| {
        *received_clone.borrow_mut() += 1;
      })
      .detach();
    });

    window.update(cx, |bar, _, cx| bar.close(cx)).unwrap();
    cx.run_until_parked();

    assert_eq!(*received.borrow(), 1);
  }

  #[gpui::test]
  fn save_and_restore_state_round_trip(cx: &mut TestAppContext) {
    crate::test_support::init_test_app(cx);
    let window = cx.add_window(|window, cx| SearchBar::new(window, cx));
    cx.run_until_parked();

    // Populate some state, then save it.
    let saved = window
      .update(cx, |bar, _window, cx| {
        bar.match_case = true;
        bar.use_regex = true;
        bar.query = SharedString::from("needle");
        bar.save_state(true, cx)
      })
      .unwrap();

    assert_eq!(saved.query.as_ref(), "");
    assert!(saved.visible);
    assert!(saved.match_case);
    assert!(saved.use_regex);

    // Restore onto a fresh bar and verify flags/query propagate.
    let window2 = cx.add_window(|window, cx| SearchBar::new(window, cx));
    cx.run_until_parked();
    let restored_state = SearchBarState {
      query: SharedString::from("needle"),
      match_case: true,
      match_whole: false,
      use_regex: true,
      visible: true,
      ..Default::default()
    };
    window2
      .update(cx, |bar, window, cx| {
        bar.restore_state(&restored_state, window, cx);
      })
      .unwrap();
    cx.run_until_parked();

    let (q, mc, re) = window2
      .update(cx, |bar, _, _| {
        (bar.query.clone(), bar.match_case, bar.use_regex)
      })
      .unwrap();
    assert_eq!(q.as_ref(), "needle");
    assert!(mc);
    assert!(re);
  }
}

impl Render for SearchBar {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let active_bg = theme.accent;
    let (match_count, current_match) = self.read_match_state(cx);
    let pos = self.position;
    let font_size = self.font_size;

    let entity_id = cx.entity_id();

    div()
      .id("search-bar-drag")
      .absolute()
      .top(px(8.) + pos.y)
      .right(px(16.) - pos.x)
      .text_size(px(font_size))
      .bg(theme.popover)
      .text_color(theme.popover_foreground)
      .rounded_md()
      .shadow_lg()
      .border_1()
      .border_color(theme.border)
      .py_0p5()
      .px_1p5()
      .cursor_grab()
      .on_mouse_down(
        gpui::MouseButton::Left,
        cx.listener(|this, e: &MouseDownEvent, _, cx| {
          this.drag_offset = Some(e.position);
          cx.stop_propagation();
        }),
      )
      .on_drag(DragSearchBar(entity_id), |drag, _, _, cx| {
        cx.stop_propagation();
        cx.new(|_| drag.clone())
      })
      .on_drag_move(
        cx.listener(|this, e: &DragMoveEvent<DragSearchBar>, _, cx| {
          let drag = e.drag(cx);
          if cx.entity_id() != drag.0 {
            return;
          }
          if let Some(start) = this.drag_offset {
            let delta = e.event.position - start;
            this.position.x += delta.x;
            this.position.y += delta.y;
            this.drag_offset = Some(e.event.position);
            cx.notify();
          }
        }),
      )
      .on_mouse_up(
        gpui::MouseButton::Left,
        cx.listener(|this, _, _, cx| {
          this.drag_offset = None;
          cx.stop_propagation();
        }),
      )
      .on_action(cx.listener(|this, _: &InputEscape, _window, cx| {
        this.close(cx);
      }))
      .on_mouse_down(gpui::MouseButton::Right, |_, _, cx| {
        cx.stop_propagation();
      })
      .on_mouse_up(gpui::MouseButton::Right, |_, _, cx| {
        cx.stop_propagation();
      })
      .child(
        gpui_component::h_flex()
          .gap_1()
          .items_center()
          .child(
            Input::new(&self.search_input_state)
              .prefix(IconName::Search)
              .w(px(160.))
              .cursor_text(),
          )
          .child(
            div()
              .text_color(theme.muted_foreground)
              .min_w(px(40.))
              .text_center()
              .child(format!("{}/{}", current_match, match_count)),
          )
          .child(
            gpui_component::h_flex()
              .gap_0p5()
              .items_center()
              .child(
                Button::new("prev-match")
                  .ghost()
                  .xsmall()
                  .label("↑")
                  .on_click(cx.listener(|this, _, _window, cx| {
                    this.go_to_previous_match(cx);
                  })),
              )
              .child(
                Button::new("next-match")
                  .ghost()
                  .xsmall()
                  .label("↓")
                  .on_click(cx.listener(|this, _, _window, cx| {
                    this.go_to_next_match(cx);
                  })),
              ),
          )
          .child(div().h(px(14.)).w(px(1.)).bg(theme.border))
          .child(
            gpui_component::h_flex()
              .gap_0p5()
              .items_center()
              .child(
                Button::new("match-case")
                  .ghost()
                  .xsmall()
                  .label("Aa")
                  .when(self.match_case, |btn| btn.bg(active_bg))
                  .on_click(cx.listener(|this, _, _window, cx| {
                    this.toggle_match_case(cx);
                  })),
              )
              .child(
                Button::new("match-whole")
                  .ghost()
                  .xsmall()
                  .label("\"\"")
                  .when(self.match_whole, |btn| btn.bg(active_bg))
                  .on_click(cx.listener(|this, _, _window, cx| {
                    this.toggle_match_whole(cx);
                  })),
              )
              .child(
                Button::new("regex")
                  .ghost()
                  .xsmall()
                  .label(".*")
                  .when(self.use_regex, |btn| btn.bg(active_bg))
                  .on_click(cx.listener(|this, _, _window, cx| {
                    this.toggle_use_regex(cx);
                  })),
              ),
          )
          .child(
            Button::new("close-search")
              .ghost()
              .xsmall()
              .label("×")
              .on_click(cx.listener(|this, _, _window, cx| {
                this.close(cx);
              })),
          ),
      )
  }
}
