use anyhow::{Result, bail};

use crate::action::{UIAction, SplitChild};
use crate::node::*;

impl UITree {
  /// Apply an action to the tree, mutating it in place.
  /// Returns `Ok(())` on success, or an error if the action is invalid.
  pub fn apply(&mut self, action: UIAction) -> Result<()> {
    match action {
      UIAction::Batch { actions } => {
        for a in actions {
          self.apply(a)?;
        }
        Ok(())
      }

      // ── Window management ──
      UIAction::AddWindow { width, height } => {
        let id = self.next_id("win");
        self.windows.push(WindowNode {
          id,
          size: Size {
            width: width.unwrap_or(800.0),
            height: height.unwrap_or(600.0),
          },
          maximized: false,
          active_tab: None,
          tab_bar: TabBarState::default(),
          search: SearchState::default(),
          tabs: Vec::new(),
          overlay: None,
          key_debug: KeyDebugState::default(),
        });
        Ok(())
      }

      UIAction::CloseWindow { window_id } => {
        let len_before = self.windows.len();
        self.windows.retain(|w| w.id != window_id);
        if self.windows.len() == len_before {
          bail!("Window '{}' not found", window_id);
        }
        Ok(())
      }

      UIAction::ResizeWindow {
        window_id,
        width,
        height,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        win.size = Size { width, height };
        Ok(())
      }

      UIAction::SetWindowMaximized {
        window_id,
        maximized,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        win.maximized = maximized;
        Ok(())
      }

      // ── Tab management ──
      UIAction::AddTab {
        window_id,
        shell_path,
        shell_args,
        profile,
        working_directory,
      } => {
        let tab_id = self.next_id("tab");
        let pane_id = self.next_id("pane");
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        let tab = TabNode {
          id: tab_id,
          custom_title: None,
          shell: ShellConfig {
            path: shell_path,
            args: shell_args,
            profile,
          },
          pane_tree: PaneNode::Terminal {
            id: pane_id,
            working_directory,
            title: String::new(),
            focused: true,
          },
          search: SearchState::default(),
        };
        win.tabs.push(tab);
        win.active_tab = Some(win.tabs.len() - 1);
        Ok(())
      }

      UIAction::CloseTab { window_id, tab_id } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        let (ix, _) = win
          .tab(&tab_id)
          .ok_or_else(|| anyhow::anyhow!("Tab '{}' not found", tab_id))?;
        win.tabs.remove(ix);
        // Adjust active tab
        if win.tabs.is_empty() {
          win.active_tab = None;
        } else if let Some(active) = win.active_tab {
          if active >= win.tabs.len() {
            win.active_tab = Some(win.tabs.len() - 1);
          } else if active > ix {
            win.active_tab = Some(active - 1);
          }
        }
        Ok(())
      }

      UIAction::ActivateTab {
        window_id,
        tab_index,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        if tab_index >= win.tabs.len() {
          bail!(
            "Tab index {} out of range (have {} tabs)",
            tab_index,
            win.tabs.len()
          );
        }
        win.active_tab = Some(tab_index);
        Ok(())
      }

      UIAction::NextTab { window_id } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        if win.tabs.is_empty() {
          return Ok(());
        }
        let current = win.active_tab.unwrap_or(0);
        win.active_tab = Some((current + 1) % win.tabs.len());
        Ok(())
      }

