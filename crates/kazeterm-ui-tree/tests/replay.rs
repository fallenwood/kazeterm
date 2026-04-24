//! Replay determinism tests for the UITree.
//!
//! These tests verify that given an initial state and a sequence of actions,
//! the resulting tree is always identical — the core guarantee of a
//! data-driven architecture.

use kazeterm_ui_tree::action::UIAction;
use kazeterm_ui_tree::node::*;

/// Helper: apply a sequence of actions to a fresh tree and return the result.
fn replay(actions: &[UIAction]) -> UITree {
  let mut tree = UITree::new();
  for action in actions {
    tree.apply(action.clone()).unwrap();
  }
  tree
}

/// Helper: apply a sequence of actions to a fresh tree and return the JSON.
fn replay_json(actions: &[UIAction]) -> String {
  serde_json::to_string_pretty(&replay(actions)).unwrap()
}

/// The simplest replay: same actions → same tree.
#[test]
fn identical_action_sequences_produce_identical_trees() {
  let actions = vec![
    UIAction::AddWindow {
      width: Some(1024.0),
      height: Some(768.0),
    },
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "bash".into(),
      shell_args: vec!["-l".into()],
      profile: None,
      working_directory: Some("/home".into()),
    },
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "zsh".into(),
      shell_args: vec![],
      profile: None,
      working_directory: None,
    },
    UIAction::ActivateTab {
      window_id: "win-1".into(),
      tab_index: 0,
    },
  ];

  let json1 = replay_json(&actions);
  let json2 = replay_json(&actions);
  assert_eq!(json1, json2);
}

/// Replay with splits and focus cycling.
#[test]
fn replay_with_splits_and_focus() {
  let actions = vec![
    UIAction::AddWindow {
      width: None,
      height: None,
    },
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "bash".into(),
      shell_args: vec![],
      profile: None,
      working_directory: None,
    },
    UIAction::SplitPane {
      window_id: "win-1".into(),
      tab_id: "tab-2".into(),
      pane_id: "pane-3".into(),
      direction: SplitDirection::Horizontal,
      shell_path: "bash".into(),
      shell_args: vec![],
      working_directory: None,
    },
    UIAction::FocusNextPane {
      window_id: "win-1".into(),
      tab_id: "tab-2".into(),
    },
    UIAction::FocusPreviousPane {
      window_id: "win-1".into(),
      tab_id: "tab-2".into(),
    },
  ];

  let tree1 = replay(&actions);
  let tree2 = replay(&actions);
  assert_eq!(tree1, tree2);

  // Verify the split structure
  match &tree1.windows[0].tabs[0].pane_tree {
    PaneNode::Split { direction, .. } => {
      assert_eq!(*direction, SplitDirection::Horizontal);
    }
    _ => panic!("expected split"),
  }
}

/// Replay with search operations.
#[test]
fn replay_search_operations() {
  let actions = vec![
    UIAction::AddWindow {
      width: None,
      height: None,
    },
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "bash".into(),
      shell_args: vec![],
      profile: None,
      working_directory: None,
    },
    UIAction::ToggleSearch {
      window_id: "win-1".into(),
    },
    UIAction::SetSearchQuery {
      window_id: "win-1".into(),
      query: "hello world".into(),
    },
    UIAction::SetSearchFlags {
      window_id: "win-1".into(),
      match_case: Some(true),
      match_whole: Some(false),
      use_regex: Some(true),
    },
  ];

  let tree1 = replay(&actions);
  let tree2 = replay(&actions);
  assert_eq!(tree1, tree2);

  assert!(tree1.windows[0].search.visible);
  assert_eq!(tree1.windows[0].search.query, "hello world");
  assert!(tree1.windows[0].search.match_case);
  assert!(tree1.windows[0].search.use_regex);
}

