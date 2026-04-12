// Disable command line from opening on release mode
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use gpui::{
  App, AppContext, Application, MenuItem, Point, Size, WindowBackgroundAppearance, WindowOptions,
  actions, px,
};
use themeing::SettingsStore;

use crate::assets::Assets;
use crate::event_system::EventSourceConfig;
use ::config::Config;

mod app_icon;
mod assets;
mod components;
mod config;
mod config_watcher;
pub mod event_system;

actions!(kazeterm, [NewWindow]);

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
  if let Some(ref themes_path) = config.themes_path {
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

/// Detect system dark mode preference
/// TODO: Implement proper system detection for each platform
fn detect_system_dark_mode() -> bool {
  #[cfg(target_os = "windows")]
  {
    // Check Windows registry for dark mode setting
    use windows::Win32::System::Registry::{
      HKEY_CURRENT_USER, KEY_READ, REG_DWORD, RegOpenKeyExW, RegQueryValueExW,
    };
    use windows::core::w;

    unsafe {
      let mut key = HKEY_CURRENT_USER;
      let subkey = w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
      let value_name = w!("AppsUseLightTheme");

      if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, Some(0), KEY_READ, &mut key).is_ok() {
        let mut data: u32 = 1;
        let mut data_size = std::mem::size_of::<u32>() as u32;
        let mut value_type = REG_DWORD;

        if RegQueryValueExW(
          key,
          value_name,
          None,
          Some(&mut value_type),
          Some(&mut data as *mut u32 as *mut u8),
          Some(&mut data_size),
        )
        .is_ok()
        {
          // If AppsUseLightTheme is 0, dark mode is enabled
          return data == 0;
        }
      }
    }
    true // Default to dark mode
  }
  #[cfg(not(target_os = "windows"))]
  {
    true // Default to dark mode on other platforms
  }
}

/// Open a new Kazeterm window using the current global config.
fn open_kazeterm_window(event_source_config: EventSourceConfig, cx: &mut App) {
  let config = cx.global::<Config>().clone();
  let window_width = config.window_width;
  let window_height = config.window_height;
  let start_maximized = config.start_maximized;
  let background_opacity = config.get_background_opacity();
  let background_blur = config.background_blur;

  cx.spawn(async move |cx| {
    let window_background = if background_opacity < 1.0 {
      if background_blur {
        WindowBackgroundAppearance::Blurred
      } else {
        WindowBackgroundAppearance::Transparent
      }
    } else {
      WindowBackgroundAppearance::Opaque
    };

    let restore_bounds = gpui::Bounds {
      origin: Point {
        x: px(100f32),
        y: px(100f32),
      },
      size: Size {
        width: px(window_width),
        height: px(window_height),
      },
    };

    let options = WindowOptions {
      window_bounds: Some(if start_maximized {
        gpui::WindowBounds::Maximized(restore_bounds)
      } else {
        gpui::WindowBounds::Windowed(restore_bounds)
      }),
      titlebar: Some(gpui::TitlebarOptions {
        title: Some("Kazeterm".into()),
        appears_transparent: true,
        traffic_light_position: Some(gpui::point(px(9.0), px(9.0))),
      }),
      window_decorations: Some(gpui::WindowDecorations::Client),
      window_background,
      app_id: Some("kazeterm".into()),
      ..Default::default()
    };

    let event_config = event_source_config;
    cx.open_window(options, |window, cx| {
      let view = crate::components::MainWindow::view(window, cx);
      let window_handle = window.window_handle();

      // Set X11 window icon from embedded PNG
      #[cfg(target_os = "linux")]
      app_icon::set_x11_window_icon(window);

      // Initialize the event system with a weak reference to the main window
      let main_window_weak = view.downgrade();
      let event_config_clone = event_config.clone();
      cx.defer(move |cx| {
        crate::event_system::start_event_system(
          main_window_weak,
          window_handle,
          event_config_clone,
          cx,
        );
      });

      cx.new(|cx| gpui_component::Root::new(view, window, cx))
    })?;

    Ok::<_, anyhow::Error>(())
  })
  .detach();
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

  let config = Config::load();

  // Initialize theme system with embedded assets and custom path
  init_theme_system(&config);

  let app = Application::new().with_assets(Assets);

  app.run(move |cx: &mut App| {
    Assets.load_fonts(cx).unwrap();
    gpui_component::init(cx);
    terminal::init(cx, &config.keybindings);

    cx.set_global(crate::config::create_settings_store(
      &config,
      detect_system_dark_mode(),
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
        open_kazeterm_window(event_config.clone(), cx);
      });
    }

    // Set macOS dock menu (long-press on dock icon)
    #[cfg(target_os = "macos")]
    cx.set_dock_menu(vec![MenuItem::action("New Window", NewWindow)]);

    open_kazeterm_window(event_source_config.clone(), cx);
  });
}
