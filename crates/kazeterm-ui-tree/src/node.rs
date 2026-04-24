use serde::{Deserialize, Serialize};

/// The root of the entire UI state. Serializable as JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UITree {
  pub version: u32,
  pub windows: Vec<WindowNode>,
  /// Monotonic counter for generating unique IDs.
  #[serde(default)]
  pub next_id: u64,
}

impl Default for UITree {
  fn default() -> Self {
    Self {
      version: 1,
      windows: Vec::new(),
      next_id: 1,
    }
  }
}

impl UITree {
  pub fn new() -> Self {
    Self::default()
  }

  /// Generate the next unique node ID with the given prefix.
  pub fn next_id(&mut self, prefix: &str) -> String {
    let id = format!("{}-{}", prefix, self.next_id);
    self.next_id += 1;
    id
  }

  /// Find a window by ID.
  pub fn window(&self, id: &str) -> Option<&WindowNode> {
    self.windows.iter().find(|w| w.id == id)
  }

  /// Find a window by ID (mutable).
  pub fn window_mut(&mut self, id: &str) -> Option<&mut WindowNode> {
    self.windows.iter_mut().find(|w| w.id == id)
  }
}

/// A single application window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowNode {
  pub id: String,
  pub size: Size,
  #[serde(default)]
  pub maximized: bool,
  /// Index into `tabs` for the active tab, or `None` if no tabs.
  pub active_tab: Option<usize>,
  pub tab_bar: TabBarState,
  pub search: SearchState,
  pub tabs: Vec<TabNode>,
  /// Currently displayed overlay/dialog, if any.
  pub overlay: Option<OverlayNode>,
  pub key_debug: KeyDebugState,
}

impl WindowNode {
  /// Get the active tab, if any.
  pub fn active_tab(&self) -> Option<&TabNode> {
    self.active_tab.and_then(|ix| self.tabs.get(ix))
  }

  /// Get the active tab mutably, if any.
  pub fn active_tab_mut(&mut self) -> Option<&mut TabNode> {
    self.active_tab.and_then(|ix| self.tabs.get_mut(ix))
  }

  /// Find a tab by ID.
  pub fn tab(&self, id: &str) -> Option<(usize, &TabNode)> {
    self.tabs.iter().enumerate().find(|(_, t)| t.id == id)
  }

  /// Find a tab by ID (mutable).
  pub fn tab_mut(&mut self, id: &str) -> Option<(usize, &mut TabNode)> {
    self.tabs.iter_mut().enumerate().find(|(_, t)| t.id == id)
  }
}

/// Window dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Size {
  pub width: f32,
  pub height: f32,
}

impl Default for Size {
  fn default() -> Self {
    Self {
      width: 800.0,
      height: 600.0,
    }
  }
}

/// Tab bar visibility and layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabBarState {
  pub visible: bool,
  pub vertical: bool,
}

impl Default for TabBarState {
  fn default() -> Self {
    Self {
      visible: true,
      vertical: false,
    }
  }
}

/// Per-window search bar state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchState {
  pub visible: bool,
  #[serde(default)]
  pub query: String,
  #[serde(default)]
  pub match_case: bool,
  #[serde(default)]
  pub match_whole: bool,
  #[serde(default)]
  pub use_regex: bool,
  #[serde(default)]
  pub position: Position,
}

impl Default for SearchState {
  fn default() -> Self {
    Self {
      visible: false,
      query: String::new(),
      match_case: false,
      match_whole: false,
      use_regex: false,
      position: Position::default(),
    }
  }
}

/// 2D position for movable UI elements.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
  pub x: f32,
  pub y: f32,
}

impl Default for Position {
  fn default() -> Self {
    Self { x: 0.0, y: 0.0 }
  }
}

/// Key debug overlay state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyDebugState {
  pub enabled: bool,
}

impl Default for KeyDebugState {
  fn default() -> Self {
    Self { enabled: false }
  }
}

/// A single tab within a window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabNode {
  pub id: String,
  /// User-set custom title. `None` means use auto-generated title.
  pub custom_title: Option<String>,
  pub shell: ShellConfig,
  pub pane_tree: PaneNode,
  /// Per-tab search state (query, flags, visibility).
  #[serde(default)]
  pub search: SearchState,
}

/// Shell configuration for a tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShellConfig {
  pub path: String,
  #[serde(default)]
  pub args: Vec<String>,
  /// Profile name this shell was launched from, if any.
  #[serde(default)]
  pub profile: Option<String>,
}

