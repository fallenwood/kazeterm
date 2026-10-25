use std::sync::mpsc::Receiver as RawReceiver;

use smol::channel::{Receiver, bounded};

use super::{
  KittyAction, KittyCommand, KittyParser, PreparedImage, RawGraphicsCommand, prepare_image,
};

const PREPARED_QUEUE_CAPACITY: usize = 2;
pub const GRAPHICS_BATCH_SIZE: usize = 2;

pub enum PreparedGraphicsEvent {
  ClearAll,
  Command {
    command: KittyCommand,
    image: Option<PreparedImage>,
    cursor_line: i32,
    cursor_column: i32,
  },
}

pub fn spawn_graphics_worker(
  raw_rx: RawReceiver<RawGraphicsCommand>,
) -> Receiver<PreparedGraphicsEvent> {
  let (prepared_tx, prepared_rx) = bounded(PREPARED_QUEUE_CAPACITY);
  let spawn_result = std::thread::Builder::new()
    .name("kazeterm-kitty-decoder".to_string())
    .spawn(move || {
      let mut parser = KittyParser::new();
      while let Ok(raw) = raw_rx.recv() {
        let event = if raw.clear_all {
          parser.reset();
          Some(PreparedGraphicsEvent::ClearAll)
        } else {
          parser.parse(&raw.data).and_then(|mut command| {
            let image = if matches!(
              command.action,
              KittyAction::Transmit | KittyAction::TransmitAndDisplay
            ) {
              match prepare_image(&mut command) {
                Ok(image) => Some(image),
                Err(error) => {
                  tracing::warn!("Failed to prepare Kitty image: {error}");
                  return None;
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
          })
        };

        if let Some(event) = event
          && prepared_tx.send_blocking(event).is_err()
        {
          break;
        }
      }
    });

  if let Err(error) = spawn_result {
    tracing::error!("Failed to start Kitty graphics worker: {error}");
  }

  prepared_rx
}

#[cfg(test)]
mod tests {
  use std::sync::mpsc;

  use super::*;

  #[test]
  fn worker_prepares_images_and_releases_encoded_payloads() {
    let (raw_tx, raw_rx) = mpsc::channel();
    let prepared_rx = spawn_graphics_worker(raw_rx);
    raw_tx
      .send(RawGraphicsCommand {
        data: b"a=T,f=32,i=7,s=1,v=1;/wAA/w==".to_vec(),
        cursor_line: 4,
        cursor_column: 2,
        clear_all: false,
      })
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
        assert_eq!((cursor_line, cursor_column), (4, 2));
      }
      _ => panic!("expected a prepared image command"),
    }
  }

  #[test]
  fn worker_preserves_clear_events() {
    let (raw_tx, raw_rx) = mpsc::channel();
    let prepared_rx = spawn_graphics_worker(raw_rx);
    raw_tx
      .send(RawGraphicsCommand {
        data: Vec::new(),
        cursor_line: 0,
        cursor_column: 0,
        clear_all: true,
      })
      .unwrap();

    assert!(matches!(
      prepared_rx.recv_blocking().unwrap(),
      PreparedGraphicsEvent::ClearAll
    ));
  }
}
