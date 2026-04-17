use std::path::PathBuf;

use smol::channel::Sender;

use crate::{AppEvent, JsonEvent};

fn parse_and_send_line(sender: &Sender<AppEvent>, line: &str, source: &str) -> bool {
  match serde_json::from_str::<JsonEvent>(line) {
    Ok(json_event) => {
      let event: AppEvent = json_event.into();
      tracing::debug!("Received event from {}: {:?}", source, event);
      if sender.send_blocking(event).is_err() {
        tracing::error!("Event channel closed, stopping {} reader", source);
        return false;
      }
    }
    Err(error) => {
      tracing::warn!(
        "Failed to parse event from {}: {} - line: {}",
        source,
        error,
        line
      );
    }
  }

  true
}

/// Start reading events from stdin in a background thread.
pub(crate) fn start_stdio_reader(sender: Sender<AppEvent>) {
  std::thread::spawn(move || {
    use std::io::BufRead;

    tracing::info!("Starting stdin event reader");

    let stdin = std::io::stdin();
    let reader = stdin.lock();

    for line in reader.lines() {
      match line {
        Ok(line) => {
          let line = line.trim();
          if line.is_empty() {
            continue;
          }

          if !parse_and_send_line(&sender, line, "stdin") {
            break;
          }
        }
        Err(error) => {
          tracing::error!("Error reading from stdin: {}", error);
          break;
        }
      }
    }

    tracing::info!("Stdin event reader stopped");
  });
}

/// Start reading events from a Unix domain socket in a background thread.
pub(crate) fn start_socket_reader(sender: Sender<AppEvent>, path: PathBuf) {
  std::thread::spawn(move || {
    #[cfg(unix)]
    {
      start_unix_socket_reader_unix(sender, path);
    }

    #[cfg(windows)]
    {
      start_unix_socket_reader_windows(sender, path);
    }
  });
}

/// Unix domain socket reader (Unix platforms).
#[cfg(unix)]
fn start_unix_socket_reader_unix(sender: Sender<AppEvent>, path: PathBuf) {
  use std::io::{BufRead, BufReader};
  use std::os::unix::net::UnixListener;

  tracing::info!("Starting Unix socket event reader at: {:?}", path);

  let _ = std::fs::remove_file(&path);

  let listener = match UnixListener::bind(&path) {
    Ok(listener) => listener,
    Err(error) => {
      tracing::error!("Failed to bind Unix socket at {:?}: {}", path, error);
      return;
    }
  };

  tracing::info!("Listening for events on Unix socket: {:?}", path);

  for stream in listener.incoming() {
    match stream {
      Ok(stream) => {
        let sender = sender.clone();
        std::thread::spawn(move || {
          let reader = BufReader::new(stream);
          for line in reader.lines() {
            match line {
              Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                  continue;
                }

                if !parse_and_send_line(&sender, line, "socket") {
                  break;
                }
              }
              Err(error) => {
                tracing::debug!("Client disconnected: {}", error);
                break;
              }
            }
          }
        });
      }
      Err(error) => {
        tracing::error!("Failed to accept connection: {}", error);
      }
    }
  }
}

/// Unix domain socket reader (Windows).
///
/// Windows has supported Unix domain sockets since Windows 10 version 1803.
/// We use the `uds_windows` crate to provide UnixListener/UnixStream on Windows.
#[cfg(windows)]
fn start_unix_socket_reader_windows(sender: Sender<AppEvent>, path: PathBuf) {
  use std::io::{BufRead, BufReader};

  use uds_windows::UnixListener;

  tracing::info!("Starting Unix socket event reader at: {:?}", path);

  let _ = std::fs::remove_file(&path);

  let listener = match UnixListener::bind(&path) {
    Ok(listener) => listener,
    Err(error) => {
      tracing::error!("Failed to bind Unix socket at {:?}: {}", path, error);
      return;
    }
  };

  tracing::info!("Listening for events on Unix socket: {:?}", path);

  for stream in listener.incoming() {
    match stream {
      Ok(stream) => {
        let sender = sender.clone();
        std::thread::spawn(move || {
          let reader = BufReader::new(stream);
          for line in reader.lines() {
            match line {
              Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                  continue;
                }

                if !parse_and_send_line(&sender, line, "socket") {
                  break;
                }
              }
              Err(error) => {
                tracing::debug!("Client disconnected: {}", error);
                break;
              }
            }
          }
        });
      }
      Err(error) => {
        tracing::error!("Failed to accept connection: {}", error);
      }
    }
  }
}
