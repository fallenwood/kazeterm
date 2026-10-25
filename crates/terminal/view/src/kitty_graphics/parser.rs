use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use terminal_kernel::protocols::pty_filter::MAX_GRAPHICS_APC_BYTES;

use super::command::{
  KittyAction, KittyCommand, KittyDelete, KittyFormat, KittyResponseContext, KittyTransmission,
  MAX_CHUNK_BYTES, MAX_IMAGE_BYTES,
};

#[derive(Debug)]
pub struct KittyParseError {
  pub context: KittyResponseContext,
  pub message: String,
}

impl std::fmt::Display for KittyParseError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.message)
  }
}

impl std::error::Error for KittyParseError {}

/// Bounded parser for direct Kitty graphics transfers. Only a validated final
/// chunk produces a command; a failed upload is drained without displaying it.
pub struct KittyParser {
  chunk_buffer: Vec<u8>,
  pending_command: Option<KittyCommand>,
  discarding: bool,
  max_payload_bytes: usize,
}

impl KittyParser {
  pub fn new() -> Self {
    Self {
      chunk_buffer: Vec::new(),
      pending_command: None,
      discarding: false,
      max_payload_bytes: MAX_IMAGE_BYTES,
    }
  }

  /// Parse everything between APC G and ST. Errors never become empty images.
  pub fn parse(&mut self, raw: &[u8]) -> Result<Option<KittyCommand>, KittyParseError> {
    let (params, payload) = split_params_payload(raw);
    let explicit_action = params.split(|b| *b == b',').any(|p| p.starts_with(b"a="));
    let more = params.split(|b| *b == b',').any(|p| p == b"m=1");

    if self.discarding && !explicit_action {
      if !more {
        self.reset();
      }
      return Ok(None);
    }
    let incoming_context = response_context(params);
    let mut context = if !explicit_action {
      self
        .pending_command
        .as_ref()
        .map(KittyResponseContext::from)
        .unwrap_or(incoming_context)
    } else {
      incoming_context
    };
    if incoming_context.quiet != 0 {
      context.quiet = incoming_context.quiet;
    }
    let result = if raw.len() >= MAX_GRAPHICS_APC_BYTES - 1 {
      Err("E2BIG:Graphics APC exceeds the control-data limit".into())
    } else {
      self.parse_chunk(params, payload, explicit_action)
    };
    result.map_err(|message| {
      self.reset();
      self.discarding = more;
      KittyParseError { context, message }
    })
  }

  fn parse_chunk(
    &mut self,
    params: &[u8],
    payload: &[u8],
    explicit_action: bool,
  ) -> Result<Option<KittyCommand>, String> {
    let mut cmd = parse_params(params)?;
    if explicit_action {
      // In particular, every delete aborts an incomplete transfer, even when
      // the deletion itself has no matching image.
      self.reset();
    } else if self.pending_command.is_some()
      && params
        .split(|b| *b == b',')
        .any(|p| !p.is_empty() && !p.starts_with(b"m=") && !p.starts_with(b"q="))
    {
      return Err("EINVAL:Continuation chunks may only specify m and q".into());
    }

    if !matches!(
      cmd.action,
      KittyAction::Transmit | KittyAction::TransmitAndDisplay | KittyAction::Query
    ) {
      if cmd.more_chunks || !payload.is_empty() {
        return Err("EINVAL:This action does not accept image data or chunks".into());
      }
      return Ok(Some(cmd));
    }
    if payload.len() > MAX_CHUNK_BYTES {
      return Err("E2BIG:Base64 chunk exceeds 4096 bytes".into());
    }
    let decoded = BASE64
      .decode(payload)
      .map_err(|_| "EINVAL:Invalid base64 image data".to_string())?;
    if self.chunk_buffer.len().saturating_add(decoded.len()) > self.max_payload_bytes {
      return Err("E2BIG:Image transfer exceeds the memory quota".into());
    }

    if let Some(pending) = self.pending_command.as_mut() {
      if cmd.quiet != 0 {
        pending.quiet = cmd.quiet;
      }
      self.chunk_buffer.extend_from_slice(&decoded);
      if cmd.more_chunks {
        return Ok(None);
      }
      let mut pending = self.pending_command.take().unwrap();
      pending.payload = std::mem::take(&mut self.chunk_buffer);
      pending.more_chunks = false;
      Ok(Some(pending))
    } else if cmd.more_chunks {
      self.pending_command = Some(cmd);
      self.chunk_buffer = decoded;
      Ok(None)
    } else {
      cmd.payload = decoded;
      Ok(Some(cmd))
    }
  }

