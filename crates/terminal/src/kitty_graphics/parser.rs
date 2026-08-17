use std::io::Read;

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use flate2::read::ZlibDecoder;

use super::command::{
  KittyAction, KittyCommand, KittyCompression, KittyDelete, KittyErrorCode, KittyFormat,
  KittyProtocolError, KittyTransmission,
};

pub const MAX_APC_BYTES: usize = 64 * 1024;
pub const MAX_CHUNK_ENCODED_BYTES: usize = 4096;
pub const MAX_UPLOAD_BYTES: usize = 320 * 1024 * 1024;
pub const MAX_IMAGE_DIMENSION: u32 = 32_768;

struct ParsedControl {
  command: KittyCommand,
  continuation_only: bool,
  quiet_specified: bool,
}

/// Parser for Kitty graphics protocol commands.
///
/// Handles chunked transfers by accumulating payload data across APC sequences
/// until `m=0`, while enforcing the protocol's size and ordering rules.
pub struct KittyParser {
  chunk_buffer: Vec<u8>,
  pending_command: Option<KittyCommand>,
}

impl KittyParser {
  pub fn new() -> Self {
    Self {
      chunk_buffer: Vec::new(),
      pending_command: None,
    }
  }

  pub fn parse(
    &mut self,
    raw: &[u8],
  ) -> Result<Option<KittyCommand>, KittyProtocolError> {
    if raw.len() > MAX_APC_BYTES {
      return Err(KittyProtocolError::new(
        KittyErrorCode::NoSpace,
        "graphics APC exceeds 64 KiB",
      ));
    }

    let (params_bytes, payload_bytes) = split_params_payload(raw);
    let parsed = parse_params(params_bytes)?;
    let mut command = parsed.command;

    if payload_bytes.len() > MAX_CHUNK_ENCODED_BYTES {
      return Err(KittyProtocolError::invalid(
        "encoded payload chunk exceeds 4096 bytes",
      )
      .with_command(&command));
    }
    if command.more_chunks && payload_bytes.len() % 4 != 0 {
      return Err(KittyProtocolError::invalid(
        "non-final base64 chunk size must be a multiple of four",
      )
      .with_command(&command));
    }

    let decoded = if payload_bytes.is_empty() {
      Vec::new()
    } else {
      BASE64.decode(payload_bytes).map_err(|error| {
        KittyProtocolError::invalid(format!("invalid base64 payload: {error}"))
          .with_command(&command)
      })?
    };

    if command.action == KittyAction::Delete {
      self.reset();
      command.payload = decoded;
      return Ok(Some(command));
    }

    if self.pending_command.is_some() {
      if !parsed.continuation_only {
        self.reset();
        return Err(KittyProtocolError::invalid(
          "chunked upload interrupted by another graphics command",
        )
        .with_command(&command));
      }
      if self.chunk_buffer.len().saturating_add(decoded.len()) > MAX_UPLOAD_BYTES {
        self.reset();
        return Err(KittyProtocolError::new(
          KittyErrorCode::NoSpace,
          "chunked upload exceeds 320 MiB",
        )
        .with_command(&command));
      }

      self.chunk_buffer.extend_from_slice(&decoded);
      if command.more_chunks {
        if parsed.quiet_specified
          && let Some(pending) = &mut self.pending_command
        {
          pending.quiet = command.quiet;
        }
        return Ok(None);
      }

      let mut pending = self.pending_command.take().expect("pending command checked");
      if parsed.quiet_specified {
        pending.quiet = command.quiet;
      }
      pending.payload = std::mem::take(&mut self.chunk_buffer);
      pending.more_chunks = false;
      finish_payload(&mut pending)?;
      Ok(Some(pending))
    } else if command.more_chunks {
      self.pending_command = Some(command);
      self.chunk_buffer = decoded;
      Ok(None)
    } else {
      command.payload = decoded;
      finish_payload(&mut command)?;
      Ok(Some(command))
    }
  }

  pub fn reset(&mut self) {
    self.chunk_buffer.clear();
    self.pending_command = None;
  }
}

fn split_params_payload(raw: &[u8]) -> (&[u8], &[u8]) {
  if let Some(position) = raw.iter().position(|&byte| byte == b';') {
    (&raw[..position], &raw[position + 1..])
  } else {
    (raw, &[])
  }
}

