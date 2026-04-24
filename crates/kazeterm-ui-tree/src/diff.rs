use crate::node::*;

/// Describes a single change between two tree states.
#[derive(Debug, Clone, PartialEq)]
pub enum TreeDiff {
  // ── Window level ──
  WindowAdded {
    window: WindowNode,
  },
  WindowRemoved {
    window_id: String,
  },
  WindowResized {
    window_id: String,
    size: Size,
  },
  WindowMaximizedChanged {
    window_id: String,
    maximized: bool,
  },

  // ── Tab level ──
  TabAdded {
    window_id: String,
    tab: TabNode,
    index: usize,
  },
  TabRemoved {
    window_id: String,
    tab_id: String,
  },
  TabMoved {
    window_id: String,
    tab_id: String,
    old_index: usize,
    new_index: usize,
  },
  ActiveTabChanged {
    window_id: String,
    active_tab: Option<usize>,
  },
  TabRenamed {
    window_id: String,
    tab_id: String,
    custom_title: Option<String>,
  },

  // ── Pane level ──
  PaneTreeChanged {
    window_id: String,
    tab_id: String,
    pane_tree: PaneNode,
  },
  PaneFocusChanged {
    window_id: String,
    tab_id: String,
    pane_id: Option<String>,
  },
  PaneTitleChanged {
    window_id: String,
    tab_id: String,
    pane_id: String,
    title: String,
  },
  PaneWorkingDirectoryChanged {
    window_id: String,
    tab_id: String,
    pane_id: String,
    working_directory: Option<String>,
  },

  // ── Search ──
  SearchVisibilityChanged {
    window_id: String,
    visible: bool,
  },
  SearchQueryChanged {
    window_id: String,
    query: String,
  },
  SearchFlagsChanged {
    window_id: String,
    match_case: bool,
    match_whole: bool,
    use_regex: bool,
  },

  // ── Tab bar ──
  TabBarVisibilityChanged {
    window_id: String,
    visible: bool,
  },
  TabBarVerticalChanged {
    window_id: String,
    vertical: bool,
  },

  // ── Overlay ──
  OverlayChanged {
    window_id: String,
    overlay: Option<OverlayNode>,
  },

  // ── Key debug ──
  KeyDebugChanged {
    window_id: String,
    enabled: bool,
  },
}

/// Trait for consuming tree diffs. Implement this to bridge tree changes
/// to a concrete UI framework (e.g., GPUI).
pub trait Reconciler {
  fn apply_diffs(&mut self, diffs: &[TreeDiff]);
}

/// Compute the set of diffs between two tree states.
pub fn diff_trees(old: &UITree, new: &UITree) -> Vec<TreeDiff> {
  let mut diffs = Vec::new();

  // Find removed windows
  for old_win in &old.windows {
    if !new.windows.iter().any(|w| w.id == old_win.id) {
      diffs.push(TreeDiff::WindowRemoved {
        window_id: old_win.id.clone(),
      });
    }
  }

  // Find added or changed windows
  for new_win in &new.windows {
    match old.windows.iter().find(|w| w.id == new_win.id) {
      None => {
        diffs.push(TreeDiff::WindowAdded {
          window: new_win.clone(),
        });
      }
      Some(old_win) => {
        diff_windows(old_win, new_win, &mut diffs);
      }
    }
  }

  diffs
}

fn diff_windows(old: &WindowNode, new: &WindowNode, diffs: &mut Vec<TreeDiff>) {
  let win_id = &new.id;

  if old.size != new.size {
    diffs.push(TreeDiff::WindowResized {
      window_id: win_id.clone(),
      size: new.size,
    });
  }

  if old.maximized != new.maximized {
    diffs.push(TreeDiff::WindowMaximizedChanged {
      window_id: win_id.clone(),
      maximized: new.maximized,
    });
  }

  if old.active_tab != new.active_tab {
    diffs.push(TreeDiff::ActiveTabChanged {
      window_id: win_id.clone(),
      active_tab: new.active_tab,
    });
  }

  // Tab bar
  if old.tab_bar.visible != new.tab_bar.visible {
    diffs.push(TreeDiff::TabBarVisibilityChanged {
      window_id: win_id.clone(),
      visible: new.tab_bar.visible,
    });
  }
  if old.tab_bar.vertical != new.tab_bar.vertical {
    diffs.push(TreeDiff::TabBarVerticalChanged {
      window_id: win_id.clone(),
      vertical: new.tab_bar.vertical,
    });
  }

  // Search
  if old.search.visible != new.search.visible {
    diffs.push(TreeDiff::SearchVisibilityChanged {
      window_id: win_id.clone(),
      visible: new.search.visible,
    });
  }
  if old.search.query != new.search.query {
    diffs.push(TreeDiff::SearchQueryChanged {
      window_id: win_id.clone(),
      query: new.search.query.clone(),
    });
  }
  if old.search.match_case != new.search.match_case
    || old.search.match_whole != new.search.match_whole
    || old.search.use_regex != new.search.use_regex
  {
    diffs.push(TreeDiff::SearchFlagsChanged {
      window_id: win_id.clone(),
      match_case: new.search.match_case,
      match_whole: new.search.match_whole,
      use_regex: new.search.use_regex,
    });
  }

  // Overlay
  if old.overlay != new.overlay {
    diffs.push(TreeDiff::OverlayChanged {
      window_id: win_id.clone(),
      overlay: new.overlay.clone(),
    });
  }

  // Key debug
  if old.key_debug.enabled != new.key_debug.enabled {
    diffs.push(TreeDiff::KeyDebugChanged {
      window_id: win_id.clone(),
      enabled: new.key_debug.enabled,
    });
  }

  // Tabs
  diff_tabs(win_id, &old.tabs, &new.tabs, diffs);
}

