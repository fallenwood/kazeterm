#[cfg(target_os = "windows")]
use gpui::Styled;
use gpui::{AnyElement, IntoElement, Pixels};
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::sync::Arc;
use std::sync::{LazyLock, Mutex};

#[cfg(target_os = "windows")]
mod windows_impl {
  use gpui::RenderImage;
  use std::sync::Arc;
  use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDIBits, GetObjectW, SelectObject,
  };
  use windows::Win32::UI::Shell::{SHFILEINFOW, SHGFI_ICON, SHGFI_SMALLICON, SHGetFileInfoW};
  use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};
  use windows::core::PCWSTR;

  fn resolve_exe_path(exe_path: &str) -> Option<String> {
    use std::path::Path;

    let path = Path::new(exe_path);

    if path.is_absolute() && path.exists() {
      return Some(exe_path.to_string());
    }

    if let Ok(path_env) = std::env::var("PATH") {
      for dir in path_env.split(';') {
        let candidate = Path::new(dir).join(exe_path);
        if candidate.exists() {
          return candidate.to_str().map(|s| s.to_string());
        }

        if !exe_path.to_lowercase().ends_with(".exe") {
          let candidate_exe = Path::new(dir).join(format!("{}.exe", exe_path));
          if candidate_exe.exists() {
            return candidate_exe.to_str().map(|s| s.to_string());
          }
        }
      }
    }

    None
  }

  pub fn extract_icon_from_exe(exe_path: &str) -> Option<Arc<RenderImage>> {
    let full_path = resolve_exe_path(exe_path)?;
    extract_icon_internal(&full_path)
  }

  fn extract_icon_internal(exe_path: &str) -> Option<Arc<RenderImage>> {
    unsafe {
      let wide_path: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();

      let mut shfi = SHFILEINFOW::default();
      let result = SHGetFileInfoW(
        PCWSTR::from_raw(wide_path.as_ptr()),
        windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
        Some(&mut shfi),
        std::mem::size_of::<SHFILEINFOW>() as u32,
        SHGFI_ICON | SHGFI_SMALLICON,
      );

      if result == 0 || shfi.hIcon.is_invalid() {
        return None;
      }

      let hicon = shfi.hIcon;

      let mut icon_info = ICONINFO::default();
      if GetIconInfo(hicon, &mut icon_info).is_err() {
        let _ = DestroyIcon(hicon);
        tracing::warn!("Failed to get icon info for {}", exe_path);
        return None;
      }

      // Get bitmap dimensions
      let hdc = CreateCompatibleDC(None);
      if hdc.is_invalid() {
        if !icon_info.hbmColor.is_invalid() {
          let _ = DeleteObject(icon_info.hbmColor.into());
        }
        if !icon_info.hbmMask.is_invalid() {
          let _ = DeleteObject(icon_info.hbmMask.into());
        }
        let _ = DestroyIcon(hicon);
        tracing::warn!("Failed to create compatible DC for {}", exe_path);
        return None;
      }

      let old_bitmap = SelectObject(hdc, icon_info.hbmColor.into());

      // Get bitmap dimensions using GetObject
      let mut bitmap = BITMAP::default();
      let obj_size = GetObjectW(
        icon_info.hbmColor.into(),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bitmap as *mut _ as *mut _),
      );

      if obj_size == 0 {
        SelectObject(hdc, old_bitmap);
        let _ = DeleteDC(hdc);
        if !icon_info.hbmColor.is_invalid() {
          let _ = DeleteObject(icon_info.hbmColor.into());
        }
        if !icon_info.hbmMask.is_invalid() {
          let _ = DeleteObject(icon_info.hbmMask.into());
        }
        let _ = DestroyIcon(hicon);
        tracing::warn!("Failed to get bitmap object for {}", exe_path);
        return None;
      }

      let width = bitmap.bmWidth as u32;
      let height = bitmap.bmHeight as u32;

      if width == 0 || height == 0 {
        SelectObject(hdc, old_bitmap);
        let _ = DeleteDC(hdc);
        if !icon_info.hbmColor.is_invalid() {
          let _ = DeleteObject(icon_info.hbmColor.into());
        }
        if !icon_info.hbmMask.is_invalid() {
          let _ = DeleteObject(icon_info.hbmMask.into());
        }
        let _ = DestroyIcon(hicon);
        tracing::warn!("Icon has zero width or height for {}", exe_path);
        return None;
      }

      // Prepare bitmap info for GetDIBits
      let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
          biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
          biWidth: width as i32,
          biHeight: -(height as i32), // Negative for top-down DIB
          biPlanes: 1,
          biBitCount: 32,
          biCompression: BI_RGB.0,
          biSizeImage: 0,
          biXPelsPerMeter: 0,
          biYPelsPerMeter: 0,
          biClrUsed: 0,
          biClrImportant: 0,
        },
        bmiColors: [Default::default()],
      };

      let mut pixels: Vec<u8> = vec![0; (width * height * 4) as usize];

      // Get the actual bitmap data
      let result = GetDIBits(
        hdc,
        icon_info.hbmColor,
        0,
        height,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut bmi,
        DIB_RGB_COLORS,
      );

      SelectObject(hdc, old_bitmap);
      let _ = DeleteDC(hdc);
      if !icon_info.hbmColor.is_invalid() {
        let _ = DeleteObject(icon_info.hbmColor.into());
      }
      if !icon_info.hbmMask.is_invalid() {
        let _ = DeleteObject(icon_info.hbmMask.into());
      }
      let _ = DestroyIcon(hicon);

      if result == 0 {
        tracing::warn!("Failed to get bitmap bits for {}", exe_path);
        return None;
      }

      // The pixels are in BGRA format already, which is what RenderImage expects
      // But we need to verify alpha - if all alpha values are 0, set them to 255
      let has_alpha = pixels
        .chunks(4)
        .any(|chunk| chunk.get(3).map_or(false, |&a| a != 0));
      if !has_alpha {
        for chunk in pixels.chunks_mut(4) {
          if chunk.len() == 4 {
            chunk[3] = 255;
          }
        }
      }

      // Create a Frame from the pixel data using the image crate
      use image::{ImageBuffer, Rgba};

      // Convert BGRA to RGBA for the image crate
      let mut rgba_pixels = pixels.clone();
      for chunk in rgba_pixels.chunks_mut(4) {
        if chunk.len() == 4 {
          chunk.swap(0, 2); // Swap B and R
        }
      }

      let img_buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgba_pixels)?;

      let frame = image::Frame::new(img_buffer);

      Some(Arc::new(RenderImage::new(vec![frame])))
    }
  }
}

