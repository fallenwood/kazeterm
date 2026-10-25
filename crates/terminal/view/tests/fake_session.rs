//! Smoke test for `terminal::test_support::fake_terminal_session`.

use std::borrow::Cow;
use terminal::PtySender;
use terminal::test_support::{FakePtySender, fake_terminal_session};
use terminal_kernel::event::WindowSize;

#[test]
fn fake_session_constructs_without_panicking() {
  let (_term, _events, writes, resizes) = fake_terminal_session(80, 24);
  assert!(writes.lock().unwrap().is_empty());
  assert!(resizes.lock().unwrap().is_empty());
}

#[test]
fn fake_pty_sender_captures_input_bytes() {
  let (sender, writes, resizes) = FakePtySender::new();
  sender.send_input(Cow::Borrowed(b"hello"));
  sender.send_input(Cow::Borrowed(b"\r\n"));

  let captured = writes.lock().unwrap();
  assert_eq!(captured.len(), 2);
  assert_eq!(captured[0], b"hello");
  assert_eq!(captured[1], b"\r\n");
  assert!(resizes.lock().unwrap().is_empty());
}

#[test]
fn fake_pty_sender_captures_resizes() {
  let (sender, _, resizes) = FakePtySender::new();
  sender.send_resize(WindowSize {
    num_cols: 100,
    num_lines: 40,
    cell_width: 10,
    cell_height: 20,
  });

  let captured = resizes.lock().unwrap();
  assert_eq!(captured.len(), 1);
  assert_eq!(captured[0], (100, 40));
}
