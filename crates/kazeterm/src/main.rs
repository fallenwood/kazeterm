// Disable command line from opening on release mode
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(any(
  target_os = "macos",
  all(target_os = "windows", target_arch = "aarch64")
)))]
use mimalloc::MiMalloc;

#[cfg(not(any(
  target_os = "macos",
  all(target_os = "windows", target_arch = "aarch64")
)))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use gpui::{App, Application, KeyBinding, WindowAppearance, actions};
#[cfg(target_os = "macos")]
use gpui::{Menu, MenuItem};
use themeing::SettingsStore;

use crate::assets::Assets;
use crate::event_system::EventSourceConfig;
use ::config::Config;

mod app_icon;
mod assets;
mod auto_update;
mod build_info;
mod components;
mod config;
mod config_watcher;
pub mod event_system;
pub mod reconciler;
mod window_manager;

#[cfg(test)]
mod test_support;

actions!(
  kazeterm,
  [NewWindow, Quit, Hide, HideOthers, ShowAll, Minimize, Zoom]
);

/// Command-line arguments for Kazeterm
#[derive(Parser, Debug)]
#[command(name = "kazeterm")]
#[command(about = "A modern GPU-accelerated terminal emulator")]
#[command(version)]
struct Args {
  /// Enable the event system with the specified source
  #[arg(long, value_enum)]
  event_source: Option<EventSource>,

  /// Path to the event socket/pipe (required when event-source is "socket")
  #[arg(long)]
  event_socket: Option<PathBuf>,
}

/// Event source type for command-line parsing
#[derive(Debug, Clone, Copy, ValueEnum)]
enum EventSource {
  /// Read events from stdin (JSON, one per line)
  Stdio,
  /// Read events from a Unix domain socket (all platforms)
  Socket,
}

impl Args {
  /// Convert command-line arguments to EventSourceConfig
  fn to_event_source_config(&self) -> EventSourceConfig {
    match self.event_source {
      None => EventSourceConfig::None,
      Some(EventSource::Stdio) => EventSourceConfig::Stdio,
      Some(EventSource::Socket) => {
        if let Some(path) = &self.event_socket {
          EventSourceConfig::Socket { path: path.clone() }
        } else {
          tracing::warn!(
            "--event-socket is required when using socket event source, falling back to no events"
          );
          EventSourceConfig::None
        }
      }
    }
  }
}

/// Initialize theme system with embedded assets and custom path from config
fn init_theme_system(config: &Config) {
  use std::path::PathBuf;

  // Register embedded theme loader and lister
  ::config::register_embedded_theme_loader(crate::assets::embedded_theme_loader);
  ::config::register_embedded_theme_lister(crate::assets::embedded_theme_lister);

  // Set custom themes path if configured
  if let Some(ref themes_path) = config.appearance.themes_path {
    let path = PathBuf::from(themes_path);
    if path.exists() && path.is_dir() {
      tracing::info!("Using custom themes path: {}", path.display());
      ::config::set_custom_themes_path(path);
    } else {
      tracing::warn!(
        "Custom themes path does not exist or is not a directory: {}",
        themes_path
      );
    }
  } else {
    // Default themes path: ~/.config/kazeterm/themes/ (Linux) or %APPDATA%/kazeterm/themes/ (Windows)
    #[cfg(target_os = "windows")]
    {
      if let Some(app_data) = dirs::data_dir() {
        let default_themes_path = app_data.join("kazeterm").join("themes");
        if default_themes_path.exists() && default_themes_path.is_dir() {
          tracing::debug!(
            "Using default themes path: {}",
            default_themes_path.display()
          );
          ::config::set_custom_themes_path(default_themes_path);
        }
      }
    }

    #[cfg(not(target_os = "windows"))]
    {
      if let Some(home_dir) = dirs::home_dir() {
        let default_themes_path = home_dir.join(".config").join("kazeterm").join("themes");
        if default_themes_path.exists() && default_themes_path.is_dir() {
          tracing::debug!(
            "Using default themes path: {}",
            default_themes_path.display()
          );
          ::config::set_custom_themes_path(default_themes_path);
        }
      }
    }
  }
}

/// Detect system dark mode preference using GPUI's cross-platform appearance API.
pub(crate) fn system_is_dark(cx: &App) -> bool {
  matches!(
    cx.window_appearance(),
    WindowAppearance::Dark | WindowAppearance::VibrantDark
  )
}

