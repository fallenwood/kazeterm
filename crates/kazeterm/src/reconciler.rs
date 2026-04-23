//! Bridges the data-driven `UITree` to the live GPUI `MainWindow`.
//!
//! The reconciler owns a `UITree` and provides two main capabilities:
//! 1. **apply_action**: mutates the tree and returns diffs
//! 2. **reconcile**: translates diffs into `MainWindow` method calls
//!
//! This module does NOT replace the existing `MainWindow` methods — it wraps
//! them, providing an alternative entry point for mutations that flow through
//! the serializable tree first.

use kazeterm_ui_tree::action::UIAction;
use kazeterm_ui_tree::diff::{self, Reconciler, TreeDiff};
use kazeterm_ui_tree::node::*;

use gpui::{Context, Window};

use crate::components::MainWindow;
use crate::components::SplitDirection;

/// Holds the canonical `UITree` alongside a `MainWindow` entity.
/// All UI mutations should flow through this struct.
pub struct UITreeStore {
  tree: UITree,
  /// ID of the window managed by this store (single-window for now).
  window_id: Option<String>,
}

impl UITreeStore {
  pub fn new() -> Self {
    Self {
      tree: UITree::new(),
      window_id: None,
    }
  }

  /// Get a reference to the current tree (for serialization/snapshot).
  pub fn tree(&self) -> &UITree {
    &self.tree
  }

  /// Load the tree from a JSON string, replacing the current state.
  pub fn load_json(&mut self, json: &str) -> Result<(), serde_json::Error> {
    self.tree = serde_json::from_str(json)?;
    Ok(())
  }

  /// Dump the tree as a JSON string.
  pub fn to_json(&self) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&self.tree)
  }

  /// Dump the tree as a `serde_json::Value`.
  pub fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::to_value(&self.tree)
  }

  /// The ID of the window this store manages.
  pub fn window_id(&self) -> Option<&str> {
    self.window_id.as_deref()
  }

  /// Apply an action to the tree and return the diffs produced.
  /// Does NOT apply diffs to GPUI — call `reconcile()` separately.
  pub fn apply_action(&mut self, action: UIAction) -> Result<Vec<TreeDiff>, anyhow::Error> {
    let old_tree = self.tree.clone();
    self.tree.apply(action)?;
    Ok(diff::diff_trees(&old_tree, &self.tree))
  }

  /// Initialize the tree from the current `MainWindow` state.
  /// This captures the live GPUI state into the tree so they're in sync.
  pub fn capture_from_main_window(
    &mut self,
    main_window: &MainWindow,
    cx: &mut Context<MainWindow>,
  ) {
    let win_id = self.tree.next_id("win");
    self.window_id = Some(win_id.clone());

    let config = cx.global::<::config::Config>();
    let config_window_width = config.window.width;
    let config_window_height = config.window.height;
    let config_tab_vertical = config.tab.vertical;
    let config_key_debug = config.window.key_debug_mode;

    let mut tabs = Vec::with_capacity(main_window.items.len());
    for item in &main_window.items {
      let pane_tree = capture_split_pane(&item.split_container.root, cx);
      let tab_id = self.tree.next_id("tab");
      tabs.push(TabNode {
        id: tab_id,
        custom_title: item.custom_title.clone(),
        shell: ShellConfig {
          path: item.shell_path.clone(),
          args: item.shell_args.clone(),
          profile: None,
        },
        pane_tree,
        search: SearchState {
          visible: item.search_bar_state.visible,
          query: item.search_bar_state.query.to_string(),
          match_case: item.search_bar_state.match_case,
          match_whole: item.search_bar_state.match_whole,
          use_regex: item.search_bar_state.use_regex,
          position: Position::default(),
        },
      });
    }

    let window_node = WindowNode {
      id: win_id,
      size: Size {
        width: config_window_width,
        height: config_window_height,
      },
      maximized: false,
      active_tab: main_window.active_tab_ix,
      tab_bar: TabBarState {
        visible: main_window.tab_bar_visible,
        vertical: config_tab_vertical,
      },
      search: SearchState {
        visible: main_window.search_visible,
        ..SearchState::default()
      },
      tabs,
      overlay: capture_overlay(main_window),
      key_debug: KeyDebugState {
        enabled: config_key_debug,
      },
    };

    self.tree.windows = vec![window_node];
  }

  /// Apply tree diffs to the live `MainWindow`.
  /// This is the core reconciliation step.
  pub fn reconcile(
    diffs: &[TreeDiff],
    main_window: &mut MainWindow,
    window: &mut Window,
    cx: &mut Context<MainWindow>,
  ) {
    for d in diffs {
      match d {
        TreeDiff::TabAdded {
          tab,
          ..
        } => {
          main_window.insert_new_tab_with_profile(
            tab.shell.profile.as_deref(),
            find_pane_working_directory(&tab.pane_tree).map(|s| s.to_string()),
            window,
            cx,
          );
          // If tab has a custom title, apply it
          if let Some(title) = &tab.custom_title {
            if let Some(item) = main_window.items.last_mut() {
              item.custom_title = Some(title.clone());
            }
          }
        }

        TreeDiff::TabRemoved {
          tab_id, ..
        } => {
          if let Some(item) = main_window.items.iter().find(|i| {
            // Match by tab position in items list
            // (tree tab IDs don't directly map to MainWindow indices yet)
            i.custom_title.as_deref() == Some(tab_id.as_str())
          }) {
            let index = item.index;
            main_window.remove_tab_by(index, window, cx);
          }
        }

        TreeDiff::ActiveTabChanged {
          active_tab, ..
        } => {
          if let Some(ix) = active_tab {
            if *ix < main_window.items.len() {
              main_window.set_active_tab(*ix, window, cx);
            }
          }
        }

        TreeDiff::TabRenamed {
          custom_title, ..
        } => {
          if let Some(ix) = main_window.active_tab_ix {
            if let Some(item) = main_window.items.get_mut(ix) {
              item.custom_title = custom_title.clone();
            }
          }
          cx.notify();
        }

        TreeDiff::PaneTreeChanged { .. } => {
          // Full pane tree rebuild — complex, will be implemented
          // when we do full integration. For now, incremental ops
          // (split/close/focus) handle common cases.
          cx.notify();
        }

        TreeDiff::PaneFocusChanged { .. } => {
          main_window.focus_active_terminal(window, cx);
        }

        TreeDiff::SearchVisibilityChanged {
          visible, ..
        } => {
          if *visible != main_window.search_visible {
            main_window.toggle_search(window, cx);
          }
        }

        TreeDiff::SearchQueryChanged { .. } => {
          // Search query is managed by SearchBar entity directly
        }

        TreeDiff::SearchFlagsChanged { .. } => {
          // Search flags are managed by SearchBar entity directly
        }

        TreeDiff::TabBarVisibilityChanged {
          visible, ..
        } => {
          if *visible != main_window.tab_bar_visible {
            main_window.toggle_tab_bar(cx);
          }
        }

        TreeDiff::OverlayChanged {
          overlay, ..
        } => {
          reconcile_overlay(overlay, main_window, window, cx);
        }

        TreeDiff::KeyDebugChanged { .. } => {
          cx.notify();
        }

        // These diffs update tree metadata but don't require GPUI changes
        TreeDiff::WindowAdded { .. }
        | TreeDiff::WindowRemoved { .. }
        | TreeDiff::WindowResized { .. }
        | TreeDiff::WindowMaximizedChanged { .. }
        | TreeDiff::TabMoved { .. }
        | TreeDiff::TabBarVerticalChanged { .. }
        | TreeDiff::PaneTitleChanged { .. }
        | TreeDiff::PaneWorkingDirectoryChanged { .. } => {}
      }
    }
  }

  /// Convenience: apply an action and immediately reconcile.
  pub fn dispatch(
    &mut self,
    action: UIAction,
    main_window: &mut MainWindow,
    window: &mut Window,
    cx: &mut Context<MainWindow>,
  ) -> Result<(), anyhow::Error> {
    let diffs = self.apply_action(action)?;
    Self::reconcile(&diffs, main_window, window, cx);
    Ok(())
  }
}

