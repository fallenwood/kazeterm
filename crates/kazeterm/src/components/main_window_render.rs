use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
  ActiveTheme, Icon, IconName, Sizable, StyledExt, TITLE_BAR_HEIGHT, TitleBar,
  button::{Button, ButtonVariants},
  h_flex,
  label::Label,
  menu::{ContextMenuExt, DropdownMenu, PopupMenu},
};
use smol::Timer;
use themeing::SettingsStore;

use super::main_window::{KeyDebugModifiers, KeyDebugPressedKey, KeyDebugRecentKey, MainWindow};
use super::menu_builder::{build_new_tab_menu, build_tab_context_menu};
use super::terminal_tab_bar::{TerminalTab, TerminalTabBar};
use crate::components::dragged_tab::{DraggedTab, DraggedTabView};
use crate::components::shell_icon::ShellIcon;
use crate::components::tab_button::{TabButton, TabButtonClickEvent};

#[derive(Clone)]
struct ResizeVerticalTabbar(pub EntityId);

impl Render for ResizeVerticalTabbar {
  fn render(&mut self, _window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    Empty
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KeyDebugEntry {
  raw_key: String,
  shortcut: String,
  action: Option<String>,
}

const KEY_DEBUG_RELEASE_PERSIST_DURATION: Duration = Duration::from_secs(3);
const KEY_DEBUG_MAX_ROWS: usize = 16;

impl KeyDebugModifiers {
  fn is_empty(self) -> bool {
    !self.control && !self.shift && !self.alt && !self.platform
  }

  fn display_text(self) -> String {
    let mut parts = Vec::new();
    if self.platform {
      parts.push(if cfg!(target_os = "macos") {
        "Cmd"
      } else if cfg!(target_os = "windows") {
        "Win"
      } else {
        "Super"
      });
    }
    if self.control {
      parts.push("Ctrl");
    }
    if self.shift {
      parts.push("Shift");
    }
    if self.alt {
      parts.push("Alt");
    }
    if parts.is_empty() {
      "No modifiers".to_string()
    } else {
      parts.join("+")
    }
  }
}

impl MainWindow {
  fn set_key_debug_modifiers(&mut self, modifiers: KeyDebugModifiers, cx: &mut Context<Self>) {
    if self.key_debug_modifiers == modifiers {
      return;
    }

    self.key_debug_modifiers = modifiers;
    if cx.global::<::config::Config>().window.key_debug_mode {
      cx.notify();
    }
  }

  fn press_key_debug_key(
    &mut self,
    key: &str,
    modifiers: KeyDebugModifiers,
    cx: &mut Context<Self>,
  ) {
    let mut changed = false;

    if !is_modifier_key(key) {
      self.key_debug_pressed_keys.push(KeyDebugPressedKey {
        raw_key: key.to_string(),
        modifiers,
        action: None,
      });
      changed = true;
    }

    if changed && cx.global::<::config::Config>().window.key_debug_mode {
      cx.notify();
    }
  }

  fn annotate_latest_key_debug_key(
    &mut self,
    key: &str,
    action: Option<String>,
    cx: &mut Context<Self>,
  ) {
    if let Some(pressed) = self
      .key_debug_pressed_keys
      .iter_mut()
      .rev()
      .find(|pressed| same_key_identity(&pressed.raw_key, key))
      && pressed.action != action
    {
      pressed.action = action;
      if cx.global::<::config::Config>().window.key_debug_mode {
        cx.notify();
      }
    }
  }

  fn release_key_debug_key(&mut self, key: &str, cx: &mut Context<Self>) {
    let released = if let Some(ix) = self
      .key_debug_pressed_keys
      .iter()
      .position(|pressed| same_key_identity(&pressed.raw_key, key))
    {
      let pressed = self.key_debug_pressed_keys.remove(ix);
      KeyDebugRecentKey {
        raw_key: pressed.raw_key.clone(),
        modifiers: pressed.modifiers,
        shortcut: format_pressed_shortcut(pressed.modifiers, &pressed.raw_key),
        action: pressed.action,
        expires_at: Instant::now() + KEY_DEBUG_RELEASE_PERSIST_DURATION,
      }
    } else {
      KeyDebugRecentKey {
        raw_key: key.to_string(),
        modifiers: self.key_debug_modifiers,
        shortcut: if is_modifier_key(key) {
          display_key_name(key)
        } else {
          format_pressed_shortcut(self.key_debug_modifiers, key)
        },
        action: None,
        expires_at: Instant::now() + KEY_DEBUG_RELEASE_PERSIST_DURATION,
      }
    };

    self.key_debug_recent_keys.insert(0, released);
    self.key_debug_recent_keys.truncate(KEY_DEBUG_MAX_ROWS);

    if cx.global::<::config::Config>().window.key_debug_mode {
      cx.notify();
    }

    let view = cx.entity();
    cx.spawn(async move |_this, cx| {
      Timer::after(KEY_DEBUG_RELEASE_PERSIST_DURATION).await;
      let _ = view.update(cx, |this, cx| {
        this.prune_expired_key_debug_history(cx);
      });
    })
    .detach();
  }

  fn prune_expired_key_debug_history(&mut self, cx: &mut Context<Self>) {
    let len_before = self.key_debug_recent_keys.len();
    let now = Instant::now();
    self
      .key_debug_recent_keys
      .retain(|recent| recent.expires_at > now);
    if len_before != self.key_debug_recent_keys.len()
      && cx.global::<::config::Config>().window.key_debug_mode
    {
      cx.notify();
    }
  }
}

fn is_modifier_key(key: &str) -> bool {
  matches!(
    key.to_ascii_lowercase().as_str(),
    "control" | "ctrl" | "shift" | "alt" | "meta" | "super" | "win" | "cmd"
  )
}

fn same_key_identity(left: &str, right: &str) -> bool {
  left.eq_ignore_ascii_case(right)
}

fn display_key_name(key: &str) -> String {
  let normalized = key.to_ascii_lowercase();
  match normalized.as_str() {
    "control" | "ctrl" => "Ctrl".to_string(),
    "shift" => "Shift".to_string(),
    "alt" => "Alt".to_string(),
    "meta" | "super" | "win" => {
      if cfg!(target_os = "macos") {
        "Cmd".to_string()
      } else if cfg!(target_os = "windows") {
        "Win".to_string()
      } else {
        "Super".to_string()
      }
    }
    "cmd" => "Cmd".to_string(),
    "escape" => "Escape".to_string(),
    "enter" => "Enter".to_string(),
    "tab" => "Tab".to_string(),
    "space" => "Space".to_string(),
    "backspace" => "Backspace".to_string(),
    "delete" => "Delete".to_string(),
    "insert" => "Insert".to_string(),
    "home" => "Home".to_string(),
    "end" => "End".to_string(),
    "pageup" => "Page Up".to_string(),
    "pagedown" => "Page Down".to_string(),
    "up" => "Up".to_string(),
    "down" => "Down".to_string(),
    "left" => "Left".to_string(),
    "right" => "Right".to_string(),
    _ if key.len() == 1 => key.to_uppercase(),
    _ if normalized.starts_with('f') && normalized[1..].chars().all(|ch| ch.is_ascii_digit()) => {
      normalized.to_uppercase()
    }
    _ => {
      let mut chars = normalized.chars();
      match chars.next() {
        Some(first) => first.to_uppercase().to_string() + chars.as_str(),
        None => String::new(),
      }
    }
  }
}

fn modifier_labels(modifiers: KeyDebugModifiers) -> Vec<&'static str> {
  let mut parts = Vec::new();
  if modifiers.platform {
    parts.push(if cfg!(target_os = "macos") {
      "Cmd"
    } else if cfg!(target_os = "windows") {
      "Win"
    } else {
      "Super"
    });
  }
  if modifiers.control {
    parts.push("Ctrl");
  }
  if modifiers.shift {
    parts.push("Shift");
  }
  if modifiers.alt {
    parts.push("Alt");
  }
  parts
}

fn modifier_raw_labels(modifiers: KeyDebugModifiers) -> Vec<&'static str> {
  let mut parts = Vec::new();
  if modifiers.platform {
    parts.push(if cfg!(target_os = "macos") {
      "cmd"
    } else if cfg!(target_os = "windows") {
      "win"
    } else {
      "super"
    });
  }
  if modifiers.control {
    parts.push("control");
  }
  if modifiers.shift {
    parts.push("shift");
  }
  if modifiers.alt {
    parts.push("alt");
  }
  parts
}

