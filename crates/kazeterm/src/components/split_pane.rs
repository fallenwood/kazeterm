use gpui::*;
use gpui_component::{h_flex, v_flex};
use terminal::TerminalView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
  Horizontal,
  Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneId(pub usize);

pub enum SplitPane {
  Terminal {
    id: PaneId,
    terminal: Entity<TerminalView>,
  },
  Split {
    direction: SplitDirection,
    first: Box<SplitPane>,
    second: Box<SplitPane>,
    ratio: f32, // 0.0 to 1.0, representing the first pane's size
  },
}

pub struct ClosePaneResult {
  pub replacement: Option<SplitPane>,
  pub closed: bool,
}

impl SplitPane {
  pub fn new_terminal(id: PaneId, terminal: Entity<TerminalView>) -> Self {
    SplitPane::Terminal { id, terminal }
  }

  pub fn split(
    &mut self,
    pane_id: PaneId,
    direction: SplitDirection,
    new_terminal: Entity<TerminalView>,
    next_id: PaneId,
  ) -> bool {
    match self {
      SplitPane::Terminal { id, terminal } if *id == pane_id => {
        let old_terminal = terminal.clone();
        let old_id = *id;
        *self = SplitPane::Split {
          direction,
          first: Box::new(SplitPane::Terminal {
            id: old_id,
            terminal: old_terminal,
          }),
          second: Box::new(SplitPane::Terminal {
            id: next_id,
            terminal: new_terminal,
          }),
          ratio: 0.5,
        };
        true
      }
      SplitPane::Split { first, second, .. } => {
        first.split(pane_id, direction, new_terminal.clone(), next_id)
          || second.split(pane_id, direction, new_terminal, next_id)
      }
      _ => false,
    }
  }

  pub fn close_pane(&mut self, pane_id: PaneId) -> ClosePaneResult {
    // Important: this must distinguish "not found" from "closed in a subtree".
    // Otherwise callers can't tell whether to close the entire tab when a pane is
    // closed from a nested split tree (e.g. split twice and close the middle pane).
    match self {
      SplitPane::Terminal { id, .. } if *id == pane_id => ClosePaneResult {
        replacement: None,
        closed: true,
      },
      SplitPane::Split { first, second, .. } => {
        // Check if either immediate child is the terminal we want to close
        match (first.as_ref(), second.as_ref()) {
          (SplitPane::Terminal { id: first_id, .. }, _) if *first_id == pane_id => {
            // First child is the terminal to close, return the second child
            return ClosePaneResult {
              replacement: Some(*second.clone()),
              closed: true,
            };
          }
          (_, SplitPane::Terminal { id: second_id, .. }) if *second_id == pane_id => {
            // Second child is the terminal to close, return the first child
            return ClosePaneResult {
              replacement: Some(*first.clone()),
              closed: true,
            };
          }
          _ => {}
        }

        // The terminal to close is deeper in the tree
        // Try to close in the first subtree
        let first_result = first.as_mut().close_pane(pane_id);
        if first_result.closed {
          if let Some(new_first) = first_result.replacement {
            *first = Box::new(new_first);
            return ClosePaneResult {
              replacement: None,
              closed: true,
            };
          }

          return ClosePaneResult {
            replacement: Some(*second.clone()),
            closed: true,
          };
        }

        // Try to close in the second subtree
        let second_result = second.as_mut().close_pane(pane_id);
        if second_result.closed {
          if let Some(new_second) = second_result.replacement {
            *second = Box::new(new_second);
            return ClosePaneResult {
              replacement: None,
              closed: true,
            };
          }

          return ClosePaneResult {
            replacement: Some(*first.clone()),
            closed: true,
          };
        }

        // Not found in either subtree
        ClosePaneResult {
          replacement: None,
          closed: false,
        }
      }
      _ => ClosePaneResult {
        replacement: None,
        closed: false,
      },
    }
  }

  pub fn find_pane_by_terminal_index(
    &self,
    terminal_index: usize,
    cx: &gpui::App,
  ) -> Option<PaneId> {
    match self {
      SplitPane::Terminal { id, terminal } => {
        if terminal.read(cx).index == terminal_index {
          Some(*id)
        } else {
          None
        }
      }
      SplitPane::Split { first, second, .. } => {
        first.find_pane_by_terminal_index(terminal_index, cx)
          .or_else(|| second.find_pane_by_terminal_index(terminal_index, cx))
      }
    }
  }

  pub fn find_terminal(&self, pane_id: PaneId) -> Option<Entity<TerminalView>> {
    match self {
      SplitPane::Terminal { id, terminal } if *id == pane_id => Some(terminal.clone()),
      SplitPane::Split { first, second, .. } => {
        first.find_terminal(pane_id).or_else(|| second.find_terminal(pane_id))
      }
      _ => None,
    }
  }

  pub fn all_terminals(&self) -> Vec<(PaneId, Entity<TerminalView>)> {
    match self {
      SplitPane::Terminal { id, terminal } => vec![(*id, terminal.clone())],
      SplitPane::Split { first, second, .. } => {
        let mut terminals = first.all_terminals();
        terminals.extend(second.all_terminals());
        terminals
      }
    }
  }

  pub fn count_panes(&self) -> usize {
    match self {
      SplitPane::Terminal { .. } => 1,
      SplitPane::Split { first, second, .. } => first.count_panes() + second.count_panes(),
    }
  }