/// Replay with overlay show/dismiss.
#[test]
fn replay_overlay_lifecycle() {
  let actions = vec![
    UIAction::AddWindow {
      width: None,
      height: None,
    },
    UIAction::ShowOverlay {
      window_id: "win-1".into(),
      overlay: OverlayNode::AboutDialog,
    },
    UIAction::DismissOverlay {
      window_id: "win-1".into(),
    },
    UIAction::ShowOverlay {
      window_id: "win-1".into(),
      overlay: OverlayNode::CloseConfirm,
    },
  ];

  let tree1 = replay(&actions);
  let tree2 = replay(&actions);
  assert_eq!(tree1, tree2);
  assert_eq!(tree1.windows[0].overlay, Some(OverlayNode::CloseConfirm));
}

/// Replay a complex multi-tab workspace with renames, reorders, and closes.
#[test]
fn replay_complex_workspace() {
  let actions = vec![
    UIAction::AddWindow {
      width: Some(1920.0),
      height: Some(1080.0),
    },
    // Add 4 tabs
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "bash".into(),
      shell_args: vec![],
      profile: None,
      working_directory: Some("/home".into()),
    },
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "zsh".into(),
      shell_args: vec![],
      profile: None,
      working_directory: Some("/tmp".into()),
    },
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "fish".into(),
      shell_args: vec![],
      profile: None,
      working_directory: None,
    },
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "pwsh".into(),
      shell_args: vec!["-NoProfile".into()],
      profile: None,
      working_directory: Some("C:\\Users".into()),
    },
    // Rename tabs
    UIAction::RenameTab {
      window_id: "win-1".into(),
      tab_id: "tab-2".into(),
      title: Some("Home".into()),
    },
    UIAction::RenameTab {
      window_id: "win-1".into(),
      tab_id: "tab-4".into(),
      title: Some("Build".into()),
    },
    // Navigate to first tab
    UIAction::ActivateTab {
      window_id: "win-1".into(),
      tab_index: 0,
    },
    // Split the first tab
    UIAction::SplitPane {
      window_id: "win-1".into(),
      tab_id: "tab-2".into(),
      pane_id: "pane-3".into(),
      direction: SplitDirection::Vertical,
      shell_path: "bash".into(),
      shell_args: vec![],
      working_directory: None,
    },
    // Close the second tab (zsh, renamed "Build")
    UIAction::CloseTab {
      window_id: "win-1".into(),
      tab_id: "tab-4".into(),
    },
    // Move tab
    UIAction::MoveTab {
      window_id: "win-1".into(),
      tab_id: "tab-2".into(),
      new_index: 1,
    },
    // Toggle search
    UIAction::ToggleSearch {
      window_id: "win-1".into(),
    },
  ];

  let tree1 = replay(&actions);
  let tree2 = replay(&actions);

  // Exact equality (same actions → same tree)
  assert_eq!(tree1, tree2);

  // JSON equality (serialization is deterministic)
  let json1 = serde_json::to_string_pretty(&tree1).unwrap();
  let json2 = serde_json::to_string_pretty(&tree2).unwrap();
  assert_eq!(json1, json2);

  // Structural checks
  assert_eq!(tree1.windows[0].tabs.len(), 3); // 4 added, 1 closed
  assert!(tree1.windows[0].search.visible);
}

/// Replay produces the same result whether done atomically or via a Batch.
#[test]
fn batch_vs_sequential_equivalence() {
  let individual_actions = vec![
    UIAction::AddWindow {
      width: Some(800.0),
      height: Some(600.0),
    },
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "bash".into(),
      shell_args: vec![],
      profile: None,
      working_directory: None,
    },
    UIAction::ToggleSearch {
      window_id: "win-1".into(),
    },
    UIAction::SetSearchQuery {
      window_id: "win-1".into(),
      query: "test".into(),
    },
  ];

  // Apply individually
  let tree_sequential = replay(&individual_actions);

  // Apply first action, then batch the rest
  let mut tree_batch = UITree::new();
  tree_batch.apply(individual_actions[0].clone()).unwrap();
  tree_batch
    .apply(UIAction::Batch {
      actions: individual_actions[1..].to_vec(),
    })
    .unwrap();

  assert_eq!(tree_sequential, tree_batch);
}