fn format_pressed_shortcut(modifiers: KeyDebugModifiers, key: &str) -> String {
  let mut parts = modifier_labels(modifiers)
    .into_iter()
    .map(ToOwned::to_owned)
    .collect::<Vec<_>>();
  parts.push(display_key_name(key));
  parts.join("+")
}

fn push_key_debug_action(
  actions: &mut Vec<String>,
  label: impl Into<String>,
  bindings: &::config::KeybindingList,
  modifiers: KeyDebugModifiers,
  key: &str,
) {
  if bindings.matches(
    modifiers.control,
    modifiers.shift,
    modifiers.alt,
    modifiers.platform,
    key,
  ) {
    let label = label.into();
    if !actions.iter().any(|action| action == &label) {
      actions.push(label);
    }
  }
}

fn resolve_key_debug_action(
  config: &::config::Config,
  modifiers: KeyDebugModifiers,
  key: &str,
  search_visible: bool,
) -> Option<String> {
  let keybindings = &config.keybindings;
  let mut actions = Vec::new();

  push_key_debug_action(&mut actions, "No-op", &keybindings.noop, modifiers, key);
  push_key_debug_action(&mut actions, "Copy", &keybindings.copy, modifiers, key);
  push_key_debug_action(&mut actions, "Paste", &keybindings.paste, modifiers, key);
  push_key_debug_action(
    &mut actions,
    "Zoom In",
    &keybindings.zoom_in,
    modifiers,
    key,
  );
  push_key_debug_action(
    &mut actions,
    "Zoom Out",
    &keybindings.zoom_out,
    modifiers,
    key,
  );
  push_key_debug_action(
    &mut actions,
    "Zoom Reset",
    &keybindings.zoom_reset,
    modifiers,
    key,
  );
  push_key_debug_action(
    &mut actions,
    "Next Tab",
    &keybindings.next_tab,
    modifiers,
    key,
  );
  push_key_debug_action(
    &mut actions,
    "Previous Tab",
    &keybindings.previous_tab,
    modifiers,
    key,
  );
  push_key_debug_action(
    &mut actions,
    "Toggle Search",
    &keybindings.toggle_search,
    modifiers,
    key,
  );
  if search_visible
    && key.eq_ignore_ascii_case("escape")
    && !actions.iter().any(|action| action == "Toggle Search")
  {
    actions.push("Toggle Search".to_string());
  }
  push_key_debug_action(
    &mut actions,
    "Split Horizontal",
    &keybindings.split_horizontal,
    modifiers,
    key,
  );
  push_key_debug_action(
    &mut actions,
    "Split Vertical",
    &keybindings.split_vertical,
    modifiers,
    key,
  );
  push_key_debug_action(
    &mut actions,
    "Close Pane",
    &keybindings.close_pane,
    modifiers,
    key,
  );
  push_key_debug_action(
    &mut actions,
    "Focus Next Pane",
    &keybindings.focus_next_pane,
    modifiers,
    key,
  );
  push_key_debug_action(
    &mut actions,
    "Focus Previous Pane",
    &keybindings.focus_previous_pane,
    modifiers,
    key,
  );
  push_key_debug_action(
    &mut actions,
    "Swap Split Panes",
    &keybindings.swap_split_panes,
    modifiers,
    key,
  );
  push_key_debug_action(
    &mut actions,
    "Toggle Fullscreen",
    &keybindings.toggle_fullscreen,
    modifiers,
    key,
  );
  push_key_debug_action(
    &mut actions,
    "Toggle Tab Bar",
    &keybindings.toggle_tab_bar,
    modifiers,
    key,
  );
  push_key_debug_action(
    &mut actions,
    "New Tab",
    &keybindings.new_tab,
    modifiers,
    key,
  );
  push_key_debug_action(
    &mut actions,
    "New Window",
    &keybindings.new_window,
    modifiers,
    key,
  );
  push_key_debug_action(&mut actions, "Quit", &keybindings.quit, modifiers, key);

  for (profile_name, binding) in config.get_local_profile_names().into_iter().zip([
    &keybindings.new_tab_profile_1,
    &keybindings.new_tab_profile_2,
    &keybindings.new_tab_profile_3,
    &keybindings.new_tab_profile_4,
    &keybindings.new_tab_profile_5,
    &keybindings.new_tab_profile_6,
    &keybindings.new_tab_profile_7,
    &keybindings.new_tab_profile_8,
    &keybindings.new_tab_profile_9,
  ]) {
    push_key_debug_action(
      &mut actions,
      format!("New Tab: {}", profile_name),
      binding,
      modifiers,
      key,
    );
  }

  if actions.is_empty() {
    None
  } else {
    Some(actions.join(" / "))
  }
}

