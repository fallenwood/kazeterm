use gpui::{AppContext, Context, Entity, Window};
use kazeterm_ui_tree::{action::UIAction, node::TabGroupColor};

use super::{
  main_window::MainWindow,
  tab_group_dialogs::{GroupDeleteDialog, GroupDeleteEvent, GroupRenameDialog, GroupRenameEvent},
};

impl MainWindow {
  fn group_action(
    &mut self,
    action: impl FnOnce(String) -> UIAction,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if let Some(window_id) = self.ensure_ui_tree_window_id(cx) {
      self.dispatch_default_ui_action(action(window_id), "update tab group", window, cx);
    }
  }

  pub(crate) fn create_tab_group(
    &mut self,
    index: usize,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if let Some(tab_id) = self
      .items
      .iter()
      .find(|item| item.index == index)
      .map(|item| item.ui_tree_id.clone())
    {
      self.group_action(
        |window_id| UIAction::CreateTabGroup { window_id, tab_id },
        window,
        cx,
      );
    }
  }

  pub(crate) fn move_tab_to_group(
    &mut self,
    index: usize,
    group_id: String,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if let Some(tab_id) = self
      .items
      .iter()
      .find(|item| item.index == index)
      .map(|item| item.ui_tree_id.clone())
    {
      self.group_action(
        |window_id| UIAction::MoveTabToGroup {
          window_id,
          tab_id,
          group_id,
        },
        window,
        cx,
      );
    }
  }

  pub(crate) fn ungroup_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(tab_id) = self
      .items
      .iter()
      .find(|item| item.index == index)
      .map(|item| item.ui_tree_id.clone())
    {
      self.group_action(
        |window_id| UIAction::UngroupTab { window_id, tab_id },
        window,
        cx,
      );
    }
  }

  pub(crate) fn move_group(
    &mut self,
    group_id: String,
    new_index: usize,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.group_action(
      |window_id| UIAction::MoveTabGroup {
        window_id,
        group_id,
        new_index,
      },
      window,
      cx,
    );
  }

  pub(crate) fn set_group_color(
    &mut self,
    group_id: String,
    color: TabGroupColor,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.group_action(
      |window_id| UIAction::SetTabGroupColor {
        window_id,
        group_id,
        color,
      },
      window,
      cx,
    );
  }

  pub(crate) fn show_group_rename(
    &mut self,
    group_id: String,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let Some(group) = self.groups.iter().find(|g| g.id == group_id) else {
      return;
    };
    let dialog = cx.new(|cx| {
      GroupRenameDialog::new(group_id, group.name.clone().unwrap_or_default(), window, cx)
    });
    self._group_rename_subscription = Some(cx.subscribe_in(&dialog, window, Self::on_group_rename));
    self.group_rename_dialog = Some(dialog);
    cx.notify();
  }

  fn on_group_rename(
    &mut self,
    _dialog: &Entity<GroupRenameDialog>,
    event: &GroupRenameEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.group_rename_dialog = None;
    self._group_rename_subscription = None;
    if let GroupRenameEvent::Save { group_id, name } = event {
      self.group_action(
        |window_id| UIAction::RenameTabGroup {
          window_id,
          group_id: group_id.clone(),
          name: name.clone(),
        },
        window,
        cx,
      );
    }
    self.refocus_active_terminal(window, cx);
    cx.notify();
  }

  pub(crate) fn show_group_delete(
    &mut self,
    group_id: String,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if self.group_delete_dialog.is_some() || !self.groups.iter().any(|g| g.id == group_id) {
      return;
    }
    let member_ids = self.group_member_ids(&group_id);
    let count = member_ids.len();
    if count == 0 {
      return;
    }
    let dialog = cx.new(|cx| GroupDeleteDialog::new(count, window, cx));
    self._group_delete_subscription =
      Some(
        cx.subscribe_in(&dialog, window, move |this, _, event, window, cx| {
          this.on_group_delete(&group_id, &member_ids, *event, window, cx);
        }),
      );
    self.group_delete_dialog = Some(dialog);
    cx.notify();
  }

  fn group_member_ids(&self, group_id: &str) -> Vec<String> {
    self
      .items
      .iter()
      .filter(|item| item.group_id.as_deref() == Some(group_id))
      .map(|item| item.ui_tree_id.clone())
      .collect()
  }

  fn on_group_delete(
    &mut self,
    group_id: &str,
    confirmed_member_ids: &[String],
    event: GroupDeleteEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.group_delete_dialog = None;
    self._group_delete_subscription = None;
    if !matches!(event, GroupDeleteEvent::Cancel) {
      if self.group_member_ids(group_id) != confirmed_member_ids {
        tracing::warn!("Tab group membership changed during confirmation; deletion cancelled");
        self.refocus_active_terminal(window, cx);
        cx.notify();
        return;
      }
      let Some(window_id) = self.ensure_ui_tree_window_id(cx) else {
        return;
      };
      let close_tabs = matches!(event, GroupDeleteEvent::CloseTabs);
      let delete = UIAction::DeleteTabGroup {
        window_id: window_id.clone(),
        group_id: group_id.to_string(),
        close_tabs,
      };
      let count = self
        .items
        .iter()
        .filter(|item| item.group_id.as_deref() == Some(group_id))
        .count();
      let action = if close_tabs
        && count == self.items.len()
        && !cx.global::<::config::Config>().tab.close_on_last
      {
        UIAction::Batch {
          actions: vec![
            delete,
            Self::build_add_tab_ui_action(window_id, None, None, cx),
          ],
        }
      } else {
        delete
      };
      self.dispatch_default_ui_action(action, "delete tab group", window, cx);
    }
    if !self.items.is_empty() {
      self.refocus_active_terminal(window, cx);
    }
    cx.notify();
  }

  pub(crate) fn drop_tab_into_group(
    &mut self,
    dragged: &super::dragged_tab::DraggedTab,
    group_id: String,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let Some(end) = self
      .items
      .iter()
      .rposition(|item| item.group_id.as_deref() == Some(&group_id))
    else {
      return;
    };
    let local = dragged.source_entity_id == cx.entity_id();
    let index = if local { dragged.tab_index } else { usize::MAX };
    if !self.drop_tab_at(dragged, Some(end + 1), window, cx) {
      return;
    }
    let index = if local {
      index
    } else {
      self
        .active_tab_ix
        .and_then(|ix| self.items.get(ix))
        .map_or(usize::MAX, |item| item.index)
    };
    self.move_tab_to_group(index, group_id, window, cx);
  }
}