/// Snapshot → deserialize → apply more actions → snapshot again.
/// The intermediate deserialization should not affect the outcome.
#[test]
fn snapshot_restore_continue() {
  let setup_actions = vec![
    UIAction::AddWindow {
      width: None,
      height: None,
    },
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "bash".into(),
      shell_args: vec![],
      profile: None,
      working_directory: None,
    },
  ];

  let continuation_actions = vec![
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "zsh".into(),
      shell_args: vec![],
      profile: None,
      working_directory: None,
    },
    UIAction::ToggleSearch {
      window_id: "win-1".into(),
    },
  ];

  // Path 1: straight through
  let mut tree_straight = replay(&setup_actions);
  for a in &continuation_actions {
    tree_straight.apply(a.clone()).unwrap();
  }

  // Path 2: snapshot, restore, then continue
  let tree_checkpoint = replay(&setup_actions);
  let json = serde_json::to_string(&tree_checkpoint).unwrap();
  let mut tree_restored: UITree = serde_json::from_str(&json).unwrap();
  for a in &continuation_actions {
    tree_restored.apply(a.clone()).unwrap();
  }

  assert_eq!(tree_straight, tree_restored);
}

/// JSON action serialization roundtrip — actions can be stored and replayed.
#[test]
fn action_json_roundtrip_and_replay() {
  let actions = vec![
    UIAction::AddWindow {
      width: Some(1024.0),
      height: Some(768.0),
    },
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "bash".into(),
      shell_args: vec![],
      profile: None,
      working_directory: Some("/home".into()),
    },
    UIAction::SplitPane {
      window_id: "win-1".into(),
      tab_id: "tab-2".into(),
      pane_id: "pane-3".into(),
      direction: SplitDirection::Horizontal,
      shell_path: "bash".into(),
      shell_args: vec![],
      working_directory: None,
    },
  ];

  // Serialize each action to JSON
  let json_actions: Vec<String> = actions
    .iter()
    .map(|a| serde_json::to_string(a).unwrap())
    .collect();

  // Deserialize and replay
  let deserialized: Vec<UIAction> = json_actions
    .iter()
    .map(|j| serde_json::from_str(j).unwrap())
    .collect();

  let tree_original = replay(&actions);
  let tree_replayed = replay(&deserialized);

  assert_eq!(tree_original, tree_replayed);
}

/// Multiple replays with different orderings produce different trees,
/// confirming the system is not trivially ignoring actions.
#[test]
fn different_orderings_produce_different_trees() {
  let actions_a = vec![
    UIAction::AddWindow {
      width: None,
      height: None,
    },
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "bash".into(),
      shell_args: vec![],
      profile: None,
      working_directory: None,
    },
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "zsh".into(),
      shell_args: vec![],
      profile: None,
      working_directory: None,
    },
    UIAction::ActivateTab {
      window_id: "win-1".into(),
      tab_index: 0,
    },
  ];

  let actions_b = vec![
    UIAction::AddWindow {
      width: None,
      height: None,
    },
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "zsh".into(),
      shell_args: vec![],
      profile: None,
      working_directory: None,
    },
    UIAction::AddTab {
      window_id: "win-1".into(),
      shell_path: "bash".into(),
      shell_args: vec![],
      profile: None,
      working_directory: None,
    },
    UIAction::ActivateTab {
      window_id: "win-1".into(),
      tab_index: 0,
    },
  ];

  let tree_a = replay(&actions_a);
  let tree_b = replay(&actions_b);

  // Different orderings → different shell paths on tab 0
  assert_ne!(tree_a, tree_b);
  assert_eq!(tree_a.windows[0].tabs[0].shell.path, "bash");
  assert_eq!(tree_b.windows[0].tabs[0].shell.path, "zsh");
}
