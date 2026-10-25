use std::sync::Arc;

use gpui::{Bounds, Pixels, RenderImage};
pub use terminal_kernel::protocols::{RawGraphicsCommand, RawGraphicsEvent};

/// Per-image and in-progress transfer quota, matching Kitty's base quota.
pub const MAX_IMAGE_BYTES: usize = 320 * 1024 * 1024;
pub const MAX_CHUNK_BYTES: usize = 4096;

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
  ById {
    image_id: u32,
    placement_id: Option<u32>,
  },
  /// Delete the newest image with this image number.
  ByNumber {
    image_number: u32,
    placement_id: Option<u32>,
  },
  /// Delete all placements at the cursor position.
  AtCursor,
  /// Delete placements intersecting a cell (one-based screen coordinates).
  AtCell {
    x: u32,
    y: u32,
    z_index: Option<i32>,
  },
  /// Delete images in an inclusive client-ID range.
  IdRange { first: u32, last: u32 },
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
  /// Non-unique image number, distinct from a placement ID.
  pub image_number: u32,
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
  /// Zlib compression (o=z), applied before base64 encoding.
  pub compressed: bool,
  /// Uncompressed PNG size when using o=z (S=).
  pub data_size: u32,
  /// Whether more chunks follow (m=1).
  pub more_chunks: bool,
  /// Quiet mode: 0=default, 1=suppress OK, 2=suppress errors too.
  pub quiet: u8,
  /// Cursor movement policy: 0=move cursor, 1=don't move.
  pub cursor_movement: u8,
  /// Delete specification (only for Delete action).
  pub delete: Option<KittyDelete>,
  /// Uppercase deletion also frees data once the image has no placements.
  pub delete_data: bool,
  /// Decoded payload bytes (possibly still zlib compressed).
  pub payload: Vec<u8>,
}

impl Default for KittyCommand {
  fn default() -> Self {
    Self {
      action: KittyAction::Transmit,
      format: KittyFormat::default(),
      transmission: KittyTransmission::default(),
      image_id: 0,
      image_number: 0,
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
      compressed: false,
      data_size: 0,
      more_chunks: false,
      quiet: 0,
      cursor_movement: 0,
      delete: None,
      delete_data: false,
      payload: Vec::new(),
    }
  }
}

/// Reply identity and suppression policy, retained even if a later chunk or
/// another control parameter is invalid. Never contains untrusted payload text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KittyResponseContext {
  pub image_id: u32,
  pub image_number: u32,
  pub placement_id: u32,
  pub quiet: u8,
}

impl From<&KittyCommand> for KittyResponseContext {
  fn from(cmd: &KittyCommand) -> Self {
    Self {
      image_id: cmd.image_id,
      image_number: cmd.image_number,
      placement_id: cmd.placement_id,
      quiet: cmd.quiet,
    }
  }
}

impl KittyResponseContext {
  pub fn respond(self, result: Result<u32, String>) -> Option<KittyResponse> {
    // Anonymous uploads never solicit replies, even with a placement ID.
    if (self.image_id == 0 && self.image_number == 0)
      || self.quiet >= 2
      || (self.quiet == 1 && result.is_ok())
    {
      return None;
    }
    let (image_id, message, ok) = match result {
      Ok(id) => (id, "OK".to_string(), true),
      Err(message) => (self.image_id, message, false),
    };
    Some(KittyResponse {
      image_id,
      image_number: self.image_number,
      placement_id: self.placement_id,
      message,
      ok,
    })
  }
}

/// Response sent back through the PTY to the client application.
#[derive(Debug, Clone)]
pub struct KittyResponse {
  pub image_id: u32,
  pub image_number: u32,
  pub placement_id: u32,
  pub message: String,
  pub ok: bool,
}

impl KittyResponse {
  pub fn ok(image_id: u32) -> Self {
    Self {
      image_id,
      image_number: 0,
      placement_id: 0,
      message: "OK".to_string(),
      ok: true,
    }
  }

  pub fn ok_with_placement(image_id: u32, placement_id: u32) -> Self {
    Self {
      image_id,
      image_number: 0,
      placement_id,
      message: "OK".to_string(),
      ok: true,
    }
  }

  pub fn error(image_id: u32, msg: impl Into<String>) -> Self {
    Self {
      image_id,
      image_number: 0,
      placement_id: 0,
      message: msg.into(),
      ok: false,
    }
  }

