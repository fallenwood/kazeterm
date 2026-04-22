use std::path::PathBuf;
use std::sync::atomic::Ordering;

use gpui::{Context, Window};
use serde::{Deserialize, Serialize};

use super::main_window::MainWindow;
use super::main_window_tab_item::TabItem;
use super::main_window_tab_management::get_working_directory_pathbuf;
use crate::components::search_bar::SearchBarState;
use crate::components::split_pane::{PaneId, SplitContainer, SplitDirection, SplitPane};

/// Serializable workspace state — the full snapshot saved to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceState {
  pub version: u32,
  pub tabs: Vec<TabState>,
  pub active_tab_index: Option<usize>,
}

/// Serializable mirror of TabItem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabState {
  pub shell_path: String,
  pub shell_args: Vec<String>,
  pub custom_title: Option<String>,
  pub pane_tree: PaneTreeState,
}

/// Serializable mirror of SplitPane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaneTreeState {
  Terminal {
    working_directory: Option<String>,
  },
  Split {
    direction: SplitDirectionState,
    first: Box<PaneTreeState>,
    second: Box<PaneTreeState>,
    ratio: f32,
  },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SplitDirectionState {
  Horizontal,
  Vertical,
}

impl WorkspaceState {
  pub fn workspace_file_path() -> PathBuf {
    config::Config::get_config_path().join("workspace.json")
  }

  pub fn save(&self) {
    let path = Self::workspace_file_path();
    if let Some(parent) = path.parent() {
      if let Err(e) = std::fs::create_dir_all(parent) {
        tracing::error!("Failed to create workspace directory: {}", e);
        return;
      }
    }
    match serde_json::to_string_pretty(self) {
      Ok(json) => {
        if let Err(e) = std::fs::write(&path, json) {
          tracing::error!("Failed to write workspace state: {}", e);
        } else {
          tracing::info!("Saved workspace state to {}", path.display());
        }
      }
      Err(e) => {
        tracing::error!("Failed to serialize workspace state: {}", e);
      }
    }
  }

  pub fn load() -> Option<Self> {
    let path = Self::workspace_file_path();
    if !path.exists() {
      return None;
    }
    match std::fs::read_to_string(&path) {
      Ok(content) => match serde_json::from_str::<Self>(&content) {
        Ok(state) if !state.tabs.is_empty() => Some(state),
        Ok(_) => None,
        Err(e) => {
          tracing::error!("Failed to parse workspace state: {}", e);
          None
        }
      },
      Err(e) => {
        tracing::error!("Failed to read workspace state file: {}", e);
        None
      }
    }
  }

  pub fn delete() {
    let path = Self::workspace_file_path();
    if path.exists() {
      let _ = std::fs::remove_file(&path);
    }
  }
}

impl PaneTreeState {
  fn from_split_pane(pane: &SplitPane, cx: &mut Context<MainWindow>) -> Self {
    match pane {
      SplitPane::Terminal { terminal, .. } => {
        let terminal_entity = terminal.read(cx).terminal().clone();
        let cwd = terminal_entity.update(cx, |t, _cx| t.current_working_directory());
        PaneTreeState::Terminal {
          working_directory: cwd,
        }
      }
      SplitPane::Split {
        direction,
        first,
        second,
        ratio,
      } => PaneTreeState::Split {
        direction: match direction {
          SplitDirection::Horizontal => SplitDirectionState::Horizontal,
          SplitDirection::Vertical => SplitDirectionState::Vertical,
        },
        first: Box::new(Self::from_split_pane(first, cx)),
        second: Box::new(Self::from_split_pane(second, cx)),
        ratio: *ratio,
      },
    }
  }

  /// Get the working directory of the first terminal leaf in the tree.
  #[allow(dead_code)]
  fn first_leaf(&self) -> &PaneTreeState {
    match self {
      PaneTreeState::Terminal { .. } => self,
      PaneTreeState::Split { first, .. } => first.first_leaf(),
    }
  }
}