fn parse_params(params: &[u8]) -> Result<ParsedControl, KittyProtocolError> {
  let mut command = KittyCommand::default();
  let params = std::str::from_utf8(params)
    .map_err(|_| KittyProtocolError::invalid("control data is not valid UTF-8"))?;
  let mut pairs = Vec::new();

  for pair in params.split(',') {
    let pair = pair.trim();
    if pair.is_empty() {
      continue;
    }
    let (key, value) = pair
      .split_once('=')
      .ok_or_else(|| KittyProtocolError::invalid("control field is missing '='"))?;
    pairs.push((key, value));
  }

  for &(key, value) in &pairs {
    match key {
      "i" => command.image_id = parse_u32(key, value, &command)?,
      "I" => command.image_number = parse_u32(key, value, &command)?,
      "p" => command.placement_id = parse_u32(key, value, &command)?,
      "q" => {
        command.quiet = parse_u8(key, value, &command)?;
        if command.quiet > 2 {
          return Err(
            KittyProtocolError::invalid("q must be 0, 1, or 2").with_command(&command)
          );
        }
      }
      _ => {}
    }
  }

  if command.image_id != 0 && command.image_number != 0 {
    return Err(
      KittyProtocolError::invalid("i and I are mutually exclusive").with_command(&command)
    );
  }

  let mut delete_selector = None;
  for &(key, value) in &pairs {
    match key {
      "a" => {
        command.action = match value {
          "t" => KittyAction::Transmit,
          "T" => KittyAction::TransmitAndDisplay,
          "p" => KittyAction::Display,
          "d" => KittyAction::Delete,
          "q" => KittyAction::Query,
          "f" => KittyAction::TransmitFrame,
          "a" => KittyAction::ControlAnimation,
          "c" => KittyAction::ComposeFrames,
          _ => {
            return Err(
              KittyProtocolError::invalid(format!("unsupported action: {value}"))
                .with_command(&command),
            );
          }
        };
      }
      "f" => {
        command.format = match value {
          "24" => KittyFormat::Rgb,
          "32" => KittyFormat::Rgba,
          "100" => KittyFormat::Png,
          _ => {
            return Err(
              KittyProtocolError::invalid(format!("unsupported format: {value}"))
                .with_command(&command),
            );
          }
        };
      }
      "t" => {
        command.transmission = match value {
          "d" => KittyTransmission::Direct,
          "f" => KittyTransmission::File,
          "t" => KittyTransmission::TemporaryFile,
          "s" => KittyTransmission::SharedMemory,
          _ => {
            return Err(
              KittyProtocolError::invalid(format!("unsupported transmission: {value}"))
                .with_command(&command),
            );
          }
        };
      }
      "o" => {
        command.compression = match value {
          "z" => Some(KittyCompression::Zlib),
          _ => {
            return Err(
              KittyProtocolError::invalid(format!("unsupported compression: {value}"))
                .with_command(&command),
            );
          }
        };
      }
      "s" => command.source_width = parse_u32(key, value, &command)?,
      "v" => command.source_height = parse_u32(key, value, &command)?,
      "S" => command.data_size = parse_u32(key, value, &command)?,
      "O" | "N" => {
        let _ = parse_u32(key, value, &command)?;
      }
      "c" => command.display_columns = parse_u32(key, value, &command)?,
      "r" => command.display_rows = parse_u32(key, value, &command)?,
      "x" => command.crop_x = parse_u32(key, value, &command)?,
      "y" => command.crop_y = parse_u32(key, value, &command)?,
      "w" => command.crop_width = parse_u32(key, value, &command)?,
      "h" => command.crop_height = parse_u32(key, value, &command)?,
      "X" => command.x_offset = parse_u32(key, value, &command)?,
      "Y" => command.y_offset = parse_u32(key, value, &command)?,
      "z" => command.z_index = parse_i32(key, value, &command)?,
      "m" => {
        let more = parse_u8(key, value, &command)?;
        if more > 1 {
          return Err(
            KittyProtocolError::invalid("m must be 0 or 1").with_command(&command)
          );
        }
        command.more_chunks = more == 1;
      }
      "C" => {
        command.cursor_movement = parse_u8(key, value, &command)?;
        if command.cursor_movement > 1 {
          return Err(
            KittyProtocolError::invalid("C must be 0 or 1").with_command(&command)
          );
        }
      }
      "U" => {
        let virtual_placement = parse_u8(key, value, &command)?;
        if virtual_placement > 1 {
          return Err(
            KittyProtocolError::invalid("U must be 0 or 1").with_command(&command)
          );
        }
        command.virtual_placement = virtual_placement == 1;
      }
      "P" => command.parent_image_id = parse_u32(key, value, &command)?,
      "Q" => command.parent_placement_id = parse_u32(key, value, &command)?,
      "H" => command.relative_x = parse_i32(key, value, &command)?,
      "V" => command.relative_y = parse_i32(key, value, &command)?,
      "d" => delete_selector = Some(value),
      "i" | "I" | "p" | "q" => {}
      _ => {}
    }
  }

  if command.source_width > MAX_IMAGE_DIMENSION
    || command.source_height > MAX_IMAGE_DIMENSION
  {
    return Err(
      KittyProtocolError::invalid("image dimensions exceed 32768 pixels")
        .with_command(&command),
    );
  }
  if command.data_size as usize > MAX_UPLOAD_BYTES {
    return Err(
      KittyProtocolError::new(KittyErrorCode::NoSpace, "declared data size exceeds 320 MiB")
        .with_command(&command),
    );
  }

  if command.action == KittyAction::Delete {
    let selector = delete_selector.unwrap_or("a");
    let (delete, delete_data) = parse_delete(selector, &command)?;
    command.delete = Some(delete);
    command.delete_data = delete_data;
  }

  let continuation_only = pairs.iter().all(|(key, _)| matches!(*key, "m" | "q"));
  let quiet_specified = pairs.iter().any(|(key, _)| *key == "q");
  Ok(ParsedControl {
    command,
    continuation_only,
    quiet_specified,
  })
}