  pub fn render(
    &self,
    active_pane_id: Option<PaneId>,
    window: &mut Window,
    cx: &mut App,
  ) -> AnyElement {
    match self {
      SplitPane::Terminal { id, terminal } => {
        let is_active = Some(*id) == active_pane_id;
        let border_color = if is_active {
          let setting_store = cx.global::<themeing::SettingsStore>();
          let theme = setting_store.theme();
          theme.colors().border_focused
        } else {
          gpui::transparent_black()
        };

        div()
          .size_full()
          .border_2()
          .border_color(border_color)
          .child(terminal.clone())
          .into_any_element()
      }
      SplitPane::Split {
        direction,
        first,
        second,
        ratio,
      } => {
        let ratio = *ratio;
        match direction {
          SplitDirection::Horizontal => v_flex()
            .size_full()
            .child(
              div()
                .flex_basis(relative(ratio))
                .size_full()
                .child(first.render(active_pane_id, window, cx)),
            )
            .child(
              div()
                .h_1()
                .w_full()
                .bg(gpui::rgb(0x3a3a3a)),
            )
            .child(
              div()
                .flex_basis(relative(1.0 - ratio))
                .size_full()
                .child(second.render(active_pane_id, window, cx)),
            )
            .into_any_element(),
          SplitDirection::Vertical => h_flex()
            .size_full()
            .child(
              div()
                .flex_basis(relative(ratio))
                .size_full()
                .child(first.render(active_pane_id, window, cx)),
            )
            .child(
              div()
                .w_1()
                .h_full()
                .bg(gpui::rgb(0x3a3a3a)),
            )
            .child(
              div()
                .flex_basis(relative(1.0 - ratio))
                .size_full()
                .child(second.render(active_pane_id, window, cx)),
            )
            .into_any_element(),
        }
      }
    }
  }
}

impl Clone for SplitPane {
  fn clone(&self) -> Self {
    match self {
      SplitPane::Terminal { id, terminal } => SplitPane::Terminal {
        id: *id,
        terminal: terminal.clone(),
      },
      SplitPane::Split {
        direction,
        first,
        second,
        ratio,
      } => SplitPane::Split {
        direction: *direction,
        first: first.clone(),
        second: second.clone(),
        ratio: *ratio,
      },
    }
  }
}

pub struct SplitContainer {
  pub root: SplitPane,
  pub active_pane_id: Option<PaneId>,
  pub next_pane_id: usize,
}

impl SplitContainer {
  pub fn new(terminal: Entity<TerminalView>) -> Self {
    Self {
      root: SplitPane::new_terminal(PaneId(0), terminal),
      active_pane_id: Some(PaneId(0)),
      next_pane_id: 1,
    }
  }

  pub fn split_active_pane(
    &mut self,
    direction: SplitDirection,
    new_terminal: Entity<TerminalView>,
  ) -> Option<PaneId> {
    if let Some(active_id) = self.active_pane_id {
      let new_id = PaneId(self.next_pane_id);
      if self.root.split(active_id, direction, new_terminal, new_id) {
        self.next_pane_id += 1;
        self.active_pane_id = Some(new_id);
        return Some(new_id);
      }
    }
    None
  }

  pub fn close_active_pane(&mut self) -> bool {
    if let Some(active_id) = self.active_pane_id {
      // Don't close if it's the last pane
      if self.root.count_panes() <= 1 {
        return false;
      }

      let result = self.root.close_pane(active_id);
      if let Some(new_root) = result.replacement {
        self.root = new_root;
      }

      if result.closed {
        if self.active_pane_id == Some(active_id) && self.root.find_terminal(active_id).is_none() {
          let terminals = self.root.all_terminals();
          self.active_pane_id = terminals.first().map(|(id, _)| *id);
        }
        return true;
      }
    }
    false
  }

  pub fn close_pane_by_terminal_index(&mut self, terminal_index: usize, cx: &gpui::App) -> bool {
    // Find the pane ID for this terminal
    if let Some(pane_id) = self.root.find_pane_by_terminal_index(terminal_index, cx) {
      // Don't close if it's the last pane
      if self.root.count_panes() <= 1 {
        return false;
      }

      let result = self.root.close_pane(pane_id);
      if let Some(new_root) = result.replacement {
        self.root = new_root;
      }

      if result.closed {
        // Update active pane if we closed the active one (or if it no longer exists)
        if self.active_pane_id == Some(pane_id)
          || self
            .active_pane_id
            .is_some_and(|active| self.root.find_terminal(active).is_none())
        {
          let terminals = self.root.all_terminals();
          self.active_pane_id = terminals.first().map(|(id, _)| *id);
        }

        return true;
      }
    }
    false
  }

  pub fn get_active_terminal(&self) -> Option<Entity<TerminalView>> {
    self.active_pane_id.and_then(|id| self.root.find_terminal(id))
  }

  pub fn set_active_pane(&mut self, pane_id: PaneId) {
    if self.root.find_terminal(pane_id).is_some() {
      self.active_pane_id = Some(pane_id);
    }
  }

  pub fn all_terminals(&self) -> Vec<(PaneId, Entity<TerminalView>)> {
    self.root.all_terminals()
  }

  pub fn render(&self, window: &mut Window, cx: &mut App) -> AnyElement {
    self.root.render(self.active_pane_id, window, cx)
  }
}
