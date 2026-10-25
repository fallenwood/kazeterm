//! PTY protocol observation and filtering, independent of the renderer.

pub mod osc7;
pub mod program_status;
pub mod pty_filter;

/// Ordered graphics work and protocol replies from the PTY reader. Replies
/// following a graphics command use this same FIFO so DA cannot overtake a query.
#[derive(Debug)]
pub enum RawGraphicsEvent {
  Command(RawGraphicsCommand),
  ClearAll,
  Reply(Vec<u8>),
}

/// Raw graphics command with cursor position captured at intercept time.
/// Decoding and GPU image allocation belong to the presentation layer.
#[derive(Debug)]
pub struct RawGraphicsCommand {
  /// The raw APC content (everything after the 'G' prefix).
  pub data: Vec<u8>,
  pub cursor_line: i32,
  pub cursor_column: i32,
}