  pub fn reset(&mut self) {
    self.chunk_buffer = Vec::new();
    self.pending_command = None;
    self.discarding = false;
  }
}

fn split_params_payload(raw: &[u8]) -> (&[u8], &[u8]) {
  match raw.iter().position(|&b| b == b';') {
    Some(pos) => (&raw[..pos], &raw[pos + 1..]),
    None => (raw, &[]),
  }
}

// Extract only bounded numeric reply metadata before validating the rest of
// the header. An unsupported action/format can precede i= and q= in any order.
fn response_context(params: &[u8]) -> KittyResponseContext {
  let mut context = KittyResponseContext::default();
  for pair in params.split(|b| *b == b',') {
    let Ok(pair) = std::str::from_utf8(pair) else {
      continue;
    };
    let Some((key, value)) = pair.split_once('=') else {
      continue;
    };
    match key {
      "i" => context.image_id = unsigned(value).unwrap_or(0),
      "I" => context.image_number = unsigned(value).unwrap_or(0),
      "p" => context.placement_id = unsigned(value).unwrap_or(0),
      "q" => context.quiet = unsigned(value).ok().filter(|q| *q <= 2).unwrap_or(0) as u8,
      _ => {}
    }
  }
  context
}

fn unsigned(value: &str) -> Result<u32, String> {
  if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
    return Err("EINVAL:Expected an unsigned 32-bit integer".into());
  }
  value
    .parse()
    .map_err(|_| "EINVAL:Integer exceeds 32-bit range".into())
}