#[cfg(test)]
mod tests {
  use gpui::TestAppContext;

  use super::{GroupDeleteEvent, MainWindow};
  use crate::components::{
    main_window_e2e_tests::{install_fake_factory, test_lock},
    terminal_window::clear_terminal_session_factory_for_testing,
  };

  #[gpui::test]
  fn closing_only_group_closes_its_window_when_configured(cx: &mut TestAppContext) {
    let _guard = test_lock();
    crate::test_support::init_test_app(cx);
    install_fake_factory();
    let sibling = cx.add_window(|window, cx| MainWindow::new(window, cx));
    let closing = cx.add_window(|window, cx| MainWindow::new(window, cx));
    closing
      .update(cx, |root: &mut MainWindow, window, cx| {
        root.create_tab_group(root.items[0].index, window, cx);
        let group_id = root.groups[0].id.clone();
        root.show_group_delete(group_id.clone(), window, cx);
        let confirmed = root.group_member_ids(&group_id);
        root.on_group_delete(
          &group_id,
          &confirmed,
          GroupDeleteEvent::CloseTabs,
          window,
          cx,
        );
      })
      .unwrap();
    cx.run_until_parked();
    assert!(closing.root(cx).is_err());
    assert_eq!(
      sibling
        .root(cx)
        .unwrap()
        .read_with(cx, |root, _| root.items.len()),
      1
    );
    clear_terminal_session_factory_for_testing();
  }

  #[gpui::test]
  fn closing_only_group_respects_keep_last_tab_setting(cx: &mut TestAppContext) {
    let _guard = test_lock();
    crate::test_support::init_test_app(cx);
    cx.update(|cx| {
      let mut config = cx.global::<::config::Config>().clone();
      config.tab.close_on_last = false;
      cx.set_global(config);
    });
    let calls = install_fake_factory();
    let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
    window
      .update(cx, |root: &mut MainWindow, window, cx| {
        let first = root.items[0].index;
        root.create_tab_group(first, window, cx);
        let group_id = root.groups[0].id.clone();
        root.show_group_delete(group_id.clone(), window, cx);
        let confirmed = root.group_member_ids(&group_id);
        root.on_group_delete(
          &group_id,
          &confirmed,
          GroupDeleteEvent::CloseTabs,
          window,
          cx,
        );
        assert!(root.groups.is_empty());
        assert_eq!(root.items.len(), 1);
        assert!(root.items[0].group_id.is_none());
        assert_eq!(root.ui_tree.tree().windows[0].tabs.len(), 1);
      })
      .unwrap();
    assert_eq!(calls.lock().unwrap().programs.len(), 2);
    clear_terminal_session_factory_for_testing();
  }

  #[gpui::test]
  fn deleting_a_group_ungroups_or_closes_exactly_its_tabs(cx: &mut TestAppContext) {
    let _guard = test_lock();
    crate::test_support::init_test_app(cx);
    let calls = install_fake_factory();
    let window = cx.add_window(|window, cx| MainWindow::new(window, cx));
    window
      .update(cx, |root: &mut MainWindow, window, cx| {
        root.insert_new_tab(window, cx);
        root.insert_new_tab(window, cx);
        let first = root.items[0].index;
        let second = root.items[1].index;
        root.create_tab_group(first, window, cx);
        let group_id = root.groups[0].id.clone();
        root.move_tab_to_group(second, group_id.clone(), window, cx);
        root.show_group_delete(group_id.clone(), window, cx);
        let confirmed = root.group_member_ids(&group_id);
        root.on_group_delete(&group_id, &confirmed, GroupDeleteEvent::Ungroup, window, cx);
        assert_eq!(root.items.len(), 3);
        assert!(root.groups.is_empty());
        assert!(root.items.iter().all(|item| item.group_id.is_none()));
        root.create_tab_group(first, window, cx);
        let group_id = root.groups[0].id.clone();
        root.move_tab_to_group(second, group_id.clone(), window, cx);
        root.show_group_delete(group_id.clone(), window, cx);
        let confirmed = root.group_member_ids(&group_id);
        root.on_group_delete(
          &group_id,
          &confirmed,
          GroupDeleteEvent::CloseTabs,
          window,
          cx,
        );
        assert!(root.groups.is_empty());
        assert_eq!(root.items.len(), 1);
        assert_eq!(root.ui_tree.tree().windows[0].tabs.len(), 1);
      })
      .unwrap();
    assert_eq!(calls.lock().unwrap().programs.len(), 3);
    clear_terminal_session_factory_for_testing();
  }
}