fn diff_tabs(win_id: &str, old_tabs: &[TabNode], new_tabs: &[TabNode], diffs: &mut Vec<TreeDiff>) {
  // Removed tabs
  for old_tab in old_tabs {
    if !new_tabs.iter().any(|t| t.id == old_tab.id) {
      diffs.push(TreeDiff::TabRemoved {
        window_id: win_id.to_string(),
        tab_id: old_tab.id.clone(),
      });
    }
  }

  // Added or changed tabs
  for (new_ix, new_tab) in new_tabs.iter().enumerate() {
    match old_tabs
      .iter()
      .enumerate()
      .find(|(_, t)| t.id == new_tab.id)
    {
      None => {
        diffs.push(TreeDiff::TabAdded {
          window_id: win_id.to_string(),
          tab: new_tab.clone(),
          index: new_ix,
        });
      }
      Some((old_ix, old_tab)) => {
        if old_ix != new_ix {
          diffs.push(TreeDiff::TabMoved {
            window_id: win_id.to_string(),
            tab_id: new_tab.id.clone(),
            old_index: old_ix,
            new_index: new_ix,
          });
        }

        if old_tab.custom_title != new_tab.custom_title {
          diffs.push(TreeDiff::TabRenamed {
            window_id: win_id.to_string(),
            tab_id: new_tab.id.clone(),
            custom_title: new_tab.custom_title.clone(),
          });
        }

        // Pane tree structural diff
        if old_tab.pane_tree != new_tab.pane_tree {
          // Check if only focus changed
          let old_focus = old_tab.pane_tree.focused_pane_id().map(String::from);
          let new_focus = new_tab.pane_tree.focused_pane_id().map(String::from);

          // Check for title/cwd changes in individual panes
          diff_pane_contents(
            win_id,
            &new_tab.id,
            &old_tab.pane_tree,
            &new_tab.pane_tree,
            diffs,
          );

          if old_focus != new_focus {
            diffs.push(TreeDiff::PaneFocusChanged {
              window_id: win_id.to_string(),
              tab_id: new_tab.id.clone(),
              pane_id: new_focus,
            });
          }

          // If structural change (not just focus/title/cwd), emit full tree diff
          if structurally_different(&old_tab.pane_tree, &new_tab.pane_tree) {
            diffs.push(TreeDiff::PaneTreeChanged {
              window_id: win_id.to_string(),
              tab_id: new_tab.id.clone(),
              pane_tree: new_tab.pane_tree.clone(),
            });
          }
        }
      }
    }
  }
}

/// Check if two pane trees are structurally different (ignoring focus, title, cwd).
fn structurally_different(a: &PaneNode, b: &PaneNode) -> bool {
  match (a, b) {
    (PaneNode::Terminal { id: id_a, .. }, PaneNode::Terminal { id: id_b, .. }) => id_a != id_b,
    (
      PaneNode::Split {
        direction: dir_a,
        first: first_a,
        second: second_a,
        ..
      },
      PaneNode::Split {
        direction: dir_b,
        first: first_b,
        second: second_b,
        ..
      },
    ) => {
      dir_a != dir_b
        || structurally_different(first_a, first_b)
        || structurally_different(second_a, second_b)
    }
    _ => true, // Different node types
  }
}

