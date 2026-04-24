use std::path::PathBuf;
use std::sync::atomic::Ordering;

use gpui::{Context, Window, px};
use kazeterm_ui_tree::node::{PaneNode, TabNode, UITree};
use serde::Deserialize;

use super::main_window::MainWindow;
use super::main_window_tab_item::TabItem;
use super::main_window_tab_management::get_working_directory_pathbuf;
use crate::components::search_bar::SearchBarState;
use crate::components::split_pane::{PaneId, SplitContainer, SplitDirection, SplitPane};
use crate::reconciler::UITreeStore;

// ── UITree-based workspace persistence ──

impl UITreeStore {
  /// Path to the workspace file on disk.
  pub fn workspace_file_path() -> PathBuf {
    config::Config::get_config_path().join("workspace.json")
  }

  /// Save the current tree to disk.
  pub fn save_workspace(&self) {
    let path = Self::workspace_file_path();
    if let Some(parent) = path.parent() {
      if let Err(e) = std::fs::create_dir_all(parent) {
        tracing::error!("Failed to create workspace directory: {e}");
        return;
      }
    }
    match self.to_json() {
      Ok(json) => {
        if let Err(e) = std::fs::write(&path, json) {
          tracing::error!("Failed to write workspace state: {e}");
        } else {
          tracing::info!("Saved workspace state to {}", path.display());
        }
      }
      Err(e) => {
        tracing::error!("Failed to serialize workspace state: {e}");
      }
    }
  }

  /// Load a UITree from the workspace file. Returns `None` if no file
  /// or the tree has no windows with tabs.
  pub fn load_workspace() -> Option<UITree> {
    let path = Self::workspace_file_path();
    if !path.exists() {
      // Try migrating from legacy format
      return migrate_legacy_workspace();
    }
    match std::fs::read_to_string(&path) {
      Ok(content) => match serde_json::from_str::<UITree>(&content) {
        Ok(tree) if tree.windows.first().is_some_and(|w| !w.tabs.is_empty()) => Some(tree),
        Ok(_) => None,
        Err(e) => {
          tracing::error!("Failed to parse workspace state: {e}");
          // Try legacy format as fallback
          migrate_legacy_workspace()
        }
      },
      Err(e) => {
        tracing::error!("Failed to read workspace state file: {e}");
        None
      }
    }
  }

  /// Delete the workspace file from disk.
  pub fn delete_workspace() {
    let path = Self::workspace_file_path();
    if path.exists() {
      let _ = std::fs::remove_file(&path);
    }
    // Also clean up legacy file if it exists
    let legacy = config::Config::get_config_path().join("workspace_legacy.json");
    if legacy.exists() {
      let _ = std::fs::remove_file(&legacy);
    }
  }
}

// ── Restore from UITree ──

impl MainWindow {
  /// Restore the entire workspace from a UITree snapshot.
  pub fn restore_from_ui_tree(
    &mut self,
    tree: &UITree,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let was_reconciling = self.reconciling_ui_tree;
    self.reconciling_ui_tree = true;

    let win = match tree.windows.first() {
      Some(w) => w,
      None => {
        self.reconciling_ui_tree = was_reconciling;
        return;
      }
    };

    self.tab_bar_visible = win.tab_bar.visible;
    self.search_visible = win.search.visible;

    for tab_node in &win.tabs {
      self.restore_tab_from_node(tab_node, window, cx);
    }

    if let Some(active_ix) = win.active_tab {
      if active_ix < self.items.len() {
        self.set_active_tab(active_ix, window, cx);
      }
    } else if !self.items.is_empty() {
      self.set_active_tab(0, window, cx);
    }

    self.reconciling_ui_tree = was_reconciling;
  }

