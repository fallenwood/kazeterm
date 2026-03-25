use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, Sizable};
use terminal::TerminalView;

#[derive(Clone)]
pub struct SearchBarCloseEvent;

pub struct SearchBar {
  query: SharedString,
  terminal_view: Option<Entity<TerminalView>>,
  match_case: bool,
  match_whole: bool,
  use_regex: bool,
  search_input_state: Entity<InputState>,
  _subscription: Subscription,
}

impl EventEmitter<SearchBarCloseEvent> for SearchBar {}

impl SearchBar {
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let search_input_state = cx.new(|cx| InputState::new(window, cx));

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
    }
  }

  pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
    let focus_handle = self.search_input_state.focus_handle(cx);
    window.focus(&focus_handle);
  }

  pub fn set_terminal_view(&mut self, terminal_view: Entity<TerminalView>) {
    self.terminal_view = Some(terminal_view);
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
}

impl Focusable for SearchBar {
  fn focus_handle(&self, cx: &App) -> FocusHandle {
    self.search_input_state.focus_handle(cx)
  }
}

impl Render for SearchBar {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let active_bg = theme.accent;
    let (match_count, current_match) = self.read_match_state(cx);

    div()
      .absolute()
      .top_2()
      .right_4()
      .bg(theme.popover)
      .text_color(theme.popover_foreground)
      .rounded_md()
      .shadow_lg()
      .border_1()
      .border_color(theme.border)
      .py_1()
      .px_2()
      .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
        cx.stop_propagation();
      })
      .on_mouse_down(gpui::MouseButton::Right, |_, _, cx| {
        cx.stop_propagation();
      })
      .on_mouse_up(gpui::MouseButton::Left, |_, _, cx| {
        cx.stop_propagation();
      })
      .on_mouse_up(gpui::MouseButton::Right, |_, _, cx| {
        cx.stop_propagation();
      })
      .child(
        gpui_component::h_flex()
          .gap_2()
          .items_center()
          .child(
            Input::new(&self.search_input_state)
              .w(px(200.))
              .cursor_text(),
          )
          .child(
            div()
              .text_sm()
              .text_color(theme.muted_foreground)
              .min_w(px(50.))
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
                  .small()
                  .label("↑")
                  .on_click(cx.listener(|this, _, _window, cx| {
                    this.go_to_previous_match(cx);
                  })),
              )
              .child(
                Button::new("next-match")
                  .ghost()
                  .small()
                  .label("↓")
                  .on_click(cx.listener(|this, _, _window, cx| {
                    this.go_to_next_match(cx);
                  })),
              ),
          )
          .child(div().h(px(16.)).w(px(1.)).bg(theme.border))
          .child(
            gpui_component::h_flex()
              .gap_0p5()
              .items_center()
              .child(
                Button::new("match-case")
                  .ghost()
                  .small()
                  .label("Aa")
                  .when(self.match_case, |btn| btn.bg(active_bg))
                  .on_click(cx.listener(|this, _, _window, cx| {
                    this.toggle_match_case(cx);
                  })),
              )
              .child(
                Button::new("match-whole")
                  .ghost()
                  .small()
                  .label("\"\"")
                  .when(self.match_whole, |btn| btn.bg(active_bg))
                  .on_click(cx.listener(|this, _, _window, cx| {
                    this.toggle_match_whole(cx);
                  })),
              )
              .child(
                Button::new("regex")
                  .ghost()
                  .small()
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
              .small()
              .label("×")
              .on_click(cx.listener(|this, _, _window, cx| {
                this.close(cx);
              })),
          ),
      )
  }
}
