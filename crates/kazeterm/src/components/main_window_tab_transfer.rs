use std::sync::atomic::Ordering;

use gpui::{AppContext, Bounds, Context, MouseUpEvent, Window, point, px};

use super::dragged_tab::DraggedTab;
use super::main_window::{MainWindow, TabItem};
use super::split_pane::{PaneId, SplitContainer, SplitDirection};

struct TakenTab {
  item: TabItem,
  close_source: bool,
}

impl MainWindow {
  pub(crate) fn dragged_tab(
    &self,
    tab_ix: usize,
    window: &Window,
    cx: &Context<Self>,
  ) -> Option<DraggedTab> {
    let item = self.items.get(tab_ix)?;
    Some(DraggedTab::new(
      item.index,
      item.display_title().to_string(),
      item.shell_path.clone(),
      cx.entity().downgrade(),
      cx.entity_id(),
      window.window_handle(),
    ))
  }

  pub(crate) fn start_tab_drag(&mut self, dragged: DraggedTab) {
    self.active_tab_drag = Some(dragged);
  }

  pub(crate) fn finish_tab_drag(
    &mut self,
    event: &MouseUpEvent,
    window: &Window,
    cx: &mut Context<Self>,
  ) {
    let Some(dragged) = self.active_tab_drag.take() else {
      return;
    };

    let source_bounds = window.bounds();
    let screen_position = point(
      source_bounds.origin.x + event.position.x,
      source_bounds.origin.y + event.position.y,
    );
    let detached_bounds = Bounds {
      origin: point(screen_position.x - px(80.0), screen_position.y - px(16.0)),
      size: source_bounds.size,
    };

    cx.defer(move |cx| {
      if dragged.claim()
        && !crate::window_manager::drop_tab_on_existing_window(&dragged, screen_position, cx)
      {
        crate::window_manager::open_detached_tab_window(dragged, detached_bounds, cx);
      }
    });
  }

  pub(crate) fn drop_tab_at(
    &mut self,
    dragged: &DraggedTab,
    target_ix: Option<usize>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if !dragged.claim() {
      return;
    }

    self.active_tab_drag = None;
    if dragged.source_entity_id == cx.entity_id() {
      self.reorder_local_tab(dragged.tab_index, target_ix, window, cx);
      return;
    }

    if !self.receive_claimed_tab(dragged, target_ix, window, cx) {
      tracing::warn!("Dragged tab no longer exists in its source window");
    }
  }

