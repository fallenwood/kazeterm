//! Integration tests for the UITree data-driven architecture.
//!
//! These tests verify the full pipeline:
//! JSON snapshot → load → apply actions → diff → verify final state
//!
//! No GPUI dependency — these exercise the pure data layer.

use kazeterm_ui_tree::action::UIAction;
use kazeterm_ui_tree::diff::{TreeDiff, diff_trees};
use kazeterm_ui_tree::node::*;

/// Load a UITree from a JSON snapshot and verify its structure.
#[test]
fn load_json_snapshot_and_verify_structure() {
  let json = r##"{
    "version": 1,
    "windows": [{
      "id": "win-1",
      "size": { "width": 1024, "height": 768 },
      "maximized": false,
      "active_tab": 0,
      "tab_bar": { "visible": true, "vertical": false },
      "search": { "visible": false, "query": "", "match_case": false, "match_whole": false, "use_regex": false, "position": { "x": 0, "y": 0 } },
      "tabs": [{
        "id": "tab-1",
        "custom_title": null,
        "shell": { "path": "pwsh.exe", "args": [] },
        "pane_tree": {
          "type": "terminal",
          "id": "pane-1",
          "working_directory": "D:\\Workspace",
          "title": "pwsh",
          "focused": true
        }
      }, {
        "id": "tab-2",
        "custom_title": "Build",
        "shell": { "path": "pwsh.exe", "args": ["-NoProfile"] },
        "pane_tree": {
          "type": "split",
          "direction": "vertical",
          "ratio": 0.5,
          "first": {
            "type": "terminal",
            "id": "pane-2",
            "working_directory": null,
            "title": "cargo",
            "focused": true
          },
          "second": {
            "type": "terminal",
            "id": "pane-3",
            "working_directory": null,
            "title": "pwsh",
            "focused": false
          }
        }
      }],
      "overlay": null,
      "key_debug": { "enabled": false }
    }],
    "next_id": 10
  }"##;

  let tree: UITree = serde_json::from_str(json).unwrap();

  assert_eq!(tree.version, 1);
  assert_eq!(tree.windows.len(), 1);

  let win = &tree.windows[0];
  assert_eq!(win.id, "win-1");
  assert_eq!(win.size.width, 1024.0);
  assert_eq!(win.size.height, 768.0);
  assert_eq!(win.tabs.len(), 2);
  assert_eq!(win.active_tab, Some(0));

  // Tab 1: single terminal
  let tab1 = &win.tabs[0];
  assert_eq!(tab1.id, "tab-1");
  assert!(tab1.custom_title.is_none());
  assert_eq!(tab1.shell.path, "pwsh.exe");
  match &tab1.pane_tree {
    PaneNode::Terminal {
      id,
      working_directory,
      title,
      focused,
    } => {
      assert_eq!(id, "pane-1");
      assert_eq!(working_directory.as_deref(), Some("D:\\Workspace"));
      assert_eq!(title, "pwsh");
      assert!(*focused);
    }
    _ => panic!("expected terminal"),
  }

  // Tab 2: split with two terminals
  let tab2 = &win.tabs[1];
  assert_eq!(tab2.custom_title.as_deref(), Some("Build"));
  assert_eq!(tab2.shell.args, vec!["-NoProfile"]);
  match &tab2.pane_tree {
    PaneNode::Split {
      direction,
      ratio,
      first,
      second,
    } => {
      assert_eq!(*direction, SplitDirection::Vertical);
      assert!((ratio - 0.5).abs() < f32::EPSILON);
      assert!(matches!(first.as_ref(), PaneNode::Terminal { id, .. } if id == "pane-2"));
      assert!(matches!(second.as_ref(), PaneNode::Terminal { id, .. } if id == "pane-3"));
    }
    _ => panic!("expected split"),
  }

  // Roundtrip back to JSON and reload
  let json_out = serde_json::to_string_pretty(&tree).unwrap();
  let tree2: UITree = serde_json::from_str(&json_out).unwrap();
  assert_eq!(tree, tree2);
}

