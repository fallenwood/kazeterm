//! PTY protocol observation and filtering, independent of the renderer.

pub mod osc7;
pub mod program_status;
pub mod pty_filter;

/// Raw graphics command with cursor position captured at intercept time.
/// Decoding and GPU image allocation belong to the presentation layer.
pub struct RawGraphicsCommand {
  /// The raw APC content (everything after the 'G' prefix).
  pub data: Vec<u8>,
  pub cursor_line: i32,
  pub cursor_column: i32,
  /// Clear/reset notification rather than an image command.
  pub clear_all: bool,
}
