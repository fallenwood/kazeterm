use alacritty_terminal::term::TermMode;
use gpui::*;
use gpui_component::{h_flex, menu::ContextMenuExt, v_flex};
use terminal::TerminalView;
use themeing::SettingsStore;

use super::main_window::MainWindow;
use super::split_pane_context_menu::build_terminal_context_menu;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
  Horizontal,
  Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneId(pub usize);

/// Drag payload for resizing split pane dividers.
#[derive(Clone)]
pub(crate) struct ResizeSplitDivider {
  entity_id: EntityId,
  /// Path through the split tree to identify which split node owns this divider.
  /// Each entry: `false` = first child, `true` = second child.
  path: Vec<bool>,
  direction: SplitDirection,
}

impl Render for ResizeSplitDivider {
  fn render(&mut self, _window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    Empty
  }
}

/// Encode a tree path as a single `usize` for use in element IDs.
fn path_to_usize(path: &[bool]) -> usize {
  let mut result = 0usize;
  for (i, b) in path.iter().enumerate() {
    if *b {
      result |= 1 << i;
    }
  }
  // Use bit length to disambiguate paths of different lengths (e.g. [] vs [false])
  result | (1 << path.len())
}

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
            **first = new_first;
          }
          // Whether replacement was Some (child collapsed) or None (deeper
          // restructure already applied), the current Split is still valid.
          return ClosePaneResult {
            replacement: None,
            closed: true,
          };
        }

        // Try to close in the second subtree
        let second_result = second.as_mut().close_pane(pane_id);
        if second_result.closed {
          if let Some(new_second) = second_result.replacement {
            **second = new_second;
          }
          return ClosePaneResult {
            replacement: None,
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
      SplitPane::Split { first, second, .. } => first
        .find_pane_by_terminal_index(terminal_index, cx)
        .or_else(|| second.find_pane_by_terminal_index(terminal_index, cx)),
    }
  }

  pub fn find_terminal(&self, pane_id: PaneId) -> Option<Entity<TerminalView>> {
    match self {
      SplitPane::Terminal { id, terminal } if *id == pane_id => Some(terminal.clone()),
      SplitPane::Split { first, second, .. } => first
        .find_terminal(pane_id)
        .or_else(|| second.find_terminal(pane_id)),
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

  #[allow(dead_code)]
  pub fn contains_pane(&self, pane_id: PaneId) -> bool {
    match self {
      SplitPane::Terminal { id, .. } => *id == pane_id,
      SplitPane::Split { first, second, .. } => {
        first.contains_pane(pane_id) || second.contains_pane(pane_id)
      }
    }
  }

  /// Swap the children of the innermost split that directly contains the active pane.
  pub fn swap_at_active(&mut self, active_id: PaneId) -> bool {
    match self {
      SplitPane::Terminal { .. } => false,
      SplitPane::Split {
        first,
        second,
        ratio,
        ..
      } => {
        // Check if either immediate child is (or contains) the active terminal.
        // Prefer recursing deeper first so we swap at the innermost level.
        if first.swap_at_active(active_id) || second.swap_at_active(active_id) {
          return true;
        }

        // If the active pane is a direct child terminal, swap the two halves.
        let first_is_active =
          matches!(first.as_ref(), SplitPane::Terminal { id, .. } if *id == active_id);
        let second_is_active =
          matches!(second.as_ref(), SplitPane::Terminal { id, .. } if *id == active_id);

        if first_is_active || second_is_active {
          std::mem::swap(first, second);
          *ratio = 1.0 - *ratio;
          return true;
        }

        false
      }
    }
  }

  /// Update the split ratio at the given tree path.
  pub fn update_ratio(&mut self, path: &[bool], new_ratio: f32) {
    if let SplitPane::Split {
      ratio,
      first,
      second,
      ..
    } = self
    {
      if path.is_empty() {
        *ratio = new_ratio;
      } else if !path[0] {
        first.update_ratio(&path[1..], new_ratio);
      } else {
        second.update_ratio(&path[1..], new_ratio);
      }
    }
  }

  #[allow(clippy::only_used_in_recursion)]
  pub fn render(
    &self,
    active_pane_id: Option<PaneId>,
    path: Vec<bool>,
    window: &mut Window,
    cx: &mut Context<MainWindow>,
  ) -> AnyElement {
    match self {
      SplitPane::Terminal { id: _, terminal } => {
        let right_click_context_menu = cx
          .try_global::<config::Config>()
          .map(|c| c.right_click_context_menu)
          .unwrap_or(false);

        let base = div().size_full().child(terminal.clone());

        if right_click_context_menu {
          let terminal = terminal.clone();
          let main_window = cx.entity().clone();
          base
            .context_menu(move |menu, window, cx| {
              let mode = terminal.read(cx).terminal.read(cx).last_content.mode;
              if mode.intersects(TermMode::MOUSE_MODE) {
                return menu;
              }
              build_terminal_context_menu(menu, &terminal, &main_window, window, cx)
            })
            .into_any_element()
        } else {
          base.into_any_element()
        }
      }
      SplitPane::Split {
        direction,
        first,
        second,
        ratio,
      } => {
        let ratio = *ratio;
        let direction = *direction;
        let divider_width = cx
          .try_global::<config::Config>()
          .map(|config| config.get_split_pane_divider_width())
          .unwrap_or(6.0);
        let colors = cx.global::<SettingsStore>().theme().colors().clone();

        let mut first_path = path.clone();
        first_path.push(false);
        let mut second_path = path.clone();
        second_path.push(true);

        let first_element = first.render(active_pane_id, first_path, window, cx);
        let second_element = second.render(active_pane_id, second_path, window, cx);

        let path_id = path_to_usize(&path);
        let container_id = ElementId::from(("split-container", path_id));
        let divider_id = ElementId::from(("split-divider", path_id));

        let drag_value = ResizeSplitDivider {
          entity_id: cx.entity_id(),
          path: path.clone(),
          direction,
        };

        let divider = match direction {
          SplitDirection::Horizontal => v_flex()
            .id(divider_id)
            .h(px(divider_width))
            .w_full()
            .flex_shrink_0()
            .justify_center()
            .cursor(CursorStyle::ResizeUpDown)
            .hover(|style| style.bg(colors.border))
            .on_drag(drag_value, |drag, _, _, cx| {
              cx.stop_propagation();
              cx.new(|_| drag.clone())
            })
            .child(div().h(px(1.0)).w_full().bg(colors.border_variant)),
          SplitDirection::Vertical => h_flex()
            .id(divider_id)
            .w(px(divider_width))
            .h_full()
            .flex_shrink_0()
            .justify_center()
            .cursor(CursorStyle::ResizeLeftRight)
            .hover(|style| style.bg(colors.border))
            .on_drag(drag_value, |drag, _, _, cx| {
              cx.stop_propagation();
              cx.new(|_| drag.clone())
            })
            .child(div().w(px(1.0)).h_full().bg(colors.border_variant)),
        };

        let container = match direction {
          SplitDirection::Horizontal => v_flex(),
          SplitDirection::Vertical => h_flex(),
        };

        container
          .id(container_id)
          .size_full()
          .on_drag_move(cx.listener({
            let path = path.clone();
            move |this, e: &DragMoveEvent<ResizeSplitDivider>, _window, cx| {
              let drag = e.drag(cx);
              if cx.entity_id() != drag.entity_id || drag.path != path {
                return;
              }
              let new_ratio = match drag.direction {
                SplitDirection::Horizontal => {
                  ((e.event.position.y - e.bounds.origin.y) / e.bounds.size.height).clamp(0.1, 0.9)
                }
                SplitDirection::Vertical => {
                  ((e.event.position.x - e.bounds.origin.x) / e.bounds.size.width).clamp(0.1, 0.9)
                }
              };
              if let Some(active_ix) = this.active_tab_ix {
                if let Some(item) = this.items.get_mut(active_ix) {
                  item.split_container.update_ratio(&path, new_ratio);
                }
              }
              cx.notify();
            }
          }))
          .child(
            div()
              .flex_basis(relative(ratio))
              .size_full()
              .min_h_0()
              .min_w_0()
              .child(first_element),
          )
          .child(divider)
          .child(
            div()
              .flex_basis(relative(1.0 - ratio))
              .size_full()
              .min_h_0()
              .min_w_0()
              .child(second_element),
          )
          .into_any_element()
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
    self
      .active_pane_id
      .and_then(|id| self.root.find_terminal(id))
  }

  pub fn set_active_pane(&mut self, pane_id: PaneId) {
    if self.root.find_terminal(pane_id).is_some() {
      self.active_pane_id = Some(pane_id);
    }
  }

  /// Swap the two halves of the split containing the active pane.
  pub fn swap_panes(&mut self) -> bool {
    if let Some(active_id) = self.active_pane_id {
      self.root.swap_at_active(active_id)
    } else {
      false
    }
  }

  /// Move focus to the next pane (cycles through terminals in tree order).
  pub fn focus_next_pane(&mut self) -> Option<Entity<TerminalView>> {
    let terminals = self.root.all_terminals();
    if terminals.len() <= 1 {
      return None;
    }
    if let Some(active_id) = self.active_pane_id {
      let current_ix = terminals.iter().position(|(id, _)| *id == active_id);
      let next_ix = match current_ix {
        Some(ix) => (ix + 1) % terminals.len(),
        None => 0,
      };
      self.active_pane_id = Some(terminals[next_ix].0);
      Some(terminals[next_ix].1.clone())
    } else {
      None
    }
  }

  /// Move focus to the previous pane (cycles through terminals in tree order).
  pub fn focus_prev_pane(&mut self) -> Option<Entity<TerminalView>> {
    let terminals = self.root.all_terminals();
    if terminals.len() <= 1 {
      return None;
    }
    if let Some(active_id) = self.active_pane_id {
      let current_ix = terminals.iter().position(|(id, _)| *id == active_id);
      let prev_ix = match current_ix {
        Some(0) => terminals.len() - 1,
        Some(ix) => ix - 1,
        None => 0,
      };
      self.active_pane_id = Some(terminals[prev_ix].0);
      Some(terminals[prev_ix].1.clone())
    } else {
      None
    }
  }

  pub fn all_terminals(&self) -> Vec<(PaneId, Entity<TerminalView>)> {
    self.root.all_terminals()
  }

  pub fn update_ratio(&mut self, path: &[bool], new_ratio: f32) {
    self.root.update_ratio(path, new_ratio);
  }

  pub fn render(&self, window: &mut Window, cx: &mut Context<MainWindow>) -> AnyElement {
    self.root.render(self.active_pane_id, vec![], window, cx)
  }
}
