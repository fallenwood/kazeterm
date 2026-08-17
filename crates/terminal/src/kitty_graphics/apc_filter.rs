use super::parser::MAX_APC_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KittyApcEvent {
  Command {
    data: Vec<u8>,
    passthrough_offset: usize,
  },
  Oversized {
    passthrough_offset: usize,
  },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterState {
  Normal,
  Escape,
  ApcProbe,
  NonKittyApc,
  NonKittyApcEscape,
  KittyApc,
  KittyApcEscape,
  OversizedKittyApc,
  OversizedKittyApcEscape,
}

/// Stateful byte-stream filter for Kitty graphics APC sequences.
///
/// Kitty APCs are consumed and emitted as events. Every other byte, including
/// non-Kitty APCs, is preserved exactly in `passthrough` across arbitrary input
/// fragmentation.
pub struct KittyApcFilter {
  state: FilterState,
  command: Vec<u8>,
}

impl KittyApcFilter {
  pub fn new() -> Self {
    Self {
      state: FilterState::Normal,
      command: Vec::new(),
    }
  }

  pub fn feed(
    &mut self,
    input: &[u8],
    passthrough: &mut Vec<u8>,
    events: &mut Vec<KittyApcEvent>,
  ) {
    for &byte in input {
      match self.state {
        FilterState::Normal => {
          if byte == 0x1b {
            self.state = FilterState::Escape;
          } else {
            passthrough.push(byte);
          }
        }
        FilterState::Escape => {
          if byte == b'_' {
            self.state = FilterState::ApcProbe;
          } else {
            passthrough.push(0x1b);
            if byte == 0x1b {
              self.state = FilterState::Escape;
            } else {
              passthrough.push(byte);
              self.state = FilterState::Normal;
            }
          }
        }
        FilterState::ApcProbe => {
          if byte == b'G' {
            self.command.clear();
            self.state = FilterState::KittyApc;
          } else {
            passthrough.extend_from_slice(b"\x1b_");
            passthrough.push(byte);
            self.state = if byte == 0x1b {
              FilterState::NonKittyApcEscape
            } else {
              FilterState::NonKittyApc
            };
          }
        }
        FilterState::NonKittyApc => {
          passthrough.push(byte);
          if byte == 0x1b {
            self.state = FilterState::NonKittyApcEscape;
          }
        }
        FilterState::NonKittyApcEscape => {
          passthrough.push(byte);
          if byte == b'\\' {
            self.state = FilterState::Normal;
          } else if byte != 0x1b {
            self.state = FilterState::NonKittyApc;
          }
        }
        FilterState::KittyApc => {
          if byte == 0x1b {
            self.state = FilterState::KittyApcEscape;
          } else {
            self.push_command_byte(byte);
          }
        }
        FilterState::KittyApcEscape => {
          if byte == b'\\' {
            events.push(KittyApcEvent::Command {
              data: std::mem::take(&mut self.command),
              passthrough_offset: passthrough.len(),
            });
            self.state = FilterState::Normal;
          } else {
            self.push_command_byte(0x1b);
            if self.state == FilterState::KittyApc {
              if byte == 0x1b {
                self.state = FilterState::KittyApcEscape;
              } else {
                self.push_command_byte(byte);
              }
            }
          }
        }
        FilterState::OversizedKittyApc => {
          if byte == 0x1b {
            self.state = FilterState::OversizedKittyApcEscape;
          }
        }
        FilterState::OversizedKittyApcEscape => {
          if byte == b'\\' {
            events.push(KittyApcEvent::Oversized {
              passthrough_offset: passthrough.len(),
            });
            self.state = FilterState::Normal;
          } else if byte != 0x1b {
            self.state = FilterState::OversizedKittyApc;
          }
        }
      }
    }
  }

  fn push_command_byte(&mut self, byte: u8) {
    if self.command.len() < MAX_APC_BYTES {
      self.command.push(byte);
      self.state = FilterState::KittyApc;
    } else {
      self.command.clear();
      self.state = FilterState::OversizedKittyApc;
    }
  }
}

impl Default for KittyApcFilter {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn filter_chunks(chunks: &[&[u8]]) -> (Vec<u8>, Vec<KittyApcEvent>) {
    let mut filter = KittyApcFilter::new();
    let mut passthrough = Vec::new();
    let mut events = Vec::new();
    for chunk in chunks {
      filter.feed(chunk, &mut passthrough, &mut events);
    }
    (passthrough, events)
  }

  #[test]
  fn consumes_kitty_apc_and_preserves_surrounding_output() {
    let (passthrough, events) = filter_chunks(&[b"before\x1b_Ga=p,i=7\x1b\\after"]);

    assert_eq!(passthrough, b"beforeafter");
    assert_eq!(
      events,
      [KittyApcEvent::Command {
        data: b"a=p,i=7".to_vec(),
        passthrough_offset: 6,
      }]
    );
  }

  #[test]
  fn preserves_non_kitty_apc_exactly() {
    let input = b"a\x1b_Xarbitrary\x1b\\b";
    let (passthrough, events) = filter_chunks(&[input]);

    assert_eq!(passthrough, input);
    assert!(events.is_empty());
  }

  #[test]
  fn handles_every_split_boundary() {
    let input = b"left\x1b_Ga=p,i=7\x1b\\right";
    for split in 0..=input.len() {
      let (passthrough, events) = filter_chunks(&[&input[..split], &input[split..]]);
      assert_eq!(passthrough, b"leftright", "split at {split}");
      assert_eq!(
        events,
        [KittyApcEvent::Command {
          data: b"a=p,i=7".to_vec(),
          passthrough_offset: 4,
        }],
        "split at {split}"
      );
    }
  }

  #[test]
  fn preserves_non_kitty_apc_at_every_split_boundary() {
    let input = b"left\x1b_Xpayload\x1b\\right";
    for split in 0..=input.len() {
      let (passthrough, events) = filter_chunks(&[&input[..split], &input[split..]]);
      assert_eq!(passthrough, input, "split at {split}");
      assert!(events.is_empty(), "split at {split}");
    }
  }

  #[test]
  fn reports_oversized_kitty_apc_without_buffering_it() {
    let mut input = b"\x1b_G".to_vec();
    input.extend(std::iter::repeat_n(b'A', MAX_APC_BYTES + 1));
    input.extend_from_slice(b"\x1b\\tail");
    let (passthrough, events) = filter_chunks(&[&input]);

    assert_eq!(passthrough, b"tail");
    assert_eq!(
      events,
      [KittyApcEvent::Oversized {
        passthrough_offset: 0,
      }]
    );
  }
}
