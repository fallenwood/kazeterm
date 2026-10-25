use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use futures::stream::{FuturesUnordered, StreamExt};
use smol::{Async, Timer, channel::Sender};

use super::{
  MAX_REQUEST_BYTES, PREFIX, RemoteCall, RemoteRequest, RemoteResponse, suppress_response,
};

const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CLIENTS: usize = 8;
const MAX_COMMANDS_PER_CONNECTION: usize = 128;

/// Owns only the socket inode created by this listener. Never unlink an existing
/// endpoint at startup, or a replacement file created after our bind.
struct SocketFile {
  path: PathBuf,
  device: u64,
  inode: u64,
}

impl Drop for SocketFile {
  fn drop(&mut self) {
    if fs::symlink_metadata(&self.path)
      .is_ok_and(|m| m.file_type().is_socket() && m.dev() == self.device && m.ino() == self.inode)
    {
      let _ = fs::remove_file(&self.path);
    }
  }
}

pub struct RemoteControlServer {
  listener: Async<UnixListener>,
  socket: SocketFile,
}

impl RemoteControlServer {
  /// Require a private, current-user-owned parent directory. This closes the
  /// bind/chmod permission race without changing the process-global umask.
  pub fn bind(address: &str) -> io::Result<Self> {
    let raw = address.strip_prefix("unix:").ok_or_else(|| {
      io::Error::other("Only unix:/absolute/path remote-control addresses are supported")
    })?;
    let path = Path::new(raw);
    if !path.is_absolute() || path.file_name().is_none() {
      return Err(io::Error::other(
        "Remote-control socket path must be absolute",
      ));
    }
    let parent = path.parent().unwrap().canonicalize()?;
    let metadata = fs::metadata(&parent)?;
    // SAFETY: geteuid has no arguments, does not dereference memory, and is safe
    // to call concurrently. Unix-only code is not compiled for Windows.
    let uid = unsafe { libc::geteuid() };
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
      return Err(io::Error::other(
        "Socket parent must be owned by the current user and have mode 0700 (or stricter)",
      ));
    }
    let path = parent.join(path.file_name().unwrap());
    let listener = UnixListener::bind(&path)?;
    let metadata = fs::symlink_metadata(&path)?;
    let socket = SocketFile {
      path,
      device: metadata.dev(),
      inode: metadata.ino(),
    };
    fs::set_permissions(&socket.path, fs::Permissions::from_mode(0o600))?;
    Ok(Self {
      listener: Async::new(listener)?,
      socket,
    })
  }

  pub async fn run(self, sender: Sender<RemoteCall>) -> io::Result<()> {
    // The guard and all client futures belong to this future. Cancellation
    // closes the listener/clients and removes only our own socket file.
    let Self {
      listener,
      socket: _socket,
    } = self;
    let mut clients = FuturesUnordered::new();
    loop {
      if clients.len() >= MAX_CLIENTS {
        clients.next().await;
        continue;
      }
      enum Ready<T> {
        Accepted(io::Result<T>),
        Finished,
      }
      let ready = if clients.is_empty() {
        Ready::Accepted(listener.accept().await)
      } else {
        smol::future::race(async { Ready::Accepted(listener.accept().await) }, async {
          clients.next().await;
          Ready::Finished
        })
        .await
      };
      match ready {
        Ready::Accepted(Ok((stream, _))) => clients.push(serve_client(stream, sender.clone())),
        Ready::Accepted(Err(error)) => return Err(error),
        Ready::Finished => {}
      }
    }
  }
}

async fn with_timeout<T>(
  future: impl std::future::Future<Output = io::Result<T>>,
) -> io::Result<T> {
  smol::future::race(future, async {
    Timer::after(TIMEOUT).await;
    Err(io::Error::new(
      io::ErrorKind::TimedOut,
      "Remote-control request timed out",
    ))
  })
  .await
}

/// Read bounded chunks, not an unbounded read_until allocation. Escapes in JSON
/// strings are encoded as backslash-u; a literal ESC can only begin the ST.
async fn read_frame(reader: &mut (impl AsyncBufRead + Unpin)) -> io::Result<Option<Vec<u8>>> {
  let mut prefix = [0; PREFIX.len()];
  if reader.read(&mut prefix[..1]).await? == 0 {
    return Ok(None);
  }
  reader.read_exact(&mut prefix[1..]).await?;
  if prefix != PREFIX {
    return Err(io::Error::other("Expected DCS @kitty-cmd framing"));
  }
  let mut body = Vec::new();
  loop {
    let available = reader.fill_buf().await?;
    if available.is_empty() {
      return Err(io::ErrorKind::UnexpectedEof.into());
    }
    let escape = available.iter().position(|b| *b == 27);
    let count = escape.unwrap_or(available.len());
    if body.len().saturating_add(count) > MAX_REQUEST_BYTES {
      return Err(io::Error::other("Remote-control request exceeds 64 KiB"));
    }
    body.extend_from_slice(&available[..count]);
    reader.consume_unpin(count + usize::from(escape.is_some()));
    if escape.is_some() {
      let mut end = [0];
      reader.read_exact(&mut end).await?;
      if end[0] != b'\\' {
        return Err(io::Error::other("Invalid DCS terminator"));
      }
      return Ok(Some(body));
    }
  }
}