fn collect_key_debug_entries(
  recent_keys: &[KeyDebugRecentKey],
  config: &::config::Config,
  modifiers: KeyDebugModifiers,
  pressed_keys: &[KeyDebugPressedKey],
) -> Vec<KeyDebugEntry> {
  let mut entries = recent_keys
    .iter()
    .map(|recent| KeyDebugEntry {
      raw_key: recent.raw_key.clone(),
      shortcut: recent.shortcut.clone(),
      action: recent
        .action
        .clone()
        .or_else(|| resolve_key_debug_action(config, recent.modifiers, &recent.raw_key, false)),
    })
    .collect::<Vec<_>>();

  for (raw_key, shortcut) in modifier_raw_labels(modifiers)
    .into_iter()
    .zip(modifier_labels(modifiers).into_iter())
  {
    entries.push(KeyDebugEntry {
      raw_key: raw_key.to_string(),
      shortcut: shortcut.to_string(),
      action: None,
    });
  }

  entries.extend(pressed_keys.iter().map(|pressed| {
    KeyDebugEntry {
      raw_key: pressed.raw_key.clone(),
      shortcut: format_pressed_shortcut(pressed.modifiers, &pressed.raw_key),
      action: pressed
        .action
        .clone()
        .or_else(|| resolve_key_debug_action(config, pressed.modifiers, &pressed.raw_key, false)),
    }
  }));

  entries.truncate(KEY_DEBUG_MAX_ROWS);
  entries
}