  pub(crate) fn receive_claimed_tab(
    &mut self,
    dragged: &DraggedTab,
    target_ix: Option<usize>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> bool {
    let Some(taken) = self.take_external_tab(dragged, cx) else {
      return false;
    };

    self.insert_transferred_tab(taken.item, target_ix, window, cx);
    if taken.close_source {
      Self::close_empty_source(dragged.clone(), cx);
    }
    true
  }

  pub(crate) fn drop_tab_into_split(
    &mut self,
    dragged: &DraggedTab,
    target_pane_id: PaneId,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let Some(target_tab_ix) = self.active_tab_ix else {
      return;
    };
    let Some(target_item) = self.items.get(target_tab_ix) else {
      return;
    };
    if target_item
      .split_container
      .root
      .find_terminal(target_pane_id)
      .is_none()
    {
      return;
    }
    if dragged.source_entity_id == cx.entity_id() && target_item.index == dragged.tab_index {
      let _ = dragged.claim();
      self.active_tab_drag = None;
      return;
    }
    if !dragged.claim() {
      return;
    }

    self.active_tab_drag = None;
    let is_external = dragged.source_entity_id != cx.entity_id();
    let taken = if !is_external {
      self
        .detach_tab_for_transfer(dragged.tab_index, window, cx)
        .map(|(item, _)| TakenTab {
          item,
          close_source: false,
        })
    } else {
      self.take_external_tab(dragged, cx)
    };
    let Some(taken) = taken else {
      tracing::warn!("Dragged tab no longer exists in its source window");
      return;
    };

    if is_external {
      self.prepare_transferred_terminals(&taken.item.split_container, window, cx);
    }
    let Some(target_tab_ix) = self.active_tab_ix else {
      self.insert_transferred_tab(taken.item, None, window, cx);
      return;
    };
    let dragged_split_container = taken.item.split_container;
    let new_active_terminal = self.items.get_mut(target_tab_ix).and_then(|item| {
      item.split_container.set_active_pane(target_pane_id);
      item
        .split_container
        .split_active_pane_with_container(SplitDirection::Vertical, dragged_split_container)?;
      item.split_container.get_active_terminal()
    });

    let Some(new_active_terminal) = new_active_terminal else {
      tracing::error!("Failed to merge dragged tab into the target split");
      return;
    };

    self.resubscribe_tab_terminals(target_tab_ix, window, cx);
    let search_visible = self.search_visible && self.settings_page.is_none();
    let terminal_for_search = new_active_terminal.clone();
    self.search_bar.update(cx, |search_bar, cx| {
      search_bar.set_terminal_view(terminal_for_search);
      if search_visible {
        search_bar.focus(window, cx);
      }
    });
    if !search_visible {
      self.focus_terminal(window, &new_active_terminal, cx);
    }

    self.sync_ui_tree(cx);
    cx.notify();
    if taken.close_source {
      Self::close_empty_source(dragged.clone(), cx);
    }
  }

  #[cfg(test)]
  pub(crate) fn move_tab_into_split(
    &mut self,
    from_ix: usize,
    target_pane_id: PaneId,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let Some(dragged) = self.dragged_tab(from_ix, window, cx) else {
      return;
    };
    self.drop_tab_into_split(&dragged, target_pane_id, window, cx);
  }

  fn reorder_local_tab(
    &mut self,
    tab_index: usize,
    target_ix: Option<usize>,
    _window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let Some(from_ix) = self.items.iter().position(|item| item.index == tab_index) else {
      return;
    };
    let to_ix = target_ix.unwrap_or(self.items.len());
    let item = self.items.remove(from_ix);
    let to_ix = to_ix.min(self.items.len());
    self.items.insert(to_ix, item);

    if let Some(active) = self.active_tab_ix {
      self.active_tab_ix = Some(if active == from_ix {
        to_ix
      } else if from_ix < active && active <= to_ix {
        active - 1
      } else if to_ix <= active && active < from_ix {
        active + 1
      } else {
        active
      });
    }

    self.sync_ui_tree(cx);
    cx.notify();
  }

  fn take_external_tab(
    &mut self,
    dragged: &DraggedTab,
    cx: &mut Context<Self>,
  ) -> Option<TakenTab> {
    let source = dragged.source.clone();
    let tab_index = dragged.tab_index;
    let result = cx
      .update_window(dragged.source_window, move |_root, source_window, cx| {
        source
          .update(cx, |source, cx| {
            source.detach_tab_for_transfer(tab_index, source_window, cx)
          })
          .ok()
          .flatten()
      })
      .ok()
      .flatten()?;

    Some(TakenTab {
      item: result.0,
      close_source: result.1,
    })
  }

  fn detach_tab_for_transfer(
    &mut self,
    tab_index: usize,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Option<(TabItem, bool)> {
    let pos = self.items.iter().position(|item| item.index == tab_index)?;
    let was_active = self.active_tab_ix == Some(pos);
    if was_active {
      self.items[pos].search_bar_state =
        self.search_bar.read(cx).save_state(self.search_visible, cx);
    }

    let item = self.items.remove(pos);
    self.active_tab_drag = None;
    if self.items.is_empty() {
      self.active_tab_ix = None;
      self.search_visible = false;
    } else if was_active {
      self.active_tab_ix = None;
      self.set_active_tab_direct(pos.min(self.items.len() - 1), window, cx);
    } else if let Some(active_ix) = self.active_tab_ix
      && active_ix > pos
    {
      self.active_tab_ix = Some(active_ix - 1);
    }

    self.sync_ui_tree(cx);
    cx.notify();
    Some((item, self.items.is_empty()))
  }

  fn insert_transferred_tab(
    &mut self,
    mut item: TabItem,
    target_ix: Option<usize>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.sync_ui_tree(cx);
    self.prepare_transferred_terminals(&item.split_container, window, cx);
    item.index = self.tab_index.fetch_add(1, Ordering::SeqCst);
    item.ui_tree_id = self.ui_tree.alloc_id("tab");
    item.terminal_subscriptions =
      Self::subscribe_to_split_container(&item.split_container, window, cx);

    let target_ix = target_ix.unwrap_or(self.items.len()).min(self.items.len());
    self.items.insert(target_ix, item);
    self.set_active_tab_direct(target_ix, window, cx);
    self.scroll_to_active_tab = true;
    self.sync_ui_tree(cx);
    cx.notify();
  }

  fn prepare_transferred_terminals(
    &mut self,
    split_container: &SplitContainer,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if let Some(max_existing_index) = self
      .items
      .iter()
      .flat_map(|item| {
        std::iter::once(item.index).chain(
          item
            .split_container
            .all_terminals()
            .into_iter()
            .map(|(_, terminal)| terminal.read(cx).index),
        )
      })
      .max()
    {
      self
        .tab_index
        .fetch_max(max_existing_index.saturating_add(1), Ordering::SeqCst);
    }

    for (_, terminal) in split_container.all_terminals() {
      let index = self.tab_index.fetch_add(1, Ordering::SeqCst);
      terminal.update(cx, |terminal_view, cx| {
        terminal_view.index = index;
        terminal_view.rebind_window(window, cx);
      });
    }
  }

  fn close_empty_source(dragged: DraggedTab, cx: &mut Context<Self>) {
    cx.defer(move |cx| {
      let should_close = dragged
        .source
        .upgrade()
        .is_some_and(|source| source.read(cx).items.is_empty());
      if should_close {
        let _ = cx.update_window(dragged.source_window, |_root, window, _cx| {
          window.remove_window();
        });
      }
    });
  }
}