fn parse_delete(
  value: &str,
  command: &KittyCommand,
) -> Result<(KittyDelete, bool), KittyProtocolError> {
  if value.len() != 1 {
    return Err(
      KittyProtocolError::invalid("d must be one selector character").with_command(command)
    );
  }

  let selector = value.as_bytes()[0];
  let delete_data = selector.is_ascii_uppercase();
  let delete = match selector {
    b'a' | b'A' => KittyDelete::All,
    b'i' | b'I' => KittyDelete::ById {
      image_id: command.image_id,
      placement_id: (command.placement_id != 0).then_some(command.placement_id),
    },
    b'n' | b'N' => KittyDelete::ByNumber {
      image_number: command.image_number,
      placement_id: (command.placement_id != 0).then_some(command.placement_id),
    },
    b'c' | b'C' => KittyDelete::AtCursor,
    b'p' | b'P' => KittyDelete::AtCell {
      column: command.crop_x,
      row: command.crop_y,
    },
    b'q' | b'Q' => KittyDelete::AtCellAndZIndex {
      column: command.crop_x,
      row: command.crop_y,
      z_index: command.z_index,
    },
    b'r' | b'R' => KittyDelete::ByIdRange {
      first: command.crop_x,
      last: command.crop_y,
    },
    b'x' | b'X' => KittyDelete::AtColumn(command.crop_x),
    b'y' | b'Y' => KittyDelete::AtRow(command.crop_y),
    b'z' | b'Z' => KittyDelete::ByZIndex(command.z_index),
    b'f' | b'F' => KittyDelete::AnimationFrames,
    _ => {
      return Err(
        KittyProtocolError::invalid(format!("unsupported delete selector: {value}"))
          .with_command(command),
      );
    }
  };
  Ok((delete, delete_data))
}

fn parse_u32(
  key: &str,
  value: &str,
  command: &KittyCommand,
) -> Result<u32, KittyProtocolError> {
  value.parse().map_err(|_| {
    KittyProtocolError::invalid(format!("{key} must be an unsigned 32-bit integer"))
      .with_command(command)
  })
}

fn parse_u8(
  key: &str,
  value: &str,
  command: &KittyCommand,
) -> Result<u8, KittyProtocolError> {
  value.parse().map_err(|_| {
    KittyProtocolError::invalid(format!("{key} must be an unsigned integer"))
      .with_command(command)
  })
}

fn parse_i32(
  key: &str,
  value: &str,
  command: &KittyCommand,
) -> Result<i32, KittyProtocolError> {
  value.parse().map_err(|_| {
    KittyProtocolError::invalid(format!("{key} must be a signed 32-bit integer"))
      .with_command(command)
  })
}