fn render_key_debug_overlay(
  entries: &[KeyDebugEntry],
  modifiers: KeyDebugModifiers,
  cx: &mut Context<MainWindow>,
) -> impl IntoElement {
  let theme = cx.theme().clone();
  let header = if modifiers.is_empty() {
    "Pressed Keys".to_string()
  } else {
    format!("Pressed Keys ({})", modifiers.display_text())
  };
  let empty_state = "Press and hold keys to inspect the current input state.";

  div()
    .absolute()
    .right(px(16.0))
    .bottom(px(16.0))
    .w(px(520.0))
    .p_2()
    .child(
      div()
        .flex()
        .flex_col()
        .items_end()
        .gap_1()
        .child(
          div()
            .text_size(px(11.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.muted_foreground)
            .child(header),
        )
        .when(entries.is_empty(), |this| {
          this.child(
            div()
              .text_size(px(11.0))
              .text_color(theme.muted_foreground)
              .child(empty_state),
          )
        })
        .when(!entries.is_empty(), |this| {
          this.children(entries.iter().map(|entry| {
            h_flex()
              .w_full()
              .gap_4()
              .child(
                div()
                  .w(px(90.0))
                  .text_size(px(13.0))
                  .text_color(theme.muted_foreground)
                  .child(entry.raw_key.clone()),
              )
              .child(
                div()
                  .w(px(170.0))
                  .text_size(px(15.0))
                  .text_color(theme.foreground)
                  .child(entry.shortcut.clone()),
              )
              .child(
                div()
                  .flex_1()
                  .text_size(px(12.0))
                  .text_color(theme.muted_foreground)
                  .child(entry.action.clone().unwrap_or_default()),
              )
          }))
        }),
    )
}

impl Render for MainWindow {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let search_visible = self.search_visible;
    let search_bar = self.search_bar.clone();
    let config = cx.global::<::config::Config>();
    let vertical_tabs = config.tab.vertical;
    let ui_font_size = config.font.ui_size;
    let tab_label_min_width = config.tab.get_label_min_width(ui_font_size);
    let tab_label_max_width = config.tab.get_label_max_width(ui_font_size);
    let vertical_tabbar_min_width = config.tab.get_vertical_tabbar_min_width(ui_font_size);
    let tab_bar_visible = self.tab_bar_visible;
    let setting_store = cx.global::<SettingsStore>();
    let local_profiles = config.get_local_profiles_with_shells();
    let container_profiles = config.get_container_profiles_with_shells();
    let ssh_hosts = ::config::Config::get_ssh_hosts();
    let new_tab_shortcut = config.keybindings.new_tab.display_text();
    let profile_shortcuts: Vec<String> = [
      &config.keybindings.new_tab_profile_1,
      &config.keybindings.new_tab_profile_2,
      &config.keybindings.new_tab_profile_3,
      &config.keybindings.new_tab_profile_4,
      &config.keybindings.new_tab_profile_5,
      &config.keybindings.new_tab_profile_6,
      &config.keybindings.new_tab_profile_7,
      &config.keybindings.new_tab_profile_8,
      &config.keybindings.new_tab_profile_9,
    ]
    .iter()
    .map(|binding| binding.display_text())
    .collect();
    let toggle_tab_bar_shortcut = config.keybindings.toggle_tab_bar.display_text();
    let key_debug_mode = config.window.key_debug_mode;
    let key_debug_entries = if key_debug_mode {
      collect_key_debug_entries(
        &self.key_debug_recent_keys,
        config,
        self.key_debug_modifiers,
        &self.key_debug_pressed_keys,
      )
    } else {
      Vec::new()
    };

    // Get current window bounds to detect resize
    let current_bounds = window.bounds();
    let bounds_changed = self.last_bounds != Some(current_bounds);

    if bounds_changed {
      self.last_bounds = Some(current_bounds);
      // Set flag to scroll active tab into view on resize
      self.scroll_to_active_tab = true;
    }

    if self.scroll_tabs_to_end {
      self.scroll_tabs_to_end = false;
      let scroll_handle = self.tab_scroll_handle.clone();
      cx.spawn(async move |_this, cx| {
        // Small delay to allow layout to complete
        // smol::Timer::after(std::time::Duration::from_millis(50)).await;
        cx.update(|_cx| {
          let max_offset = scroll_handle.max_offset();
          let offset = if vertical_tabs {
            gpui::point(px(0.0), -max_offset.height)
          } else {
            gpui::point(-max_offset.width, px(0.0))
          };
          scroll_handle.set_offset(offset);
        })
        .ok();
      })
      .detach();
    }

    if self.scroll_to_active_tab {
      self.scroll_to_active_tab = false;
      let scroll_handle = self.tab_scroll_handle.clone();
      let active_tab_ix = self.active_tab_ix.unwrap_or_default();
      let total_tabs = self.items.len();
      cx.spawn(async move |_this, cx| {
        cx.update(|_cx| {
          if total_tabs > 0 && active_tab_ix < total_tabs {
            // Calculate the approximate position of the active tab
            // This is a simple approach - scroll proportionally based on tab index
            let max_offset = scroll_handle.max_offset();
            let scroll_ratio = active_tab_ix as f32 / total_tabs.max(1) as f32;
            let offset = if vertical_tabs {
              gpui::point(px(0.0), -max_offset.height * scroll_ratio)
            } else {
              gpui::point(-max_offset.width * scroll_ratio, px(0.0))
            };
            scroll_handle.set_offset(offset);
          }
        })
        .ok();
      })
      .detach();
    }

    let view = cx.entity();
    let menu_view = view.clone();

    let colors = setting_store.theme().colors().clone();
    div()
      .flex()
      .flex_col()
      .size_full()
      .key_context("MainWindow")
      .on_key_down(cx.listener(move |this, e: &KeyDownEvent, window, cx| {
        let key_debug_modifiers = KeyDebugModifiers {
          control: e.keystroke.modifiers.control,
          shift: e.keystroke.modifiers.shift,
          alt: e.keystroke.modifiers.alt,
          platform: e.keystroke.modifiers.platform,
        };
        this.set_key_debug_modifiers(key_debug_modifiers, cx);
        this.press_key_debug_key(&e.keystroke.key, key_debug_modifiers, cx);
        let key_debug_action = resolve_key_debug_action(
          cx.global::<::config::Config>(),
          key_debug_modifiers,
          &e.keystroke.key,
          this.search_visible,
        );
        this.annotate_latest_key_debug_key(&e.keystroke.key, key_debug_action, cx);

        let keybindings = &cx.global::<config::Config>().keybindings;
        let mods = &e.keystroke.modifiers;
        let key = &e.keystroke.key;

        let kb_new_tab_profiles = [
          &keybindings.new_tab_profile_1,
          &keybindings.new_tab_profile_2,
          &keybindings.new_tab_profile_3,
          &keybindings.new_tab_profile_4,
          &keybindings.new_tab_profile_5,
          &keybindings.new_tab_profile_6,
          &keybindings.new_tab_profile_7,
          &keybindings.new_tab_profile_8,
          &keybindings.new_tab_profile_9,
        ];
        let kb_select_tabs = [
          &keybindings.select_tab_1,
          &keybindings.select_tab_2,
          &keybindings.select_tab_3,
          &keybindings.select_tab_4,
          &keybindings.select_tab_5,
          &keybindings.select_tab_6,
          &keybindings.select_tab_7,
          &keybindings.select_tab_8,
          &keybindings.select_last_tab,
        ];
        let tab_switcher_popup = cx.global::<config::Config>().tab.switcher_popup;

        let matched = if keybindings
          .next_tab
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          if tab_switcher_popup {
            this.show_tab_switcher(true, window, cx);
          } else {
            let current_ix = this.active_tab_ix.unwrap_or(0);
            let next_ix = (current_ix + 1) % this.items.len();
            this.set_active_tab(next_ix, window, cx);
          }
          true
        } else if keybindings
          .previous_tab
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          if tab_switcher_popup {
            this.show_tab_switcher(false, window, cx);
          } else {
            let current_ix = this.active_tab_ix.unwrap_or(0);
            let prev_ix = if current_ix == 0 {
              this.items.len() - 1
            } else {
              current_ix - 1
            };
            this.set_active_tab(prev_ix, window, cx);
          }
          true
        } else if keybindings
          .toggle_search
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
          || (e.keystroke.key == "Escape" && this.search_visible)
        {
          this.toggle_search(window, cx);
          true
        } else if keybindings
          .split_horizontal
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          this.split_pane_horizontal(window, cx);
          true
        } else if keybindings
          .split_vertical
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          this.split_pane_vertical(window, cx);
          true
        } else if keybindings
          .close_pane
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          this.close_active_pane(window, cx);
          true
        } else if keybindings
          .focus_next_pane
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          this.focus_next_pane(window, cx);
          true
        } else if keybindings
          .focus_previous_pane
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          this.focus_prev_pane(window, cx);
          true
        } else if keybindings
          .focus_pane_up
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          this.focus_pane_up(window, cx);
          true
        } else if keybindings
          .focus_pane_down
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          this.focus_pane_down(window, cx);
          true
        } else if keybindings
          .focus_pane_left
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          this.focus_pane_left(window, cx);
          true
        } else if keybindings
          .focus_pane_right
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          this.focus_pane_right(window, cx);
          true
        } else if keybindings
          .swap_split_panes
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          this.swap_split_panes(window, cx);
          true
        } else if keybindings
          .toggle_fullscreen
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          window.toggle_fullscreen();
          true
        } else if keybindings
          .toggle_tab_bar
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          this.toggle_tab_bar(cx);
          true
        } else if keybindings
          .new_tab
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          this.insert_new_tab(window, cx);
          true
        } else if let Some((i, _)) = kb_select_tabs.iter().enumerate().find(|(_, kb_select_tab)| {
          kb_select_tab.matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        }) {
          this.select_tab_by_shortcut(i + 1, window, cx);
          true
        } else if keybindings
          .quit
          .matches(mods.control, mods.shift, mods.alt, mods.platform, key)
        {
          this.show_close_confirm_dialog(window, cx);
          true
        } else {
          // Check profile-specific new tab shortcuts.
          let profiles = cx.global::<config::Config>().get_local_profile_names();
          let mut found = false;
          for (i, kb_profile) in kb_new_tab_profiles.iter().enumerate() {
            if kb_profile.matches(mods.control, mods.shift, mods.alt, mods.platform, key) {
              if let Some(profile_name) = profiles.get(i) {
                this.insert_new_tab_with_profile(Some(profile_name), None, window, cx);
              }
              found = true;
              break;
            }
          }
          found
        };

        if matched {
          cx.stop_propagation();
        }
      }))
      .on_key_up(cx.listener(move |this, e: &KeyUpEvent, _window, cx| {
        this.set_key_debug_modifiers(
          KeyDebugModifiers {
            control: e.keystroke.modifiers.control,
            shift: e.keystroke.modifiers.shift,
            alt: e.keystroke.modifiers.alt,
            platform: e.keystroke.modifiers.platform,
          },
          cx,
        );
        this.release_key_debug_key(&e.keystroke.key, cx);
      }))
      .on_modifiers_changed(cx.listener(
        move |this, e: &ModifiersChangedEvent, window, cx| {
          this.set_key_debug_modifiers(
            KeyDebugModifiers {
              control: e.modifiers.control,
              shift: e.modifiers.shift,
              alt: e.modifiers.alt,
              platform: e.modifiers.platform,
            },
            cx,
          );

          // Dismiss tab switcher when Ctrl is released
          if this.tab_switcher_visible && !e.modifiers.control {
            this.hide_tab_switcher(window, cx);
          }
        },
      ))
      .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _window, cx| {
        this.set_key_debug_modifiers(
          KeyDebugModifiers {
            control: e.modifiers.control,
            shift: e.modifiers.shift,
            alt: e.modifiers.alt,
            platform: e.modifiers.platform,
          },
          cx,
        );
      }))
      .on_mouse_down(
        MouseButton::Left,
        cx.listener(move |_this, _e: &MouseDownEvent, _window, _cx| {
          // No-op
        }),
      )
      .on_mouse_down(
        MouseButton::Right,
        cx.listener(move |_this, _e: &MouseDownEvent, _window, _cx| {
          // No-op
        }),
      )
      .child({
        let titlebar_content = div()
              .h_flex()
              .flex_1()
              .flex_basis(px(0.0))
              .min_w_0()
              .overflow_x_hidden()
              .when(!vertical_tabs && tab_bar_visible, |this| {
                this.child(
                  TerminalTabBar::new("tabs")
                    .track_scroll(&self.tab_scroll_handle)
                    .children(
                      self
                        .items
                        .iter()
                        .enumerate()
                        .map(|(tab_ix, item)| {
                          let shell_icon = ShellIcon::new(&item.shell_path);
                          let tab_index = item.index;
                          let tab_title = item.display_title().to_string();
                          let total_tabs = self.items.len();
                          let is_first = tab_ix == 0;
                          let is_last = tab_ix == total_tabs - 1;
                          let is_selected = self.active_tab_ix == Some(tab_ix);
                          let has_bell = item
                            .split_container
                            .all_terminals()
                            .iter()
                            .any(|(_, t)| t.read(cx).has_bell());
                          let view = cx.entity();
                          let view_for_click = view.clone();
                          let all_terminals = item.split_container.all_terminals();
                          // Define colors for selected tab highlight
                          let selected_bg: gpui::Hsla = colors.tab_active_background;
                          let normal_bg = colors.tab_inactive_background;
                          let hover_bg = colors.element_hover;
                          let text_color = colors.text;
                          let text_muted = colors.text_muted;
                          let accent_color = colors.text_accent;
                          let warning_color = colors.terminal_ansi_yellow;

                          TerminalTab::new()
                            .selected(is_selected)
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                              // Select this tab
                              view_for_click.update(cx, |this, cx| {
                                this.set_active_tab(tab_ix, window, cx);
                              });
                              // Clear bell when clicking on tab
                              for (_, terminal) in &all_terminals {
                                terminal.update(cx, |terminal_view, cx| {
                                  terminal_view.clear_bell(cx);
                                });
                              }
                              // Prevent TitleBar from starting window drag when clicking on tabs
                              cx.stop_propagation();
                            })
                            .child(
                              div()
                                .id(ElementId::NamedInteger(
                                  "tab-container".into(),
                                  tab_ix as u64,
                                ))
                                .group("tab-item")
                                .child(
                                  div()
                                    .id(ElementId::NamedInteger("tab-drag".into(), tab_ix as u64))
                                    .cursor(CursorStyle::OpenHand)
                                    .on_drag(
                                      DraggedTab {
                                        from_ix: tab_ix,
                                        title: tab_title.clone(),
                                        shell_path: item.shell_path.clone(),
                                      },
                                      |dragged: &DraggedTab, _offset, _window, cx| {
                                        cx.new(|_cx| {
                                          DraggedTabView::new(
                                            dragged.title.clone(),
                                            dragged.shell_path.clone(),
                                          )
                                        })
                                      },
                                    )
                                    .drag_over::<DraggedTab>(
                                      move |style, _dragged, _window, _cx| {
                                        // Visual feedback during drag - show drop indicator
                                        style
                                          .bg(accent_color.opacity(0.15))
                                          .border_l_2()
                                          .border_color(accent_color)
                                      },
                                    )
                                    .on_drop(cx.listener(
                                      move |this, dragged: &DraggedTab, _window, cx| {
                                        let from_ix = dragged.from_ix;
                                        let to_ix = tab_ix;
                                        if from_ix != to_ix {
                                          // Remove the item from the original position and insert at new position
                                          let item = this.items.remove(from_ix);
                                          this.items.insert(to_ix, item);
                                          // Update active tab index
                                          if let Some(active) = this.active_tab_ix {
                                            if active == from_ix {
                                              this.active_tab_ix = Some(to_ix);
                                            } else if from_ix < active && active <= to_ix {
                                              this.active_tab_ix = Some(active - 1);
                                            } else if to_ix <= active && active < from_ix {
                                              this.active_tab_ix = Some(active + 1);
                                            }
                                          }
                                          cx.notify();
                                        }
                                      },
                                    ))
                                    .child(
                                      h_flex()
                                        .id(ElementId::NamedInteger(
                                          "tab-inner".into(),
                                          tab_ix as u64,
                                        ))
                                        .mt_1()
                                        .h_full()
                                        .gap_1p5()
                                        .pl_2p5()
                                        .pr_1()
                                        .items_center()
                                        .min_w(px(tab_label_min_width))
                                        .max_w(px(tab_label_max_width))
                                        // Background styling
                                        .when(is_selected, |this| {
                                          this
                                            .bg(selected_bg)
                                            .border_b_2()
                                            .border_color(accent_color)
                                        })
                                        .when(!is_selected, |this| {
                                          this.bg(normal_bg).hover(|style| style.bg(hover_bg))
                                        })
                                        .rounded_t_md()
                                        // Shell icon
                                        .child(
                                          div()
                                            .flex_shrink_0()
                                            .child(shell_icon.into_element(px(14.0))),
                                        )
                                        // Bell indicator
                                        .when(has_bell, |this| {
                                          this.child(
                                            div().flex_shrink_0().child(
                                              Icon::new(IconName::Bell)
                                                .size_3()
                                                .text_color(warning_color),
                                            ),
                                          )
                                        })
                                        // Tab label with text truncation
                                        .child(
                                          div().flex_1().min_w_0().overflow_x_hidden().child(
                                            Label::new(tab_title.clone())
                                              .text_color(if is_selected {
                                                text_color
                                              } else {
                                                text_muted
                                              })
                                              .whitespace_nowrap(),
                                          ),
                                        )
                                        // Close button - visible on hover or when selected
                                        .child({
                                          let close_visible = is_selected;
                                          div()
                                            .flex_shrink_0()
                                            .when(!close_visible, |this| {
                                              this
                                                .invisible()
                                                .group_hover("tab-item", |style| style.visible())
                                            })
                                            .child(
                                              TabButton::new("close", tab_index)
                                                .visible(true)
                                                .on_click(cx.listener(
                                                  |this, e: &TabButtonClickEvent, window, cx| {
                                                    let tab_index = e.index;
                                                    this.remove_tab_by(tab_index, window, cx);
                                                  },
                                                )),
                                            )
                                        })
                                        .on_mouse_down(MouseButton::Right, |_, _, cx| {
                                          cx.stop_propagation();
                                        })
                                        .context_menu({
                                          let view = view.clone();
                                          move |menu, _window, _cx| {
                                            build_tab_context_menu(
                                              menu,
                                              view.clone(),
                                              tab_index,
                                              tab_ix,
                                              is_first,
                                              is_last,
                                              total_tabs,
                                              "Move Left",
                                              IconName::ArrowLeft,
                                              "Move Right",
                                              IconName::ArrowRight,
                                            )
                                          }
                                        }),
                                    ),
                                ),
                            )
                        })
                        .collect::<Vec<_>>(),
                    ),
                )
              })
              .child(
                h_flex()
                  .flex_shrink_0()
                  .gap_0()
                  .pl_1()
                  .child(
                    Button::new("toggle-tab-bar")
                      .ghost()
                      .small()
                      .icon(if tab_bar_visible {
                        IconName::PanelLeftClose
                      } else {
                        IconName::PanelLeftOpen
                      })
                      .tooltip(if tab_bar_visible {
                        format!("Hide Tab Bar ({})", toggle_tab_bar_shortcut)
                      } else {
                        format!("Show Tab Bar ({})", toggle_tab_bar_shortcut)
                      })
                      .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                      })
                      .on_click(cx.listener(|this, _e, _window, cx| {
                        this.toggle_tab_bar(cx);
                      })),
                  )
                  .child(
                    Button::new("new")
                      .ghost()
                      .small()
                      .icon(IconName::Plus)
                      .tooltip(format!("New Tab ({})", new_tab_shortcut))
                      .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                      })
                      .on_click(cx.listener(|this, _e, window, cx| {
                        this.insert_new_tab(window, cx);
                      })),
                  )
                  .child(
                    Button::new("more")
                      .ghost()
                      .small()
                      .icon(IconName::ChevronDown)
                      .dropdown_menu({
                        let view = menu_view.clone();
                        let profile_shortcuts = profile_shortcuts.clone();
                        move |menu: PopupMenu,
                              _window: &mut Window,
                              _cx: &mut Context<PopupMenu>| {
                          build_new_tab_menu(
                            menu,
                            view.clone(),
                            &local_profiles,
                            &container_profiles,
                            &ssh_hosts,
                            &profile_shortcuts,
                          )
                        }
                      }),
                  ),
              );

        if window.is_fullscreen() {
          div()
            .flex_shrink_0()
            .id("title-bar")
            .flex()
            .flex_row()
            .items_center()
            .h(TITLE_BAR_HEIGHT)
            .pl_3()
            .border_b_1()
            .border_color(cx.theme().title_bar_border)
            .bg(cx.theme().title_bar)
            .child(titlebar_content)
            .into_any_element()
        } else {
          TitleBar::new()
            .on_close_window({
              let main_window = view.clone();
              move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                main_window.update(cx, |this, cx| {
                  this.show_close_confirm_dialog(window, cx);
                });
              }
            })
            .child(titlebar_content)
            .into_any_element()
        }
      })
      .child({
        let active_ix = self.active_tab_ix.unwrap_or_default();
        // Sync active pane from OS focus so the inactive-pane dimming
        // reflects the currently focused terminal immediately.
        self.sync_active_pane_from_focus(window, cx);
        let content = div()
          .flex_1()
          .size_full()
          .child(
            self
              .items
              .get(active_ix)
              .map(|item| item.split_container.render(window, cx))
              .unwrap_or_else(|| {
                tracing::warn!(
                  "render: NO ITEM FOUND at index {}, showing empty div",
                  active_ix
                );
                div().into_any_element()
              }),
          )
          .when(key_debug_mode, |this| {
            this.child(render_key_debug_overlay(&key_debug_entries, self.key_debug_modifiers, cx))
          })
          .when(search_visible, |this| this.child(search_bar))
          .when(self.tab_switcher_visible, |this| {
            if let Some(tab_switcher) = &self.tab_switcher {
              this.child(tab_switcher.clone())
            } else {
              this
            }
          })
          .when(self.rename_dialog.is_some(), |this| {
            if let Some(rename_dialog) = &self.rename_dialog {
              this.child(rename_dialog.clone())
            } else {
              this
            }
          })
          .when(self.close_confirm_dialog.is_some(), |this| {
            if let Some(close_confirm_dialog) = &self.close_confirm_dialog {
              this.child(close_confirm_dialog.clone())
            } else {
              this
            }
          })
          .when(self.about_dialog.is_some(), |this| {
            if let Some(about_dialog) = &self.about_dialog {
              this.child(about_dialog.clone())
            } else {
              this
            }
          })
          .when(self.import_alacritty_dialog.is_some(), |this| {
            if let Some(import_dialog) = &self.import_alacritty_dialog {
              this.child(import_dialog.clone())
            } else {
              this
            }
          })
          .when(self.shell_error_dialog.is_some(), |this| {
            if let Some(shell_error_dialog) = &self.shell_error_dialog {
              this.child(shell_error_dialog.clone())
            } else {
              this
            }
          });

        if vertical_tabs {
          h_flex()
            .flex_1()
            .size_full()
            .when(tab_bar_visible, |this| {
              this
                .child(
                  div()
                    .h_full()
                    .flex_shrink_0()
                    .w(self.vertical_tabbar_width)
                    .bg(colors.title_bar_background)
                    .p_1()
                    .child(
                      TerminalTabBar::new("tabs-vertical")
                        .vertical(true)
                        .track_scroll(&self.tab_scroll_handle)
                        .children(
                          self
                            .items
                            .iter()
                            .enumerate()
                            .map(|(tab_ix, item)| {
                              let shell_icon = ShellIcon::new(&item.shell_path);
                              let tab_index = item.index;
                              let tab_title = item.display_title().to_string();
                              let total_tabs = self.items.len();
                              let is_first = tab_ix == 0;
                              let is_last = tab_ix == total_tabs - 1;
                              let is_selected = self.active_tab_ix == Some(tab_ix);
                              let has_bell = item
                                .split_container
                                .all_terminals()
                                .iter()
                                .any(|(_, t)| t.read(cx).has_bell());
                              let view = cx.entity();
                              let view_for_click = view.clone();
                              let all_terminals = item.split_container.all_terminals();
                              let selected_bg: gpui::Hsla = colors.tab_active_background;
                              let normal_bg = colors.tab_inactive_background;
                              let hover_bg = colors.element_hover;
                              let text_color = colors.text;
                              let text_muted = colors.text_muted;
                              let accent_color = colors.text_accent;
                              let warning_color = colors.terminal_ansi_yellow;

                              TerminalTab::new()
                                .selected(is_selected)
                                .fill_height(false)
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                  view_for_click.update(cx, |this, cx| {
                                    this.set_active_tab(tab_ix, window, cx);
                                  });
                                  for (_, terminal) in &all_terminals {
                                    terminal.update(cx, |terminal_view, cx| {
                                      terminal_view.clear_bell(cx);
                                    });
                                  }
                                  cx.stop_propagation();
                                })
                                .child(
                                  div()
                                    .id(ElementId::NamedInteger(
                                      "tab-container".into(),
                                      tab_ix as u64,
                                    ))
                                    .group("tab-item")
                                    .w_full()
                                    .child(
                                      div()
                                        .id(ElementId::NamedInteger("tab-drag".into(), tab_ix as u64))
                                        .w_full()
                                        .cursor(CursorStyle::OpenHand)
                                        .on_drag(
                                          DraggedTab {
                                            from_ix: tab_ix,
                                            title: tab_title.clone(),
                                            shell_path: item.shell_path.clone(),
                                          },
                                          |dragged: &DraggedTab, _offset, _window, cx| {
                                            cx.new(|_cx| {
                                              DraggedTabView::new(
                                                dragged.title.clone(),
                                                dragged.shell_path.clone(),
                                              )
                                            })
                                          },
                                        )
                                        .drag_over::<DraggedTab>(
                                          move |style, _dragged, _window, _cx| {
                                            style
                                              .bg(accent_color.opacity(0.15))
                                              .border_l_2()
                                              .border_color(accent_color)
                                          },
                                        )
                                        .on_drop(cx.listener(
                                          move |this, dragged: &DraggedTab, _window, cx| {
                                            let from_ix = dragged.from_ix;
                                            let to_ix = tab_ix;
                                            if from_ix != to_ix {
                                              let item = this.items.remove(from_ix);
                                              this.items.insert(to_ix, item);
                                              if let Some(active) = this.active_tab_ix {
                                                if active == from_ix {
                                                  this.active_tab_ix = Some(to_ix);
                                                } else if from_ix < active && active <= to_ix {
                                                  this.active_tab_ix = Some(active - 1);
                                                } else if to_ix <= active && active < from_ix {
                                                  this.active_tab_ix = Some(active + 1);
                                                }
                                              }
                                              cx.notify();
                                            }
                                          },
                                        ))
                                        .child(
                                          h_flex()
                                            .w_full()
                                            .gap_1p5()
                                            .pl_2p5()
                                            .pr_1()
                                            .py_1()
                                            .items_center()
                                            .when(is_selected, |this| {
                                              this
                                                .bg(selected_bg)
                                                .border_l_2()
                                                .border_color(accent_color)
                                            })
                                            .when(!is_selected, |this| {
                                              this.bg(normal_bg).hover(|style| style.bg(hover_bg))
                                            })
                                            .rounded_md()
                                            .child(
                                              div()
                                                .flex_shrink_0()
                                                .child(shell_icon.into_element(px(14.0))),
                                            )
                                            .when(has_bell, |this| {
                                              this.child(
                                                div().flex_shrink_0().child(
                                                  Icon::new(IconName::Bell)
                                                    .size_3()
                                                    .text_color(warning_color),
                                                ),
                                              )
                                            })
                                            .child(
                                              div().flex_1().min_w_0().overflow_x_hidden().child(
                                                Label::new(tab_title.clone())
                                                  .text_color(if is_selected {
                                                    text_color
                                                  } else {
                                                    text_muted
                                                  })
                                                  .whitespace_nowrap(),
                                              ),
                                            )
                                            .child({
                                              let close_visible = is_selected;
                                              div()
                                                .flex_shrink_0()
                                                .when(!close_visible, |this| {
                                                  this
                                                    .invisible()
                                                    .group_hover("tab-item", |style| style.visible())
                                                })
                                                .child(
                                                  TabButton::new("close-vertical", tab_index)
                                                    .visible(true)
                                                    .on_click(cx.listener(
                                                      |this, e: &TabButtonClickEvent, window, cx| {
                                                        this.remove_tab_by(e.index, window, cx);
                                                      },
                                                    )),
                                                )
                                            })
                                            .on_mouse_down(MouseButton::Right, |_, _, cx| {
                                              cx.stop_propagation();
                                            })
                                            .context_menu({
                                              let view = view.clone();
                                              move |menu, _window, _cx| {
                                                build_tab_context_menu(
                                                  menu,
                                                  view.clone(),
                                                  tab_index,
                                                  tab_ix,
                                                  is_first,
                                                  is_last,
                                                  total_tabs,
                                                  "Move Up",
                                                  IconName::ArrowUp,
                                                  "Move Down",
                                                  IconName::ArrowDown,
                                                )
                                              }
                                            }),
                                        ),
                                    ),
                                )
                            })
                            .collect::<Vec<_>>(),
                        ),
                    ),
                )
                .child(
                  div()
                    .id("vertical-tabbar-resize-handle")
                    .h_full()
                    .w(px(6.0))
                    .cursor(CursorStyle::ResizeLeftRight)
                    .on_drag_move(cx.listener(
                      move |this, e: &DragMoveEvent<ResizeVerticalTabbar>, window, cx| {
                        let ResizeVerticalTabbar(entity_id) = e.drag(cx);
                        if cx.entity_id() != *entity_id {
                          return;
                        }

                        let min_width = px(vertical_tabbar_min_width);
                        let max_width = (window.bounds().size.width - px(160.0)).max(min_width);
                        let next_width = e.event.position.x.max(min_width).min(max_width);
                        if this.vertical_tabbar_width != next_width {
                          this.vertical_tabbar_width = next_width;
                          cx.notify();
                        }
                      },
                    ))
                    .on_drag(ResizeVerticalTabbar(cx.entity_id()), |drag, _, _, cx| {
                      cx.stop_propagation();
                      cx.new(|_| drag.clone())
                    })
                    .bg(colors.border),
                )
            })
            .child(content)
            .into_any_element()
        } else {
          content.into_any_element()
        }
      })
  }
}
