//! Windows named pipe server for Kitty graphics protocol.
//!
//! On Windows, ConPTY strips APC sequences before they reach the terminal
//! emulator. This module provides an alternative transport: a named pipe
//! that scripts can write Kitty graphics commands to directly, bypassing
//! ConPTY entirely.
//!
//! ## Usage
//!
//! When kazeterm starts a terminal on Windows, it:
//! 1. Creates a named pipe (e.g. `\\.\pipe\kazeterm-graphics-1234-0`)
//! 2. Sets `KAZETERM_GRAPHICS_PIPE=<pipe_name>` in the child shell environment
//! 3. Starts a background thread listening on the pipe
//!
//! Scripts detect the env var and write commands to the pipe instead of stdout.
//!
//! ## Protocol
//!
//! The pipe uses a line-based protocol. Each line is a Kitty graphics
//! command body (the content that would appear between `ESC_G` and `ESC\`
//! in the standard APC format):
//!
//! ```text
//! a=T,f=100,m=1;BASE64DATA\n
//! m=1;BASE64DATA\n
//! m=0;\n
//! ```

use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::System::Pipes::{
  ConnectNamedPipe, CreateNamedPipeW, NAMED_PIPE_MODE, PIPE_UNLIMITED_INSTANCES,
};

use super::command::RawGraphicsCommand;
use super::pty_filter::{parse_apc_params, try_png_height_from_payload};

/// Callback that tries to get the cursor position from the terminal.
pub type CursorFn = Box<dyn Fn() -> Option<(i32, i32)> + Send + Sync>;

/// PIPE_ACCESS_INBOUND (0x1) — pipe is read-only from server's perspective.
const PIPE_ACCESS_INBOUND: FILE_FLAGS_AND_ATTRIBUTES = FILE_FLAGS_AND_ATTRIBUTES(1);

/// PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT — all zero, byte-mode blocking pipe.
const PIPE_MODE: NAMED_PIPE_MODE = NAMED_PIPE_MODE(0);

/// Start a named pipe server for receiving Kitty graphics commands.
///
/// Creates a background thread that listens on the given pipe name.
/// Graphics commands received through the pipe are sent to `graphics_tx`.
/// The `cursor_fn` is called to capture the terminal cursor position when
/// each command is received.
pub fn start_server(
  pipe_name: String,
  graphics_tx: mpsc::Sender<RawGraphicsCommand>,
  cursor_fn: CursorFn,
  pending_cnl: Arc<AtomicU32>,
) {
  thread::Builder::new()
    .name("kazeterm-graphics-pipe".into())
    .spawn(move || {
      server_loop(&pipe_name, &graphics_tx, &cursor_fn, &pending_cnl);
    })
    .expect("failed to start graphics pipe server");
}

fn server_loop(
  pipe_name: &str,
  graphics_tx: &mpsc::Sender<RawGraphicsCommand>,
  cursor_fn: &dyn Fn() -> Option<(i32, i32)>,
  pending_cnl: &AtomicU32,
) {
  let pipe_name_w: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();

  loop {
    // Create a new pipe instance for each client connection.
    let handle = unsafe {
      CreateNamedPipeW(
        windows::core::PCWSTR(pipe_name_w.as_ptr()),
        PIPE_ACCESS_INBOUND,
        PIPE_MODE,
        PIPE_UNLIMITED_INSTANCES,
        0,     // output buffer size (not used for inbound pipe)
        65536, // input buffer size
        0,     // default timeout
        None,  // default security attributes
      )
    };

    if handle == INVALID_HANDLE_VALUE {
      tracing::error!(
        "Failed to create named pipe: {}",
        std::io::Error::last_os_error()
      );
      break;
    }

    // Wait for a client to connect (blocking call).
    if unsafe { ConnectNamedPipe(handle, None) }.is_err() {
      let err = std::io::Error::last_os_error();
      // ERROR_PIPE_CONNECTED (535) means client connected before we called ConnectNamedPipe.
      if err.raw_os_error() != Some(535) {
        unsafe {
          let _ = windows::Win32::Foundation::CloseHandle(handle);
        }
        continue;
      }
    }

    // Wrap the pipe handle in a File for buffered reading.
    // Safety: handle is a valid pipe handle we just created.
    let file =
      unsafe { <std::fs::File as std::os::windows::io::FromRawHandle>::from_raw_handle(handle.0) };
    let reader = BufReader::new(file);

    // Track whether we've already signalled CNL for the current image
    // sequence (avoids double-injection for multi-chunk images).
    let mut cnl_stored = false;

    for line in reader.lines() {
      let line = match line {
        Ok(l) if !l.is_empty() => l,
        Ok(_) => continue,
        Err(_) => break,
      };

      let data = line.into_bytes();
      let (cursor_line, cursor_column) = cursor_fn().unwrap_or((0, 0));

      // Estimate image height and signal the PTY filter to inject CNL
      // (cursor advancement) immediately, before the next shell prompt
      // flows through the PTY.
      let params = parse_apc_params(&data);
      if params.is_display && !cnl_stored {
        let effective_rows = if params.display_rows > 0 {
          params.display_rows
        } else if params.source_height > 0 {
          (params.source_height + 19) / 20
        } else if params.format == 100 {
          let h = try_png_height_from_payload(&data);
          if h > 0 { (h + 19) / 20 } else { 0 }
        } else {
          0
        };
        if effective_rows > 0 {
          pending_cnl.store(effective_rows, Ordering::Release);
          cnl_stored = true;
        }
      }
      if !params.more_chunks {
        cnl_stored = false;
      }

      if graphics_tx
        .send(RawGraphicsCommand {
          data,
          cursor_line,
          cursor_column,
          clear_all: false,
          from_filter: false,
        })
        .is_err()
      {
        // Receiver dropped — terminal is shutting down.
        return;
      }
    }

    // Client disconnected. The File drop closes the pipe handle.
    // Loop back to create a new pipe instance for the next client.
  }
}
