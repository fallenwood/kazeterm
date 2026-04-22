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
pub enum PaneFocusDirection {
  Up,
  Down,
  Left,
  Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq)]
struct PaneBounds {
  id: PaneId,
  left: f32,
  top: f32,
  right: f32,
  bottom: f32,
}

impl PaneBounds {
  fn center_x(self) -> f32 {
    (self.left + self.right) * 0.5
  }

  fn center_y(self) -> f32 {
    (self.top + self.bottom) * 0.5
  }
}

#[derive(Debug, Clone, Copy)]
struct DirectionalPaneCandidate {
  pane_id: PaneId,
  primary_gap: f32,
  perpendicular_gap: f32,
  overlap: f32,
  center_gap: f32,
}

const PANE_FOCUS_EPSILON: f32 = 0.000_01;

fn axis_overlap(a_start: f32, a_end: f32, b_start: f32, b_end: f32) -> f32 {
  (a_end.min(b_end) - a_start.max(b_start)).max(0.0)
}

fn axis_gap(a_start: f32, a_end: f32, b_start: f32, b_end: f32) -> f32 {
  if a_end < b_start {
    b_start - a_end
  } else if b_end < a_start {
    a_start - b_end
  } else {
    0.0
  }
}

fn candidate_in_direction(
  active: PaneBounds,
  candidate: PaneBounds,
  direction: PaneFocusDirection,
) -> Option<DirectionalPaneCandidate> {
  let (primary_gap, perpendicular_gap, overlap, center_gap) = match direction {
    PaneFocusDirection::Up => {
      let gap = active.top - candidate.bottom;
      if gap < -PANE_FOCUS_EPSILON {
        return None;
      }
      (
        gap.max(0.0),
        axis_gap(active.left, active.right, candidate.left, candidate.right),
        axis_overlap(active.left, active.right, candidate.left, candidate.right),
        (active.center_x() - candidate.center_x()).abs(),
      )
    }
    PaneFocusDirection::Down => {
      let gap = candidate.top - active.bottom;
      if gap < -PANE_FOCUS_EPSILON {
        return None;
      }
      (
        gap.max(0.0),
        axis_gap(active.left, active.right, candidate.left, candidate.right),
        axis_overlap(active.left, active.right, candidate.left, candidate.right),
        (active.center_x() - candidate.center_x()).abs(),
      )
    }
    PaneFocusDirection::Left => {
      let gap = active.left - candidate.right;
      if gap < -PANE_FOCUS_EPSILON {
        return None;
      }
      (
        gap.max(0.0),
        axis_gap(active.top, active.bottom, candidate.top, candidate.bottom),
        axis_overlap(active.top, active.bottom, candidate.top, candidate.bottom),
        (active.center_y() - candidate.center_y()).abs(),
      )
    }
    PaneFocusDirection::Right => {
      let gap = candidate.left - active.right;
      if gap < -PANE_FOCUS_EPSILON {
        return None;
      }
      (
        gap.max(0.0),
        axis_gap(active.top, active.bottom, candidate.top, candidate.bottom),
        axis_overlap(active.top, active.bottom, candidate.top, candidate.bottom),
        (active.center_y() - candidate.center_y()).abs(),
      )
    }
  };

  Some(DirectionalPaneCandidate {
    pane_id: candidate.id,
    primary_gap,
    perpendicular_gap,
    overlap,
    center_gap,
  })
}