/// Build a complex workspace entirely through actions and snapshot it.
#[test]
fn build_workspace_via_actions_and_snapshot() {
  let mut tree = UITree::new();

  // Create a window
  tree
    .apply(UIAction::AddWindow {
      width: Some(1920.0),
      height: Some(1080.0),
    })
    .unwrap();
  let win_id = tree.windows[0].id.clone();

  // Add 3 tabs
  for (shell, wd) in [
    ("bash", Some("/home/user")),
    ("zsh", Some("/tmp")),
    ("fish", None),
  ] {
    tree
      .apply(UIAction::AddTab {
        window_id: win_id.clone(),
        shell_path: shell.into(),
        shell_args: vec![],
        profile: None,
        working_directory: wd.map(String::from),
      })
      .unwrap();
  }

  assert_eq!(tree.windows[0].tabs.len(), 3);
  // Last added tab should be active
  assert_eq!(tree.windows[0].active_tab, Some(2));

  // Navigate to first tab
  tree
    .apply(UIAction::ActivateTab {
      window_id: win_id.clone(),
      tab_index: 0,
    })
    .unwrap();
  assert_eq!(tree.windows[0].active_tab, Some(0));

  // Split the first tab's pane
  let tab_id = tree.windows[0].tabs[0].id.clone();
  let pane_id = match &tree.windows[0].tabs[0].pane_tree {
    PaneNode::Terminal { id, .. } => id.clone(),
    _ => panic!("expected terminal"),
  };
  tree
    .apply(UIAction::SplitPane {
      window_id: win_id.clone(),
      tab_id,
      pane_id,
      direction: SplitDirection::Horizontal,
      shell_path: "bash".into(),
      shell_args: vec![],
      working_directory: None,
    })
    .unwrap();

  // Verify the split happened
  match &tree.windows[0].tabs[0].pane_tree {
    PaneNode::Split { direction, .. } => {
      assert_eq!(*direction, SplitDirection::Horizontal);
    }
    _ => panic!("expected split after splitting"),
  }

  // Rename the second tab
  let tab2_id = tree.windows[0].tabs[1].id.clone();
  tree
    .apply(UIAction::RenameTab {
      window_id: win_id.clone(),
      tab_id: tab2_id,
      title: Some("Work".into()),
    })
    .unwrap();
  assert_eq!(
    tree.windows[0].tabs[1].custom_title.as_deref(),
    Some("Work")
  );

  // Toggle search
  tree
    .apply(UIAction::ToggleSearch {
      window_id: win_id.clone(),
    })
    .unwrap();
  assert!(tree.windows[0].search.visible);

  // Set search query
  tree
    .apply(UIAction::SetSearchQuery {
      window_id: win_id.clone(),
      query: "error".into(),
    })
    .unwrap();
  assert_eq!(tree.windows[0].search.query, "error");

  // Snapshot to JSON
  let json = serde_json::to_string_pretty(&tree).unwrap();
  let restored: UITree = serde_json::from_str(&json).unwrap();
  assert_eq!(tree, restored);

  // Verify the snapshot has all expected fields
  let val: serde_json::Value = serde_json::from_str(&json).unwrap();
  assert_eq!(val["windows"][0]["tabs"].as_array().unwrap().len(), 3);
  assert_eq!(val["windows"][0]["search"]["visible"], true);
  assert_eq!(val["windows"][0]["search"]["query"], "error");
  assert_eq!(
    val["windows"][0]["tabs"][1]["custom_title"],
    serde_json::Value::String("Work".into())
  );
}

/// Test loading a snapshot, applying mutations, diffing, and verifying diffs.
#[test]
fn load_mutate_diff_pipeline() {
  let mut tree = UITree::new();
  tree
    .apply(UIAction::AddWindow {
      width: Some(800.0),
      height: Some(600.0),
    })
    .unwrap();
  let win_id = tree.windows[0].id.clone();
  tree
    .apply(UIAction::AddTab {
      window_id: win_id.clone(),
      shell_path: "bash".into(),
      shell_args: vec![],
      profile: None,
      working_directory: Some("/home".into()),
    })
    .unwrap();
  tree
    .apply(UIAction::AddTab {
      window_id: win_id.clone(),
      shell_path: "zsh".into(),
      shell_args: vec![],
      profile: None,
      working_directory: None,
    })
    .unwrap();

  // Snapshot before mutations
  let snapshot_before = tree.clone();

  // Apply a batch of mutations
  tree
    .apply(UIAction::Batch {
      actions: vec![
        UIAction::ActivateTab {
          window_id: win_id.clone(),
          tab_index: 0,
        },
        UIAction::ToggleSearch {
          window_id: win_id.clone(),
        },
        UIAction::RenameTab {
          window_id: win_id.clone(),
          tab_id: tree.windows[0].tabs[0].id.clone(),
          title: Some("Primary".into()),
        },
      ],
    })
    .unwrap();

  // Diff
  let diffs = diff_trees(&snapshot_before, &tree);
  assert!(!diffs.is_empty());

  // Verify expected diffs
  let has_active_tab_changed = diffs
    .iter()
    .any(|d| matches!(d, TreeDiff::ActiveTabChanged { .. }));
  let has_search_changed = diffs
    .iter()
    .any(|d| matches!(d, TreeDiff::SearchVisibilityChanged { .. }));
  let has_rename = diffs
    .iter()
    .any(|d| matches!(d, TreeDiff::TabRenamed { .. }));

  assert!(has_active_tab_changed);
  assert!(has_search_changed);
  assert!(has_rename);
}

