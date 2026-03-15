use std::sync::Arc;

use gpui::RenderImage;

/// Kitty graphics protocol action types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KittyAction {
  /// Transmit image data (store but don't display).
  Transmit,
  /// Transmit and display image at cursor position.
  TransmitAndDisplay,
  /// Display a previously transmitted image.
  Display,
  /// Delete images.
  Delete,
  /// Query terminal for graphics support.
  Query,
}

/// Image data format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KittyFormat {
  /// 24-bit RGB raw pixels.
  Rgb,
  /// 32-bit RGBA raw pixels.
  Rgba,
  /// PNG encoded image.
  Png,
}

impl Default for KittyFormat {
  fn default() -> Self {
    KittyFormat::Rgba
  }
}

/// Image transmission medium.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KittyTransmission {
  /// Direct (inline base64 data).
  Direct,
}

impl Default for KittyTransmission {
  fn default() -> Self {
    KittyTransmission::Direct
  }
}

/// What to delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KittyDelete {
  /// Delete all images visible on screen.
  All,
  /// Delete image by ID (and optionally placement).
  ById { image_id: u32, placement_id: Option<u32> },
  /// Delete all placements at the cursor position.
  AtCursor,
  /// Delete all images with z-index matching a given value.
  ByZIndex(i32),
  /// Delete all images on the current column.
  AtColumn(u32),
  /// Delete all images on the current row.
  AtRow(u32),
  /// Delete all animation frames.
  AnimationFrames,
}

/// A parsed Kitty graphics protocol command.
#[derive(Debug, Clone)]
pub struct KittyCommand {
  pub action: KittyAction,
  pub format: KittyFormat,
  pub transmission: KittyTransmission,
  /// Image ID (0 = auto-assign).
  pub image_id: u32,
  /// Placement ID (0 = none).
  pub placement_id: u32,
  /// Image width in pixels (for raw formats).
  pub source_width: u32,
  /// Image height in pixels (for raw formats).
  pub source_height: u32,
  /// Display columns (0 = auto from image).
  pub display_columns: u32,
  /// Display rows (0 = auto from image).
  pub display_rows: u32,
  /// X offset within the cell in pixels.
  pub x_offset: u32,
  /// Y offset within the cell in pixels.
  pub y_offset: u32,
  /// Source rect: left pixel offset for cropping.
  pub crop_x: u32,
  /// Source rect: top pixel offset for cropping.
  pub crop_y: u32,
  /// Source rect: width in pixels for cropping (0 = full).
  pub crop_width: u32,
  /// Source rect: height in pixels for cropping (0 = full).
  pub crop_height: u32,
  /// Z-index for layering (default 0).
  pub z_index: i32,
  /// Whether more chunks follow (m=1).
  pub more_chunks: bool,
  /// Quiet mode: 0=default, 1=suppress OK, 2=suppress errors too.
  pub quiet: u8,
  /// Cursor movement policy: 0=move cursor, 1=don't move.
  pub cursor_movement: u8,
  /// Delete specification (only for Delete action).
  pub delete: Option<KittyDelete>,
  /// The base64-encoded payload data.
  pub payload: Vec<u8>,
}

impl Default for KittyCommand {
  fn default() -> Self {
    Self {
      action: KittyAction::TransmitAndDisplay,
      format: KittyFormat::default(),
      transmission: KittyTransmission::default(),
      image_id: 0,
      placement_id: 0,
      source_width: 0,
      source_height: 0,
      display_columns: 0,
      display_rows: 0,
      x_offset: 0,
      y_offset: 0,
      crop_x: 0,
      crop_y: 0,
      crop_width: 0,
      crop_height: 0,
      z_index: 0,
      more_chunks: false,
      quiet: 0,
      cursor_movement: 0,
      delete: None,
      payload: Vec::new(),
    }
  }
}

/// Response sent back through the PTY to the client application.
#[derive(Debug, Clone)]
pub struct KittyResponse {
  pub image_id: u32,
  pub placement_id: u32,
  pub message: String,
  pub ok: bool,
}

impl KittyResponse {
  pub fn ok(image_id: u32) -> Self {
    Self {
      image_id,
      placement_id: 0,
      message: "OK".to_string(),
      ok: true,
    }
  }

  pub fn ok_with_placement(image_id: u32, placement_id: u32) -> Self {
    Self {
      image_id,
      placement_id,
      message: "OK".to_string(),
      ok: true,
    }
  }

  pub fn error(image_id: u32, msg: impl Into<String>) -> Self {
    Self {
      image_id,
      placement_id: 0,
      message: msg.into(),
      ok: false,
    }
  }

  /// Encode as an APC response: `\x1b_Gi=<id>,I=<pid>;OK\x1b\\`
  pub fn encode(&self) -> Vec<u8> {
    let mut buf = Vec::with_capacity(64);
    buf.extend_from_slice(b"\x1b_G");
    buf.extend_from_slice(format!("i={}", self.image_id).as_bytes());
    if self.placement_id != 0 {
      buf.extend_from_slice(format!(",I={}", self.placement_id).as_bytes());
    }
    buf.push(b';');
    if self.ok {
      buf.extend_from_slice(self.message.as_bytes());
    } else {
      buf.extend_from_slice(b"ENOENT:");
      buf.extend_from_slice(self.message.as_bytes());
    }
    buf.extend_from_slice(b"\x1b\\");
    buf
  }
}

/// A decoded image ready for rendering.
#[derive(Clone)]
pub struct StoredImage {
  pub id: u32,
  pub render_image: Arc<RenderImage>,
  pub width: u32,
  pub height: u32,
  /// Estimated memory usage in bytes.
  pub memory_bytes: usize,
}

/// An active image placement in the terminal grid.
#[derive(Debug, Clone)]
pub struct ImagePlacement {
  pub image_id: u32,
  pub placement_id: u32,
  /// Absolute line in the terminal grid (includes scrollback).
  pub line: i32,
  /// Column position.
  pub column: i32,
  /// Display width in cells.
  pub width_cells: u32,
  /// Display height in cells.
  pub height_cells: u32,
  /// Source crop region (x, y, w, h) in pixels. (0,0,0,0) = full image.
  pub crop: (u32, u32, u32, u32),
  /// Z-index for layering.
  pub z_index: i32,
  /// Pixel offsets within the starting cell.
  pub x_offset: u32,
  pub y_offset: u32,
}

/// A placement that's been resolved for the current viewport with its image data.
#[derive(Clone)]
pub struct VisiblePlacement {
  pub render_image: Arc<RenderImage>,
  /// Display line relative to viewport (0 = top visible line).
  pub viewport_line: i32,
  /// Column position.
  pub column: i32,
  /// Display width in cells.
  pub width_cells: u32,
  /// Display height in cells.
  pub height_cells: u32,
  /// Z-index for layering.
  pub z_index: i32,
  /// Pixel offsets within the starting cell.
  pub x_offset: u32,
  pub y_offset: u32,
}