impl Reconciler for UITreeStore {
  fn apply_diffs(&mut self, _diffs: &[TreeDiff]) {
    // Standalone reconciler trait impl — used for non-GPUI contexts (testing).
    // GPUI reconciliation requires Window/Context, so use `reconcile()` instead.
  }
}

// ── Capture helpers ──

fn capture_split_pane(
  pane: &crate::components::SplitPane,
  cx: &mut Context<MainWindow>,
) -> PaneNode {
  match pane {
    crate::components::SplitPane::Terminal { id, terminal } => {
      let terminal_entity = terminal.read(cx).terminal().clone();
      let title = terminal_entity.read(cx).title_text.clone();
      let cwd = terminal_entity.update(cx, |t, _cx| t.current_working_directory());
      PaneNode::Terminal {
        id: format!("pane-{}", id.0),
        working_directory: cwd,
        title,
        focused: false, // Will be set by caller based on active_pane_id
      }
    }
    crate::components::SplitPane::Split {
      direction,
      first,
      second,
      ratio,
    } => PaneNode::Split {
      direction: match direction {
        SplitDirection::Horizontal => kazeterm_ui_tree::node::SplitDirection::Horizontal,
        SplitDirection::Vertical => kazeterm_ui_tree::node::SplitDirection::Vertical,
      },
      ratio: *ratio,
      first: Box::new(capture_split_pane(first, cx)),
      second: Box::new(capture_split_pane(second, cx)),
    },
  }
}

fn capture_overlay(main_window: &MainWindow) -> Option<OverlayNode> {
  if main_window.about_dialog.is_some() {
    return Some(OverlayNode::AboutDialog);
  }
  if main_window.close_confirm_dialog.is_some() {
    return Some(OverlayNode::CloseConfirm);
  }
  if main_window.rename_dialog.is_some() {
    // We don't have easy access to the dialog's tab_id from here,
    // so we capture a minimal representation.
    return Some(OverlayNode::RenameDialog {
      tab_id: String::new(),
      current_title: String::new(),
    });
  }
  if main_window.import_alacritty_dialog.is_some() {
    return Some(OverlayNode::ImportAlacritty {
      path: String::new(),
      error: None,
    });
  }
  if main_window.shell_error_dialog.is_some() {
    return Some(OverlayNode::ShellError {
      message: String::new(),
    });
  }
  None
}

