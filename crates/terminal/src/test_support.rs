//! Test-only helpers for constructing in-memory `Terminal` instances without a PTY.
//!
//! Not hidden behind a feature flag because the overhead is negligible and
//! downstream crates' integration tests need access. Marked `#[doc(hidden)]`
//! so it doesn't pollute published API docs.

use std::borrow::Cow;
use std::sync::{
  Arc, Mutex,
  atomic::{AtomicU32, Ordering},
};

use terminal_kernel::event::{VoidListener, WindowSize};
use terminal_kernel::grid::Dimensions;
use terminal_kernel::sync::FairMutex;
use terminal_kernel::term::{Config as AlacConfig, Term};
use terminal_kernel::{AlacrittyBackend, SessionEvents};

use crate::{PtyProcessInfo, PtySender, Terminal};

/// A `PtySender` that records every byte written to it, for assertion in tests.
#[doc(hidden)]
pub struct FakePtySender {
  writes: Arc<Mutex<Vec<Vec<u8>>>>,
  resizes: Arc<Mutex<Vec<(u16, u16)>>>,
}

impl FakePtySender {
  pub fn new() -> (
    Box<dyn PtySender>,
    Arc<Mutex<Vec<Vec<u8>>>>,
    Arc<Mutex<Vec<(u16, u16)>>>,
  ) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let resizes = Arc::new(Mutex::new(Vec::new()));
    let sender = Box::new(FakePtySender {
      writes: writes.clone(),
      resizes: resizes.clone(),
    });
    (sender, writes, resizes)
  }
}

impl PtySender for FakePtySender {
  fn send_input(&self, bytes: Cow<'static, [u8]>) {
    self.writes.lock().unwrap().push(bytes.into_owned());
  }

  fn send_resize(&self, size: WindowSize) {
    self
      .resizes
      .lock()
      .unwrap()
      .push((size.num_cols, size.num_lines));
  }
}

struct StubDims {
  cols: usize,
  lines: usize,
}
impl Dimensions for StubDims {
  fn total_lines(&self) -> usize {
    self.lines
  }
  fn screen_lines(&self) -> usize {
    self.lines
  }
  fn columns(&self) -> usize {
    self.cols
  }
}

/// Build a fully in-memory `Terminal` + `SessionEvents` pair for tests.
///
/// - No child process is spawned.
/// - No OS PTY is opened.
/// - The returned `SessionEvents` is a dangling receiver (the sender is kept
///   alive by the caller only if needed; otherwise `next().await` returns
///   `None` on the first poll once dropped).
/// - All input bytes written by the `Terminal` are captured in `writes`
///   (the second return value).
#[doc(hidden)]
pub fn fake_terminal_session(
  cols: usize,
  lines: usize,
) -> (
  Terminal,
  SessionEvents,
  Arc<Mutex<Vec<Vec<u8>>>>,
  Arc<Mutex<Vec<(u16, u16)>>>,
) {
  let (sender, writes, resizes) = FakePtySender::new();

  let term = Term::new(
    AlacConfig::default(),
    &StubDims { cols, lines },
    VoidListener,
  );
  let term = Arc::new(FairMutex::new(term));
  let backend = Box::new(AlacrittyBackend::new(term));

  let (_tx, rx) = futures::channel::mpsc::unbounded();

  let keyboard_flags = Arc::new(AtomicU32::new(0));
  // Silence the "unused" warning by touching the atomic.
  let _ = keyboard_flags.load(Ordering::Relaxed);

  let terminal = Terminal::new(
    sender,
    backend,
    PtyProcessInfo::test_stub(),
    None,
    None,
    keyboard_flags,
    None,
    None,
  );

  (terminal, rx, writes, resizes)
}