      UIAction::PreviousTab { window_id } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        if win.tabs.is_empty() {
          return Ok(());
        }
        let current = win.active_tab.unwrap_or(0);
        win.active_tab = Some(if current == 0 {
          win.tabs.len() - 1
        } else {
          current - 1
        });
        Ok(())
      }

      UIAction::MoveTab {
        window_id,
        tab_id,
        new_index,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        let (old_ix, _) = win
          .tab(&tab_id)
          .ok_or_else(|| anyhow::anyhow!("Tab '{}' not found", tab_id))?;
        let new_index = new_index.min(win.tabs.len() - 1);
        let tab = win.tabs.remove(old_ix);
        win.tabs.insert(new_index, tab);
        // Update active tab to follow the moved tab if it was active
        if win.active_tab == Some(old_ix) {
          win.active_tab = Some(new_index);
        }
        Ok(())
      }

      UIAction::RenameTab {
        window_id,
        tab_id,
        title,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        let (_, tab) = win
          .tab_mut(&tab_id)
          .ok_or_else(|| anyhow::anyhow!("Tab '{}' not found", tab_id))?;
        tab.custom_title = title;
        Ok(())
      }

      // ── Pane management ──
      UIAction::SplitPane {
        window_id,
        tab_id,
        pane_id,
        direction,
        shell_path,
        shell_args,
        working_directory,
      } => {
        let new_pane_id = self.next_id("pane");
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        let (_, tab) = win
          .tab_mut(&tab_id)
          .ok_or_else(|| anyhow::anyhow!("Tab '{}' not found", tab_id))?;

        // Create the new terminal pane
        let new_terminal = PaneNode::Terminal {
          id: new_pane_id.clone(),
          working_directory,
          title: String::new(),
          focused: true,
        };

        // Find the target pane and replace it with a split
        let target = tab
          .pane_tree
          .find_pane(&pane_id)
          .ok_or_else(|| anyhow::anyhow!("Pane '{}' not found", pane_id))?
          .clone();

        // Unfocus the old pane in the clone
        let mut old_pane = target;
        if let PaneNode::Terminal { focused, .. } = &mut old_pane {
          *focused = false;
        }

        let _ = shell_path;
        let _ = shell_args;

        let split = PaneNode::Split {
          direction,
          ratio: 0.5,
          first: Box::new(old_pane),
          second: Box::new(new_terminal),
        };

        tab.pane_tree.replace_pane(&pane_id, split);
        Ok(())
      }

      UIAction::ClosePane {
        window_id,
        tab_id,
        pane_id,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        let (_, tab) = win
          .tab_mut(&tab_id)
          .ok_or_else(|| anyhow::anyhow!("Tab '{}' not found", tab_id))?;

        // If it's the only terminal, this is equivalent to closing the tab
        if tab.pane_tree.terminal_count() <= 1 {
          bail!("Cannot close the last pane in a tab; close the tab instead");
        }

        // Check if the closed pane was focused; if so, focus the next one
        let was_focused = tab
          .pane_tree
          .find_pane(&pane_id)
          .map(|p| matches!(p, PaneNode::Terminal { focused: true, .. }))
          .unwrap_or(false);

        tab
          .pane_tree
          .remove_pane(&pane_id)
          .ok_or_else(|| anyhow::anyhow!("Pane '{}' not found", pane_id))?;

        // Auto-focus the first terminal if the closed pane was focused
        if was_focused {
          if let Some(first_id) = tab.pane_tree.terminal_ids().first().map(|s| s.to_string()) {
            tab.pane_tree.set_focus(&first_id);
          }
        }
        Ok(())
      }

      UIAction::FocusPane {
        window_id,
        tab_id,
        pane_id,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        let (_, tab) = win
          .tab_mut(&tab_id)
          .ok_or_else(|| anyhow::anyhow!("Tab '{}' not found", tab_id))?;
        tab.pane_tree.set_focus(&pane_id);
        Ok(())
      }

      UIAction::ResizeSplit {
        window_id,
        tab_id,
        split_path,
        ratio,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        let (_, tab) = win
          .tab_mut(&tab_id)
          .ok_or_else(|| anyhow::anyhow!("Tab '{}' not found", tab_id))?;

        let ratio = ratio.clamp(0.1, 0.9);
        let mut node = &mut tab.pane_tree;
        for step in &split_path {
          match node {
            PaneNode::Split {
              first, second, ..
            } => {
              node = match step {
                SplitChild::First => first.as_mut(),
                SplitChild::Second => second.as_mut(),
              };
            }
            _ => bail!("Split path navigates into a terminal node"),
          }
        }
        match node {
          PaneNode::Split {
            ratio: r, ..
          } => {
            *r = ratio;
            Ok(())
          }
          _ => bail!("Split path target is not a split node"),
        }
      }

      UIAction::SwapPanes {
        window_id,
        tab_id,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        let (_, tab) = win
          .tab_mut(&tab_id)
          .ok_or_else(|| anyhow::anyhow!("Tab '{}' not found", tab_id))?;
        swap_innermost_split(&mut tab.pane_tree);
        Ok(())
      }

      UIAction::FocusNextPane {
        window_id,
        tab_id,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        let (_, tab) = win
          .tab_mut(&tab_id)
          .ok_or_else(|| anyhow::anyhow!("Tab '{}' not found", tab_id))?;
        cycle_focus(&mut tab.pane_tree, true);
        Ok(())
      }

      UIAction::FocusPreviousPane {
        window_id,
        tab_id,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        let (_, tab) = win
          .tab_mut(&tab_id)
          .ok_or_else(|| anyhow::anyhow!("Tab '{}' not found", tab_id))?;
        cycle_focus(&mut tab.pane_tree, false);
        Ok(())
      }

      // ── Pane content updates ──
      UIAction::UpdatePaneTitle {
        window_id,
        tab_id,
        pane_id,
        title,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        let (_, tab) = win
          .tab_mut(&tab_id)
          .ok_or_else(|| anyhow::anyhow!("Tab '{}' not found", tab_id))?;
        if let Some(PaneNode::Terminal {
          title: t, ..
        }) = tab.pane_tree.find_pane_mut(&pane_id)
        {
          *t = title;
        }
        Ok(())
      }

      UIAction::UpdatePaneWorkingDirectory {
        window_id,
        tab_id,
        pane_id,
        working_directory,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        let (_, tab) = win
          .tab_mut(&tab_id)
          .ok_or_else(|| anyhow::anyhow!("Tab '{}' not found", tab_id))?;
        if let Some(PaneNode::Terminal {
          working_directory: wd,
          ..
        }) = tab.pane_tree.find_pane_mut(&pane_id)
        {
          *wd = working_directory;
        }
        Ok(())
      }

      // ── Search ──
      UIAction::ToggleSearch { window_id } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        win.search.visible = !win.search.visible;
        Ok(())
      }

      UIAction::SetSearchQuery { window_id, query } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        win.search.query = query;
        Ok(())
      }

      UIAction::SetSearchFlags {
        window_id,
        match_case,
        match_whole,
        use_regex,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        if let Some(v) = match_case {
          win.search.match_case = v;
        }
        if let Some(v) = match_whole {
          win.search.match_whole = v;
        }
        if let Some(v) = use_regex {
          win.search.use_regex = v;
        }
        Ok(())
      }

      UIAction::MoveSearch {
        window_id,
        position,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        win.search.position = position;
        Ok(())
      }

      // ── Tab bar ──
      UIAction::ToggleTabBar { window_id } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        win.tab_bar.visible = !win.tab_bar.visible;
        Ok(())
      }

      UIAction::SetTabBarVertical {
        window_id,
        vertical,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        win.tab_bar.vertical = vertical;
        Ok(())
      }

      // ── Overlays ──
      UIAction::ShowOverlay {
        window_id,
        overlay,
      } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        win.overlay = Some(overlay);
        Ok(())
      }

      UIAction::DismissOverlay { window_id } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        win.overlay = None;
        Ok(())
      }

      // ── Key debug ──
      UIAction::ToggleKeyDebug { window_id } => {
        let win = self
          .window_mut(&window_id)
          .ok_or_else(|| anyhow::anyhow!("Window '{}' not found", window_id))?;
        win.key_debug.enabled = !win.key_debug.enabled;
        Ok(())
      }
    }
  }
}