/// Verify that the tree can handle overlay lifecycle.
#[test]
fn overlay_lifecycle() {
  let mut tree = UITree::new();
  tree
    .apply(UIAction::AddWindow {
      width: None,
      height: None,
    })
    .unwrap();
  let win_id = tree.windows[0].id.clone();

  // No overlay initially
  assert!(tree.windows[0].overlay.is_none());

  // Show about dialog
  tree
    .apply(UIAction::ShowOverlay {
      window_id: win_id.clone(),
      overlay: OverlayNode::AboutDialog,
    })
    .unwrap();
  assert_eq!(tree.windows[0].overlay, Some(OverlayNode::AboutDialog));

  // Replace with close confirm
  tree
    .apply(UIAction::ShowOverlay {
      window_id: win_id.clone(),
      overlay: OverlayNode::CloseConfirm,
    })
    .unwrap();
  assert_eq!(tree.windows[0].overlay, Some(OverlayNode::CloseConfirm));

  // Dismiss
  tree
    .apply(UIAction::DismissOverlay {
      window_id: win_id.clone(),
    })
    .unwrap();
  assert!(tree.windows[0].overlay.is_none());

  // Roundtrip with overlay present
  tree
    .apply(UIAction::ShowOverlay {
      window_id: win_id.clone(),
      overlay: OverlayNode::RenameDialog {
        tab_id: "tab-1".into(),
        current_title: "My Tab".into(),
      },
    })
    .unwrap();
  let json = serde_json::to_string(&tree).unwrap();
  let restored: UITree = serde_json::from_str(&json).unwrap();
  assert_eq!(tree, restored);
}

/// Test that close_tab on the last tab removes it and sets active to None.
#[test]
fn close_last_tab() {
  let mut tree = UITree::new();
  tree
    .apply(UIAction::AddWindow {
      width: None,
      height: None,
    })
    .unwrap();
  let win_id = tree.windows[0].id.clone();

  tree
    .apply(UIAction::AddTab {
      window_id: win_id.clone(),
      shell_path: "sh".into(),
      shell_args: vec![],
      profile: None,
      working_directory: None,
    })
    .unwrap();

  let tab_id = tree.windows[0].tabs[0].id.clone();
  tree
    .apply(UIAction::CloseTab {
      window_id: win_id.clone(),
      tab_id,
    })
    .unwrap();

  assert!(tree.windows[0].tabs.is_empty());
  assert_eq!(tree.windows[0].active_tab, None);
}

/// Test complex split pane operations with nested splits.
#[test]
fn nested_split_operations() {
  let mut tree = UITree::new();
  tree
    .apply(UIAction::AddWindow {
      width: None,
      height: None,
    })
    .unwrap();
  let win_id = tree.windows[0].id.clone();

  tree
    .apply(UIAction::AddTab {
      window_id: win_id.clone(),
      shell_path: "bash".into(),
      shell_args: vec![],
      profile: None,
      working_directory: None,
    })
    .unwrap();

  // Get the initial tab and pane IDs
  let tab_id = tree.windows[0].tabs[0].id.clone();
  let pane_id = match &tree.windows[0].tabs[0].pane_tree {
    PaneNode::Terminal { id, .. } => id.clone(),
    _ => unreachable!(),
  };

  // Split horizontally → creates first/second
  tree
    .apply(UIAction::SplitPane {
      window_id: win_id.clone(),
      tab_id: tab_id.clone(),
      pane_id: pane_id.clone(),
      direction: SplitDirection::Horizontal,
      shell_path: "bash".into(),
      shell_args: vec![],
      working_directory: None,
    })
    .unwrap();

  // Get the "second" pane (newly created)
  let second_pane_id = match &tree.windows[0].tabs[0].pane_tree {
    PaneNode::Split { second, .. } => match second.as_ref() {
      PaneNode::Terminal { id, .. } => id.clone(),
      _ => panic!("second should be terminal"),
    },
    _ => panic!("expected split"),
  };

  // Split the second pane vertically → creates a nested split
  tree
    .apply(UIAction::SplitPane {
      window_id: win_id.clone(),
      tab_id: tab_id.clone(),
      pane_id: second_pane_id.clone(),
      direction: SplitDirection::Vertical,
      shell_path: "bash".into(),
      shell_args: vec![],
      working_directory: None,
    })
    .unwrap();

  // Verify nested structure
  match &tree.windows[0].tabs[0].pane_tree {
    PaneNode::Split {
      direction: d1,
      second,
      ..
    } => {
      assert_eq!(*d1, SplitDirection::Horizontal);
      match second.as_ref() {
        PaneNode::Split { direction: d2, .. } => {
          assert_eq!(*d2, SplitDirection::Vertical);
        }
        _ => panic!("expected nested split"),
      }
    }
    _ => panic!("expected outer split"),
  }

  // Count total terminal panes (should be 3: original + 2 from splits)
  assert_eq!(tree.windows[0].tabs[0].pane_tree.terminal_count(), 3);

  // Snapshot and restore
  let json = serde_json::to_string_pretty(&tree).unwrap();
  let restored: UITree = serde_json::from_str(&json).unwrap();
  assert_eq!(tree, restored);
}