fn is_better_directional_candidate(
  candidate: DirectionalPaneCandidate,
  current_best: DirectionalPaneCandidate,
) -> bool {
  let candidate_overlaps = candidate.perpendicular_gap <= PANE_FOCUS_EPSILON;
  let best_overlaps = current_best.perpendicular_gap <= PANE_FOCUS_EPSILON;

  if candidate_overlaps != best_overlaps {
    return candidate_overlaps;
  }
  if candidate
    .primary_gap
    .total_cmp(&current_best.primary_gap)
    .is_ne()
  {
    return candidate.primary_gap < current_best.primary_gap;
  }
  if candidate_overlaps && candidate.overlap.total_cmp(&current_best.overlap).is_ne() {
    return candidate.overlap > current_best.overlap;
  }
  if !candidate_overlaps
    && candidate
      .perpendicular_gap
      .total_cmp(&current_best.perpendicular_gap)
      .is_ne()
  {
    return candidate.perpendicular_gap < current_best.perpendicular_gap;
  }
  if candidate
    .center_gap
    .total_cmp(&current_best.center_gap)
    .is_ne()
  {
    return candidate.center_gap < current_best.center_gap;
  }
  candidate.pane_id.0 < current_best.pane_id.0
}

fn find_directional_pane(
  panes: &[PaneBounds],
  active_id: PaneId,
  direction: PaneFocusDirection,
) -> Option<PaneId> {
  let active = panes.iter().find(|pane| pane.id == active_id).copied()?;
  let mut best: Option<DirectionalPaneCandidate> = None;

  for pane in panes {
    if pane.id == active_id {
      continue;
    }
    let Some(candidate) = candidate_in_direction(active, *pane, direction) else {
      continue;
    };
    if best.is_none_or(|current_best| is_better_directional_candidate(candidate, current_best)) {
      best = Some(candidate);
    }
  }

  best.map(|candidate| candidate.pane_id)
}

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

#[derive(Debug, Clone)]
struct HiddenPanesState {
  visible_pane_ids: Vec<PaneId>,
}

impl HiddenPanesState {
  fn new(active_pane_id: PaneId) -> Self {
    Self {
      visible_pane_ids: vec![active_pane_id],
    }
  }

  fn contains(&self, pane_id: PaneId) -> bool {
    self.visible_pane_ids.contains(&pane_id)
  }

  fn insert_visible(&mut self, pane_id: PaneId) {
    if !self.contains(pane_id) {
      self.visible_pane_ids.push(pane_id);
    }
  }

  fn remove(&mut self, pane_id: PaneId) {
    self.visible_pane_ids.retain(|id| *id != pane_id);
  }
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

  fn matching_visible_panes(&self, visible_pane_ids: &[PaneId]) -> usize {
    match self {
      SplitPane::Terminal { id, .. } => usize::from(visible_pane_ids.contains(id)),
      SplitPane::Split { first, second, .. } => {
        first.matching_visible_panes(visible_pane_ids)
          + second.matching_visible_panes(visible_pane_ids)
      }
    }
  }