/// Emit diffs for pane title/cwd changes by comparing matching pane IDs.
fn diff_pane_contents(
  win_id: &str,
  tab_id: &str,
  old: &PaneNode,
  new: &PaneNode,
  diffs: &mut Vec<TreeDiff>,
) {
  match (old, new) {
    (
      PaneNode::Terminal {
        id: old_id,
        title: old_title,
        working_directory: old_wd,
        ..
      },
      PaneNode::Terminal {
        id: new_id,
        title: new_title,
        working_directory: new_wd,
        ..
      },
    ) if old_id == new_id => {
      if old_title != new_title {
        diffs.push(TreeDiff::PaneTitleChanged {
          window_id: win_id.to_string(),
          tab_id: tab_id.to_string(),
          pane_id: new_id.clone(),
          title: new_title.clone(),
        });
      }
      if old_wd != new_wd {
        diffs.push(TreeDiff::PaneWorkingDirectoryChanged {
          window_id: win_id.to_string(),
          tab_id: tab_id.to_string(),
          pane_id: new_id.clone(),
          working_directory: new_wd.clone(),
        });
      }
    }
    (
      PaneNode::Split {
        first: old_first,
        second: old_second,
        ..
      },
      PaneNode::Split {
        first: new_first,
        second: new_second,
        ..
      },
    ) => {
      diff_pane_contents(win_id, tab_id, old_first, new_first, diffs);
      diff_pane_contents(win_id, tab_id, old_second, new_second, diffs);
    }
    _ => {}
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::action::UIAction;

  #[test]
  fn test_diff_empty_trees() {
    let diffs = diff_trees(&UITree::new(), &UITree::new());
    assert!(diffs.is_empty());
  }

  #[test]
  fn test_diff_window_added() {
    let old = UITree::new();
    let mut new = UITree::new();
    new
      .apply(UIAction::AddWindow {
        width: None,
        height: None,
      })
      .unwrap();

    let diffs = diff_trees(&old, &new);
    assert_eq!(diffs.len(), 1);
    assert!(matches!(&diffs[0], TreeDiff::WindowAdded { .. }));
  }

  #[test]
  fn test_diff_tab_operations() {
    let mut old = UITree::new();
    old
      .apply(UIAction::AddWindow {
        width: None,
        height: None,
      })
      .unwrap();
    let win_id = old.windows[0].id.clone();
    old
      .apply(UIAction::AddTab {
        window_id: win_id.clone(),
        shell_path: "bash".into(),
        shell_args: vec![],
        profile: None,
        working_directory: None,
      })
      .unwrap();

    let mut new = old.clone();
    new
      .apply(UIAction::AddTab {
        window_id: win_id.clone(),
        shell_path: "zsh".into(),
        shell_args: vec![],
        profile: None,
        working_directory: None,
      })
      .unwrap();

    let diffs = diff_trees(&old, &new);
    assert!(diffs.iter().any(|d| matches!(d, TreeDiff::TabAdded { .. })));
    assert!(
      diffs
        .iter()
        .any(|d| matches!(d, TreeDiff::ActiveTabChanged { .. }))
    );
  }

  #[test]
  fn test_diff_search_changes() {
    let mut old = UITree::new();
    old
      .apply(UIAction::AddWindow {
        width: None,
        height: None,
      })
      .unwrap();
    let win_id = old.windows[0].id.clone();

    let mut new = old.clone();
    new
      .apply(UIAction::ToggleSearch {
        window_id: win_id.clone(),
      })
      .unwrap();
    new
      .apply(UIAction::SetSearchQuery {
        window_id: win_id.clone(),
        query: "test".into(),
      })
      .unwrap();

    let diffs = diff_trees(&old, &new);
    assert!(
      diffs
        .iter()
        .any(|d| matches!(d, TreeDiff::SearchVisibilityChanged { visible: true, .. }))
    );
    assert!(
      diffs
        .iter()
        .any(|d| matches!(d, TreeDiff::SearchQueryChanged { .. }))
    );
  }

  #[test]
  fn test_diff_pane_focus_change() {
    let mut old = UITree::new();
    old
      .apply(UIAction::AddWindow {
        width: None,
        height: None,
      })
      .unwrap();
    let win_id = old.windows[0].id.clone();
    old
      .apply(UIAction::AddTab {
        window_id: win_id.clone(),
        shell_path: "bash".into(),
        shell_args: vec![],
        profile: None,
        working_directory: None,
      })
      .unwrap();
    let tab_id = old.windows[0].tabs[0].id.clone();
    let pane_id = old.windows[0].tabs[0].pane_tree.terminal_ids()[0].to_string();

    old
      .apply(UIAction::SplitPane {
        window_id: win_id.clone(),
        tab_id: tab_id.clone(),
        pane_id: pane_id.clone(),
        direction: SplitDirection::Vertical,
        shell_path: "bash".into(),
        shell_args: vec![],
        working_directory: None,
      })
      .unwrap();

    let mut new = old.clone();
    new
      .apply(UIAction::FocusNextPane {
        window_id: win_id.clone(),
        tab_id: tab_id.clone(),
      })
      .unwrap();

    let diffs = diff_trees(&old, &new);
    assert!(
      diffs
        .iter()
        .any(|d| matches!(d, TreeDiff::PaneFocusChanged { .. }))
    );
    // No structural change, so no PaneTreeChanged
    assert!(
      !diffs
        .iter()
        .any(|d| matches!(d, TreeDiff::PaneTreeChanged { .. }))
    );
  }
}