fn finish_payload(command: &mut KittyCommand) -> Result<(), KittyProtocolError> {
  if command.compression == Some(KittyCompression::Zlib) {
    if command.format == KittyFormat::Png && command.data_size == 0 {
      return Err(
        KittyProtocolError::invalid("compressed PNG requires S").with_command(command)
      );
    }

    let mut decoder = ZlibDecoder::new(command.payload.as_slice());
    let mut decompressed = Vec::new();
    decoder
      .by_ref()
      .take(MAX_UPLOAD_BYTES as u64 + 1)
      .read_to_end(&mut decompressed)
      .map_err(|error| {
        KittyProtocolError::invalid(format!("invalid zlib payload: {error}"))
          .with_command(command)
      })?;
    if decompressed.len() > MAX_UPLOAD_BYTES {
      return Err(
        KittyProtocolError::new(KittyErrorCode::NoSpace, "decompressed payload exceeds 320 MiB")
          .with_command(command),
      );
    }
    if command.data_size != 0 && decompressed.len() != command.data_size as usize {
      return Err(
        KittyProtocolError::invalid("decompressed payload size does not match S")
          .with_command(command),
      );
    }
    command.payload = decompressed;
  }

  if command.payload.len() > MAX_UPLOAD_BYTES {
    return Err(
      KittyProtocolError::new(KittyErrorCode::NoSpace, "image payload exceeds 320 MiB")
        .with_command(command),
    );
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn command(parser: &mut KittyParser, raw: &[u8]) -> KittyCommand {
    parser
      .parse(raw)
      .expect("valid command")
      .expect("complete command")
  }

  #[test]
  fn parses_transmit_and_display() {
    let mut parser = KittyParser::new();
    let parsed = command(&mut parser, b"a=T,f=100,i=1;iVBORw0KGgo=");

    assert_eq!(parsed.action, KittyAction::TransmitAndDisplay);
    assert_eq!(parsed.format, KittyFormat::Png);
    assert_eq!(parsed.image_id, 1);
    assert!(!parsed.payload.is_empty());
  }

  #[test]
  fn default_action_is_transmit() {
    let mut parser = KittyParser::new();
    let parsed = command(&mut parser, b"f=32,s=1,v=1;AAAAAA==");

    assert_eq!(parsed.action, KittyAction::Transmit);
  }

  #[test]
  fn parses_query() {
    let mut parser = KittyParser::new();
    let parsed = command(&mut parser, b"a=q,i=31,s=1,v=1,f=24;AAAA");

    assert_eq!(parsed.action, KittyAction::Query);
    assert_eq!(parsed.image_id, 31);
  }

  #[test]
  fn parses_chunked_transfer() {
    let mut parser = KittyParser::new();

    assert!(
      parser
        .parse(b"a=T,f=100,i=5,m=1;AAAA")
        .unwrap()
        .is_none()
    );
    assert!(parser.parse(b"m=1;BBBB").unwrap().is_none());
    let parsed = command(&mut parser, b"m=0;CCCC");

    assert_eq!(parsed.action, KittyAction::TransmitAndDisplay);
    assert_eq!(parsed.image_id, 5);
    assert!(!parsed.payload.is_empty());
  }

  #[test]
  fn parses_image_number_and_placement_id() {
    let mut parser = KittyParser::new();
    let parsed = command(&mut parser, b"a=p,I=13,p=7");

    assert_eq!(parsed.image_number, 13);
    assert_eq!(parsed.placement_id, 7);
    assert_eq!(parsed.image_id, 0);
  }

  #[test]
  fn parses_complete_delete_selector() {
    let mut parser = KittyParser::new();
    let parsed = command(&mut parser, b"a=d,d=Q,x=3,y=4,z=-2");

    assert!(parsed.delete_data);
    assert_eq!(
      parsed.delete,
      Some(KittyDelete::AtCellAndZIndex {
        column: 3,
        row: 4,
        z_index: -2,
      })
    );
  }

  #[test]
  fn rejects_invalid_base64() {
    let mut parser = KittyParser::new();
    let error = parser.parse(b"a=t,i=9;not-base64!").unwrap_err();

    assert_eq!(error.code, KittyErrorCode::Invalid);
    assert_eq!(error.image_id, 9);
  }

  #[test]
  fn rejects_conflicting_image_identity() {
    let mut parser = KittyParser::new();
    let error = parser.parse(b"a=p,i=1,I=2").unwrap_err();

    assert_eq!(error.code, KittyErrorCode::Invalid);
  }

  #[test]
  fn rejects_oversized_encoded_chunk() {
    let mut parser = KittyParser::new();
    let mut raw = b"a=t;".to_vec();
    raw.extend(std::iter::repeat_n(b'A', MAX_CHUNK_ENCODED_BYTES + 4));

    let error = parser.parse(&raw).unwrap_err();
    assert_eq!(error.code, KittyErrorCode::Invalid);
  }

  #[test]
  fn rejects_interrupted_chunked_upload() {
    let mut parser = KittyParser::new();
    assert!(parser.parse(b"a=t,m=1;AAAA").unwrap().is_none());

    let error = parser.parse(b"a=p,i=1").unwrap_err();
    assert_eq!(error.code, KittyErrorCode::Invalid);
  }
}