fn reconcile_overlay(
  overlay: &Option<OverlayNode>,
  main_window: &mut MainWindow,
  window: &mut Window,
  cx: &mut Context<MainWindow>,
) {
  match overlay {
    None => {
      // Dismiss all dialogs
      main_window.rename_dialog = None;
      main_window._rename_dialog_subscription = None;
      main_window.close_confirm_dialog = None;
      main_window._close_confirm_subscription = None;
      main_window.about_dialog = None;
      main_window._about_dialog_subscription = None;
      main_window.import_alacritty_dialog = None;
      main_window._import_alacritty_subscription = None;
      main_window.shell_error_dialog = None;
      main_window._shell_error_subscription = None;
      main_window.refocus_active_terminal(window, cx);
      cx.notify();
    }
    Some(OverlayNode::AboutDialog) => {
      main_window.show_about_dialog(window, cx);
    }
    Some(OverlayNode::CloseConfirm) => {
      main_window.show_close_confirm_dialog(window, cx);
    }
    Some(OverlayNode::RenameDialog { .. }) => {
      if let Some(ix) = main_window.active_tab_ix {
        if let Some(item) = main_window.items.get(ix) {
          main_window.show_rename_dialog(item.index, window, cx);
        }
      }
    }
    Some(OverlayNode::ImportAlacritty { .. }) => {
      main_window.show_import_alacritty_dialog(window, cx);
    }
    Some(OverlayNode::ShellError { message }) => {
      main_window.show_shell_error_dialog(message.clone(), window, cx);
    }
    Some(OverlayNode::TabSwitcher { .. }) => {
      // Tab switcher is managed by keyboard state, not the reconciler
    }
  }
}

// ── PaneNode helper ──

/// Find the first working directory in a pane tree.
fn find_pane_working_directory(pane: &PaneNode) -> Option<&str> {
  match pane {
    PaneNode::Terminal {
      working_directory, ..
    } => working_directory.as_deref(),
    PaneNode::Split { first, second, .. } => find_pane_working_directory(first)
      .or_else(|| find_pane_working_directory(second)),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_store_json_roundtrip() {
    let mut store = UITreeStore::new();
    store
      .apply_action(UIAction::AddWindow {
        width: Some(1024.0),
        height: Some(768.0),
      })
      .unwrap();

    let win_id = store.tree().windows[0].id.clone();
    store
      .apply_action(UIAction::AddTab {
        window_id: win_id.clone(),
        shell_path: "bash".into(),
        shell_args: vec![],
        profile: None,
        working_directory: Some("/home".into()),
      })
      .unwrap();

    let json = store.to_json().unwrap();
    let mut store2 = UITreeStore::new();
    store2.load_json(&json).unwrap();
    assert_eq!(store.tree(), store2.tree());
  }

  #[test]
  fn test_apply_action_returns_diffs() {
    let mut store = UITreeStore::new();
    let diffs = store
      .apply_action(UIAction::AddWindow {
        width: None,
        height: None,
      })
      .unwrap();
    assert!(!diffs.is_empty());
    assert!(diffs
      .iter()
      .any(|d| matches!(d, TreeDiff::WindowAdded { .. })));

    let win_id = store.tree().windows[0].id.clone();
    let diffs = store
      .apply_action(UIAction::AddTab {
        window_id: win_id.clone(),
        shell_path: "bash".into(),
        shell_args: vec![],
        profile: None,
        working_directory: None,
      })
      .unwrap();
    assert!(diffs
      .iter()
      .any(|d| matches!(d, TreeDiff::TabAdded { .. })));
  }

  #[test]
  fn test_snapshot_and_restore() {
    let mut store = UITreeStore::new();
    store
      .apply_action(UIAction::AddWindow {
        width: Some(800.0),
        height: Some(600.0),
      })
      .unwrap();
    let win_id = store.tree().windows[0].id.clone();

    // Add two tabs
    for shell in &["bash", "zsh"] {
      store
        .apply_action(UIAction::AddTab {
          window_id: win_id.clone(),
          shell_path: shell.to_string(),
          shell_args: vec![],
          profile: None,
          working_directory: None,
        })
        .unwrap();
    }

    // Toggle search
    store
      .apply_action(UIAction::ToggleSearch {
        window_id: win_id.clone(),
      })
      .unwrap();

    // Snapshot
    let json_val = store.to_json_value().unwrap();
    assert_eq!(json_val["windows"][0]["tabs"].as_array().unwrap().len(), 2);
    assert_eq!(json_val["windows"][0]["search"]["visible"], true);

    // Restore into new store
    let json_str = store.to_json().unwrap();
    let mut store2 = UITreeStore::new();
    store2.load_json(&json_str).unwrap();
    assert_eq!(store.tree(), store2.tree());
  }
}
