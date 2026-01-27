use alacritty_terminal::grid::Dimensions as _;
use alacritty_terminal::index::{Column, Point as AlacPoint};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, Sizable};
use regex::Regex;
use terminal::TerminalView;

#[derive(Clone)]
pub struct SearchBarCloseEvent;

pub struct SearchBar {
  query: SharedString,
  match_count: usize,
  current_match: usize,
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
        gpui_component::input::InputEvent::Focus => {
          tracing::debug!("Search input focused");
        }
        gpui_component::input::InputEvent::PressEnter { secondary } => {
          _ = secondary;
          tracing::debug!("Performing search for query: {}", state.read(cx).value());
          view.query = state.read(cx).value().clone();
          view.perform_search(cx);
        }
        _ => {}
      },
    );

    Self {
      query: SharedString::from(""),
      match_count: 0,
      current_match: 0,
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
    self.match_count = 0;
    self.current_match = 0;

    if let Some(terminal_view) = &self.terminal_view {
      let terminal_view = terminal_view.clone();
      cx.update_entity(&terminal_view, |term_view, cx| {
        term_view.terminal.update(cx, |terminal, _cx| {
          terminal.set_search_matches(vec![]);
          terminal.set_current_search_match(0);
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
    let mut match_count = 0;
    let mut match_ranges = Vec::new();

    let match_case = self.match_case;
    let match_whole = self.match_whole;
    let use_regex = self.use_regex;

    // Build regex only if regex mode is enabled
    let regex = if use_regex {
      let pattern = if match_whole {
        format!(r"\b{}\b", query)
      } else {
        query.clone()
      };
      if match_case {
        Regex::new(&pattern).ok()
      } else {
        Regex::new(&format!("(?i){}", pattern)).ok()
      }
    } else {
      None
    };

    // For regex mode, check if regex is valid
    if use_regex && regex.is_none() {
      self.match_count = 0;
      self.current_match = 0;
      cx.notify();
      return;
    }

    // Helper function to check if character is a word boundary
    fn is_word_char(c: char) -> bool {
      c.is_alphanumeric() || c == '_'
    }

    // Helper function to find matches in a line without regex
    fn find_matches_simple(
      line: &str,
      query: &str,
      match_case: bool,
      match_whole: bool,
    ) -> Vec<(usize, usize)> {
      let mut matches = Vec::new();
      let (search_line, search_query) = if match_case {
        (line.to_string(), query.to_string())
      } else {
        (line.to_lowercase(), query.to_lowercase())
      };

      let mut start = 0;
      while let Some(pos) = search_line[start..].find(&search_query) {
        let match_start = start + pos;
        let match_end = match_start + query.len();

        if match_whole {
          // Check word boundaries
          let before_ok =
            match_start == 0 || !is_word_char(line.chars().nth(match_start - 1).unwrap_or(' '));
          let after_ok =
            match_end >= line.len() || !is_word_char(line.chars().nth(match_end).unwrap_or(' '));
          if before_ok && after_ok {
            matches.push((match_start, match_end));
          }
        } else {
          matches.push((match_start, match_end));
        }
        start = match_start + 1;
      }
      matches
    }

    cx.update_entity(&terminal_view, |term_view, cx| {
      term_view.terminal.update(cx, |terminal, _cx| {
        let term_lock = terminal.term.lock();

        // Get the full range of the terminal including scrollback history
        let topmost_line = term_lock.topmost_line();
        let bottommost_line = term_lock.bottommost_line();
        let columns = term_lock.columns();

        // Iterate through all lines from topmost (history) to bottommost (current)
        let mut line = topmost_line;
        while line <= bottommost_line {
          let mut current_line_text = String::new();
          let mut current_line_cells = Vec::new();

          // Collect all characters in this line
          for col in 0..columns {
            let point = AlacPoint::new(line, Column(col));
            let cell = &term_lock.grid()[point];
            current_line_text.push(cell.c);
            current_line_cells.push(point);
          }

          // Trim trailing spaces for matching purposes but keep cell positions
          let trimmed_len = current_line_text.trim_end().len();

          if trimmed_len > 0 {
            // Find all matches in the line
            let line_matches: Vec<(usize, usize)> = if let Some(ref regex) = regex {
              regex
                .find_iter(&current_line_text[..trimmed_len])
                .map(|m| (m.start(), m.end()))
                .collect()
            } else {
              find_matches_simple(
                &current_line_text[..trimmed_len],
                &query,
                match_case,
                match_whole,
              )
            };

            for (start_pos, end_pos) in line_matches {
              let end_pos = end_pos.saturating_sub(1);
              if start_pos < current_line_cells.len() {
                let match_start = current_line_cells[start_pos];
                let match_end_idx = end_pos.min(current_line_cells.len().saturating_sub(1));
                let match_end = current_line_cells[match_end_idx];
                match_ranges.push(match_start..=match_end);
                match_count += 1;
              }
            }
          }

          line += 1;
        }

        drop(term_lock);

        terminal.set_search_matches(match_ranges.clone());
        terminal.set_current_search_match(if match_count > 0 { 1 } else { 0 });
      });
    });

    self.match_count = match_count;
    self.current_match = if match_count > 0 { 1 } else { 0 };

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

  fn go_to_previous_match(&mut self, cx: &mut Context<Self>) {
    if self.match_count == 0 {
      return;
    }
    if self.current_match > 1 {
      self.current_match -= 1;
    } else {
      // Wrap to last match
      self.current_match = self.match_count;
    }
    self.update_terminal_match(cx);
    cx.notify();
  }

  fn go_to_next_match(&mut self, cx: &mut Context<Self>) {
    if self.match_count == 0 {
      return;
    }
    if self.current_match < self.match_count {
      self.current_match += 1;
    } else {
      self.current_match = 1;
    }
    self.update_terminal_match(cx);
    cx.notify();
  }

  fn update_terminal_match(&self, cx: &mut Context<Self>) {
    let Some(terminal_view) = &self.terminal_view else {
      return;
    };

    let terminal_view = terminal_view.clone();
    let current_match = self.current_match;
    cx.update_entity(&terminal_view, |term_view, cx| {
      term_view.terminal.update(cx, |terminal, _cx| {
        terminal.set_current_search_match(current_match);
      });
    });
  }

  fn close(&mut self, cx: &mut Context<Self>) {
    cx.emit(SearchBarCloseEvent);
  }
}

impl Focusable for SearchBar {
  fn focus_handle(&self, cx: &App) -> FocusHandle {
    let focus_handle = self.search_input_state.focus_handle(cx);
    return focus_handle;
  }
}

impl Render for SearchBar {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let active_bg = theme.accent;

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
      .p_2()
      .min_w(px(300.))
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
              .flex_1()
              .px_2()
              .py_1()
              .border_1()
              .rounded_sm()
              .cursor_text(),
          )
          .child(
            gpui_component::h_flex()
              .gap_1()
              .items_center()
              .child(
                div()
                  .px_2()
                  .child(format!("{} / {}", self.current_match, self.match_count)),
              )
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
              .label("[ ]")
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