  /// Encode as an APC response: `\x1b_Gi=<id>,p=<pid>;OK\x1b\\`
  pub fn encode(&self) -> Vec<u8> {
    let mut params = Vec::with_capacity(3);
    if self.image_id != 0 {
      params.push(format!("i={}", self.image_id));
    }
    if self.image_number != 0 {
      params.push(format!("I={}", self.image_number));
    }
    if self.placement_id != 0 {
      params.push(format!("p={}", self.placement_id));
    }
    // Protocol responses must be bounded printable ASCII. Never reflect an ESC,
    // control character, or arbitrarily large decoder diagnostic to the client.
    let mut message: String = self
      .message
      .chars()
      .filter(|c| c.is_ascii() && !c.is_ascii_control())
      .take(256)
      .collect();
    if self.ok {
      message = "OK".into();
    } else if !message.split_once(':').is_some_and(|(code, _)| {
      code.starts_with('E')
        && code
          .bytes()
          .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    }) {
      message = format!("EINVAL:{message}");
    }
    message.truncate(256);
    format!("\x1b_G{};{}\x1b\\", params.join(","), message).into_bytes()
  }
}

/// A decoded image ready for rendering.
#[derive(Clone)]
pub struct StoredImage {
  pub id: u32,
  /// Kitty sorts equal-z images by creation identity, not client IDs or LRU.
  pub(crate) internal_id: u64,
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
  /// Requested cell dimensions; zero retains automatic/native sizing on resize.
  pub display_columns: u32,
  pub display_rows: u32,
  /// Effective cell coverage, used for deletion and cursor feedback.
  pub width_cells: u32,
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
  pub image_id: u32,
  pub(crate) internal_image_id: u64,
  pub render_image: Arc<RenderImage>,
  /// Cropped destination rectangle, in pixels relative to the viewport origin.
  pub bounds: Bounds<Pixels>,
  /// Full image transform, including the part outside the cropped rectangle.
  /// GPUI samples only `bounds`, without allocating another cropped image.
  pub image_bounds: Bounds<Pixels>,
  pub z_index: i32,
}

/// Kitty has three image layers separated by cell backgrounds and text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphicsLayer {
  BelowBackground,
  BelowText,
  AboveText,
}

impl GraphicsLayer {
  pub(crate) fn for_z_index(z_index: i32) -> Self {
    if z_index < i32::MIN / 2 {
      Self::BelowBackground
    } else if z_index < 0 {
      Self::BelowText
    } else {
      Self::AboveText
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{KittyResponse, KittyResponseContext};

  #[test]
  fn reply_identity_quiet_and_printable_error_encoding() {
    let mut context = KittyResponseContext {
      image_id: 0,
      image_number: 9,
      placement_id: 3,
      quiet: 0,
    };
    assert_eq!(
      context.respond(Ok(42)).unwrap().encode(),
      b"\x1b_Gi=42,I=9,p=3;OK\x1b\\"
    );
    assert_eq!(
      context
        .respond(Err("ENOENT:Image not found".into()))
        .unwrap()
        .encode(),
      b"\x1b_GI=9,p=3;ENOENT:Image not found\x1b\\"
    );
    context.quiet = 1;
    assert!(context.respond(Ok(42)).is_none());
    assert!(context.respond(Err("EINVAL:Bad data".into())).is_some());
    context.quiet = 2;
    assert!(context.respond(Err("EINVAL:Bad data".into())).is_none());
    assert!(KittyResponseContext::default().respond(Ok(42)).is_none());
    assert!(
      KittyResponseContext::default()
        .respond(Err("EINVAL:Bad data".into()))
        .is_none()
    );
    let response =
      KittyResponse::error(7, format!("E2BIG:Bad\x1b\\\n\u{85}{}", "x".repeat(1024))).encode();
    assert!(response.starts_with(b"\x1b_Gi=7;E2BIG:"));
    let body = &response[response.iter().position(|b| *b == b';').unwrap() + 1..response.len() - 2];
    assert!(body.len() <= 256);
    assert!(body.iter().all(|b| b.is_ascii() && !b.is_ascii_control()));
  }

  #[test]
  fn placement_response_uses_p_not_the_image_number_key() {
    assert_eq!(KittyResponse::ok(7).encode(), b"\x1b_Gi=7;OK\x1b\\");
    assert_eq!(
      KittyResponse::ok_with_placement(7, 3).encode(),
      b"\x1b_Gi=7,p=3;OK\x1b\\"
    );
  }
}
