use futures::{StreamExt, channel::mpsc::UnboundedReceiver};
use smol::channel::{Receiver, bounded};

use super::{
  KittyAction, KittyCommand, KittyParser, KittyResponse, KittyResponseContext, PreparedImage,
  RawGraphicsCommand, RawGraphicsEvent, prepare_image, validate_image,
};

const PREPARED_QUEUE_CAPACITY: usize = 2;
pub const GRAPHICS_BATCH_SIZE: usize = 2;

pub enum PreparedGraphicsEvent {
  ClearAll,
  /// A filter-generated reply queued behind preceding graphics work.
  Reply(Vec<u8>),
  /// A query/decoding/parse result that needs no mutation of terminal resources.
  Response(KittyResponse),
  Command {
    command: KittyCommand,
    image: Option<PreparedImage>,
    cursor_line: i32,
    cursor_column: i32,
  },
}

fn prepare_command(
  parser: &mut KittyParser,
  raw: RawGraphicsCommand,
) -> Option<PreparedGraphicsEvent> {
  let mut command = match parser.parse(&raw.data) {
    Ok(Some(command)) => command,
    Ok(None) => return None,
    Err(error) => {
      tracing::warn!("Invalid Kitty graphics command: {error}");
      return error
        .context
        .respond(Err(error.message))
        .map(PreparedGraphicsEvent::Response);
    }
  };
  let mut context = KittyResponseContext::from(&command);
  if command.action == KittyAction::Query {
    if command.image_id == 0 {
      return None; // Kitty queries require an explicit, nonzero image ID.
    }
    // Queries validate the full image, never store/replace it, and identify
    // only the probe ID in the response (not a placement).
    context.image_number = 0;
    context.placement_id = 0;
    let result = validate_image(&mut command).map(|()| command.image_id);
    return context.respond(result).map(PreparedGraphicsEvent::Response);
  }
  let image = if matches!(
    command.action,
    KittyAction::Transmit | KittyAction::TransmitAndDisplay
  ) {
    match prepare_image(&mut command) {
      Ok(image) => Some(image),
      Err(error) => {
        tracing::warn!("Failed to prepare Kitty image: {error}");
        return context
          .respond(Err(error))
          .map(PreparedGraphicsEvent::Response);
      }
    }
  } else {
    command.payload.clear();
    None
  };
  Some(PreparedGraphicsEvent::Command {
    command,
    image,
    cursor_line: raw.cursor_line,
    cursor_column: raw.cursor_column,
  })
}

/// The caller schedules this future on its background executor and owns its
/// cancellable task. No blocking receive or private OS thread is needed; the
/// exact same worker can run on GPUI's deterministic test scheduler.
pub fn graphics_worker(
  mut raw_rx: UnboundedReceiver<RawGraphicsEvent>,
) -> (
  Receiver<PreparedGraphicsEvent>,
  impl Future<Output = ()> + Send + 'static,
) {
  let (prepared_tx, prepared_rx) = bounded(PREPARED_QUEUE_CAPACITY);
  let worker = async move {
    let mut parser = KittyParser::new();
    while let Some(raw) = raw_rx.next().await {
      let event = match raw {
        RawGraphicsEvent::ClearAll => {
          parser.reset();
          Some(PreparedGraphicsEvent::ClearAll)
        }
        RawGraphicsEvent::Reply(reply) => Some(PreparedGraphicsEvent::Reply(reply)),
        RawGraphicsEvent::Command(raw) => prepare_command(&mut parser, raw),
      };
      if let Some(event) = event
        && prepared_tx.send(event).await.is_err()
      {
        break;
      }
    }
  };
  (prepared_rx, worker)
}