  pub(crate) fn load_ui_tree_from_path(
    &mut self,
    path: &std::path::Path,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Result<(), String> {
    let content = std::fs::read_to_string(path).map_err(|err| {
      format!(
        "Failed to read UI tree JSON from '{}': {err}",
        path.display()
      )
    })?;
    self.load_ui_tree_from_str(&content, window, cx)?;
    tracing::info!("Loaded UI tree JSON from {}", path.display());
    Ok(())
  }

  pub(crate) fn load_ui_tree_from_str(
    &mut self,
    json: &str,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Result<(), String> {
    let tree: UITree =
      serde_json::from_str(json).map_err(|err| format!("Failed to parse UI tree JSON: {err}"))?;
    if tree.windows.first().is_none_or(|win| win.tabs.is_empty()) {
      return Err("UI tree must contain at least one window with tabs".to_string());
    }

    self.replace_with_ui_tree(&tree, window, cx);
    Ok(())
  }

  fn replace_with_ui_tree(&mut self, tree: &UITree, window: &mut Window, cx: &mut Context<Self>) {
    let was_reconciling = self.reconciling_ui_tree;
    self.reconciling_ui_tree = true;

    self.items.clear();
    self.active_tab_ix = None;
    self.search_visible = false;
    self.tab_bar_visible = true;
    self.scroll_tabs_to_end = false;
    self.scroll_to_active_tab = false;
    self.tab_switcher_visible = false;
    self.tab_switcher = None;
    self.tab_switcher_selection = 0;

    self.restore_from_ui_tree(tree, window, cx);

    self.reconciling_ui_tree = was_reconciling;
    self.ui_tree = UITreeStore::from_tree(tree.clone());
    self.sync_ui_tree(cx);
  }

  pub(crate) fn restore_tab_from_node(
    &mut self,
    tab: &TabNode,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let (split_container, first_sub) =
      match Self::build_split_container_from_tab_node(tab, &self.tab_index, window, cx) {
        Ok(result) => result,
        Err(err) => {
          tracing::error!("Failed to restore tab: {err}");
          self.show_shell_error_dialog(err, window, cx);
          return;
        }
      };

    let index = self.tab_index.fetch_add(1, Ordering::SeqCst);
    let shell_name = Self::shell_name_for_tab(tab);
    let title = tab
      .custom_title
      .clone()
      .unwrap_or_else(|| shell_name.clone());

    let item = TabItem {
      ui_tree_id: tab.id.clone(),
      index,
      title,
      custom_title: tab.custom_title.clone(),
      shell_path: tab.shell.path.clone(),
      shell_args: tab.shell.args.clone(),
      _shell_name: shell_name,
      split_container,
      _subscription: first_sub,
      search_bar_state: Self::search_bar_state_from_node(&tab.search),
    };
    self.items.push(item);

    let new_ix = self.items.len() - 1;
    self.set_active_tab(new_ix, window, cx);
  }

  pub(crate) fn rebuild_tab_from_node(
    &mut self,
    tab_id: &str,
    tab: &TabNode,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Result<(), String> {
    let (split_container, first_sub) =
      Self::build_split_container_from_tab_node(tab, &self.tab_index, window, cx)?;
    let Some(ix) = self.items.iter().position(|item| item.ui_tree_id == tab_id) else {
      return Err(format!("Tab '{tab_id}' not found"));
    };

    let was_active = self.active_tab_ix == Some(ix);
    let shell_name = Self::shell_name_for_tab(tab);
    let title = tab
      .custom_title
      .clone()
      .unwrap_or_else(|| self.items[ix].title.clone());

    {
      let item = &mut self.items[ix];
      item.ui_tree_id = tab.id.clone();
      item.title = title;
      item.custom_title = tab.custom_title.clone();
      item.shell_path = tab.shell.path.clone();
      item.shell_args = tab.shell.args.clone();
      item._shell_name = shell_name;
      item.split_container = split_container;
      item._subscription = first_sub;
      item.search_bar_state = Self::search_bar_state_from_node(&tab.search);
    }

    if was_active {
      self.active_tab_ix = None;
      self.set_active_tab(ix, window, cx);
    } else {
      cx.notify();
    }

    Ok(())
  }

  fn shell_name_for_tab(tab: &TabNode) -> String {
    std::path::Path::new(&tab.shell.path)
      .file_stem()
      .and_then(|n| n.to_str())
      .unwrap_or(&tab.shell.path)
      .to_lowercase()
  }

  fn search_bar_state_from_node(search: &kazeterm_ui_tree::node::SearchState) -> SearchBarState {
    SearchBarState {
      query: search.query.clone().into(),
      match_case: search.match_case,
      match_whole: search.match_whole,
      use_regex: search.use_regex,
      visible: search.visible,
      position: gpui::Point::new(px(search.position.x), px(search.position.y)),
    }
  }

  fn build_split_container_from_tab_node(
    tab: &TabNode,
    tab_index_counter: &std::sync::atomic::AtomicUsize,
    window: &mut Window,
    cx: &mut Context<MainWindow>,
  ) -> Result<(SplitContainer, gpui::Subscription), String> {
    let mut next_pane_id: usize = 0;
    let (root_pane, subscriptions) = Self::build_split_pane_from_node(
      &tab.pane_tree,
      &tab.shell.path,
      &tab.shell.args,
      &mut next_pane_id,
      tab_index_counter,
      window,
      cx,
    )?;
    let active_pane_id =
      Self::active_pane_id_from_node(&tab.pane_tree).or_else(|| Self::first_pane_id(&root_pane));
    let split_container =
      SplitContainer::from_restored_root(root_pane, active_pane_id, next_pane_id);
    let first_sub = Self::take_primary_subscription(subscriptions)?;
    Ok((split_container, first_sub))
  }

  fn take_primary_subscription(
    subscriptions: Vec<gpui::Subscription>,
  ) -> Result<gpui::Subscription, String> {
    let mut sub_iter = subscriptions.into_iter();
    let first_sub = sub_iter
      .next()
      .ok_or_else(|| "expected at least one terminal subscription".to_string())?;
    for sub in sub_iter {
      std::mem::forget(sub);
    }
    Ok(first_sub)
  }

  fn build_split_pane_from_node(
    pane: &PaneNode,
    tab_shell: &str,
    tab_shell_args: &[String],
    next_pane_id: &mut usize,
    tab_index_counter: &std::sync::atomic::AtomicUsize,
    window: &mut Window,
    cx: &mut Context<MainWindow>,
  ) -> Result<(SplitPane, Vec<gpui::Subscription>), String> {
    match pane {
      PaneNode::Terminal {
        id,
        working_directory,
        ..
      } => {
        let index = tab_index_counter.fetch_add(1, Ordering::SeqCst);
        let wd = get_working_directory_pathbuf(working_directory.clone());
        let terminal = crate::components::terminal_window::new_terminal_window_with_shell(
          window,
          index,
          tab_shell,
          tab_shell_args.to_vec(),
          wd,
          cx,
        )?;
        let sub = cx.subscribe_in(&terminal, window, Self::subscribe_terminal_view_event);
        let pane_id = Self::parse_pane_id(id).unwrap_or(PaneId(*next_pane_id));
        *next_pane_id = (*next_pane_id).max(pane_id.0 + 1);
        Ok((SplitPane::new_terminal(pane_id, terminal), vec![sub]))
      }
      PaneNode::Split {
        direction,
        first,
        second,
        ratio,
      } => {
        let (first_pane, mut subs) = Self::build_split_pane_from_node(
          first,
          tab_shell,
          tab_shell_args,
          next_pane_id,
          tab_index_counter,
          window,
          cx,
        )?;
        let (second_pane, subs2) = Self::build_split_pane_from_node(
          second,
          tab_shell,
          tab_shell_args,
          next_pane_id,
          tab_index_counter,
          window,
          cx,
        )?;
        subs.extend(subs2);
        let dir = match direction {
          kazeterm_ui_tree::node::SplitDirection::Horizontal => SplitDirection::Horizontal,
          kazeterm_ui_tree::node::SplitDirection::Vertical => SplitDirection::Vertical,
        };
        let pane = SplitPane::Split {
          direction: dir,
          first: Box::new(first_pane),
          second: Box::new(second_pane),
          ratio: *ratio,
        };
        Ok((pane, subs))
      }
    }
  }

  fn first_pane_id(pane: &SplitPane) -> Option<PaneId> {
    match pane {
      SplitPane::Terminal { id, .. } => Some(*id),
      SplitPane::Split { first, .. } => Self::first_pane_id(first),
    }
  }

  fn active_pane_id_from_node(pane: &PaneNode) -> Option<PaneId> {
    pane.focused_pane_id().and_then(Self::parse_pane_id)
  }

  fn parse_pane_id(pane_id: &str) -> Option<PaneId> {
    pane_id
      .strip_prefix("pane-")
      .and_then(|id| id.parse::<usize>().ok())
      .map(PaneId)
  }
}

// ── Legacy migration ──

/// Legacy workspace state (v1). Used only for migration from old format.
#[derive(Debug, Clone, Deserialize)]
struct LegacyWorkspaceState {
  #[serde(default)]
  tabs: Vec<LegacyTabState>,
  active_tab_index: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyTabState {
  shell_path: String,
  #[serde(default)]
  shell_args: Vec<String>,
  custom_title: Option<String>,
  pane_tree: LegacyPaneTreeState,
}

#[derive(Debug, Clone, Deserialize)]
enum LegacyPaneTreeState {
  Terminal {
    working_directory: Option<String>,
  },
  Split {
    direction: LegacySplitDirectionState,
    first: Box<LegacyPaneTreeState>,
    second: Box<LegacyPaneTreeState>,
    ratio: f32,
  },
}

#[derive(Debug, Clone, Copy, Deserialize)]
enum LegacySplitDirectionState {
  Horizontal,
  Vertical,
}

/// Attempt to load and migrate a legacy `workspace.json` to UITree format.
fn migrate_legacy_workspace() -> Option<UITree> {
  let path = config::Config::get_config_path().join("workspace.json");
  if !path.exists() {
    return None;
  }

  let content = std::fs::read_to_string(&path).ok()?;
  let legacy: LegacyWorkspaceState = match serde_json::from_str(&content) {
    Ok(state) => state,
    Err(_) => return None,
  };

  if legacy.tabs.is_empty() {
    return None;
  }

  let tree = convert_legacy_to_ui_tree(&legacy);

  // Save the migrated tree
  if let Ok(json) = serde_json::to_string_pretty(&tree) {
    let _ = std::fs::write(&path, json);
    tracing::info!("Migrated legacy workspace.json to UITree format");
  }

  Some(tree)
}

fn convert_legacy_to_ui_tree(legacy: &LegacyWorkspaceState) -> UITree {
  let mut tree = UITree::new();
  let win_id = tree.next_id("win");

  let mut tabs = Vec::with_capacity(legacy.tabs.len());
  for tab in &legacy.tabs {
    let tab_id = tree.next_id("tab");
    let pane_tree = convert_legacy_pane_tree(&tab.pane_tree, &mut tree);
    tabs.push(TabNode {
      id: tab_id,
      custom_title: tab.custom_title.clone(),
      shell: kazeterm_ui_tree::node::ShellConfig {
        path: tab.shell_path.clone(),
        args: tab.shell_args.clone(),
        profile: None,
      },
      pane_tree,
      search: kazeterm_ui_tree::node::SearchState::default(),
    });
  }

  let window = kazeterm_ui_tree::node::WindowNode {
    id: win_id,
    size: kazeterm_ui_tree::node::Size::default(),
    maximized: false,
    active_tab: legacy.active_tab_index,
    tab_bar: kazeterm_ui_tree::node::TabBarState::default(),
    search: kazeterm_ui_tree::node::SearchState::default(),
    tabs,
    overlay: None,
    key_debug: kazeterm_ui_tree::node::KeyDebugState::default(),
  };
  tree.windows.push(window);
  tree
}

fn convert_legacy_pane_tree(pane: &LegacyPaneTreeState, tree: &mut UITree) -> PaneNode {
  match pane {
    LegacyPaneTreeState::Terminal { working_directory } => PaneNode::Terminal {
      id: tree.next_id("pane"),
      working_directory: working_directory.clone(),
      title: String::new(),
      focused: false,
    },
    LegacyPaneTreeState::Split {
      direction,
      first,
      second,
      ratio,
    } => PaneNode::Split {
      direction: match direction {
        LegacySplitDirectionState::Horizontal => kazeterm_ui_tree::node::SplitDirection::Horizontal,
        LegacySplitDirectionState::Vertical => kazeterm_ui_tree::node::SplitDirection::Vertical,
      },
      ratio: *ratio,
      first: Box::new(convert_legacy_pane_tree(first, tree)),
      second: Box::new(convert_legacy_pane_tree(second, tree)),
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_migrate_legacy_workspace() {
    let legacy = LegacyWorkspaceState {
      tabs: vec![
        LegacyTabState {
          shell_path: "pwsh.exe".into(),
          shell_args: vec![],
          custom_title: Some("Build".into()),
          pane_tree: LegacyPaneTreeState::Split {
            direction: LegacySplitDirectionState::Vertical,
            first: Box::new(LegacyPaneTreeState::Terminal {
              working_directory: Some("D:\\Workspace".into()),
            }),
            second: Box::new(LegacyPaneTreeState::Terminal {
              working_directory: None,
            }),
            ratio: 0.5,
          },
        },
        LegacyTabState {
          shell_path: "bash".into(),
          shell_args: vec!["-l".into()],
          custom_title: None,
          pane_tree: LegacyPaneTreeState::Terminal {
            working_directory: Some("/home/user".into()),
          },
        },
      ],
      active_tab_index: Some(0),
    };

    let tree = convert_legacy_to_ui_tree(&legacy);

    assert_eq!(tree.windows.len(), 1);
    let win = &tree.windows[0];
    assert_eq!(win.tabs.len(), 2);
    assert_eq!(win.active_tab, Some(0));

    // First tab: split pane
    let tab0 = &win.tabs[0];
    assert_eq!(tab0.custom_title, Some("Build".into()));
    assert_eq!(tab0.shell.path, "pwsh.exe");
    match &tab0.pane_tree {
      PaneNode::Split {
        direction, ratio, ..
      } => {
        assert_eq!(*direction, kazeterm_ui_tree::node::SplitDirection::Vertical);
        assert_eq!(*ratio, 0.5);
      }
      _ => panic!("expected split pane"),
    }

    // Second tab: single terminal
    let tab1 = &win.tabs[1];
    assert_eq!(tab1.shell.path, "bash");
    assert_eq!(tab1.shell.args, vec!["-l"]);
    match &tab1.pane_tree {
      PaneNode::Terminal {
        working_directory, ..
      } => {
        assert_eq!(working_directory.as_deref(), Some("/home/user"));
      }
      _ => panic!("expected terminal pane"),
    }

    // Verify JSON roundtrip
    let json = serde_json::to_string_pretty(&tree).unwrap();
    let restored: UITree = serde_json::from_str(&json).unwrap();
    assert_eq!(tree, restored);
  }
}