/// A node in the pane split tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaneNode {
  Terminal {
    id: String,
    #[serde(default)]
    working_directory: Option<String>,
    #[serde(default)]
    title: String,
    /// ID of the focused pane within this tab's tree.
    #[serde(default)]
    focused: bool,
  },
  Split {
    direction: SplitDirection,
    ratio: f32,
    first: Box<PaneNode>,
    second: Box<PaneNode>,
  },
}

impl PaneNode {
  /// Find a terminal pane by ID.
  pub fn find_pane(&self, pane_id: &str) -> Option<&PaneNode> {
    match self {
      PaneNode::Terminal { id, .. } if id == pane_id => Some(self),
      PaneNode::Split { first, second, .. } => first
        .find_pane(pane_id)
        .or_else(|| second.find_pane(pane_id)),
      _ => None,
    }
  }

  /// Find a terminal pane by ID (mutable).
  pub fn find_pane_mut(&mut self, pane_id: &str) -> Option<&mut PaneNode> {
    match self {
      PaneNode::Terminal { id, .. } if id == pane_id => Some(self),
      PaneNode::Split { first, second, .. } => first
        .find_pane_mut(pane_id)
        .or_else(|| second.find_pane_mut(pane_id)),
      _ => None,
    }
  }

  /// Collect all terminal pane IDs in depth-first order.
  pub fn terminal_ids(&self) -> Vec<&str> {
    match self {
      PaneNode::Terminal { id, .. } => vec![id.as_str()],
      PaneNode::Split { first, second, .. } => {
        let mut ids = first.terminal_ids();
        ids.extend(second.terminal_ids());
        ids
      }
    }
  }

  /// Get the focused pane ID, if any terminal is focused.
  pub fn focused_pane_id(&self) -> Option<&str> {
    match self {
      PaneNode::Terminal { id, focused, .. } if *focused => Some(id.as_str()),
      PaneNode::Split { first, second, .. } => {
        first.focused_pane_id().or_else(|| second.focused_pane_id())
      }
      _ => None,
    }
  }

  /// Set focus on a specific pane, clearing focus from all others.
  pub fn set_focus(&mut self, target_id: &str) {
    match self {
      PaneNode::Terminal { id, focused, .. } => {
        *focused = id == target_id;
      }
      PaneNode::Split { first, second, .. } => {
        first.set_focus(target_id);
        second.set_focus(target_id);
      }
    }
  }

  /// Count terminal panes.
  pub fn terminal_count(&self) -> usize {
    match self {
      PaneNode::Terminal { .. } => 1,
      PaneNode::Split { first, second, .. } => first.terminal_count() + second.terminal_count(),
    }
  }

  /// Replace a terminal pane with a new subtree. Returns true if replaced.
  pub fn replace_pane(&mut self, target_id: &str, replacement: PaneNode) -> bool {
    match self {
      PaneNode::Terminal { id, .. } if id == target_id => {
        *self = replacement;
        true
      }
      PaneNode::Split { first, second, .. } => {
        first.replace_pane(target_id, replacement.clone())
          || second.replace_pane(target_id, replacement)
      }
      _ => false,
    }
  }

  /// Remove a pane by ID. Returns the sibling if removal collapses a split.
  /// Returns `None` if the pane wasn't found or is the only pane.
  pub fn remove_pane(&mut self, target_id: &str) -> Option<()> {
    match self {
      PaneNode::Split { first, second, .. } => {
        // Check if first child is the target terminal
        if matches!(first.as_ref(), PaneNode::Terminal { id, .. } if id == target_id) {
          *self = *second.clone();
          return Some(());
        }
        // Check if second child is the target terminal
        if matches!(second.as_ref(), PaneNode::Terminal { id, .. } if id == target_id) {
          *self = *first.clone();
          return Some(());
        }
        // Recurse
        first
          .remove_pane(target_id)
          .or_else(|| second.remove_pane(target_id))
      }
      _ => None,
    }
  }
}

/// Direction of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitDirection {
  Horizontal,
  Vertical,
}

