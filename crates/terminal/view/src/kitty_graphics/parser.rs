use base64::{Engine, engine::general_purpose::STANDARD as BASE64};

use super::command::{KittyAction, KittyCommand, KittyDelete, KittyFormat, KittyTransmission};

/// Parser for Kitty graphics protocol commands.
///
/// Handles chunked transfers by accumulating payload data across
/// multiple APC sequences until `more_chunks` is false.
pub struct KittyParser {
  /// Accumulated payload for chunked transfers.
  chunk_buffer: Vec<u8>,
  /// The command from the first chunk (holds params while accumulating).
  pending_command: Option<KittyCommand>,
}

impl KittyParser {
  pub fn new() -> Self {
    Self {
      chunk_buffer: Vec::new(),
      pending_command: None,
    }
  }

  /// Parse a raw APC graphics payload (everything between `\x1b_G` and `\x1b\\`).
  ///
  /// Returns `Some(command)` when a complete command is ready (single-chunk or
  /// final chunk of a multi-chunk transfer). Returns `None` when accumulating
  /// intermediate chunks.
  pub fn parse(&mut self, raw: &[u8]) -> Option<KittyCommand> {
    let (params_bytes, payload_bytes) = split_params_payload(raw);
    let mut cmd = parse_params(params_bytes);

    // Decode the base64 payload for this chunk.
    let decoded = if !payload_bytes.is_empty() {
      BASE64.decode(payload_bytes).unwrap_or_default()
    } else {
      Vec::new()
    };

    if cmd.more_chunks {
      // Intermediate chunk: accumulate and wait for more.
      if self.pending_command.is_none() {
        self.pending_command = Some(cmd);
        self.chunk_buffer = decoded;
      } else {
        self.chunk_buffer.extend_from_slice(&decoded);
      }
      None
    } else if let Some(mut pending) = self.pending_command.take() {
      // Final chunk of a multi-chunk transfer.
      self.chunk_buffer.extend_from_slice(&decoded);
      pending.payload = std::mem::take(&mut self.chunk_buffer);
      pending.more_chunks = false;
      Some(pending)
    } else {
      // Single-chunk command.
      cmd.payload = decoded;
      Some(cmd)
    }
  }

  /// Reset any in-progress chunked transfer.
  pub fn reset(&mut self) {
    self.chunk_buffer.clear();
    self.pending_command = None;
  }
}

/// Split raw APC content into params and payload at the first `;`.
fn split_params_payload(raw: &[u8]) -> (&[u8], &[u8]) {
  if let Some(pos) = raw.iter().position(|&b| b == b';') {
    (&raw[..pos], &raw[pos + 1..])
  } else {
    (raw, &[])
  }
}

/// Parse comma-separated key=value parameters into a KittyCommand.
fn parse_params(params: &[u8]) -> KittyCommand {
  let mut cmd = KittyCommand::default();
  let params_str = std::str::from_utf8(params).unwrap_or("");

  for pair in params_str.split(',') {
    let pair = pair.trim();
    if pair.is_empty() {
      continue;
    }
    let Some((key, value)) = pair.split_once('=') else {
      continue;
    };

    match key {
      "a" => {
        cmd.action = match value {
          "t" => KittyAction::Transmit,
          "T" => KittyAction::TransmitAndDisplay,
          "p" => KittyAction::Display,
          "d" => KittyAction::Delete,
          "q" => KittyAction::Query,
          _ => KittyAction::TransmitAndDisplay,
        };
      }
      "f" => {
        cmd.format = match value {
          "24" => KittyFormat::Rgb,
          "32" => KittyFormat::Rgba,
          "100" => KittyFormat::Png,
          _ => KittyFormat::Rgba,
        };
      }
      "t" => {
        cmd.transmission = match value {
          "d" => KittyTransmission::Direct,
          _ => KittyTransmission::Direct,
        };
      }
      "i" => cmd.image_id = value.parse().unwrap_or(0),
      "I" => cmd.placement_id = value.parse().unwrap_or(0),
      "s" => cmd.source_width = value.parse().unwrap_or(0),
      "v" => cmd.source_height = value.parse().unwrap_or(0),
      "c" => cmd.display_columns = value.parse().unwrap_or(0),
      "r" => cmd.display_rows = value.parse().unwrap_or(0),
      "x" => cmd.crop_x = value.parse().unwrap_or(0),
      "y" => cmd.crop_y = value.parse().unwrap_or(0),
      "w" => cmd.crop_width = value.parse().unwrap_or(0),
      "h" => cmd.crop_height = value.parse().unwrap_or(0),
      "X" => cmd.x_offset = value.parse().unwrap_or(0),
      "Y" => cmd.y_offset = value.parse().unwrap_or(0),
      "z" => cmd.z_index = value.parse().unwrap_or(0),
      "m" => cmd.more_chunks = value == "1",
      "q" => cmd.quiet = value.parse().unwrap_or(0),
      "C" => cmd.cursor_movement = value.parse().unwrap_or(0),
      "d" => {
        cmd.delete = Some(parse_delete(value, &cmd));
      }
      _ => {} // Ignore unknown keys for forward-compatibility.
    }
  }

  cmd
}