  fn visible_subtree_with_path<'a>(
    &'a self,
    visible_pane_ids: &[PaneId],
    path: &mut Vec<bool>,
  ) -> Option<&'a SplitPane> {
    if visible_pane_ids.is_empty() {
      return Some(self);
    }

    if self.matching_visible_panes(visible_pane_ids) != visible_pane_ids.len() {
      return None;
    }

    if let SplitPane::Split { first, second, .. } = self {
      if first.matching_visible_panes(visible_pane_ids) == visible_pane_ids.len() {
        path.push(false);
        return first.visible_subtree_with_path(visible_pane_ids, path);
      }

      if second.matching_visible_panes(visible_pane_ids) == visible_pane_ids.len() {
        path.push(true);
        return second.visible_subtree_with_path(visible_pane_ids, path);
      }
    }

    Some(self)
  }

  fn collect_pane_bounds(
    &self,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    bounds: &mut Vec<PaneBounds>,
  ) {
    match self {
      SplitPane::Terminal { id, .. } => bounds.push(PaneBounds {
        id: *id,
        left,
        top,
        right: left + width,
        bottom: top + height,
      }),
      SplitPane::Split {
        direction,
        first,
        second,
        ratio,
      } => match direction {
        SplitDirection::Horizontal => {
          let first_height = height * ratio;
          first.collect_pane_bounds(left, top, width, first_height, bounds);
          second.collect_pane_bounds(
            left,
            top + first_height,
            width,
            height - first_height,
            bounds,
          );
        }
        SplitDirection::Vertical => {
          let first_width = width * ratio;
          first.collect_pane_bounds(left, top, first_width, height, bounds);
          second.collect_pane_bounds(left + first_width, top, width - first_width, height, bounds);
        }
      },
    }
  }

  fn pane_bounds(&self) -> Vec<PaneBounds> {
    let mut bounds = Vec::new();
    self.collect_pane_bounds(0.0, 0.0, 1.0, 1.0, &mut bounds);
    bounds
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
    focused_pane_id: Option<PaneId>,
    has_splits: bool,
    path: Vec<bool>,
    window: &mut Window,
    cx: &mut Context<MainWindow>,
  ) -> AnyElement {
    match self {
      SplitPane::Terminal { id, terminal } => {
        let right_click_context_menu = cx
          .try_global::<config::Config>()
          .map(|c| c.terminal.right_click_context_menu)
          .unwrap_or(false);

        let effective_active_pane_id = focused_pane_id.or(active_pane_id);
        let is_active = effective_active_pane_id.is_some_and(|active| active == *id);
        let is_inactive = effective_active_pane_id.is_some_and(|active| active != *id);
        terminal.update(cx, |tv, _| {
          tv.is_inactive_pane = is_inactive;
        });

        let colors = cx.global::<SettingsStore>().theme().colors().clone();
        let pane_id = ElementId::from(("split-pane-terminal", id.0));

        let is_hovered = terminal.read(cx).is_hovered;

        let border_color = if has_splits && is_active {
          colors.border_selected
        } else if has_splits && is_hovered {
          colors.border_focused
        } else if has_splits {
          colors.border_variant
        } else {
          colors.border_transparent
        };

        let base = if has_splits {
          div()
            .id(pane_id)
            .size_full()
            .border_2()
            .border_color(border_color)
            .child(terminal.clone())
        } else {
          div().id(pane_id).size_full().child(terminal.clone())
        };

        if right_click_context_menu {
          let terminal = terminal.clone();
          let main_window = cx.entity().clone();
          base
            .context_menu(move |menu, window, cx| {
              if terminal.read(cx).mouse_reporting_enabled(cx) {
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
          .map(|config| config.pane.get_divider_width())
          .unwrap_or(6.0);
        let colors = cx.global::<SettingsStore>().theme().colors().clone();

        let mut first_path = path.clone();
        first_path.push(false);
        let mut second_path = path.clone();
        second_path.push(true);

        let first_element = first.render(
          active_pane_id,
          focused_pane_id,
          has_splits,
          first_path,
          window,
          cx,
        );
        let second_element = second.render(
          active_pane_id,
          focused_pane_id,
          has_splits,
          second_path,
          window,
          cx,
        );

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
  hidden_panes: Option<HiddenPanesState>,
}

impl SplitContainer {
  pub fn new(terminal: Entity<TerminalView>) -> Self {
    Self {
      root: SplitPane::new_terminal(PaneId(0), terminal),
      active_pane_id: Some(PaneId(0)),
      next_pane_id: 1,
      hidden_panes: None,
    }
  }

  pub(crate) fn from_restored_root(
    root: SplitPane,
    active_pane_id: Option<PaneId>,
    next_pane_id: usize,
  ) -> Self {
    Self {
      root,
      active_pane_id,
      next_pane_id,
      hidden_panes: None,
    }
  }

  fn visible_root_with_path(&self) -> (&SplitPane, Vec<bool>) {
    if let Some(hidden_panes) = &self.hidden_panes {
      let mut path = Vec::new();
      if let Some(visible_root) = self
        .root
        .visible_subtree_with_path(&hidden_panes.visible_pane_ids, &mut path)
      {
        return (visible_root, path);
      }
    }

    (&self.root, vec![])
  }

  fn visible_terminals(&self) -> Vec<(PaneId, Entity<TerminalView>)> {
    let (visible_root, _) = self.visible_root_with_path();
    visible_root.all_terminals()
  }

  fn ensure_active_pane_visible(&mut self) {
    let visible_terminals = self.visible_terminals();
    if self
      .active_pane_id
      .is_none_or(|id| visible_terminals.iter().all(|(pane_id, _)| *pane_id != id))
    {
      self.active_pane_id = visible_terminals.first().map(|(pane_id, _)| *pane_id);
    }
  }

  fn sync_hidden_panes_after_tree_change(&mut self) {
    let total_panes = self.root.count_panes();
    let fallback_active_pane_id = self
      .active_pane_id
      .filter(|id| self.root.find_terminal(*id).is_some())
      .or_else(|| {
        self
          .root
          .all_terminals()
          .first()
          .map(|(pane_id, _)| *pane_id)
      });

    let clear_hidden_panes = if let Some(hidden_panes) = self.hidden_panes.as_mut() {
      hidden_panes
        .visible_pane_ids
        .retain(|id| self.root.find_terminal(*id).is_some());

      if hidden_panes.visible_pane_ids.is_empty() {
        if let Some(fallback_active_pane_id) = fallback_active_pane_id {
          hidden_panes.visible_pane_ids.push(fallback_active_pane_id);
        }
      }

      hidden_panes.visible_pane_ids.len() >= total_panes
    } else {
      false
    };

    if clear_hidden_panes {
      self.hidden_panes = None;
    }

    self.ensure_active_pane_visible();
  }

  pub fn has_hidden_panes(&self) -> bool {
    self.hidden_panes.is_some()
  }

  pub fn can_hide_other_panes(&self) -> bool {
    self.hidden_panes.is_none() && self.root.count_panes() > 1
  }

  pub fn hide_other_panes(&mut self) -> bool {
    if !self.can_hide_other_panes() {
      return false;
    }

    self.ensure_active_pane_visible();

    let Some(active_pane_id) = self.active_pane_id else {
      return false;
    };

    self.hidden_panes = Some(HiddenPanesState::new(active_pane_id));
    self.sync_hidden_panes_after_tree_change();
    true
  }

  pub fn restore_hidden_panes(&mut self) -> bool {
    let restored = self.hidden_panes.take().is_some();
    self.ensure_active_pane_visible();
    restored
  }

  pub fn toggle_hidden_panes(&mut self) -> bool {
    if self.has_hidden_panes() {
      self.restore_hidden_panes()
    } else {
      self.hide_other_panes()
    }
  }

  #[cfg(test)]
  pub(crate) fn visible_pane_count(&self) -> usize {
    let (visible_root, _) = self.visible_root_with_path();
    visible_root.count_panes()
  }

  pub fn split_active_pane(
    &mut self,
    direction: SplitDirection,
    new_terminal: Entity<TerminalView>,
  ) -> Option<PaneId> {
    let active_id = self.active_pane_id?;
    let new_id = PaneId(self.next_pane_id);

    if self.root.split(active_id, direction, new_terminal, new_id) {
      self.next_pane_id += 1;

      if let Some(hidden_panes) = self.hidden_panes.as_mut() {
        if hidden_panes.contains(active_id) {
          hidden_panes.insert_visible(new_id);
        }
      }

      self.active_pane_id = Some(new_id);
      self.sync_hidden_panes_after_tree_change();
      return Some(new_id);
    }

    None
  }

  pub fn close_active_pane(&mut self) -> bool {
    let Some(active_id) = self.active_pane_id else {
      return false;
    };

    // Don't close if it's the last pane.
    if self.root.count_panes() <= 1 {
      return false;
    }

    let result = self.root.close_pane(active_id);
    if let Some(new_root) = result.replacement {
      self.root = new_root;
    }

    if result.closed {
      if let Some(hidden_panes) = self.hidden_panes.as_mut() {
        hidden_panes.remove(active_id);
      }

      if self.active_pane_id == Some(active_id) && self.root.find_terminal(active_id).is_none() {
        self.active_pane_id = self.root.all_terminals().first().map(|(id, _)| *id);
      }

      self.sync_hidden_panes_after_tree_change();
      return true;
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
        if let Some(hidden_panes) = self.hidden_panes.as_mut() {
          hidden_panes.remove(pane_id);
        }

        // Update active pane if we closed the active one (or if it no longer exists)
        if self.active_pane_id == Some(pane_id)
          || self
            .active_pane_id
            .is_some_and(|active| self.root.find_terminal(active).is_none())
        {
          self.active_pane_id = self.root.all_terminals().first().map(|(id, _)| *id);
        }

        self.sync_hidden_panes_after_tree_change();
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
    let terminals = self.visible_terminals();
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
    let terminals = self.visible_terminals();
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

  pub fn focus_pane_in_direction(
    &mut self,
    direction: PaneFocusDirection,
  ) -> Option<Entity<TerminalView>> {
    let active_id = self.active_pane_id?;
    let (visible_root, _) = self.visible_root_with_path();
    let bounds = visible_root.pane_bounds();
    let target_id = find_directional_pane(&bounds, active_id, direction)?;
    self.active_pane_id = Some(target_id);
    self.root.find_terminal(target_id)
  }

  pub fn all_terminals(&self) -> Vec<(PaneId, Entity<TerminalView>)> {
    self.root.all_terminals()
  }

  pub fn update_ratio(&mut self, path: &[bool], new_ratio: f32) {
    self.root.update_ratio(path, new_ratio);
  }

  pub fn render(&self, window: &mut Window, cx: &mut Context<MainWindow>) -> AnyElement {
    let (visible_root, visible_path) = self.visible_root_with_path();
    let has_splits = matches!(visible_root, SplitPane::Split { .. });
    let focused_pane_id = self
      .visible_terminals()
      .into_iter()
      .find_map(|(id, terminal)| {
        terminal
          .read(cx)
          .focus_handle
          .is_focused(window)
          .then_some(id)
      });
    visible_root.render(
      self.active_pane_id,
      focused_pane_id,
      has_splits,
      visible_path,
      window,
      cx,
    )
  }
}

#[cfg(test)]
mod tests {
  use super::{PaneBounds, PaneFocusDirection, PaneId, find_directional_pane};

  #[test]
  fn directional_focus_prefers_adjacent_overlapping_panes() {
    let panes = vec![
      PaneBounds {
        id: PaneId(1),
        left: 0.0,
        top: 0.0,
        right: 0.5,
        bottom: 0.5,
      },
      PaneBounds {
        id: PaneId(2),
        left: 0.5,
        top: 0.0,
        right: 1.0,
        bottom: 0.5,
      },
      PaneBounds {
        id: PaneId(3),
        left: 0.0,
        top: 0.5,
        right: 1.0,
        bottom: 1.0,
      },
    ];

    assert_eq!(
      find_directional_pane(&panes, PaneId(1), PaneFocusDirection::Right),
      Some(PaneId(2))
    );
    assert_eq!(
      find_directional_pane(&panes, PaneId(1), PaneFocusDirection::Down),
      Some(PaneId(3))
    );
    assert_eq!(
      find_directional_pane(&panes, PaneId(2), PaneFocusDirection::Left),
      Some(PaneId(1))
    );
  }

  #[test]
  fn directional_focus_returns_none_when_no_pane_exists_in_that_direction() {
    let panes = vec![
      PaneBounds {
        id: PaneId(1),
        left: 0.0,
        top: 0.0,
        right: 0.5,
        bottom: 1.0,
      },
      PaneBounds {
        id: PaneId(2),
        left: 0.5,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
      },
    ];

    assert_eq!(
      find_directional_pane(&panes, PaneId(1), PaneFocusDirection::Up),
      None
    );
    assert_eq!(
      find_directional_pane(&panes, PaneId(2), PaneFocusDirection::Right),
      None
    );
  }

  #[test]
  fn directional_focus_returns_none_for_unknown_active_pane() {
    let panes = vec![PaneBounds {
      id: PaneId(1),
      left: 0.0,
      top: 0.0,
      right: 1.0,
      bottom: 1.0,
    }];
    assert_eq!(
      find_directional_pane(&panes, PaneId(999), PaneFocusDirection::Right),
      None
    );
  }

  #[test]
  fn directional_focus_returns_none_for_single_pane() {
    let panes = vec![PaneBounds {
      id: PaneId(1),
      left: 0.0,
      top: 0.0,
      right: 1.0,
      bottom: 1.0,
    }];
    for direction in [
      PaneFocusDirection::Left,
      PaneFocusDirection::Right,
      PaneFocusDirection::Up,
      PaneFocusDirection::Down,
    ] {
      assert_eq!(find_directional_pane(&panes, PaneId(1), direction), None);
    }
  }

  #[test]
  fn directional_focus_picks_closest_of_multiple_candidates_in_a_row() {
    // Three panes stacked horizontally: 1 | 2 | 3
    // From 1, moving right, should pick 2 (not 3).
    let panes = vec![
      PaneBounds {
        id: PaneId(1),
        left: 0.0,
        top: 0.0,
        right: 0.33,
        bottom: 1.0,
      },
      PaneBounds {
        id: PaneId(2),
        left: 0.33,
        top: 0.0,
        right: 0.66,
        bottom: 1.0,
      },
      PaneBounds {
        id: PaneId(3),
        left: 0.66,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
      },
    ];
    assert_eq!(
      find_directional_pane(&panes, PaneId(1), PaneFocusDirection::Right),
      Some(PaneId(2))
    );
    assert_eq!(
      find_directional_pane(&panes, PaneId(3), PaneFocusDirection::Left),
      Some(PaneId(2))
    );
  }

  #[test]
  fn directional_focus_prefers_best_vertical_overlap_tiebreak() {
    // Active (1) on the left. Two right-side candidates (2, 3) with different
    // vertical overlap with (1). The pane with more overlap wins.
    // 1: full-height left
    // 2: top-right quarter (small overlap)
    // 3: full-height right (larger overlap)
    let panes = vec![
      PaneBounds {
        id: PaneId(1),
        left: 0.0,
        top: 0.0,
        right: 0.3,
        bottom: 1.0,
      },
      PaneBounds {
        id: PaneId(2),
        left: 0.3,
        top: 0.0,
        right: 0.6,
        bottom: 0.2,
      },
      PaneBounds {
        id: PaneId(3),
        left: 0.3,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
      },
    ];
    let picked = find_directional_pane(&panes, PaneId(1), PaneFocusDirection::Right);
    // Either candidate is geometrically to the right, but the one with full
    // vertical overlap should win.
    assert_eq!(picked, Some(PaneId(3)));
  }

  #[test]
  fn directional_focus_ignores_panes_on_the_same_side() {
    // A pane that sits fully to the LEFT of active should never be reachable
    // via PaneFocusDirection::Right.
    let panes = vec![
      PaneBounds {
        id: PaneId(1),
        left: 0.4,
        top: 0.0,
        right: 0.7,
        bottom: 1.0,
      },
      PaneBounds {
        id: PaneId(2),
        left: 0.0,
        top: 0.0,
        right: 0.4,
        bottom: 1.0,
      },
    ];
    assert_eq!(
      find_directional_pane(&panes, PaneId(1), PaneFocusDirection::Right),
      None
    );
    assert_eq!(
      find_directional_pane(&panes, PaneId(1), PaneFocusDirection::Left),
      Some(PaneId(2))
    );
  }
}
