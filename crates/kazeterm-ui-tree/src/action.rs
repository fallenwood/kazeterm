use serde::{Deserialize, Serialize};

use crate::node::{SplitDirection, Position};

/// Every mutation to the UI tree is expressed as a `UIAction`.
/// Actions are serializable so they can be replayed, logged, or sent via JSON API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum UIAction {
  // ── Window management ──
  AddWindow {
    #[serde(default)]
    width: Option<f32>,
    #[serde(default)]
    height: Option<f32>,
  },
  CloseWindow {
    window_id: String,
  },
  ResizeWindow {
    window_id: String,
    width: f32,
    height: f32,
  },
  SetWindowMaximized {
    window_id: String,
    maximized: bool,
  },

  // ── Tab management ──
  AddTab {
    window_id: String,
    shell_path: String,
    #[serde(default)]
    shell_args: Vec<String>,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    working_directory: Option<String>,
  },
  CloseTab {
    window_id: String,
    tab_id: String,
  },
  ActivateTab {
    window_id: String,
    tab_index: usize,
  },
  NextTab {
    window_id: String,
  },
  PreviousTab {
    window_id: String,
  },
  MoveTab {
    window_id: String,
    tab_id: String,
    new_index: usize,
  },
  RenameTab {
    window_id: String,
    tab_id: String,
    /// `None` resets to auto-generated title.
    title: Option<String>,
  },

  // ── Pane management ──
  SplitPane {
    window_id: String,
    tab_id: String,
    pane_id: String,
    direction: SplitDirection,
    /// Shell for the new pane.
    shell_path: String,
    #[serde(default)]
    shell_args: Vec<String>,
    #[serde(default)]
    working_directory: Option<String>,
  },
  ClosePane {
    window_id: String,
    tab_id: String,
    pane_id: String,
  },
  FocusPane {
    window_id: String,
    tab_id: String,
    pane_id: String,
  },
  ResizeSplit {
    window_id: String,
    tab_id: String,
    /// Path to the split node (sequence of "first"/"second" from root).
    split_path: Vec<SplitChild>,
    ratio: f32,
  },
  SwapPanes {
    window_id: String,
    tab_id: String,
  },
  FocusNextPane {
    window_id: String,
    tab_id: String,
  },
  FocusPreviousPane {
    window_id: String,
    tab_id: String,
  },

  // ── Pane content updates (from terminal) ──
  UpdatePaneTitle {
    window_id: String,
    tab_id: String,
    pane_id: String,
    title: String,
  },
  UpdatePaneWorkingDirectory {
    window_id: String,
    tab_id: String,
    pane_id: String,
    working_directory: Option<String>,
  },

  // ── Search ──
  ToggleSearch {
    window_id: String,
  },
  SetSearchQuery {
    window_id: String,
    query: String,
  },
  SetSearchFlags {
    window_id: String,
    #[serde(default)]
    match_case: Option<bool>,
    #[serde(default)]
    match_whole: Option<bool>,
    #[serde(default)]
    use_regex: Option<bool>,
  },
  MoveSearch {
    window_id: String,
    position: Position,
  },

  // ── Tab bar ──
  ToggleTabBar {
    window_id: String,
  },
  SetTabBarVertical {
    window_id: String,
    vertical: bool,
  },

  // ── Overlays / Dialogs ──
  ShowOverlay {
    window_id: String,
    overlay: crate::node::OverlayNode,
  },
  DismissOverlay {
    window_id: String,
  },

  // ── Key debug ──
  ToggleKeyDebug {
    window_id: String,
  },

  // ── Batch: apply multiple actions atomically ──
  Batch {
    actions: Vec<UIAction>,
  },
}

/// Identifies which child of a split to navigate into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitChild {
  First,
  Second,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_action_json_roundtrip() {
    let actions = vec![
      UIAction::AddWindow {
        width: Some(1024.0),
        height: Some(768.0),
      },
      UIAction::AddTab {
        window_id: "win-1".into(),
        shell_path: "pwsh.exe".into(),
        shell_args: vec!["-NoLogo".into()],
        profile: Some("PowerShell".into()),
        working_directory: Some("D:\\Workspace".into()),
      },
      UIAction::SplitPane {
        window_id: "win-1".into(),
        tab_id: "tab-1".into(),
        pane_id: "pane-1".into(),
        direction: SplitDirection::Vertical,
        shell_path: "pwsh.exe".into(),
        shell_args: vec![],
        working_directory: None,
      },
      UIAction::CloseTab {
        window_id: "win-1".into(),
        tab_id: "tab-1".into(),
      },
      UIAction::ToggleSearch {
        window_id: "win-1".into(),
      },
      UIAction::ShowOverlay {
        window_id: "win-1".into(),
        overlay: crate::node::OverlayNode::AboutDialog,
      },
      UIAction::Batch {
        actions: vec![
          UIAction::NextTab {
            window_id: "win-1".into(),
          },
          UIAction::DismissOverlay {
            window_id: "win-1".into(),
          },
        ],
      },
    ];

    for action in &actions {
      let json = serde_json::to_string(action).unwrap();
      let deserialized: UIAction = serde_json::from_str(&json).unwrap();
      assert_eq!(action, &deserialized);
    }
  }
}