/// Parse the delete specifier.
fn parse_delete(value: &str, _cmd: &KittyCommand) -> KittyDelete {
  // Single-character codes: a=all, i=by id, c=at cursor, etc.
  if value.is_empty() {
    return KittyDelete::All;
  }

  let first = value.as_bytes()[0];
  match first {
    b'a' | b'A' => KittyDelete::All,
    b'i' | b'I' => KittyDelete::ById {
      image_id: 0, // Will be filled from i= param.
      placement_id: None,
    },
    b'c' | b'C' => KittyDelete::AtCursor,
    b'p' | b'P' => KittyDelete::AtColumn(0),
    b'q' | b'Q' => KittyDelete::AtRow(0),
    b'x' | b'X' => KittyDelete::ByZIndex(0),
    b'f' | b'F' => KittyDelete::AnimationFrames,
    _ => KittyDelete::All,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_simple_transmit_and_display() {
    let mut parser = KittyParser::new();
    let raw = b"a=T,f=100,i=1;iVBORw0KGgo=";
    let cmd = parser.parse(raw).expect("should produce command");

    assert_eq!(cmd.action, KittyAction::TransmitAndDisplay);
    assert_eq!(cmd.format, KittyFormat::Png);
    assert_eq!(cmd.image_id, 1);
    assert!(!cmd.more_chunks);
    assert!(!cmd.payload.is_empty());
  }

  #[test]
  fn test_parse_query() {
    let mut parser = KittyParser::new();
    let raw = b"a=q,i=31,s=1,v=1,f=24;AAAA";
    let cmd = parser.parse(raw).expect("should produce command");

    assert_eq!(cmd.action, KittyAction::Query);
    assert_eq!(cmd.image_id, 31);
    assert_eq!(cmd.format, KittyFormat::Rgb);
    assert_eq!(cmd.source_width, 1);
    assert_eq!(cmd.source_height, 1);
  }

  #[test]
  fn test_chunked_transfer() {
    let mut parser = KittyParser::new();

    // First chunk (m=1 means more coming).
    let chunk1 = b"a=T,f=100,i=5,m=1;AAAA";
    assert!(
      parser.parse(chunk1).is_none(),
      "intermediate chunk returns None"
    );

    // Second chunk (m=1 still).
    let chunk2 = b"m=1;BBBB";
    assert!(parser.parse(chunk2).is_none());

    // Final chunk (m=0 or absent).
    let chunk3 = b"m=0;CCCC";
    let cmd = parser.parse(chunk3).expect("final chunk produces command");

    assert_eq!(cmd.action, KittyAction::TransmitAndDisplay);
    assert_eq!(cmd.format, KittyFormat::Png);
    assert_eq!(cmd.image_id, 5);
    assert!(!cmd.more_chunks);
    // Payload should be concatenation of all decoded chunks.
    assert!(!cmd.payload.is_empty());
  }

  #[test]
  fn test_parse_delete() {
    let mut parser = KittyParser::new();
    let raw = b"a=d,d=a";
    let cmd = parser.parse(raw).expect("should produce command");

    assert_eq!(cmd.action, KittyAction::Delete);
    assert!(matches!(cmd.delete, Some(KittyDelete::All)));
  }

  #[test]
  fn test_parse_display_placement() {
    let mut parser = KittyParser::new();
    let raw = b"a=p,i=10,I=2,c=40,r=20,z=-1";
    let cmd = parser.parse(raw).expect("should produce command");

    assert_eq!(cmd.action, KittyAction::Display);
    assert_eq!(cmd.image_id, 10);
    assert_eq!(cmd.placement_id, 2);
    assert_eq!(cmd.display_columns, 40);
    assert_eq!(cmd.display_rows, 20);
    assert_eq!(cmd.z_index, -1);
  }

  #[test]
  fn test_no_payload() {
    let mut parser = KittyParser::new();
    let raw = b"a=d,d=a,i=5";
    let cmd = parser.parse(raw).expect("should produce command");
    assert!(cmd.payload.is_empty());
  }
}