async fn serve_client(stream: Async<UnixStream>, sender: Sender<RemoteCall>) {
  let mut reader = BufReader::new(stream);
  for _ in 0..MAX_COMMANDS_PER_CONNECTION {
    let body = match with_timeout(read_frame(&mut reader)).await {
      Ok(Some(body)) => body,
      Ok(None) => break,
      Err(_) => break, // Cannot trust framing or suppression on a partial frame.
    };
    let value = match serde_json::from_slice(&body) {
      Ok(value) => value,
      Err(_) => {
        if with_timeout(
          reader
            .get_mut()
            .write_all(&RemoteResponse::failure("Invalid JSON request").encode()),
        )
        .await
        .is_err()
        {
          break;
        }
        continue;
      }
    };
    let silent = suppress_response(&value);
    let response = match RemoteRequest::from_value(value) {
      Err(error) => RemoteResponse::failure(error),
      Ok(request) => {
        let (reply, result) = smol::channel::bounded(1);
        let call = RemoteCall {
          request,
          deadline: Instant::now() + TIMEOUT,
          reply,
        };
        let result = with_timeout(async {
          sender
            .send(call)
            .await
            .map_err(|_| io::Error::other("Remote dispatcher is unavailable"))?;
          result
            .recv()
            .await
            .map_err(|_| io::Error::other("Remote dispatcher disconnected"))
        })
        .await;
        match result {
          Ok(response) => response,
          Err(error) => RemoteResponse::failure(error.to_string()),
        }
      }
    };
    if !silent
      && with_timeout(reader.get_mut().write_all(&response.encode()))
        .await
        .is_err()
    {
      break;
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::io::Cursor;

  #[test]
  fn framing_supports_fragmented_and_concatenated_requests_and_rejects_overflow() {
    smol::block_on(async {
      let frame = b"\x1bP@kitty-cmd{\"cmd\":\"ls\"}\x1b\\";
      let mut reader = BufReader::with_capacity(
        1,
        Cursor::new([frame.as_slice(), frame.as_slice()].concat()),
      );
      for _ in 0..2 {
        assert_eq!(
          read_frame(&mut reader).await.unwrap().unwrap(),
          b"{\"cmd\":\"ls\"}"
        );
      }
      assert!(read_frame(&mut reader).await.unwrap().is_none());
      for bytes in [
        b"not-kitty-protocol".to_vec(),
        b"\x1bP@kitty-cmd{}\x1bX".to_vec(),
        b"\x1bP@kitty-cmd{}".to_vec(),
        [PREFIX, &vec![b'x'; MAX_REQUEST_BYTES + 1], b"\x1b\\"].concat(),
      ] {
        assert!(
          read_frame(&mut BufReader::new(Cursor::new(bytes)))
            .await
            .is_err()
        );
      }
    });
  }

  #[test]
  fn pipelined_socket_calls_execute_in_order_and_send_text_never_responds() {
    smol::block_on(async {
      let (server, client) = UnixStream::pair().unwrap();
      let (sender, receiver) = smol::channel::bounded(2);
      let server = serve_client(Async::new(server).unwrap(), sender);
      let client = async {
        let mut client = BufReader::new(Async::new(client).unwrap());
        let mut bytes = Vec::new();
        for cmd in ["send-text", "ls"] {
          bytes.extend_from_slice(PREFIX);
          bytes.extend_from_slice(format!(r#"{{"cmd":"{cmd}","version":[0,49,2]}}"#).as_bytes());
          bytes.extend_from_slice(super::super::SUFFIX);
        }
        client.get_mut().write_all(&bytes).await.unwrap();
        let response = with_timeout(read_frame(&mut client))
          .await
          .unwrap()
          .unwrap();
        let response: serde_json::Value = serde_json::from_slice(&response).unwrap();
        assert_eq!(response["data"], "listing"); // Not a reply to send-text.
        client
          .get_mut()
          .get_ref()
          .shutdown(std::net::Shutdown::Write)
          .unwrap();
        assert!(
          with_timeout(read_frame(&mut client))
            .await
            .unwrap()
            .is_none()
        );
      };
      let dispatcher = async {
        for expected in ["send-text", "ls"] {
          let call = receiver.recv().await.unwrap();
          assert_eq!(call.request.cmd, expected);
          call
            .reply
            .send(RemoteResponse::success(Some(serde_json::Value::String(
              "listing".into(),
            ))))
            .await
            .unwrap();
        }
      };
      futures::join!(server, client, dispatcher);
    });
  }

  #[test]
  fn private_socket_permissions_collision_and_owned_cleanup() {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dir = std::env::temp_dir().join(format!(
      "kazeterm-rc-{}-{}",
      std::process::id(),
      SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
    ));
    fs::create_dir(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.join("control.sock");
    let address = format!("unix:{}", path.display());
    let server = RemoteControlServer::bind(&address).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    assert!(RemoteControlServer::bind(&address).is_err());
    drop(server);
    assert!(!path.exists());
    let server = RemoteControlServer::bind(&address).unwrap();
    fs::remove_file(&path).unwrap();
    fs::write(&path, "replacement").unwrap();
    drop(server);
    assert_eq!(fs::read_to_string(&path).unwrap(), "replacement");
    fs::write(&path, "must not delete").unwrap();
    assert!(RemoteControlServer::bind(&address).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "must not delete");
    fs::remove_file(&path).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(RemoteControlServer::bind(&address).is_err());
    assert!(RemoteControlServer::bind("tcp:127.0.0.1:1234").is_err());
    assert!(RemoteControlServer::bind("unix:relative.sock").is_err());
    fs::remove_dir(&dir).unwrap();
  }
}
