use crate::assets::Assets;

/// Set the application icon in the macOS Dock.
#[cfg(target_os = "macos")]
pub fn set_macos_app_icon() {
  let Some(png_file) = Assets::get("icons/kazeterm.png") else {
    tracing::warn!("Failed to load embedded icon for macOS dock");
    return;
  };
  let data = png_file.data;

  unsafe {
    let ns_data: *mut objc::runtime::Object = msg_send![
      class!(NSData),
      dataWithBytes: data.as_ptr()
      length: data.len()
    ];
    if ns_data.is_null() {
      return;
    }
    let ns_image_alloc: *mut objc::runtime::Object =
      msg_send![class!(NSImage), alloc];
    let ns_image: *mut objc::runtime::Object =
      msg_send![ns_image_alloc, initWithData: ns_data];
    if ns_image.is_null() {
      return;
    }
    let ns_app: *mut objc::runtime::Object =
      msg_send![class!(NSApplication), sharedApplication];
    let _: () = msg_send![ns_app, setApplicationIconImage: ns_image];
  }
}

/// Install icon and `.desktop` file to user-local XDG directories so that
/// both X11 window managers and Wayland compositors can find the app icon.
///
/// Files are only written when they do not already exist.
#[cfg(target_os = "linux")]
pub fn install_linux_desktop_icon() {
  let Some(data_dir) = dirs::data_dir() else {
    return;
  };

  install_xdg_icon_files(&data_dir);
  install_desktop_file(&data_dir);
}

#[cfg(target_os = "linux")]
fn install_xdg_icon_files(data_dir: &std::path::Path) {
  // Install SVG as scalable icon (works at any size)
  let scalable_dir = data_dir.join("icons/hicolor/scalable/apps");
  let svg_path = scalable_dir.join("kazeterm.svg");
  if !svg_path.exists() {
    if let Some(svg_file) = Assets::get("icons/kazeterm.svg") {
      if std::fs::create_dir_all(&scalable_dir).is_ok() {
        let _ = std::fs::write(&svg_path, &svg_file.data);
      }
    }
  }

  // Install raster icons at common sizes for compositors that prefer PNG
  let Some(png_file) = Assets::get("icons/kazeterm.png") else {
    return;
  };
  let Ok(img) = image::load_from_memory(&png_file.data) else {
    return;
  };

  for size in [256, 128, 64, 48] {
    let icon_dir =
      data_dir.join(format!("icons/hicolor/{size}x{size}/apps"));
    let icon_path = icon_dir.join("kazeterm.png");
    if icon_path.exists() {
      continue;
    }
    if std::fs::create_dir_all(&icon_dir).is_err() {
      continue;
    }
    let resized = img.resize_exact(
      size,
      size,
      image::imageops::FilterType::Lanczos3,
    );
    let _ = resized.save(&icon_path);
  }
}

#[cfg(target_os = "linux")]
fn install_desktop_file(data_dir: &std::path::Path) {
  let apps_dir = data_dir.join("applications");
  let desktop_path = apps_dir.join("kazeterm.desktop");
  if desktop_path.exists() {
    return;
  }

  let exec = std::env::current_exe()
    .ok()
    .and_then(|p| p.to_str().map(String::from))
    .unwrap_or_else(|| "kazeterm".into());

  let content = format!(
    "\
[Desktop Entry]
Name=Kazeterm
Comment=A modern GPU-accelerated terminal emulator
Exec={exec}
Icon=kazeterm
Terminal=false
Type=Application
Categories=System;TerminalEmulator;
StartupWMClass=kazeterm
Keywords=terminal;console;shell;prompt;command;commandline;
"
  );

  if std::fs::create_dir_all(&apps_dir).is_ok() {
    let _ = std::fs::write(&desktop_path, content);
  }
}

/// Set the window icon on Linux via X11 `_NET_WM_ICON` property.
///
/// This embeds the icon pixels directly in the window properties so
/// the icon displays even when the `.desktop` file is not installed.
#[cfg(target_os = "linux")]
pub fn set_x11_window_icon(window: &gpui::Window) {
  use raw_window_handle::{HasWindowHandle, RawWindowHandle};

  let Ok(wh) = <gpui::Window as HasWindowHandle>::window_handle(window)
  else {
    return;
  };

  match wh.as_raw() {
    RawWindowHandle::Xcb(xcb_wh) => {
      let window_id = xcb_wh.window.get();
      set_x11_net_wm_icon(window_id);
    }
    RawWindowHandle::Xlib(xlib_wh) => {
      let window_id = xlib_wh.window as u32;
      set_x11_net_wm_icon(window_id);
    }
    _ => {}
  }
}

/// Set `_NET_WM_ICON` on an X11 window using the embedded PNG icon.
#[cfg(target_os = "linux")]
fn set_x11_net_wm_icon(window_id: u32) {
  use x11rb::connection::Connection;
  use x11rb::protocol::xproto::{self, ConnectionExt};
  use x11rb::rust_connection::RustConnection;
  use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

  let Some(png_file) = Assets::get("icons/kazeterm.png") else {
    tracing::warn!("Failed to load embedded icon for X11 window");
    return;
  };

  let Ok(img) = image::load_from_memory(&png_file.data) else {
    tracing::warn!("Failed to decode embedded PNG icon");
    return;
  };

  // Provide multiple sizes for the window manager to choose from
  let sizes: &[u32] = &[48, 32, 16];
  let mut icon_data: Vec<u32> = Vec::new();

  for &size in sizes {
    let resized =
      img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    icon_data.push(size);
    icon_data.push(size);
    for pixel in rgba.pixels() {
      let [r, g, b, a] = pixel.0;
      icon_data.push(
        (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32,
      );
    }
  }

  let Ok((conn, _)) = RustConnection::connect(None) else {
    tracing::warn!("Failed to connect to X server for icon setting");
    return;
  };

  let Ok(atom_cookie) = conn.intern_atom(false, b"_NET_WM_ICON") else {
    return;
  };
  let Ok(atom_reply) = atom_cookie.reply() else {
    return;
  };

  let _ = conn.change_property32(
    xproto::PropMode::REPLACE,
    window_id,
    atom_reply.atom,
    xproto::AtomEnum::CARDINAL,
    &icon_data,
  );
  let _ = conn.flush();
}