// Ordinary Rust tests have no GPUI executor. Drive the identical production
// future on smol, never a different decoding implementation or protocol path.
#[cfg(test)]
pub(crate) fn spawn_test_graphics_worker(
  raw_rx: UnboundedReceiver<RawGraphicsEvent>,
) -> Receiver<PreparedGraphicsEvent> {
  let (events, worker) = graphics_worker(raw_rx);
  std::thread::spawn(move || smol::block_on(worker));
  events
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::channel::mpsc::unbounded;

  fn raw(data: impl Into<Vec<u8>>, line: i32) -> RawGraphicsEvent {
    RawGraphicsEvent::Command(RawGraphicsCommand {
      data: data.into(),
      cursor_line: line,
      cursor_column: 7,
    })
  }

  #[test]
  fn worker_prepares_images_and_releases_encoded_payloads() {
    let (raw_tx, raw_rx) = unbounded();
    let prepared_rx = spawn_test_graphics_worker(raw_rx);
    raw_tx
      .unbounded_send(raw(b"a=T,f=32,i=7,s=1,v=1;/wAA/w==".to_vec(), 4))
      .unwrap();
    let event = prepared_rx.recv_blocking().unwrap();
    match event {
      PreparedGraphicsEvent::Command {
        command,
        image: Some(image),
        cursor_line,
        cursor_column,
      } => {
        assert_eq!(command.image_id, 7);
        assert!(command.payload.is_empty());
        assert_eq!((image.width, image.height, image.memory_bytes), (1, 1, 4));
        assert_eq!((cursor_line, cursor_column), (4, 7));
      }
      _ => panic!("expected a prepared image command"),
    }
  }

  #[test]
  fn worker_handles_chunked_zlib_and_recovers_after_invalid_uploads() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use std::io::Write;
    let (raw_tx, raw_rx) = unbounded();
    let prepared_rx = spawn_test_graphics_worker(raw_rx);
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&[255, 0, 0, 255]).unwrap();
    let encoded = STANDARD.encode(encoder.finish().unwrap());
    let commands = [
      "a=T,i=8,s=1,v=1,m=1,q=2;***".to_string(),
      "m=0;AAAA".to_string(),
      format!("a=T,i=9,p=3,s=1,v=1,o=z,m=1;{}", &encoded[..4]),
      format!("m=0;{}", &encoded[4..]),
    ];
    for (index, data) in commands.into_iter().enumerate() {
      raw_tx
        .unbounded_send(raw(data.into_bytes(), index as i32))
        .unwrap();
    }
    drop(raw_tx);
    match prepared_rx.recv_blocking().unwrap() {
      PreparedGraphicsEvent::Command {
        command,
        image: Some(image),
        cursor_line,
        cursor_column,
      } => {
        assert_eq!((command.image_id, command.placement_id), (9, 3));
        assert_eq!((image.width, image.height), (1, 1));
        assert!(command.payload.is_empty());
        assert_eq!((cursor_line, cursor_column), (3, 7));
      }
      _ => panic!("expected a decoded compressed image"),
    }
    assert!(prepared_rx.recv_blocking().is_err());
  }

  #[test]
  fn queries_and_errors_precede_deferred_replies_and_honor_quiet() {
    let (tx, rx) = unbounded();
    let events = spawn_test_graphics_worker(rx);
    for command in [
      "a=q,i=31,f=24,s=1,v=1;AAAA",
      "a=q,i=32,f=24,s=1,v=1,q=1;AAAA",
      "a=q,i=33,f=24,s=1,v=1,q=1;AA==",
      "a=q,i=34,f=24,s=1,v=1,q=2;AA==",
      "f=99,i=35,q=2;AAAA",
      "f=99,i=36,p=3;AAAA",
    ] {
      tx.unbounded_send(raw(command.as_bytes().to_vec(), 0))
        .unwrap();
    }
    tx.unbounded_send(RawGraphicsEvent::Reply(b"\x1b[?62;22c".to_vec()))
      .unwrap();
    drop(tx);
    for (id, expected_prefix) in [(31, "OK"), (33, "ENODATA:"), (36, "ENOTSUP:")] {
      let PreparedGraphicsEvent::Response(response) = events.recv_blocking().unwrap() else {
        panic!("expected response")
      };
      assert_eq!(response.image_id, id);
      assert!(response.message.starts_with(expected_prefix));
    }
    assert!(
      matches!(events.recv_blocking().unwrap(), PreparedGraphicsEvent::Reply(reply) if reply == b"\x1b[?62;22c")
    );
    assert!(events.recv_blocking().is_err());
  }

  #[test]
  fn worker_preserves_clear_order_and_aborts_partial_uploads() {
    let (tx, rx) = unbounded();
    let events = spawn_test_graphics_worker(rx);
    tx.unbounded_send(raw(b"i=1,s=2,v=1,m=1;AAAA".to_vec(), 0))
      .unwrap();
    tx.unbounded_send(RawGraphicsEvent::ClearAll).unwrap();
    tx.unbounded_send(raw(b"a=q,i=2,f=24,s=1,v=1;AAAA".to_vec(), 0))
      .unwrap();
    drop(tx);
    assert!(matches!(
      events.recv_blocking().unwrap(),
      PreparedGraphicsEvent::ClearAll
    ));
    assert!(
      matches!(events.recv_blocking().unwrap(), PreparedGraphicsEvent::Response(response) if response.image_id == 2 && response.ok)
    );
    assert!(events.recv_blocking().is_err());
  }
}
