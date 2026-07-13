use std::cell::Cell;
use std::rc::Rc;

use gpui::{
  AnyWindowHandle, App, AppContext, Bounds, Entity, Global, Pixels, Point, Size, WeakEntity,
  Window, WindowBackgroundAppearance, WindowBounds, WindowOptions, point, px,
};

use crate::components::{DraggedTab, MainWindow};
use crate::event_system::EventSourceConfig;
use ::config::Config;

#[derive(Clone)]
struct RegisteredWindow {
  handle: AnyWindowHandle,
  view: WeakEntity<MainWindow>,
}

#[derive(Default)]
struct WindowRegistry {
  windows: Vec<RegisteredWindow>,
}

impl Global for WindowRegistry {}

pub(crate) fn open_kazeterm_window(event_source_config: EventSourceConfig, cx: &mut App) {
  let options = window_options(cx.global::<Config>(), None);

  cx.spawn(async move |cx| {
    cx.open_window(options, |window, cx| {
      let view = MainWindow::view_with_event_source(window, event_source_config.clone(), cx);
      initialize_window(&view, event_source_config, window, cx);
      cx.new(|cx| gpui_component::Root::new(view, window, cx))
    })?;

    Ok::<_, anyhow::Error>(())
  })
  .detach();
}

pub(crate) fn open_detached_tab_window(dragged: DraggedTab, bounds: Bounds<Pixels>, cx: &mut App) {
  let event_source_config = dragged
    .source
    .upgrade()
    .map(|source| source.read(cx).event_source_config.clone())
    .unwrap_or_default();
  let options = window_options(cx.global::<Config>(), Some(bounds));
  let accepted = Rc::new(Cell::new(false));
  let accepted_in_window = accepted.clone();

  let result = cx.open_window(options, move |window, cx| {
    let view = MainWindow::empty_view_with_event_source(window, event_source_config.clone(), cx);
    let did_accept = view.update(cx, |main_window, cx| {
      main_window.receive_claimed_tab(&dragged, None, window, cx)
    });
    accepted_in_window.set(did_accept);

    if did_accept {
      initialize_window(&view, event_source_config, window, cx);
    }
    cx.new(|cx| gpui_component::Root::new(view, window, cx))
  });

  match result {
    Ok(window_handle) if !accepted.get() => {
      tracing::warn!("Could not detach tab because its source tab no longer exists");
      let _ = cx.update_window(window_handle.into(), |_root, window, _cx| {
        window.remove_window();
      });
    }
    Ok(_) => {}
    Err(error) => {
      tracing::error!("Failed to open detached tab window: {error}");
    }
  }
}

pub(crate) fn drop_tab_on_existing_window(
  dragged: &DraggedTab,
  screen_position: Point<Pixels>,
  cx: &mut App,
) -> bool {
  for registered in registered_windows_front_to_back(cx) {
    if registered.handle == dragged.source_window {
      continue;
    }
    let contains_cursor = cx
      .update_window(registered.handle, |_root, window, _cx| {
        window.bounds().contains(&screen_position)
      })
      .unwrap_or(false);
    if !contains_cursor {
      continue;
    }

    let accepted = cx
      .update_window(registered.handle, |_root, window, cx| {
        registered
          .view
          .update(cx, |main_window, cx| {
            main_window.receive_claimed_tab(dragged, None, window, cx)
          })
          .unwrap_or(false)
      })
      .unwrap_or(false);
    if accepted {
      return true;
    }
  }

  false
}

fn registered_windows_front_to_back(cx: &App) -> Vec<RegisteredWindow> {
  let mut registered = cx
    .try_global::<WindowRegistry>()
    .map(|registry| registry.windows.clone())
    .unwrap_or_default();

  let Some(window_stack) = cx.window_stack() else {
    registered.reverse();
    return registered;
  };

  let mut ordered = Vec::with_capacity(registered.len());
  for handle in window_stack {
    if let Some(ix) = registered
      .iter()
      .position(|registered| registered.handle == handle)
    {
      ordered.push(registered.remove(ix));
    }
  }

  registered.reverse();
  ordered.extend(registered);
  ordered
}

pub(crate) fn mark_window_active(handle: AnyWindowHandle, cx: &mut App) {
  if cx.try_global::<WindowRegistry>().is_none() {
    return;
  }

  let registry = cx.global_mut::<WindowRegistry>();
  if let Some(ix) = registry
    .windows
    .iter()
    .position(|registered| registered.handle == handle)
  {
    let registered = registry.windows.remove(ix);
    registry.windows.push(registered);
  }
}

pub(crate) fn close_window(window: &mut Window, cx: &mut App) {
  let current_window = window.window_handle();
  let has_other_windows = cx
    .windows()
    .into_iter()
    .any(|handle| handle != current_window);

  window.remove_window();
  if !has_other_windows {
    cx.quit();
  }
}

fn window_options(config: &Config, detached_bounds: Option<Bounds<Pixels>>) -> WindowOptions {
  let background_opacity = config.appearance.get_background_opacity();
  let window_background = if background_opacity < 1.0 {
    if config.appearance.background_blur {
      WindowBackgroundAppearance::Blurred
    } else {
      WindowBackgroundAppearance::Transparent
    }
  } else {
    WindowBackgroundAppearance::Opaque
  };

  let default_bounds = Bounds {
    origin: Point {
      x: px(100.0),
      y: px(100.0),
    },
    size: Size {
      width: px(config.window.width),
      height: px(config.window.height),
    },
  };
  let window_bounds = if let Some(bounds) = detached_bounds {
    WindowBounds::Windowed(bounds)
  } else if config.window.start_maximized {
    WindowBounds::Maximized(default_bounds)
  } else {
    WindowBounds::Windowed(default_bounds)
  };

  WindowOptions {
    window_bounds: Some(window_bounds),
    titlebar: Some(gpui::TitlebarOptions {
      title: Some("Kazeterm".into()),
      appears_transparent: true,
      traffic_light_position: Some(point(px(9.0), px(9.0))),
    }),
    window_decorations: Some(gpui::WindowDecorations::Client),
    window_background,
    app_id: Some("kazeterm".into()),
    ..Default::default()
  }
}

fn initialize_window(
  view: &Entity<MainWindow>,
  event_source_config: EventSourceConfig,
  window: &mut Window,
  cx: &mut App,
) {
  let window_handle = window.window_handle();
  register_window(window_handle, view, cx);

  #[cfg(target_os = "linux")]
  crate::app_icon::set_x11_window_icon(window);

  let main_window_weak = view.downgrade();
  cx.defer(move |cx| {
    crate::event_system::start_event_system(
      main_window_weak,
      window_handle,
      event_source_config,
      cx,
    );
  });

  let main_window_weak = view.downgrade();
  cx.defer(move |cx| {
    crate::auto_update::start_auto_update(main_window_weak, window_handle, cx);
  });
}

pub(crate) fn register_window(handle: AnyWindowHandle, view: &Entity<MainWindow>, cx: &mut App) {
  if cx.try_global::<WindowRegistry>().is_none() {
    cx.set_global(WindowRegistry::default());
  }

  let registry = cx.global_mut::<WindowRegistry>();
  registry
    .windows
    .retain(|registered| registered.view.upgrade().is_some());
  if let Some(registered) = registry
    .windows
    .iter_mut()
    .find(|registered| registered.handle == handle)
  {
    registered.view = view.downgrade();
  } else {
    registry.windows.push(RegisteredWindow {
      handle,
      view: view.downgrade(),
    });
  }
}
