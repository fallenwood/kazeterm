use std::sync::Arc;

use gpui::RenderImage;

use super::scroll_tracker::GraphicsScroll;

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
  /// Transmit an animation frame (recognized but unsupported).
  TransmitFrame,
  /// Control an animation (recognized but unsupported).
  ControlAnimation,
  /// Compose animation frames (recognized but unsupported).
  ComposeFrames,
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
  /// Read image data from a regular file (unsupported).
  File,
  /// Read and delete a temporary file (unsupported).
  TemporaryFile,
  /// Read image data from shared memory (unsupported).
  SharedMemory,
}

impl Default for KittyTransmission {
  fn default() -> Self {
    KittyTransmission::Direct
  }
}

/// Compression applied before base64 encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KittyCompression {
  Zlib,
}

/// What to delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KittyDelete {
  /// Delete all images visible on screen.
  All,
  /// Delete image by ID (and optionally placement).
  ById {
    image_id: u32,
    placement_id: Option<u32>,
  },
  /// Delete the newest image with an image number.
  ByNumber {
    image_number: u32,
    placement_id: Option<u32>,
  },
  /// Delete all placements at the cursor position.
  AtCursor,
  /// Delete all placements intersecting a specific cell.
  AtCell { column: u32, row: u32 },
  /// Delete all placements intersecting a specific cell at a z-index.
  AtCellAndZIndex {
    column: u32,
    row: u32,
    z_index: i32,
  },
  /// Delete all images with IDs in an inclusive range.
  ByIdRange { first: u32, last: u32 },
  /// Delete all placements with a matching z-index.
  ByZIndex(i32),
  /// Delete all placements intersecting a column.
  AtColumn(u32),
  /// Delete all placements intersecting a row.
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
  pub compression: Option<KittyCompression>,
  /// Image ID (0 = auto-assign).
  pub image_id: u32,
  /// Image number (0 = none; mutually exclusive with image ID).
  pub image_number: u32,
  /// Placement ID (0 = none).
  pub placement_id: u32,
  /// Image width in pixels (for raw formats).
  pub source_width: u32,
  /// Image height in pixels (for raw formats).
  pub source_height: u32,
  /// Expected uncompressed byte size (`S`).
  pub data_size: u32,
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
  /// Whether this requests a Unicode-placeholder virtual placement.
  pub virtual_placement: bool,
  /// Parent image for a relative placement.
  pub parent_image_id: u32,
  /// Parent placement for a relative placement.
  pub parent_placement_id: u32,
  /// Relative horizontal displacement in cells.
  pub relative_x: i32,
  /// Relative vertical displacement in cells.
  pub relative_y: i32,
  /// Whether an uppercase delete selector requested freeing image data.
  pub delete_data: bool,
  /// Delete specification (only for Delete action).
  pub delete: Option<KittyDelete>,
  /// The base64-encoded payload data.
  pub payload: Vec<u8>,
}

