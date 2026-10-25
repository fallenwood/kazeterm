//! Opt-in Kitty-compatible local control, routed to existing live terminals and
//! UI actions. No PTY interception, TCP exposure, or implicit authorization.

use std::collections::{HashMap, HashSet};

use base64::Engine as _;
use gpui::{AnyWindowHandle, App, AppContext, Entity, Focusable};
use kazeterm_event_system::remote_control::{RemoteRequest, RemoteResponse};
use kazeterm_ui_tree::action::UIAction;
use serde::Deserialize;
use serde_json::{Value, json};
use terminal::{Terminal, TerminalView};
use terminal_kernel::index::{Column, Line, Point};

use crate::components::MainWindow;

const MAX_TEXT_BYTES: usize = 512 * 1024;
const MAX_TARGETS: usize = 2048;

#[cfg(unix)]
struct RemoteRuntime {
  _transport: gpui::Task<()>,
  _dispatch: gpui::Task<()>,
}
#[cfg(unix)]
impl gpui::Global for RemoteRuntime {}

#[cfg(unix)]
pub(crate) fn start(
  server: kazeterm_event_system::remote_control::RemoteControlServer,
  cx: &mut App,
) {
  let (sender, receiver) = smol::channel::bounded(32);
  let transport = cx.background_spawn(async move {
    if let Err(error) = server.run(sender).await {
      tracing::error!("Remote-control listener stopped: {error}");
    }
  });
  let dispatch = cx.spawn(async move |cx| {
    let mut ids = RemoteIds::default();
    while let Ok(call) = receiver.recv().await {
      let response = cx.update(|cx| {
        if std::time::Instant::now() >= call.deadline {
          RemoteResponse::failure("Request expired before dispatch")
        } else {
          execute(&call.request, &mut ids, cx)
        }
      });
      let _ = call.reply.try_send(response);
    }
  });
  cx.set_global(RemoteRuntime {
    _transport: transport,
    _dispatch: dispatch,
  });
}

#[derive(Default)]
pub(crate) struct RemoteIds {
  tabs: HashMap<(u64, String), u64>,
  next_tab: u64,
}

impl RemoteIds {
  fn tab(&mut self, os_window: u64, tab: &str) -> u64 {
    *self
      .tabs
      .entry((os_window, tab.to_owned()))
      .or_insert_with(|| {
        self.next_tab += 1;
        self.next_tab
      })
  }
}

struct Target {
  owner: Entity<MainWindow>,
  window: AnyWindowHandle,
  os_id: u64,
  tab_id: u64,
  tab_index: usize,
  tab_tree_id: String,
  pane_tree_id: String,
  tab_title: String,
  terminal: Entity<Terminal>,
  view: Entity<TerminalView>,
  id: u64,
  focused: bool,
  active: bool,
  tab_active: bool,
}