impl MainWindow {
  pub fn capture_workspace_state(&self, cx: &mut Context<Self>) -> WorkspaceState {
    let tabs: Vec<TabState> = self
      .items
      .iter()
      .map(|item| {
        let pane_tree = PaneTreeState::from_split_pane(&item.split_container.root, cx);
        TabState {
          shell_path: item.shell_path.clone(),
          shell_args: item.shell_args.clone(),
          custom_title: item.custom_title.clone(),
          pane_tree,
        }
      })
      .collect();

    WorkspaceState {
      version: 1,
      tabs,
      active_tab_index: self.active_tab_ix,
    }
  }

  pub fn restore_workspace(
    &mut self,
    state: WorkspaceState,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    for tab_state in &state.tabs {
      self.restore_tab(tab_state, window, cx);
    }
    if let Some(active_ix) = state.active_tab_index {
      if active_ix < self.items.len() {
        self.set_active_tab(active_ix, window, cx);
      }
    }
  }

  fn restore_tab(&mut self, tab_state: &TabState, window: &mut Window, cx: &mut Context<Self>) {
    let mut next_pane_id: usize = 0;
    let (root_pane, subscriptions) = match Self::build_split_pane(
      &tab_state.pane_tree,
      &tab_state.shell_path,
      &tab_state.shell_args,
      &mut next_pane_id,
      &self.tab_index,
      window,
      cx,
    ) {
      Ok(result) => result,
      Err(err) => {
        tracing::error!("Failed to restore tab: {err}");
        self.show_shell_error_dialog(err, window, cx);
        return;
      }
    };

    // Find the first terminal pane id for active_pane_id
    let first_pane_id = Self::first_pane_id(&root_pane);

    let split_container =
      SplitContainer::from_restored_root(root_pane, first_pane_id, next_pane_id);

    let index = self.tab_index.fetch_add(1, Ordering::SeqCst);

    let shell_name = std::path::Path::new(&tab_state.shell_path)
      .file_stem()
      .and_then(|n| n.to_str())
      .unwrap_or(&tab_state.shell_path)
      .to_lowercase();

    let title = tab_state
      .custom_title
      .clone()
      .unwrap_or_else(|| shell_name.clone());

    // Store the first subscription in TabItem, forget the rest (matching existing pattern)
    let mut sub_iter = subscriptions.into_iter();
    let first_sub = sub_iter.next().expect("at least one terminal in tab");
    for sub in sub_iter {
      std::mem::forget(sub);
    }

    let item = TabItem {
      index,
      title,
      custom_title: tab_state.custom_title.clone(),
      shell_path: tab_state.shell_path.clone(),
      shell_args: tab_state.shell_args.clone(),
      _shell_name: shell_name,
      split_container,
      _subscription: first_sub,
      search_bar_state: SearchBarState::default(),
    };
    self.items.push(item);

    let new_ix = self.items.len() - 1;
    self.set_active_tab(new_ix, window, cx);
  }

  fn build_split_pane(
    state: &PaneTreeState,
    tab_shell: &str,
    tab_shell_args: &[String],
    next_pane_id: &mut usize,
    tab_index_counter: &std::sync::atomic::AtomicUsize,
    window: &mut Window,
    cx: &mut Context<MainWindow>,
  ) -> Result<(SplitPane, Vec<gpui::Subscription>), String> {
    match state {
      PaneTreeState::Terminal { working_directory } => {
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
        let pane_id = PaneId(*next_pane_id);
        *next_pane_id += 1;
        Ok((SplitPane::new_terminal(pane_id, terminal), vec![sub]))
      }
      PaneTreeState::Split {
        direction,
        first,
        second,
        ratio,
      } => {
        let (first_pane, mut subs) = Self::build_split_pane(
          first,
          tab_shell,
          tab_shell_args,
          next_pane_id,
          tab_index_counter,
          window,
          cx,
        )?;
        let (second_pane, subs2) = Self::build_split_pane(
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
          SplitDirectionState::Horizontal => SplitDirection::Horizontal,
          SplitDirectionState::Vertical => SplitDirection::Vertical,
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
}