fn parse_params(params: &[u8]) -> Result<KittyCommand, String> {
  let params = std::str::from_utf8(params).map_err(|_| "EINVAL:Non-ASCII control data")?;
  let mut cmd = KittyCommand::default();
  let mut delete = b'a';
  for pair in params.split(',').filter(|pair| !pair.is_empty()) {
    let (key, value) = pair.split_once('=').ok_or("EINVAL:Expected key=value")?;
    if key.len() != 1 || !key.is_ascii() || !value.is_ascii() {
      return Err("EINVAL:Invalid control data".into());
    }
    match key {
      "a" => {
        cmd.action = match value {
          "t" => KittyAction::Transmit,
          "T" => KittyAction::TransmitAndDisplay,
          "p" => KittyAction::Display,
          "d" => KittyAction::Delete,
          "q" => KittyAction::Query,
          "f" | "a" | "c" => return Err("ENOTSUP:Animation is not supported".into()),
          _ => return Err("EINVAL:Unknown graphics action".into()),
        }
      }
      "f" => {
        cmd.format = match unsigned(value)? {
          24 => KittyFormat::Rgb,
          32 => KittyFormat::Rgba,
          100 => KittyFormat::Png,
          _ => return Err("ENOTSUP:Unsupported image format".into()),
        }
      }
      "t" => {
        cmd.transmission = match value {
          "d" => KittyTransmission::Direct,
          "f" | "t" | "s" => return Err("ENOTSUP:Only direct transmission is supported".into()),
          _ => return Err("EINVAL:Unknown transmission medium".into()),
        }
      }
      "o" => match value {
        "z" => cmd.compressed = true,
        _ => return Err("ENOTSUP:Unsupported compression".into()),
      },
      "i" => cmd.image_id = unsigned(value)?,
      "I" => cmd.image_number = unsigned(value)?,
      "p" => cmd.placement_id = unsigned(value)?,
      "s" => cmd.source_width = unsigned(value)?,
      "v" => cmd.source_height = unsigned(value)?,
      "S" => cmd.data_size = unsigned(value)?,
      "c" => cmd.display_columns = unsigned(value)?,
      "r" => cmd.display_rows = unsigned(value)?,
      "x" => cmd.crop_x = unsigned(value)?,
      "y" => cmd.crop_y = unsigned(value)?,
      "w" => cmd.crop_width = unsigned(value)?,
      "h" => cmd.crop_height = unsigned(value)?,
      "X" => cmd.x_offset = unsigned(value)?,
      "Y" => cmd.y_offset = unsigned(value)?,
      "z" => cmd.z_index = value.parse().map_err(|_| "EINVAL:Invalid signed z-index")?,
      "m" => {
        cmd.more_chunks = match value {
          "0" => false,
          "1" => true,
          _ => return Err("EINVAL:m must be 0 or 1".into()),
        }
      }
      "q" => {
        let quiet = unsigned(value)?;
        if quiet > 2 {
          return Err("EINVAL:q must be 0, 1 or 2".into());
        }
        cmd.quiet = quiet as u8;
      }
      "C" => {
        let policy = unsigned(value)?;
        if policy > 1 {
          return Err("EINVAL:C must be 0 or 1".into());
        }
        cmd.cursor_movement = policy as u8;
      }
      "d" => {
        if value.len() != 1 {
          return Err("EINVAL:Invalid delete selector".into());
        }
        delete = value.as_bytes()[0];
      }
      "U" | "P" | "Q" | "H" | "V" if value != "0" => {
        return Err("ENOTSUP:Virtual and relative placements are not supported".into());
      }
      // Usage hints and unknown keys have no effect on this implementation.
      _ => {}
    }
  }
  if cmd.image_id != 0 && cmd.image_number != 0 {
    return Err("EINVAL:Cannot specify both image id and image number".into());
  }
  if cmd.action == KittyAction::Delete {
    cmd.delete_data = delete.is_ascii_uppercase();
    let placement_id = (cmd.placement_id != 0).then_some(cmd.placement_id);
    cmd.delete = Some(match delete.to_ascii_lowercase() {
      b'a' => KittyDelete::All,
      b'i' => KittyDelete::ById {
        image_id: cmd.image_id,
        placement_id,
      },
      b'n' => KittyDelete::ByNumber {
        image_number: cmd.image_number,
        placement_id,
      },
      b'c' => KittyDelete::AtCursor,
      b'p' | b'q' => KittyDelete::AtCell {
        x: cmd.crop_x,
        y: cmd.crop_y,
        z_index: (delete.eq_ignore_ascii_case(&b'q')).then_some(cmd.z_index),
      },
      b'x' => KittyDelete::AtColumn(cmd.crop_x),
      b'y' => KittyDelete::AtRow(cmd.crop_y),
      b'z' => KittyDelete::ByZIndex(cmd.z_index),
      b'r' => KittyDelete::IdRange {
        first: cmd.crop_x,
        last: cmd.crop_y,
      },
      b'f' => return Err("ENOTSUP:Animation is not supported".into()),
      _ => return Err("EINVAL:Unknown delete selector".into()),
    });
  }
  Ok(cmd)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn command(raw: &[u8]) -> KittyCommand {
    KittyParser::new().parse(raw).unwrap().unwrap()
  }

  #[test]
  fn default_action_and_identifiers_match_kitty() {
    let cmd = command(b"i=7,p=3,s=1,v=1;AAAAAA==");
    assert_eq!(cmd.action, KittyAction::Transmit);
    assert_eq!(
      (cmd.image_id, cmd.image_number, cmd.placement_id),
      (7, 0, 3)
    );
    assert_eq!(cmd.payload, [0; 4]);
    let cmd = command(b"a=T,f=100,I=9,p=2,c=40,r=20,z=-1;iVBORw0KGgo=");
    assert_eq!(cmd.action, KittyAction::TransmitAndDisplay);
    assert_eq!(cmd.format, KittyFormat::Png);
    assert_eq!(
      (cmd.image_number, cmd.placement_id, cmd.z_index),
      (9, 2, -1)
    );
    assert_eq!((cmd.display_columns, cmd.display_rows), (40, 20));
    assert_eq!(
      command(b"a=q,i=31,s=1,v=1,f=24;AAAA").action,
      KittyAction::Query
    );
    assert_eq!(command(b"a=p,i=7,p=2").action, KittyAction::Display);
  }

  #[test]
  fn delete_targets_are_order_independent_and_case_sensitive() {
    for raw in [b"a=d,d=i,i=7,p=3".as_slice(), b"p=3,i=7,d=i,a=d"] {
      let cmd = command(raw);
      assert_eq!(
        cmd.delete,
        Some(KittyDelete::ById {
          image_id: 7,
          placement_id: Some(3)
        })
      );
      assert!(!cmd.delete_data);
    }
    assert!(command(b"a=d,d=I,i=7").delete_data);
    assert_eq!(
      command(b"a=d,d=n,I=9").delete,
      Some(KittyDelete::ByNumber {
        image_number: 9,
        placement_id: None
      })
    );
    assert_eq!(
      command(b"a=d,d=q,x=3,y=4,z=-1").delete,
      Some(KittyDelete::AtCell {
        x: 3,
        y: 4,
        z_index: Some(-1)
      })
    );
    assert_eq!(
      command(b"a=d,d=x,x=3").delete,
      Some(KittyDelete::AtColumn(3))
    );
    assert_eq!(command(b"a=d,d=y,y=4").delete, Some(KittyDelete::AtRow(4)));
    assert_eq!(
      command(b"a=d,d=Z,z=-1").delete,
      Some(KittyDelete::ByZIndex(-1))
    );
    assert_eq!(
      command(b"a=d,d=r,x=2,y=4").delete,
      Some(KittyDelete::IdRange { first: 2, last: 4 })
    );
    assert_eq!(command(b"a=d").delete, Some(KittyDelete::All));
  }

  #[test]
  fn chunked_upload_preserves_metadata_and_delete_aborts_it() {
    let mut parser = KittyParser::new();
    assert!(
      parser
        .parse(b"a=T,f=24,i=5,p=2,s=2,v=1,m=1;AAAA")
        .unwrap()
        .is_none()
    );
    let cmd = parser.parse(b"m=0,q=1;BBBB").unwrap().unwrap();
    assert_eq!((cmd.image_id, cmd.placement_id, cmd.quiet), (5, 2, 1));
    assert_eq!(cmd.payload, [0, 0, 0, 4, 16, 65]);
    assert!(!cmd.more_chunks);
    assert_eq!(cmd.action, KittyAction::TransmitAndDisplay);

    parser.parse(b"i=8,m=1;AAAA").unwrap();
    assert_eq!(
      parser.parse(b"a=d").unwrap().unwrap().action,
      KittyAction::Delete
    );
    assert!(parser.pending_command.is_none());
    assert!(parser.chunk_buffer.is_empty());
    let cmd = parser.parse(b"a=T,i=9;AAAA").unwrap().unwrap();
    assert_eq!(cmd.payload, [0; 3]);
  }

  #[test]
  fn errors_retain_reply_identity_and_quiet_across_chunks() {
    for raw in [b"f=99,i=7,p=3,q=2".as_slice(), b"i=7,p=3,q=2,f=99"] {
      let error = KittyParser::new().parse(raw).unwrap_err();
      assert_eq!(
        error.context,
        KittyResponseContext {
          image_id: 7,
          image_number: 0,
          placement_id: 3,
          quiet: 2
        }
      );
    }
    let mut parser = KittyParser::new();
    parser.parse(b"a=T,I=9,p=4,s=1,v=1,m=1,q=1;AAAA").unwrap();
    let error = parser.parse(b"m=1,q=2;***").unwrap_err();
    assert_eq!(
      error.context,
      KittyResponseContext {
        image_id: 0,
        image_number: 9,
        placement_id: 4,
        quiet: 2
      }
    );
    assert!(parser.parse(b"m=0;AAAA").unwrap().is_none()); // no duplicate error
    parser.parse(b"a=T,i=10,p=2,s=1,v=1,m=1,q=1;AAAA").unwrap();
    let error = parser.parse(b"m=0;***").unwrap_err();
    assert_eq!(
      (
        error.context.image_id,
        error.context.placement_id,
        error.context.quiet
      ),
      (10, 2, 1)
    );
  }

  #[test]
  fn invalid_commands_and_failed_transfers_do_not_become_images() {
    for raw in [
      "a=invalid",
      "f=99",
      "t=f",
      "t=s",
      "o=x",
      "a=f",
      "a=p,U=1",
      "a=p,P=1",
      "i=1,I=2",
      "i=-1",
      "i=4294967296",
      "r=abc",
      "z=2147483648",
      "q=3",
      "m=2",
      "C=2",
      "a=d,d=unknown",
      "i=1;***",
      "a=p,i=1;AAAA",
      "i=+1",
      "missing-equals",
    ] {
      assert!(KittyParser::new().parse(raw.as_bytes()).is_err(), "{raw}");
    }
    // A truncated oversized APC must not accidentally execute an early a=d.
    let oversized_control = format!("a=d,k={}", "a".repeat(MAX_GRAPHICS_APC_BYTES));
    assert!(
      KittyParser::new()
        .parse(oversized_control.as_bytes())
        .is_err()
    );
    let mut parser = KittyParser::new();
    parser.max_payload_bytes = 5;
    parser.parse(b"i=1,m=1;AAAA").unwrap();
    assert!(parser.parse(b"m=1;AAAA").is_err());
    assert!(parser.chunk_buffer.is_empty());
    assert!(parser.parse(b"m=0;AAAA").unwrap().is_none());
    assert_eq!(parser.parse(b"i=2;AAAA").unwrap().unwrap().payload, [0; 3]);

    parser.parse(b"i=3,m=1;AAAA").unwrap();
    assert!(parser.parse(b"m=1;***").is_err());
    assert!(parser.parse(b"m=0;AAAA").unwrap().is_none());
    let oversized = format!("i=4;{}", "A".repeat(MAX_CHUNK_BYTES + 4));
    assert!(parser.parse(oversized.as_bytes()).is_err());
    parser.parse(b"i=5,m=1;AAAA").unwrap();
    assert!(parser.parse(b"i=5,m=0;AAAA").is_err());
  }
}