pub fn get_default_shell_icon_path() -> &'static str {
  "icons/square-terminal.svg"
}

static SHELL_ICON_CACHE: LazyLock<Mutex<HashMap<String, ShellIcon>>> =
  LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone)]
pub enum ShellIcon {
  #[cfg(target_os = "windows")]
  Extracted(Arc<gpui::RenderImage>),
  Default(&'static str),
}

impl ShellIcon {
  /// Create a shell icon for the given shell path.
  /// On Windows, this tries to extract the icon from the executable first.
  /// Results are cached.
  pub fn new(shell_path: &str) -> Self {
    // Check cache first
    {
      let cache = SHELL_ICON_CACHE.lock().unwrap();
      if let Some(cached) = cache.get(shell_path) {
        return cached.clone();
      }
    }

    let icon = Self::create_uncached(shell_path);

    // Cache the result
    {
      let mut cache = SHELL_ICON_CACHE.lock().unwrap();
      cache.insert(shell_path.to_string(), icon.clone());
    }

    icon
  }

  fn create_uncached(shell_path: &str) -> Self {
    #[cfg(target_os = "windows")]
    {
      // Try to extract icon from the executable
      if let Some(render_image) = windows_impl::extract_icon_from_exe(shell_path) {
        return ShellIcon::Extracted(render_image);
      }
    }

    #[cfg(not(target_os = "windows"))]
    let _ = shell_path; // Suppress unused warning on non-Windows

    // Fall back to default icon
    ShellIcon::Default(get_default_shell_icon_path())
  }

  /// Convert the shell icon to an element that can be rendered
  #[allow(unused_variables)]
  pub fn into_element(self, size: Pixels) -> AnyElement {
    match self {
      #[cfg(target_os = "windows")]
      ShellIcon::Extracted(render_image) => gpui::img(gpui::ImageSource::Render(render_image))
        .w(size)
        .h(size)
        .into_any_element(),
      ShellIcon::Default(path) => {
        use gpui_component::{Icon, Sizable};
        Icon::empty().path(path).small().into_any_element()
      }
    }
  }
}