fn targets(ids: &mut RemoteIds, cx: &App) -> Result<Vec<Target>, String> {
  let mut targets = Vec::new();
  for (os_index, (weak, window)) in crate::window_manager::event_targets(cx)
    .into_iter()
    .enumerate()
  {
    let Some(owner) = weak.upgrade() else {
      continue;
    };
    let main = owner.read(cx);
    let os_id = window.window_id().as_u64();
    for (tab_index, tab) in main.items.iter().enumerate() {
      let tab_id = ids.tab(os_id, &tab.ui_tree_id);
      for (pane, view) in tab.split_container.all_terminals() {
        if targets.len() == MAX_TARGETS {
          return Err("Too many terminals for one remote request".into());
        }
        let terminal = view.read(cx).terminal().clone();
        let active = tab.split_container.active_pane_id == Some(pane);
        let tab_active = main.active_tab_ix == Some(tab_index);
        targets.push(Target {
          owner: owner.clone(),
          window,
          os_id,
          tab_id,
          tab_index,
          tab_tree_id: tab.ui_tree_id.clone(),
          pane_tree_id: format!("pane-{}", pane.0),
          tab_title: tab.display_title().chars().take(4096).collect(),
          id: terminal.entity_id().as_u64(),
          terminal,
          view,
          focused: os_index == 0 && tab_active && active,
          active,
          tab_active,
        });
      }
    }
  }
  // Dead tab keys must not accumulate as a client repeatedly creates/closes tabs.
  let live: HashSet<_> = targets
    .iter()
    .map(|t| (t.os_id, t.tab_tree_id.clone()))
    .collect();
  ids.tabs.retain(|key, _| live.contains(key));
  Ok(targets)
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Selection {
  #[serde(rename = "match")]
  match_window: Option<String>,
  match_tab: Option<String>,
  all: bool,
  #[serde(rename = "self")]
  self_window: bool,
}

fn match_id(expression: Option<&str>) -> Result<Option<u64>, String> {
  match expression.filter(|s| !s.is_empty()) {
    None => Ok(None),
    Some(expression) => expression
      .strip_prefix("id:")
      .and_then(|id| id.parse::<u64>().ok())
      .filter(|id| *id != 0)
      .map(Some)
      .ok_or_else(|| "Only exact nonzero id:N matching is supported".into()),
  }
}

fn selected<'a>(
  all: &'a [Target],
  selection: &Selection,
  origin: Option<u64>,
  default_all: bool,
) -> Result<Vec<&'a Target>, String> {
  let window_id = match_id(selection.match_window.as_deref())?;
  let tab_id = match_id(selection.match_tab.as_deref())?;
  let origin = origin.filter(|id| *id != 0);
  if selection.self_window
    && (origin.is_none() || window_id.is_some() || tab_id.is_some() || selection.all)
  {
    return Err("self requires kitty_window_id and cannot be combined with other selectors".into());
  }
  Ok(
    all
      .iter()
      .filter(|target| {
        if selection.self_window {
          return Some(target.id) == origin;
        }
        if window_id.is_some_and(|id| target.id != id)
          || tab_id.is_some_and(|id| target.tab_id != id)
        {
          return false;
        }
        if window_id.is_some() || tab_id.is_some() || selection.all || default_all {
          return true;
        }
        origin.map_or(target.focused, |id| target.id == id)
      })
      .collect(),
  )
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LsPayload {
  #[serde(rename = "match")]
  match_window: Option<String>,
  match_tab: Option<String>,
  #[serde(rename = "self")]
  self_window: bool,
  all_env_vars: bool,
  output_format: String,
}
impl Default for LsPayload {
  fn default() -> Self {
    Self {
      match_window: None,
      match_tab: None,
      self_window: false,
      all_env_vars: false,
      output_format: "json".into(),
    }
  }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct TextPayload {
  #[serde(rename = "match")]
  match_window: Option<String>,
  #[serde(rename = "self")]
  self_window: bool,
  extent: String,
  ansi: bool,
  cursor: bool,
  wrap_markers: bool,
  clear_selection: bool,
}
impl Default for TextPayload {
  fn default() -> Self {
    Self {
      match_window: None,
      self_window: false,
      extent: "screen".into(),
      ansi: false,
      cursor: false,
      wrap_markers: false,
      clear_selection: false,
    }
  }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SendPayload {
  data: String,
  #[serde(default, rename = "match")]
  match_window: Option<String>,
  #[serde(default)]
  match_tab: Option<String>,
  #[serde(default)]
  all: bool,
  #[serde(default)]
  exclude_active: bool,
  #[serde(default)]
  session_id: String,
  #[serde(default)]
  bracketed_paste: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TitlePayload {
  title: String,
  #[serde(default, rename = "match")]
  match_tab: Option<String>,
}

pub(crate) fn execute(
  request: &RemoteRequest,
  ids: &mut RemoteIds,
  cx: &mut App,
) -> RemoteResponse {
  match execute_inner(request, ids, cx) {
    Ok(data) => RemoteResponse::success(data),
    Err(error) => RemoteResponse::failure(error),
  }
}

fn execute_inner(
  request: &RemoteRequest,
  ids: &mut RemoteIds,
  cx: &mut App,
) -> Result<Option<Value>, String> {
  // Validate all payload fields/selectors before any mutation. Unknown commands
  // never fall through to Kazeterm's more permissive internal JSON action API.
  match request.cmd.as_str() {
    "ls" => {
      let payload: LsPayload = request.payload()?;
      if payload.all_env_vars || payload.output_format != "json" {
        return Err("Only JSON ls without environment disclosure is supported".into());
      }
      let all = targets(ids, cx)?;
      let selection = Selection {
        match_window: payload.match_window,
        match_tab: payload.match_tab,
        self_window: payload.self_window,
        all: false,
      };
      let selected = selected(
        &all,
        &selection,
        request.kitty_window_id,
        !selection.self_window,
      )?;
      let mut os_windows: Vec<Value> = Vec::new();
      let mut budget = 0usize;
      for target in selected {
        let main = target.owner.read(cx);
        let tab = &main.items[target.tab_index];
        if tab.shell_args.len() > 128
          || std::iter::once(&tab.shell_path)
            .chain(&tab.shell_args)
            .any(|s| s.len() > 4096)
        {
          return Err("Command-line metadata exceeds the remote-control size limit".into());
        }
        let command_line: Vec<_> = std::iter::once(&tab.shell_path)
          .chain(&tab.shell_args)
          .collect();
        let terminal = target.terminal.read(cx);
        let info = json!({
          "id": target.id, "title": terminal.title_text.chars().take(4096).collect::<String>(),
          "is_focused": target.focused, "is_active": target.active,
          "is_self": request.kitty_window_id == Some(target.id),
          "cwd": terminal.cached_working_directory().unwrap_or_default(),
          "cmdline": command_line, "env": {},
          "lines": terminal.term.screen_lines(), "columns": terminal.term.columns(),
        });
        budget = budget
          .saturating_add(serde_json::to_vec(&info).map_err(|e| e.to_string())?.len())
          .saturating_add(target.tab_title.len().saturating_mul(6) + 256);
        if budget > MAX_TEXT_BYTES {
          return Err("Listing exceeds the remote-control size limit".into());
        }
        let os_ix = os_windows.iter().position(|os| os["id"] == target.os_id).unwrap_or_else(|| {
          os_windows.push(json!({"id": target.os_id, "is_focused": all.iter().any(|t| t.os_id == target.os_id && t.focused), "tabs": []}));
          os_windows.len() - 1
        });
        let tabs = os_windows[os_ix]["tabs"].as_array_mut().unwrap();
        let tab_ix = tabs.iter().position(|tab| tab["id"] == target.tab_id).unwrap_or_else(|| {
          tabs.push(json!({"id": target.tab_id, "title": target.tab_title, "is_focused": target.tab_active, "windows": []}));
          tabs.len() - 1
        });
        tabs[tab_ix]["windows"].as_array_mut().unwrap().push(info);
      }
      // Kitty ls returns JSON encoded as a string inside response.data.
      Ok(Some(Value::String(
        serde_json::to_string(&os_windows).map_err(|e| e.to_string())?,
      )))
    }
    "get-text" => {
      let payload: TextPayload = request.payload()?;
      if payload.ansi || payload.cursor || payload.wrap_markers || payload.clear_selection {
        return Err(
          "Formatted text, cursor/wrap markers and clear_selection are not supported".into(),
        );
      }
      if !["screen", "all", "selection"].contains(&payload.extent.as_str()) {
        return Err("Unsupported text extent".into());
      }
      let all = targets(ids, cx)?;
      let selection = Selection {
        match_window: payload.match_window,
        self_window: payload.self_window,
        ..Selection::default()
      };
      let matches = selected(&all, &selection, request.kitty_window_id, false)?;
      let target = matches.first().ok_or("No matching terminal window")?;
      let terminal = target.terminal.read(cx);
      let rows = if payload.extent == "screen" {
        terminal.term.screen_lines()
      } else {
        terminal.term.total_lines()
      };
      if rows
        .saturating_mul(terminal.term.columns())
        .saturating_mul(4)
        > MAX_TEXT_BYTES
      {
        return Err("Text extent exceeds the remote-control size limit".into());
      }
      let text = if payload.extent == "selection" {
        terminal.term.selection_to_string().unwrap_or_default()
      } else {
        let first = if payload.extent == "all" {
          terminal.term.topmost_line()
        } else {
          Line(0)
        };
        terminal.term.bounds_to_string(
          Point::new(first, Column(0)),
          Point::new(terminal.term.bottommost_line(), terminal.term.last_column()),
        )
      };
      if text.len() > MAX_TEXT_BYTES {
        return Err("Text exceeds the remote-control size limit".into());
      }
      Ok(Some(Value::String(text)))
    }
    "send-text" => {
      let payload: SendPayload = request.payload()?;
      if !payload.session_id.is_empty()
        || payload
          .bracketed_paste
          .as_deref()
          .is_some_and(|s| s != "disable")
      {
        return Err("Broadcast sessions and bracketed-paste modes are not supported".into());
      }
      let bytes = if let Some(text) = payload.data.strip_prefix("text:") {
        text.as_bytes().to_vec()
      } else if let Some(encoded) = payload.data.strip_prefix("base64:") {
        base64::prelude::BASE64_STANDARD
          .decode(encoded)
          .map_err(|_| "Invalid base64 send-text data")?
      } else {
        return Err("send-text data must use text: or base64: encoding".into());
      };
      let all = targets(ids, cx)?;
      let selection = Selection {
        match_window: payload.match_window,
        match_tab: payload.match_tab,
        all: payload.all,
        self_window: false,
      };
      let matches = selected(&all, &selection, request.kitty_window_id, false)?;
      for target in matches {
        if payload.exclude_active && target.focused {
          continue;
        }
        target.terminal.update(cx, |terminal, cx| {
          terminal.input(bytes.clone());
          cx.notify();
        });
      }
      Ok(None)
    }
    "focus-window" => {
      let selection: Selection = request.payload()?;
      if selection.all {
        return Err("focus-window does not support all".into());
      }
      let all = targets(ids, cx)?;
      let matches = selected(&all, &selection, request.kitty_window_id, false)?;
      let target = matches.first().ok_or("No matching terminal window")?;
      cx.update_window(target.window, |_root, window, cx| {
        target.owner.update(cx, |main, cx| {
          let window_id = main
            .ensure_ui_tree_window_id(cx)
            .ok_or("Window is unavailable")?;
          main
            .dispatch_ui_action(
              UIAction::ActivateTab {
                window_id: window_id.clone(),
                tab_index: target.tab_index,
              },
              window,
              cx,
            )
            .map_err(|e| e.to_string())?;
          main
            .dispatch_ui_action(
              UIAction::FocusPane {
                window_id,
                tab_id: target.tab_tree_id.clone(),
                pane_id: target.pane_tree_id.clone(),
              },
              window,
              cx,
            )
            .map_err(|e| e.to_string())?;
          window.activate_window();
          let focus = target.view.read(cx).focus_handle(cx);
          focus.focus(window, cx);
          Ok::<_, String>(())
        })
      })
      .map_err(|e| e.to_string())??;
      Ok(None)
    }
    "set-tab-title" => {
      let payload: TitlePayload = request.payload()?;
      if payload.title.len() > 4096 || payload.title.chars().any(char::is_control) {
        return Err("Tab title is too long or contains control characters".into());
      }
      let tab_id = match_id(payload.match_tab.as_deref())?;
      let all = targets(ids, cx)?;
      let target = all
        .iter()
        .find(|t| {
          tab_id.map_or_else(
            || {
              request
                .kitty_window_id
                .filter(|id| *id != 0)
                .map_or(t.focused, |id| t.id == id)
            },
            |id| t.tab_id == id,
          )
        })
        .ok_or("No matching tab")?;
      cx.update_window(target.window, |_root, window, cx| {
        target.owner.update(cx, |main, cx| {
          let window_id = main
            .ensure_ui_tree_window_id(cx)
            .ok_or("Window is unavailable")?;
          main
            .dispatch_ui_action(
              UIAction::RenameTab {
                window_id,
                tab_id: target.tab_tree_id.clone(),
                title: (!payload.title.is_empty()).then_some(payload.title),
              },
              window,
              cx,
            )
            .map_err(|e| e.to_string())
        })
      })
      .map_err(|e| e.to_string())??;
      Ok(None)
    }
    _ => Err("Unsupported remote-control command".into()),
  }
}
