//! OSC 7 (Operating System Command 7) parser for terminal CWD tracking.
//!
//! Modern shells emit OSC 7 escape sequences to report the current working
//! directory. Format: `ESC ] 7 ; file://<hostname>/<path> BEL` or
//! `ESC ] 7 ; file://<hostname>/<path> ESC \`
//!
//! This module extracts the path from the **last** OSC 7 sequence found in
//! a chunk of PTY output bytes.

use std::path::PathBuf;

/// Extract the last OSC 7 path from a chunk of raw PTY output bytes.
///
/// Returns `None` if no valid OSC 7 sequence is found.
pub fn extract_osc7_path(data: &[u8]) -> Option<PathBuf> {
  let mut last_path: Option<PathBuf> = None;
  let len = data.len();
  let mut i = 0;

  while i < len {
    // Look for ESC (0x1B) followed by ']' (0x5D).
    if data[i] != 0x1B {
      i += 1;
      continue;
    }
    if i + 1 >= len || data[i + 1] != b']' {
      i += 1;
      continue;
    }

    // Found ESC ]. Now check for "7;" prefix.
    let osc_start = i + 2;
    if osc_start + 2 > len || data[osc_start] != b'7' || data[osc_start + 1] != b';' {
      i = osc_start;
      continue;
    }

    let uri_start = osc_start + 2;

    // Find the terminator: BEL (0x07) or ESC \ (0x1B 0x5C).
    let mut uri_end = None;
    let mut j = uri_start;
    while j < len {
      if data[j] == 0x07 {
        uri_end = Some(j);
        break;
      }
      if data[j] == 0x1B && j + 1 < len && data[j + 1] == b'\\' {
        uri_end = Some(j);
        break;
      }
      j += 1;
    }

    let Some(end) = uri_end else {
      // Incomplete sequence (may span chunks) — skip.
      i = uri_start;
      continue;
    };

    if let Some(path) = parse_file_uri(&data[uri_start..end]) {
      last_path = Some(path);
    }

    i = end + 1;
  }

  last_path
}

/// Parse a `file://` URI into a `PathBuf`.
///
/// Handles:
/// - `file:///absolute/path` (no hostname)
/// - `file://hostname/absolute/path` (hostname is stripped)
/// - Percent-encoded characters (e.g. `%20` for space)
/// - Windows paths: `file:///C:/Users/...`
fn parse_file_uri(uri_bytes: &[u8]) -> Option<PathBuf> {
  let uri = std::str::from_utf8(uri_bytes).ok()?;
  let rest = uri.strip_prefix("file://")?;

  // Split hostname from path. The path always starts with '/'.
  let path_str = if let Some(slash_pos) = rest.find('/') {
    &rest[slash_pos..]
  } else {
    // No slash after hostname — malformed.
    return None;
  };

  let decoded = percent_decode(path_str);

  // Normalize Windows UNC prefix if present.
  let cleaned = decoded
    .strip_prefix("\\\\?\\")
    .unwrap_or(&decoded);

  let path = PathBuf::from(cleaned);
  if path.as_os_str().is_empty() {
    return None;
  }

  Some(path)
}

/// Decode percent-encoded bytes in a URI path (e.g. `%20` → ` `).
fn percent_decode(input: &str) -> String {
  let mut out = String::with_capacity(input.len());
  let bytes = input.as_bytes();
  let mut i = 0;

  while i < bytes.len() {
    if bytes[i] == b'%' && i + 2 < bytes.len() {
      if let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
        out.push((hi << 4 | lo) as char);
        i += 3;
        continue;
      }
    }
    out.push(bytes[i] as char);
    i += 1;
  }

  out
}

fn hex_val(b: u8) -> Option<u8> {
  match b {
    b'0'..=b'9' => Some(b - b'0'),
    b'a'..=b'f' => Some(b - b'a' + 10),
    b'A'..=b'F' => Some(b - b'A' + 10),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_basic_osc7_bel_terminated() {
    let data = b"\x1b]7;file:///home/user/project\x07";
    assert_eq!(
      extract_osc7_path(data),
      Some(PathBuf::from("/home/user/project"))
    );
  }

  #[test]
  fn test_basic_osc7_st_terminated() {
    let data = b"\x1b]7;file:///home/user/project\x1b\\";
    assert_eq!(
      extract_osc7_path(data),
      Some(PathBuf::from("/home/user/project"))
    );
  }

  #[test]
  fn test_osc7_with_hostname() {
    let data = b"\x1b]7;file://myhost/home/user/project\x07";
    assert_eq!(
      extract_osc7_path(data),
      Some(PathBuf::from("/home/user/project"))
    );
  }

  #[test]
  fn test_osc7_percent_encoding() {
    let data = b"\x1b]7;file:///home/user/my%20project\x07";
    assert_eq!(
      extract_osc7_path(data),
      Some(PathBuf::from("/home/user/my project"))
    );
  }

  #[test]
  fn test_osc7_last_wins() {
    let mut data = Vec::new();
    data.extend_from_slice(b"\x1b]7;file:///first\x07");
    data.extend_from_slice(b"some output");
    data.extend_from_slice(b"\x1b]7;file:///second\x07");
    assert_eq!(extract_osc7_path(&data), Some(PathBuf::from("/second")));
  }

  #[test]
  fn test_osc7_mixed_with_normal_output() {
    let mut data = Vec::new();
    data.extend_from_slice(b"hello world\r\n");
    data.extend_from_slice(b"\x1b]7;file:///home/user\x07");
    data.extend_from_slice(b"more output\r\n");
    assert_eq!(
      extract_osc7_path(&data),
      Some(PathBuf::from("/home/user"))
    );
  }

  #[test]
  fn test_no_osc7() {
    let data = b"just normal terminal output\r\n";
    assert_eq!(extract_osc7_path(data), None);
  }

  #[test]
  fn test_incomplete_osc7() {
    // Missing terminator — should return None.
    let data = b"\x1b]7;file:///home/user";
    assert_eq!(extract_osc7_path(data), None);
  }

  #[test]
  fn test_windows_path() {
    let data = b"\x1b]7;file:///C:/Users/user/project\x07";
    assert_eq!(
      extract_osc7_path(data),
      Some(PathBuf::from("/C:/Users/user/project"))
    );
  }

  #[test]
  fn test_empty_path() {
    let data = b"\x1b]7;file://\x07";
    assert_eq!(extract_osc7_path(data), None);
  }
}