/// Overlay/dialog state — at most one is shown at a time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OverlayNode {
  AboutDialog,
  CloseConfirm,
  RenameDialog {
    tab_id: String,
    current_title: String,
  },
  ImportAlacritty {
    #[serde(default)]
    path: String,
    #[serde(default)]
    error: Option<String>,
  },
  ShellError {
    message: String,
  },
  TabSwitcher {
    selected_index: usize,
  },
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_default_tree() {
    let tree = UITree::new();
    assert_eq!(tree.version, 1);
    assert!(tree.windows.is_empty());
  }

  #[test]
  fn test_next_id() {
    let mut tree = UITree::new();
    assert_eq!(tree.next_id("tab"), "tab-1");
    assert_eq!(tree.next_id("pane"), "pane-2");
    assert_eq!(tree.next_id("win"), "win-3");
  }

  #[test]
  fn test_pane_tree_operations() {
    let mut pane = PaneNode::Split {
      direction: SplitDirection::Vertical,
      ratio: 0.5,
      first: Box::new(PaneNode::Terminal {
        id: "p1".into(),
        working_directory: None,
        title: "shell".into(),
        focused: true,
      }),
      second: Box::new(PaneNode::Terminal {
        id: "p2".into(),
        working_directory: None,
        title: "shell".into(),
        focused: false,
      }),
    };

    assert_eq!(pane.terminal_count(), 2);
    assert_eq!(pane.focused_pane_id(), Some("p1"));
    assert_eq!(pane.terminal_ids(), vec!["p1", "p2"]);

    pane.set_focus("p2");
    assert_eq!(pane.focused_pane_id(), Some("p2"));

    pane.remove_pane("p1");
    assert!(matches!(pane, PaneNode::Terminal { id, .. } if id == "p2"));
  }

  #[test]
  fn test_json_roundtrip_tree() {
    let tree = UITree {
      version: 1,
      next_id: 5,
      windows: vec![WindowNode {
        id: "win-1".into(),
        size: Size {
          width: 1024.0,
          height: 768.0,
        },
        maximized: false,
        active_tab: Some(0),
        tab_bar: TabBarState::default(),
        search: SearchState::default(),
        tabs: vec![TabNode {
          id: "tab-1".into(),
          custom_title: Some("My Tab".into()),
          shell: ShellConfig {
            path: "pwsh.exe".into(),
            args: vec![],
            profile: None,
          },
          pane_tree: PaneNode::Split {
            direction: SplitDirection::Vertical,
            ratio: 0.5,
            first: Box::new(PaneNode::Terminal {
              id: "pane-1".into(),
              working_directory: Some("D:\\Workspace".into()),
              title: "pwsh".into(),
              focused: true,
            }),
            second: Box::new(PaneNode::Terminal {
              id: "pane-2".into(),
              working_directory: None,
              title: "cargo".into(),
              focused: false,
            }),
          },
          search: SearchState::default(),
        }],
        overlay: None,
        key_debug: KeyDebugState::default(),
      }],
    };

    let json = serde_json::to_string_pretty(&tree).unwrap();
    let deserialized: UITree = serde_json::from_str(&json).unwrap();
    assert_eq!(tree, deserialized);
  }

  #[test]
  fn test_json_roundtrip_overlay_variants() {
    let overlays = vec![
      OverlayNode::AboutDialog,
      OverlayNode::CloseConfirm,
      OverlayNode::RenameDialog {
        tab_id: "tab-1".into(),
        current_title: "Hello".into(),
      },
      OverlayNode::ImportAlacritty {
        path: "/home/user/.config/alacritty.toml".into(),
        error: Some("File not found".into()),
      },
      OverlayNode::ShellError {
        message: "Shell crashed".into(),
      },
      OverlayNode::TabSwitcher { selected_index: 2 },
    ];

    for overlay in &overlays {
      let json = serde_json::to_string(overlay).unwrap();
      let deserialized: OverlayNode = serde_json::from_str(&json).unwrap();
      assert_eq!(overlay, &deserialized);
    }
  }

  #[test]
  fn test_json_roundtrip_pane_variants() {
    let terminal = PaneNode::Terminal {
      id: "p-1".into(),
      working_directory: Some("/home".into()),
      title: "bash".into(),
      focused: true,
    };
    let json = serde_json::to_string(&terminal).unwrap();
    let de: PaneNode = serde_json::from_str(&json).unwrap();
    assert_eq!(terminal, de);

    let split = PaneNode::Split {
      direction: SplitDirection::Horizontal,
      ratio: 0.3,
      first: Box::new(terminal.clone()),
      second: Box::new(PaneNode::Terminal {
        id: "p-2".into(),
        working_directory: None,
        title: "zsh".into(),
        focused: false,
      }),
    };
    let json = serde_json::to_string(&split).unwrap();
    let de: PaneNode = serde_json::from_str(&json).unwrap();
    assert_eq!(split, de);
  }
}