impl Default for KittyCommand {
  fn default() -> Self {
    Self {
      action: KittyAction::Transmit,
      format: KittyFormat::default(),
      transmission: KittyTransmission::default(),
      compression: None,
      image_id: 0,
      image_number: 0,
      placement_id: 0,
      source_width: 0,
      source_height: 0,
      data_size: 0,
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
      virtual_placement: false,
      parent_image_id: 0,
      parent_placement_id: 0,
      relative_x: 0,
      relative_y: 0,
      delete_data: false,
      delete: None,
      payload: Vec::new(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KittyErrorCode {
  Invalid,
  NoSpace,
  NotFound,
  NotSupported,
}

impl KittyErrorCode {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Invalid => "EINVAL",
      Self::NoSpace => "ENOSPC",
      Self::NotFound => "ENOENT",
      Self::NotSupported => "ENOTSUP",
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KittyProtocolError {
  pub code: KittyErrorCode,
  pub message: String,
  pub image_id: u32,
  pub image_number: u32,
  pub placement_id: u32,
  pub quiet: u8,
}

impl KittyProtocolError {
  pub fn new(code: KittyErrorCode, message: impl Into<String>) -> Self {
    Self {
      code,
      message: message.into(),
      image_id: 0,
      image_number: 0,
      placement_id: 0,
      quiet: 0,
    }
  }

  pub fn invalid(message: impl Into<String>) -> Self {
    Self::new(KittyErrorCode::Invalid, message)
  }

  pub fn with_command(mut self, command: &KittyCommand) -> Self {
    self.image_id = command.image_id;
    self.image_number = command.image_number;
    self.placement_id = command.placement_id;
    self.quiet = command.quiet;
    self
  }
}

/// Response sent back through the PTY to the client application.
#[derive(Debug, Clone)]
pub struct KittyResponse {
  pub image_id: u32,
  pub image_number: u32,
  pub placement_id: u32,
  pub message: String,
  pub error_code: Option<KittyErrorCode>,
}

impl KittyResponse {
  pub fn ok(image_id: u32) -> Self {
    Self {
      image_id,
      image_number: 0,
      placement_id: 0,
      message: "OK".to_string(),
      error_code: None,
    }
  }

  pub fn ok_with_placement(image_id: u32, placement_id: u32) -> Self {
    Self {
      image_id,
      image_number: 0,
      placement_id,
      message: "OK".to_string(),
      error_code: None,
    }
  }

  pub fn error(image_id: u32, code: KittyErrorCode, msg: impl Into<String>) -> Self {
    Self {
      image_id,
      image_number: 0,
      placement_id: 0,
      message: msg.into(),
      error_code: Some(code),
    }
  }

  pub fn from_error(error: KittyProtocolError) -> Self {
    Self {
      image_id: error.image_id,
      image_number: error.image_number,
      placement_id: error.placement_id,
      message: error.message,
      error_code: Some(error.code),
    }
  }

  /// Encode as an APC response using standard `i`, `I`, and `p` keys.
  pub fn encode(&self) -> Vec<u8> {
    let mut buf = Vec::with_capacity(64);
    buf.extend_from_slice(b"\x1b_G");
    let mut has_field = false;
    if self.image_id != 0 {
      buf.extend_from_slice(format!("i={}", self.image_id).as_bytes());
      has_field = true;
    }
    if self.image_number != 0 {
      if has_field {
        buf.push(b',');
      }
      buf.extend_from_slice(format!("I={}", self.image_number).as_bytes());
      has_field = true;
    }
    if self.placement_id != 0 {
      if has_field {
        buf.push(b',');
      }
      buf.extend_from_slice(format!("p={}", self.placement_id).as_bytes());
    }
    buf.push(b';');
    if let Some(code) = self.error_code {
      buf.extend_from_slice(code.as_str().as_bytes());
      if !self.message.is_empty() {
        buf.push(b':');
        for byte in self.message.bytes() {
          buf.push(if byte.is_ascii_graphic() || byte == b' ' {
            byte
          } else {
            b' '
          });
        }
      }
    } else {
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
  pub image_number: u32,
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
  /// Full source image dimensions in pixels.
  pub source_width: u32,
  pub source_height: u32,
  /// Absolute line in the terminal grid (includes scrollback).
  pub line: i32,
  /// Column position.
  pub column: i32,
  /// Display width in cells.
  pub width_cells: u32,
  /// Display height in cells.
  pub height_cells: u32,
  /// Exact target size in pixels before viewport clipping.
  pub width_pixels: u32,
  pub height_pixels: u32,
  /// Source crop region (x, y, w, h) in pixels. (0,0,0,0) = full image.
  pub crop: (u32, u32, u32, u32),
  /// Z-index for layering.
  pub z_index: i32,
  /// Pixel offsets within the starting cell.
  pub x_offset: u32,
  pub y_offset: u32,
}

/// Raw graphics command with cursor position captured at intercept time.
pub struct RawGraphicsCommand {
  /// The raw APC content (everything after 'G' prefix).
  pub data: Vec<u8>,
  /// A command parsed in protocol order by the PTY filter.
  pub parsed_command: Option<KittyCommand>,
  /// Absolute line number in the grid when APC was intercepted.
  pub cursor_line: i32,
  /// Column number when APC was intercepted.
  pub cursor_column: i32,
  /// When true, signals that all images should be cleared (terminal reset/clear).
  pub clear_all: bool,
  /// Ordered terminal scroll observed before the command reached the emulator backend.
  pub scroll: Option<GraphicsScroll>,
  /// Error detected before command parsing, such as an oversized APC.
  pub protocol_error: Option<KittyProtocolError>,
}

/// A placement that's been resolved for the current viewport with its image data.
#[derive(Clone)]
pub struct VisiblePlacement {
  pub image_id: u32,
  pub render_image: Arc<RenderImage>,
  /// Full source image dimensions in pixels.
  pub source_width: u32,
  pub source_height: u32,
  /// Display line relative to viewport (0 = top visible line).
  pub viewport_line: i32,
  /// Column position.
  pub column: i32,
  /// Display width in cells.
  pub width_cells: u32,
  /// Display height in cells.
  pub height_cells: u32,
  /// Exact target size in pixels before viewport clipping.
  pub width_pixels: u32,
  pub height_pixels: u32,
  /// Source crop region in pixels.
  pub crop: (u32, u32, u32, u32),
  /// Z-index for layering.
  pub z_index: i32,
  /// Pixel offsets within the starting cell.
  pub x_offset: u32,
  pub y_offset: u32,
}