fn main() {
  // Parse command-line arguments
  let args = Args::parse();
  let event_source_config = args.to_event_source_config();

  // Initialize tracing
  tracing_subscriber::fmt()
    .with_env_filter(
      tracing_subscriber::EnvFilter::from_default_env().add_directive(tracing::Level::WARN.into()),
    )
    .init();

  let config = match Config::load() {
    Ok(config) => config,
    Err(error) => {
      tracing::error!("Failed to load config: {error}");
      eprintln!("Failed to load config: {error}");
      std::process::exit(1);
    }
  };

  // Initialize theme system with embedded assets and custom path
  init_theme_system(&config);

  let app = Application::new().with_assets(Assets);

  // On macOS, clicking the dock icon while no windows are open should open a new window.
  // `on_reopen` is invoked by `applicationShouldHandleReopen:hasVisibleWindows:`.
  {
    let reopen_event_config = event_source_config.clone();
    app.on_reopen(move |cx| {
      if cx.windows().is_empty() {
        window_manager::open_kazeterm_window(reopen_event_config.clone(), cx);
      }
    });
  }

  app.run(move |cx: &mut App| {
    Assets.load_fonts(cx).unwrap();
    gpui_component::init(cx);
    terminal::init(cx, &config.keybindings);

    cx.set_global(crate::config::create_settings_store(
      &config,
      system_is_dark(cx),
    ));
    cx.set_global(config.clone());

    SettingsStore::init_gpui_component_theme(cx);

    // Start config and theme hot reload watcher
    config_watcher::start_config_watcher(cx);

    // Set macOS Dock icon from embedded PNG
    #[cfg(target_os = "macos")]
    app_icon::set_macos_app_icon();

    // Install icon + .desktop file so Wayland compositors and X11 WMs can
    // resolve the app icon from the app_id / WM_CLASS.
    #[cfg(target_os = "linux")]
    app_icon::install_linux_desktop_icon();

    // Register global dock menu actions
    {
      let event_config = event_source_config.clone();
      cx.on_action(move |_: &NewWindow, cx: &mut App| {
        window_manager::open_kazeterm_window(event_config.clone(), cx);
      });
    }
    cx.on_action(|_: &Quit, cx: &mut App| {
      cx.quit();
    });

    // Register new-window keybinding from config (all platforms)
    {
      let keybindings = &config.keybindings;
      let mut bindings: Vec<KeyBinding> = Vec::new();
      bindings.extend(
        keybindings
          .new_window
          .iter()
          .map(|binding| KeyBinding::new(binding, NewWindow, None)),
      );
      cx.bind_keys(bindings);
    }

    // Register macOS system actions
    #[cfg(target_os = "macos")]
    {
      cx.on_action(|_: &Hide, cx: &mut App| cx.hide());
      cx.on_action(|_: &HideOthers, cx: &mut App| cx.hide_other_apps());
      cx.on_action(|_: &ShowAll, cx: &mut App| cx.unhide_other_apps());
      cx.on_action(|_: &Minimize, cx: &mut App| {
        if let Some(window) = cx.active_window() {
          window
            .update(cx, |_, window, _cx| {
              window.minimize_window();
            })
            .ok();
        }
      });
      cx.on_action(|_: &Zoom, cx: &mut App| {
        if let Some(window) = cx.active_window() {
          window
            .update(cx, |_, window, _cx| {
              window.zoom_window();
            })
            .ok();
        }
      });

      cx.bind_keys([
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("cmd-alt-h", HideOthers, None),
        KeyBinding::new("cmd-m", Minimize, None),
      ]);

      cx.set_menus(vec![
        Menu {
          name: "Kazeterm".into(),
          items: vec![
            MenuItem::os_submenu("Services", gpui::SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Hide Kazeterm", Hide),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAll),
            MenuItem::separator(),
            MenuItem::action("Quit Kazeterm", Quit),
          ],
        },
        Menu {
          name: "Window".into(),
          items: vec![
            MenuItem::action("Minimize", Minimize),
            MenuItem::action("Zoom", Zoom),
            MenuItem::separator(),
            MenuItem::action("New Window", NewWindow),
          ],
        },
      ]);
    }

    // Set macOS dock menu (long-press on dock icon)
    #[cfg(target_os = "macos")]
    cx.set_dock_menu(vec![
      MenuItem::action("New Window", NewWindow),
      MenuItem::action("Quit", Quit),
    ]);

    window_manager::open_kazeterm_window(event_source_config.clone(), cx);
  });
}