/// Swap the children of the innermost split that contains the focused pane.
fn swap_innermost_split(node: &mut PaneNode) {
  if let PaneNode::Split {
    first, second, ..
  } = node
  {
    let first_has_focus = first.focused_pane_id().is_some();
    let second_has_focus = second.focused_pane_id().is_some();

    // If the focused pane is directly one of our children, swap them
    if first_has_focus || second_has_focus {
      // Check if children are terminals (innermost split)
      let first_is_terminal = matches!(first.as_ref(), PaneNode::Terminal { .. });
      let second_is_terminal = matches!(second.as_ref(), PaneNode::Terminal { .. });

      if first_is_terminal && second_is_terminal {
        std::mem::swap(first, second);
        return;
      }

      // Recurse into the subtree that has focus
      if first_has_focus && !first_is_terminal {
        swap_innermost_split(first);
      } else if second_has_focus && !second_is_terminal {
        swap_innermost_split(second);
      } else {
        // One is terminal, one is split — just swap at this level
        std::mem::swap(first, second);
      }
    }
  }
}

/// Cycle focus to the next or previous terminal pane.
fn cycle_focus(pane: &mut PaneNode, forward: bool) {
  let ids: Vec<String> = pane.terminal_ids().iter().map(|s| s.to_string()).collect();
  if ids.len() <= 1 {
    return;
  }

  let current_ix = ids
    .iter()
    .position(|id| {
      pane
        .find_pane(id)
        .map(|p| matches!(p, PaneNode::Terminal { focused: true, .. }))
        .unwrap_or(false)
    })
    .unwrap_or(0);

  let next_ix = if forward {
    (current_ix + 1) % ids.len()
  } else if current_ix == 0 {
    ids.len() - 1
  } else {
    current_ix - 1
  };

  pane.set_focus(&ids[next_ix]);
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::action::UIAction;

  fn setup_tree_with_window() -> (UITree, String) {
    let mut tree = UITree::new();
    tree.apply(UIAction::AddWindow {
      width: None,
      height: None,
    })
    .unwrap();
    let win_id = tree.windows[0].id.clone();
    (tree, win_id)
  }

  #[test]
  fn test_add_and_close_window() {
    let mut tree = UITree::new();
    tree
      .apply(UIAction::AddWindow {
        width: Some(1024.0),
        height: Some(768.0),
      })
      .unwrap();

    assert_eq!(tree.windows.len(), 1);
    assert_eq!(tree.windows[0].size.width, 1024.0);

    let win_id = tree.windows[0].id.clone();
    tree
      .apply(UIAction::CloseWindow {
        window_id: win_id,
      })
      .unwrap();
    assert!(tree.windows.is_empty());
  }

  #[test]
  fn test_add_tabs_and_navigate() {
    let (mut tree, win_id) = setup_tree_with_window();

    // Add 3 tabs
    for _ in 0..3 {
      tree
        .apply(UIAction::AddTab {
          window_id: win_id.clone(),
          shell_path: "bash".into(),
          shell_args: vec![],
          profile: None,
          working_directory: None,
        })
        .unwrap();
    }

    let win = tree.window(&win_id).unwrap();
    assert_eq!(win.tabs.len(), 3);
    assert_eq!(win.active_tab, Some(2)); // Last added is active

    // Navigate next (wraps around)
    tree
      .apply(UIAction::NextTab {
        window_id: win_id.clone(),
      })
      .unwrap();
    assert_eq!(tree.window(&win_id).unwrap().active_tab, Some(0));

    // Navigate previous (wraps around)
    tree
      .apply(UIAction::PreviousTab {
        window_id: win_id.clone(),
      })
      .unwrap();
    assert_eq!(tree.window(&win_id).unwrap().active_tab, Some(2));
  }

  #[test]
  fn test_close_tab_adjusts_active() {
    let (mut tree, win_id) = setup_tree_with_window();

    for _ in 0..3 {
      tree
        .apply(UIAction::AddTab {
          window_id: win_id.clone(),
          shell_path: "bash".into(),
          shell_args: vec![],
          profile: None,
          working_directory: None,
        })
        .unwrap();
    }

    // Active = tab 2 (index 2), close tab at index 1
    let tab1_id = tree.window(&win_id).unwrap().tabs[1].id.clone();
    tree
      .apply(UIAction::CloseTab {
        window_id: win_id.clone(),
        tab_id: tab1_id,
      })
      .unwrap();

    let win = tree.window(&win_id).unwrap();
    assert_eq!(win.tabs.len(), 2);
    // Active should shift down from 2 to 1
    assert_eq!(win.active_tab, Some(1));
  }

  #[test]
  fn test_split_and_close_pane() {
    let (mut tree, win_id) = setup_tree_with_window();

    tree
      .apply(UIAction::AddTab {
        window_id: win_id.clone(),
        shell_path: "bash".into(),
        shell_args: vec![],
        profile: None,
        working_directory: None,
      })
      .unwrap();

    let tab_id = tree.window(&win_id).unwrap().tabs[0].id.clone();
    let pane_id = tree.window(&win_id).unwrap().tabs[0]
      .pane_tree
      .terminal_ids()[0]
      .to_string();

    // Split
    tree
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

    let tab = &tree.window(&win_id).unwrap().tabs[0];
    assert_eq!(tab.pane_tree.terminal_count(), 2);

    // New pane should be focused
    let new_pane_id = tab
      .pane_tree
      .focused_pane_id()
      .unwrap()
      .to_string();
    assert_ne!(new_pane_id, pane_id);

    // Close the new pane
    tree
      .apply(UIAction::ClosePane {
        window_id: win_id.clone(),
        tab_id: tab_id.clone(),
        pane_id: new_pane_id,
      })
      .unwrap();

    let tab = &tree.window(&win_id).unwrap().tabs[0];
    assert_eq!(tab.pane_tree.terminal_count(), 1);
  }

  #[test]
  fn test_focus_cycle() {
    let (mut tree, win_id) = setup_tree_with_window();

    tree
      .apply(UIAction::AddTab {
        window_id: win_id.clone(),
        shell_path: "bash".into(),
        shell_args: vec![],
        profile: None,
        working_directory: None,
      })
      .unwrap();

    let tab_id = tree.window(&win_id).unwrap().tabs[0].id.clone();
    let pane_id = tree.window(&win_id).unwrap().tabs[0]
      .pane_tree
      .terminal_ids()[0]
      .to_string();

    // Split to create 2 panes
    tree
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

    // Focus is on second pane; cycle forward should go back to first
    tree
      .apply(UIAction::FocusNextPane {
        window_id: win_id.clone(),
        tab_id: tab_id.clone(),
      })
      .unwrap();

    let focused = tree.window(&win_id).unwrap().tabs[0]
      .pane_tree
      .focused_pane_id()
      .unwrap()
      .to_string();
    assert_eq!(focused, pane_id);
  }

  #[test]
  fn test_search_toggle_and_flags() {
    let (mut tree, win_id) = setup_tree_with_window();

    tree
      .apply(UIAction::ToggleSearch {
        window_id: win_id.clone(),
      })
      .unwrap();
    assert!(tree.window(&win_id).unwrap().search.visible);

    tree
      .apply(UIAction::SetSearchQuery {
        window_id: win_id.clone(),
        query: "hello".into(),
      })
      .unwrap();
    assert_eq!(tree.window(&win_id).unwrap().search.query, "hello");

    tree
      .apply(UIAction::SetSearchFlags {
        window_id: win_id.clone(),
        match_case: Some(true),
        match_whole: None,
        use_regex: Some(true),
      })
      .unwrap();

    let search = &tree.window(&win_id).unwrap().search;
    assert!(search.match_case);
    assert!(!search.match_whole);
    assert!(search.use_regex);
  }

  #[test]
  fn test_overlay_show_dismiss() {
    let (mut tree, win_id) = setup_tree_with_window();

    tree
      .apply(UIAction::ShowOverlay {
        window_id: win_id.clone(),
        overlay: OverlayNode::AboutDialog,
      })
      .unwrap();
    assert!(tree.window(&win_id).unwrap().overlay.is_some());

    tree
      .apply(UIAction::DismissOverlay {
        window_id: win_id.clone(),
      })
      .unwrap();
    assert!(tree.window(&win_id).unwrap().overlay.is_none());
  }

  #[test]
  fn test_rename_tab() {
    let (mut tree, win_id) = setup_tree_with_window();

    tree
      .apply(UIAction::AddTab {
        window_id: win_id.clone(),
        shell_path: "bash".into(),
        shell_args: vec![],
        profile: None,
        working_directory: None,
      })
      .unwrap();

    let tab_id = tree.window(&win_id).unwrap().tabs[0].id.clone();

    tree
      .apply(UIAction::RenameTab {
        window_id: win_id.clone(),
        tab_id: tab_id.clone(),
        title: Some("Build Server".into()),
      })
      .unwrap();

    assert_eq!(
      tree.window(&win_id).unwrap().tabs[0].custom_title.as_deref(),
      Some("Build Server")
    );

    // Reset
    tree
      .apply(UIAction::RenameTab {
        window_id: win_id.clone(),
        tab_id,
        title: None,
      })
      .unwrap();
    assert!(tree.window(&win_id).unwrap().tabs[0].custom_title.is_none());
  }

  #[test]
  fn test_batch_action() {
    let (mut tree, win_id) = setup_tree_with_window();

    tree
      .apply(UIAction::Batch {
        actions: vec![
          UIAction::AddTab {
            window_id: win_id.clone(),
            shell_path: "bash".into(),
            shell_args: vec![],
            profile: None,
            working_directory: None,
          },
          UIAction::AddTab {
            window_id: win_id.clone(),
            shell_path: "zsh".into(),
            shell_args: vec![],
            profile: None,
            working_directory: None,
          },
          UIAction::ActivateTab {
            window_id: win_id.clone(),
            tab_index: 0,
          },
        ],
      })
      .unwrap();

    let win = tree.window(&win_id).unwrap();
    assert_eq!(win.tabs.len(), 2);
    assert_eq!(win.active_tab, Some(0));
  }

  #[test]
  fn test_action_replay_determinism() {
    let actions = vec![
      UIAction::AddWindow {
        width: Some(800.0),
        height: Some(600.0),
      },
    ];

    // We need the window ID, so apply first action to get it.
    let mut tree1 = UITree::new();
    tree1.apply(actions[0].clone()).unwrap();
    let win_id = tree1.windows[0].id.clone();

    let remaining_actions = vec![
      UIAction::AddTab {
        window_id: win_id.clone(),
        shell_path: "pwsh.exe".into(),
        shell_args: vec!["-NoLogo".into()],
        profile: Some("PowerShell".into()),
        working_directory: Some("D:\\Workspace".into()),
      },
      UIAction::AddTab {
        window_id: win_id.clone(),
        shell_path: "bash".into(),
        shell_args: vec![],
        profile: None,
        working_directory: None,
      },
      UIAction::ActivateTab {
        window_id: win_id.clone(),
        tab_index: 0,
      },
      UIAction::ToggleSearch {
        window_id: win_id.clone(),
      },
      UIAction::SetSearchQuery {
        window_id: win_id.clone(),
        query: "error".into(),
      },
    ];

    for a in &remaining_actions {
      tree1.apply(a.clone()).unwrap();
    }

    // Replay the same actions on a fresh tree
    let mut tree2 = UITree::new();
    tree2.apply(actions[0].clone()).unwrap();
    for a in &remaining_actions {
      tree2.apply(a.clone()).unwrap();
    }

    // Both trees should be identical
    assert_eq!(tree1, tree2);

    // And JSON should be identical
    let json1 = serde_json::to_string(&tree1).unwrap();
    let json2 = serde_json::to_string(&tree2).unwrap();
    assert_eq!(json1, json2);
  }
}
