//! Bounded Kitty remote-control wire format. This is separate from Kazeterm's
//! newline-delimited event protocol and never enables PTY control implicitly.

use std::collections::BTreeMap;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(unix)]
mod socket;
#[cfg(unix)]
pub use socket::RemoteControlServer;

pub const PREFIX: &[u8] = b"\x1bP@kitty-cmd";
pub const SUFFIX: &[u8] = b"\x1b\\";
pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
/// Version ceiling for the pinned upstream wire format, not a claim that every
/// command in that Kitty release is implemented.
pub const REFERENCE_VERSION: [u32; 3] = [0, 49, 2];

#[derive(Debug, Deserialize)]
pub struct RemoteRequest {
  pub cmd: String,
  pub version: [u32; 3],
  #[serde(default)]
  pub no_response: bool,
  #[serde(default)]
  pub kitty_window_id: Option<u64>,
  #[serde(default)]
  pub payload: Option<Value>,
  #[serde(flatten)]
  extra: BTreeMap<String, Value>,
}

impl RemoteRequest {
  pub fn from_value(value: Value) -> Result<Self, String> {
    let request: Self =
      serde_json::from_value(value).map_err(|e| format!("Invalid request: {e}"))?;
    if request.version[..2] > REFERENCE_VERSION[..2] {
      return Err("Remote client version is newer than the supported reference protocol".into());
    }
    if !request.extra.is_empty() {
      return Err("Unsupported envelope fields (encryption, passwords, async and streaming are not supported)".into());
    }
    if request.cmd.is_empty() || request.cmd.len() > 64 {
      return Err("Invalid command name".into());
    }
    if request
      .payload
      .as_ref()
      .is_some_and(|p| !p.is_object() && !p.is_null())
    {
      return Err("Command payload must be an object".into());
    }
    Ok(request)
  }

  pub fn payload<T: serde::de::DeserializeOwned>(&self) -> Result<T, String> {
    let value = self
      .payload
      .clone()
      .filter(|v| !v.is_null())
      .unwrap_or_else(|| serde_json::json!({}));
    serde_json::from_value(value).map_err(|e| format!("Unsupported or invalid payload: {e}"))
  }
}

/// send-text deliberately never responds, as in Kitty's SendText command. This
/// also preserves no_response when another request field is invalid.
pub fn suppress_response(value: &Value) -> bool {
  value.get("no_response").and_then(Value::as_bool) == Some(true)
    || value.get("cmd").and_then(Value::as_str) == Some("send-text")
}

#[derive(Debug, Serialize)]
pub struct RemoteResponse {
  pub ok: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<String>,
}

impl RemoteResponse {
  pub fn success(data: Option<Value>) -> Self {
    Self {
      ok: true,
      data,
      error: None,
    }
  }

  pub fn failure(error: impl AsRef<str>) -> Self {
    Self {
      ok: false,
      data: None,
      error: Some(error.as_ref().chars().take(1024).collect()),
    }
  }

  pub fn encode(&self) -> Vec<u8> {
    let mut body = serde_json::to_vec(self).expect("remote response contains only JSON values");
    if body.len() > MAX_RESPONSE_BYTES {
      body = serde_json::to_vec(&Self::failure(
        "Response exceeds the remote-control size limit",
      ))
      .unwrap();
    }
    let mut frame = Vec::with_capacity(PREFIX.len() + body.len() + SUFFIX.len());
    frame.extend_from_slice(PREFIX);
    frame.extend_from_slice(&body);
    frame.extend_from_slice(SUFFIX);
    frame
  }
}

/// The bounded transport waits for a result even for no-response commands, so
/// commands on one connection execute in order. The UI checks the deadline
/// before applying mutations; expired queued requests cannot run later.
pub struct RemoteCall {
  pub request: RemoteRequest,
  pub deadline: Instant,
  pub reply: smol::channel::Sender<RemoteResponse>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn request_validation_and_suppression_reject_unsupported_envelopes() {
    let valid = json!({"cmd":"ls", "version":[0,49,99], "payload": null});
    assert!(RemoteRequest::from_value(valid).is_ok()); // Kitty compares major/minor.
    for invalid in [
      json!({"cmd":"ls", "version":[0,50,0]}),
      json!({"cmd":"ls", "version":[0,49]}),
      json!({"cmd":"ls", "version":[0,49,2], "encrypted":"x"}),
      json!({"cmd":"ls", "version":[0,49,2], "async":"x"}),
      json!({"cmd":"ls", "version":[0,49,2], "password":"x"}),
      json!({"cmd":"ls", "version":[0,49,2], "payload": []}),
    ] {
      assert!(RemoteRequest::from_value(invalid).is_err());
    }
    assert!(suppress_response(&json!({"cmd":"send-text"})));
    assert!(suppress_response(
      &json!({"no_response":true, "version":"invalid"})
    ));
    assert!(!suppress_response(&json!({"no_response":"true"})));
  }

  #[test]
  fn responses_are_framed_escaped_and_bounded() {
    let encoded = RemoteResponse::failure("bad\x1b\\\nmessage").encode();
    assert!(encoded.starts_with(PREFIX));
    assert!(encoded.ends_with(SUFFIX));
    assert!(!encoded[PREFIX.len()..encoded.len() - SUFFIX.len()].contains(&27));
    let decoded: Value = serde_json::from_slice(&encoded[PREFIX.len()..encoded.len() - 2]).unwrap();
    assert_eq!(decoded["ok"], false);
    let large =
      RemoteResponse::success(Some(Value::String("x".repeat(MAX_RESPONSE_BYTES + 1)))).encode();
    assert!(large.len() < 256);
    assert!(String::from_utf8(large).unwrap().contains("size limit"));
  }
}
