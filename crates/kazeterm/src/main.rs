// Disable command line from opening on release mode
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gpui::{App, AppContext, Application, Point, Size, WindowOptions, px};
use themeing::SettingsStore;

use crate::assets::Assets;
use ::config::Config;

mod assets;
mod components;
mod config;

/// Detect system dark mode preference
/// TODO: Implement proper system detection for each platform
fn detect_system_dark_mode() -> bool {
  #[cfg(target_os = "windows")]
  {
    // Check Windows registry for dark mode setting
    use std::process::Command;
    if let Ok(output) = Command::new("reg")
      .args([
        "query",
        "HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
        "/v",
        "AppsUseLightTheme",
      ])
      .output()
    {
      let stdout = String::from_utf8_lossy(&output.stdout);
      // If AppsUseLightTheme is 0, dark mode is enabled
      return stdout.contains("0x0");
    }
    true // Default to dark mode
  }
  #[cfg(not(target_os = "windows"))]
  {
    true // Default to dark mode on other platforms
  }
}

fn main() {
  // Initialize tracing
  tracing_subscriber::fmt()
    .with_env_filter(
      tracing_subscriber::EnvFilter::from_default_env()
        .add_directive(tracing::Level::INFO.into())
    )
    .init();

  let config = Config::load();

  let app = Application::new().with_assets(Assets);

  app.run(move |cx: &mut App| {
    Assets.load_fonts(cx).unwrap();
    gpui_component::init(cx);
    terminal::init(cx);

    cx.set_global(crate::config::create_settings_store(&config, detect_system_dark_mode()));
    cx.set_global(config.clone());

    SettingsStore::init_gpui_component_theme(cx);

    let window_width = config.window_width;
    let window_height = config.window_height;

    cx.spawn(async move |cx| {
      let mut options = WindowOptions::default();
      options.window_bounds = Some(gpui::WindowBounds::Windowed(gpui::Bounds {
        origin: Point {
          x: px(100f32),
          y: px(100f32),
        },
        size: Size {
          width: px(window_width),
          height: px(window_height),
        },
      }));

      // Hide system titlebar for custom window control
      options.titlebar = Some(gpui::TitlebarOptions {
        title: None,
        appears_transparent: true,
        traffic_light_position: Some(gpui::point(px(9.0), px(9.0))),
      });
      options.window_decorations = Some(gpui::WindowDecorations::Client);

      cx.open_window(options, |window, cx| {
        let view = crate::components::MainWindow::view(window, cx);
        cx.new(|cx| gpui_component::Root::new(view, window, cx))
      })?;

      Ok::<_, anyhow::Error>(())
    })
    .detach();
  });
}


