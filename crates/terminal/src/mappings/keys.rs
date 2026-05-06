use std::borrow::Cow;

use gpui::Keystroke;
use terminal_kernel::term::TermMode;

use crate::kitty_graphics::pty_filter::{
  KITTY_KEYBOARD_DISAMBIGUATE_ESCAPE_CODES, KITTY_KEYBOARD_REPORT_ALL_KEYS_AS_ESCAPE_CODES,
  WINDOWS_CONPTY_WIN32_INPUT_MODE,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
  MAPVK_VK_TO_VSC, MapVirtualKeyW, VK_BACK, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1, VK_HOME,
  VK_INSERT, VK_LEFT, VK_NEXT, VK_OEM_1, VK_OEM_2, VK_OEM_3, VK_OEM_4, VK_OEM_5, VK_OEM_6,
  VK_OEM_7, VK_OEM_COMMA, VK_OEM_MINUS, VK_OEM_PERIOD, VK_OEM_PLUS, VK_PRIOR, VK_RETURN, VK_RIGHT,
  VK_SPACE, VK_TAB, VK_UP,
};

#[cfg(target_os = "windows")]
const WIN32_LEFT_ALT_PRESSED: u32 = 0x0002;
#[cfg(target_os = "windows")]
const WIN32_LEFT_CTRL_PRESSED: u32 = 0x0008;
#[cfg(target_os = "windows")]
const WIN32_SHIFT_PRESSED: u32 = 0x0010;
#[cfg(target_os = "windows")]
const WIN32_ENHANCED_KEY: u32 = 0x0100;

pub enum KnownKeys {
  Tab,
  ShiftTab,
  PageUp,
  PageDown,
}

impl KnownKeys {
  pub fn as_slice(&self) -> impl Into<Cow<'static, [u8]>> {
    match self {
      KnownKeys::ShiftTab => b"\x1b[Z".as_slice(),
      KnownKeys::Tab => b"\x09".as_slice(),
      KnownKeys::PageUp => b"\x1b[5~".as_slice(),
      KnownKeys::PageDown => b"\x1b[6~".as_slice(),
    }
  }
}

#[derive(Debug, PartialEq, Eq)]
enum AlacModifiers {
  None,
  Alt,
  Ctrl,
  Shift,
  CtrlShift,
  Other,
}

impl AlacModifiers {
  fn new(ks: &Keystroke) -> Self {
    match (
      ks.modifiers.alt,
      ks.modifiers.control,
      ks.modifiers.shift,
      ks.modifiers.platform,
    ) {
      (false, false, false, false) => AlacModifiers::None,
      (true, false, false, false) => AlacModifiers::Alt,
      (false, true, false, false) => AlacModifiers::Ctrl,
      (false, false, true, false) => AlacModifiers::Shift,
      (false, true, true, false) => AlacModifiers::CtrlShift,
      _ => AlacModifiers::Other,
    }
  }

  fn any(&self) -> bool {
    match &self {
      AlacModifiers::None => false,
      AlacModifiers::Alt => true,
      AlacModifiers::Ctrl => true,
      AlacModifiers::Shift => true,
      AlacModifiers::CtrlShift => true,
      AlacModifiers::Other => true,
    }
  }
}

fn normalized_key_name(key: &str) -> Cow<'_, str> {
  if key.len() > 1 {
    Cow::Owned(key.to_ascii_lowercase())
  } else {
    Cow::Borrowed(key)
  }
}

fn kitty_c0_key_code(key: &str) -> Option<u32> {
  match key {
    "escape" => Some(27),
    "enter" => Some(13),
    "tab" => Some(9),
    "backspace" | "back" => Some(127),
    _ => None,
  }
}

fn kitty_disambiguate_escape_codes(keyboard_protocol_flags: u32) -> bool {
  keyboard_protocol_flags & KITTY_KEYBOARD_DISAMBIGUATE_ESCAPE_CODES != 0
}

fn kitty_report_all_keys_as_escape_codes(keyboard_protocol_flags: u32) -> bool {
  keyboard_protocol_flags & KITTY_KEYBOARD_REPORT_ALL_KEYS_AS_ESCAPE_CODES != 0
}

fn windows_conpty_win32_input_mode_enabled(keyboard_protocol_flags: u32) -> bool {
  keyboard_protocol_flags & WINDOWS_CONPTY_WIN32_INPUT_MODE != 0
}

fn single_char(text: &str) -> Option<char> {
  let mut chars = text.chars();
  let ch = chars.next()?;
  chars.next().is_none().then_some(ch)
}

/// Recover the base (unshifted) key for symbols GPUI reports on Windows with
/// `shift = false`, like Shift+1 arriving as `key = "!"`.
fn unshifted_symbol_key(key: &str) -> Option<&'static str> {
  match key {
    "!" => Some("1"),
    "@" => Some("2"),
    "#" => Some("3"),
    "$" => Some("4"),
    "%" => Some("5"),
    "^" => Some("6"),
    "&" => Some("7"),
    "*" => Some("8"),
    "(" => Some("9"),
    ")" => Some("0"),
    "~" => Some("`"),
    "_" => Some("-"),
    "+" => Some("="),
    "{" => Some("["),
    "}" => Some("]"),
    "|" => Some("\\"),
    ":" => Some(";"),
    "\"" => Some("'"),
    "<" => Some(","),
    ">" => Some("."),
    "?" => Some("/"),
    _ => None,
  }
}

fn kitty_modifier_code(keystroke: &Keystroke, inferred_shift: bool) -> u32 {
  let mut modifier_code = 0;
  if keystroke.modifiers.shift || inferred_shift {
    modifier_code |= 1;
  }
  if keystroke.modifiers.alt {
    modifier_code |= 1 << 1;
  }
  if keystroke.modifiers.control {
    modifier_code |= 1 << 2;
  }
  modifier_code + 1
}

fn kitty_escape(code: u32, modifier_code: u32) -> Cow<'static, str> {
  Cow::Owned(if modifier_code == 1 {
    format!("\x1b[{code}u")
  } else {
    format!("\x1b[{code};{modifier_code}u")
  })
}

fn to_kitty_c0_escape(
  key: &str,
  keystroke: &Keystroke,
  keyboard_protocol_flags: u32,
) -> Option<Cow<'static, str>> {
  let code = kitty_c0_key_code(key)?;
  let should_encode = if kitty_report_all_keys_as_escape_codes(keyboard_protocol_flags) {
    true
  } else if kitty_disambiguate_escape_codes(keyboard_protocol_flags) {
    match key {
      // Plain Escape must be disambiguated from the start of CSI/SS3/etc.
      "escape" => true,
      _ => AlacModifiers::new(keystroke).any(),
    }
  } else {
    false
  };

  should_encode.then(|| kitty_escape(code, kitty_modifier_code(keystroke, false)))
}

fn kitty_text_key_code(key: &str) -> Option<(u32, bool)> {
  if key == "space" {
    return Some((32, false));
  }

  let (normalized, inferred_shift) = if let Some(base_key) = unshifted_symbol_key(key) {
    (Cow::Borrowed(base_key), true)
  } else if key.chars().count() == 1 {
    (
      Cow::Owned(key.chars().flat_map(char::to_lowercase).collect::<String>()),
      false,
    )
  } else {
    return None;
  };

  let mut chars = normalized.chars();
  let key_char = chars.next()?;
  if chars.next().is_some() {
    return None;
  }

  Some((u32::from(key_char), inferred_shift))
}

#[cfg(target_os = "windows")]
fn windows_named_virtual_key_code(key: &str) -> Option<(u16, bool)> {
  match key {
    "space" => Some((VK_SPACE.0, false)),
    "backspace" | "back" => Some((VK_BACK.0, false)),
    "escape" => Some((VK_ESCAPE.0, false)),
    "enter" => Some((VK_RETURN.0, false)),
    "tab" => Some((VK_TAB.0, false)),
    "left" => Some((VK_LEFT.0, true)),
    "right" => Some((VK_RIGHT.0, true)),
    "up" => Some((VK_UP.0, true)),
    "down" => Some((VK_DOWN.0, true)),
    "home" => Some((VK_HOME.0, true)),
    "end" => Some((VK_END.0, true)),
    "pageup" => Some((VK_PRIOR.0, true)),
    "pagedown" => Some((VK_NEXT.0, true)),
    "insert" => Some((VK_INSERT.0, true)),
    "delete" => Some((VK_DELETE.0, true)),
    _ => {
      let function_number = key.strip_prefix('f')?.parse::<u16>().ok()?;
      (1..=24)
        .contains(&function_number)
        .then_some((VK_F1.0 + function_number - 1, false))
    }
  }
}

#[cfg(target_os = "windows")]
fn windows_printable_virtual_key_code(key: &str) -> Option<u16> {
  let ch = single_char(key)?;
  match ch.to_ascii_lowercase() {
    'a'..='z' => Some(ch.to_ascii_uppercase() as u16),
    '0'..='9' => Some(ch as u16),
    '`' => Some(VK_OEM_3.0),
    '-' => Some(VK_OEM_MINUS.0),
    '=' => Some(VK_OEM_PLUS.0),
    '[' => Some(VK_OEM_4.0),
    ']' => Some(VK_OEM_6.0),
    '\\' => Some(VK_OEM_5.0),
    ';' => Some(VK_OEM_1.0),
    '\'' => Some(VK_OEM_7.0),
    ',' => Some(VK_OEM_COMMA.0),
    '.' => Some(VK_OEM_PERIOD.0),
    '/' => Some(VK_OEM_2.0),
    _ => None,
  }
}

#[cfg(target_os = "windows")]
fn windows_virtual_key_code(key: &str) -> Option<(u16, bool)> {
  windows_named_virtual_key_code(key)
    .or_else(|| windows_printable_virtual_key_code(key).map(|virtual_key| (virtual_key, false)))
}

#[cfg(target_os = "windows")]
fn windows_scan_code(virtual_key: u16) -> u16 {
  unsafe { MapVirtualKeyW(u32::from(virtual_key), MAPVK_VK_TO_VSC) as u16 }
}

#[cfg(target_os = "windows")]
fn windows_control_char(raw_key: &str) -> Option<char> {
  match raw_key {
    "space" => Some('\0'),
    "@" => Some('\0'),
    "[" => Some('\x1b'),
    "\\" => Some('\x1c'),
    "]" => Some('\x1d'),
    "^" => Some('\x1e'),
    "_" => Some('\x1f'),
    "?" => Some('\x7f'),
    _ => {
      let ch = single_char(raw_key)?.to_ascii_lowercase();
      if ch.is_ascii_lowercase() {
        Some(char::from((ch as u8 - b'a') + 1))
      } else {
        None
      }
    }
  }
}

#[cfg(target_os = "windows")]
fn windows_typed_char(keystroke: &Keystroke, effective_shift: bool) -> Option<char> {
  if let Some(key_char) = keystroke.key_char.as_deref().and_then(single_char) {
    return Some(key_char);
  }

  match keystroke.key.as_str() {
    "space" => Some(' '),
    "backspace" | "back" => Some('\x08'),
    "tab" => Some('\t'),
    "enter" => Some('\r'),
    _ => {
      let ch = single_char(&keystroke.key)?;
      if effective_shift && ch.is_ascii_lowercase() {
        Some(ch.to_ascii_uppercase())
      } else {
        Some(ch)
      }
    }
  }
}

#[cfg(target_os = "windows")]
fn windows_control_state(keystroke: &Keystroke, inferred_shift: bool, enhanced: bool) -> u32 {
  let mut state = 0;
  if keystroke.modifiers.shift || inferred_shift {
    state |= WIN32_SHIFT_PRESSED;
  }
  if keystroke.modifiers.control {
    state |= WIN32_LEFT_CTRL_PRESSED;
  }
  if keystroke.modifiers.alt {
    state |= WIN32_LEFT_ALT_PRESSED;
  }
  if enhanced {
    state |= WIN32_ENHANCED_KEY;
  }
  state
}

#[cfg(target_os = "windows")]
fn windows_unicode_char(keystroke: &Keystroke, effective_shift: bool) -> u16 {
  if keystroke.modifiers.control
    && !keystroke.modifiers.alt
    && !keystroke.modifiers.platform
    && let Some(ch) = windows_control_char(&keystroke.key)
  {
    return ch as u16;
  }

  windows_typed_char(keystroke, effective_shift)
    .map(|ch| ch as u16)
    .unwrap_or(0)
}

#[cfg(target_os = "windows")]
fn to_windows_conpty_input_escape(
  keystroke: &Keystroke,
  keyboard_protocol_flags: u32,
) -> Option<Cow<'static, str>> {
  if !windows_conpty_win32_input_mode_enabled(keyboard_protocol_flags) {
    return None;
  }

  let key = normalized_key_name(&keystroke.key);
  let (base_key, inferred_shift) = if let Some(base_key) = unshifted_symbol_key(key.as_ref()) {
    (Cow::Borrowed(base_key), true)
  } else {
    (key, false)
  };
  let (virtual_key, enhanced) = windows_virtual_key_code(base_key.as_ref())?;
  let scan_code = windows_scan_code(virtual_key);
  let effective_shift = keystroke.modifiers.shift || inferred_shift;
  let unicode_char = windows_unicode_char(keystroke, effective_shift);
  let control_state = windows_control_state(keystroke, inferred_shift, enhanced);

  Some(Cow::Owned(format!(
    "\x1b[{virtual_key};{scan_code};{unicode_char};1;{control_state};1_"
  )))
}

fn to_kitty_text_escape(
  key: &str,
  keystroke: &Keystroke,
  keyboard_protocol_flags: u32,
) -> Option<Cow<'static, str>> {
  let report_all = kitty_report_all_keys_as_escape_codes(keyboard_protocol_flags);
  let disambiguate = kitty_disambiguate_escape_codes(keyboard_protocol_flags);
  if !report_all && !disambiguate {
    return None;
  }

  let has_non_shift_modifiers =
    keystroke.modifiers.control || keystroke.modifiers.alt || keystroke.modifiers.platform;
  if !report_all && !has_non_shift_modifiers {
    return None;
  }

  let (code, inferred_shift) = kitty_text_key_code(key)?;
  Some(kitty_escape(
    code,
    kitty_modifier_code(keystroke, inferred_shift),
  ))
}
pub fn to_esc_str(
  keystroke: &Keystroke,
  mode: &TermMode,
  alt_is_meta: bool,
  keyboard_protocol_flags: u32,
) -> Option<Cow<'static, str>> {
  let modifiers = AlacModifiers::new(keystroke);
  let key = normalized_key_name(&keystroke.key);

  #[cfg(target_os = "windows")]
  if let Some(escape) = to_windows_conpty_input_escape(keystroke, keyboard_protocol_flags) {
    return Some(escape);
  }

  if let Some(escape) = to_kitty_c0_escape(key.as_ref(), keystroke, keyboard_protocol_flags) {
    return Some(escape);
  }

  if let Some(escape) = to_kitty_text_escape(key.as_ref(), keystroke, keyboard_protocol_flags) {
    return Some(escape);
  }

  // Manual Bindings including modifiers
  let manual_esc_str: Option<&'static str> = match (key.as_ref(), &modifiers) {
    //Basic special keys
    ("tab", AlacModifiers::None) => Some("\x09"),
    ("escape", AlacModifiers::None) => Some("\x1b"),
    ("enter", AlacModifiers::None) => Some("\x0d"),
    ("enter", AlacModifiers::Shift) => Some("\x0a"),
    ("enter", AlacModifiers::Alt) => Some("\x1b\x0d"),
    ("backspace", AlacModifiers::None) => Some("\x7f"),
    //Interesting escape codes
    ("tab", AlacModifiers::Shift) => Some("\x1b[Z"),
    ("backspace", AlacModifiers::Ctrl) => Some("\x08"),
    ("backspace", AlacModifiers::CtrlShift) => Some("\x1b[127;6u"),
    ("backspace", AlacModifiers::Alt) => Some("\x1b\x7f"),
    ("backspace", AlacModifiers::Shift) => Some("\x7f"),
    ("space", AlacModifiers::Ctrl) => Some("\x00"),
    ("home", AlacModifiers::Shift) if mode.contains(TermMode::ALT_SCREEN) => Some("\x1b[1;2H"),
    ("end", AlacModifiers::Shift) if mode.contains(TermMode::ALT_SCREEN) => Some("\x1b[1;2F"),
    // ("pageup", AlacModifiers::Shift) if mode.contains(TermMode::ALT_SCREEN) => Some("\x1b[5;2~"),
    // ("pagedown", AlacModifiers::Shift) if mode.contains(TermMode::ALT_SCREEN) => Some("\x1b[6;2~"),
    ("home", AlacModifiers::None) if mode.contains(TermMode::APP_CURSOR) => Some("\x1bOH"),
    ("home", AlacModifiers::None) if !mode.contains(TermMode::APP_CURSOR) => Some("\x1b[H"),
    ("end", AlacModifiers::None) if mode.contains(TermMode::APP_CURSOR) => Some("\x1bOF"),
    ("end", AlacModifiers::None) if !mode.contains(TermMode::APP_CURSOR) => Some("\x1b[F"),
    ("up", AlacModifiers::None) if mode.contains(TermMode::APP_CURSOR) => Some("\x1bOA"),
    ("up", AlacModifiers::None) if !mode.contains(TermMode::APP_CURSOR) => Some("\x1b[A"),
    ("down", AlacModifiers::None) if mode.contains(TermMode::APP_CURSOR) => Some("\x1bOB"),
    ("down", AlacModifiers::None) if !mode.contains(TermMode::APP_CURSOR) => Some("\x1b[B"),
    ("right", AlacModifiers::None) if mode.contains(TermMode::APP_CURSOR) => Some("\x1bOC"),
    ("right", AlacModifiers::None) if !mode.contains(TermMode::APP_CURSOR) => Some("\x1b[C"),
    ("left", AlacModifiers::None) if mode.contains(TermMode::APP_CURSOR) => Some("\x1bOD"),
    ("left", AlacModifiers::None) if !mode.contains(TermMode::APP_CURSOR) => Some("\x1b[D"),
    ("back", AlacModifiers::None) => Some("\x7f"),
    ("insert", AlacModifiers::None) => Some("\x1b[2~"),
    ("delete", AlacModifiers::None) => Some("\x1b[3~"),
    // ("pageup", AlacModifiers::None) => Some("\x1b[5~"),
    // ("pagedown", AlacModifiers::None) => Some("\x1b[6~"),
    ("f1", AlacModifiers::None) => Some("\x1bOP"),
    ("f2", AlacModifiers::None) => Some("\x1bOQ"),
    ("f3", AlacModifiers::None) => Some("\x1bOR"),
    ("f4", AlacModifiers::None) => Some("\x1bOS"),
    ("f5", AlacModifiers::None) => Some("\x1b[15~"),
    ("f6", AlacModifiers::None) => Some("\x1b[17~"),
    ("f7", AlacModifiers::None) => Some("\x1b[18~"),
    ("f8", AlacModifiers::None) => Some("\x1b[19~"),
    ("f9", AlacModifiers::None) => Some("\x1b[20~"),
    ("f10", AlacModifiers::None) => Some("\x1b[21~"),
    ("f11", AlacModifiers::None) => Some("\x1b[23~"),
    ("f12", AlacModifiers::None) => Some("\x1b[24~"),
    ("f13", AlacModifiers::None) => Some("\x1b[25~"),
    ("f14", AlacModifiers::None) => Some("\x1b[26~"),
    ("f15", AlacModifiers::None) => Some("\x1b[28~"),
    ("f16", AlacModifiers::None) => Some("\x1b[29~"),
    ("f17", AlacModifiers::None) => Some("\x1b[31~"),
    ("f18", AlacModifiers::None) => Some("\x1b[32~"),
    ("f19", AlacModifiers::None) => Some("\x1b[33~"),
    ("f20", AlacModifiers::None) => Some("\x1b[34~"),
    // NumpadEnter, Action::Esc("\n".into());
    //Mappings for caret notation keys
    // Handle both lowercase (Ctrl+key) and uppercase (Ctrl+Shift+key or just Ctrl+Key on some platforms)
    ("a", AlacModifiers::Ctrl) => Some("\x01"),      //1
    ("A", AlacModifiers::Ctrl) => Some("\x01"),      //1
    ("A", AlacModifiers::CtrlShift) => Some("\x01"), //1
    ("b", AlacModifiers::Ctrl) => Some("\x02"),      //2
    ("B", AlacModifiers::Ctrl) => Some("\x02"),      //2
    ("B", AlacModifiers::CtrlShift) => Some("\x02"), //2
    ("c", AlacModifiers::Ctrl) => Some("\x03"),      //3
    ("C", AlacModifiers::Ctrl) => Some("\x03"),      //3
    ("C", AlacModifiers::CtrlShift) => Some("\x03"), //3
    ("d", AlacModifiers::Ctrl) => Some("\x04"),      //4
    ("D", AlacModifiers::Ctrl) => Some("\x04"),      //4
    ("D", AlacModifiers::CtrlShift) => Some("\x04"), //4
    ("e", AlacModifiers::Ctrl) => Some("\x05"),      //5
    ("E", AlacModifiers::Ctrl) => Some("\x05"),      //5
    ("E", AlacModifiers::CtrlShift) => Some("\x05"), //5
    ("f", AlacModifiers::Ctrl) => Some("\x06"),      //6
    ("F", AlacModifiers::Ctrl) => Some("\x06"),      //6
    ("F", AlacModifiers::CtrlShift) => Some("\x06"), //6
    ("g", AlacModifiers::Ctrl) => Some("\x07"),      //7
    ("G", AlacModifiers::Ctrl) => Some("\x07"),      //7
    ("G", AlacModifiers::CtrlShift) => Some("\x07"), //7
    ("h", AlacModifiers::Ctrl) => Some("\x08"),      //8
    ("H", AlacModifiers::Ctrl) => Some("\x08"),      //8
    ("H", AlacModifiers::CtrlShift) => Some("\x08"), //8
    ("i", AlacModifiers::Ctrl) => Some("\x09"),      //9
    ("I", AlacModifiers::Ctrl) => Some("\x09"),      //9
    ("I", AlacModifiers::CtrlShift) => Some("\x09"), //9
    ("j", AlacModifiers::Ctrl) => Some("\x0a"),      //10
    ("J", AlacModifiers::Ctrl) => Some("\x0a"),      //10
    ("J", AlacModifiers::CtrlShift) => Some("\x0a"), //10
    ("k", AlacModifiers::Ctrl) => Some("\x0b"),      //11
    ("K", AlacModifiers::Ctrl) => Some("\x0b"),      //11
    ("K", AlacModifiers::CtrlShift) => Some("\x0b"), //11
    ("l", AlacModifiers::Ctrl) => Some("\x0c"),      //12
    ("L", AlacModifiers::Ctrl) => Some("\x0c"),      //12
    ("L", AlacModifiers::CtrlShift) => Some("\x0c"), //12
    ("m", AlacModifiers::Ctrl) => Some("\x0d"),      //13
    ("M", AlacModifiers::Ctrl) => Some("\x0d"),      //13
    ("M", AlacModifiers::CtrlShift) => Some("\x0d"), //13
    ("n", AlacModifiers::Ctrl) => Some("\x0e"),      //14
    ("N", AlacModifiers::Ctrl) => Some("\x0e"),      //14
    ("N", AlacModifiers::CtrlShift) => Some("\x0e"), //14
    ("o", AlacModifiers::Ctrl) => Some("\x0f"),      //15
    ("O", AlacModifiers::Ctrl) => Some("\x0f"),      //15
    ("O", AlacModifiers::CtrlShift) => Some("\x0f"), //15
    ("p", AlacModifiers::Ctrl) => Some("\x10"),      //16
    ("P", AlacModifiers::Ctrl) => Some("\x10"),      //16
    ("P", AlacModifiers::CtrlShift) => Some("\x10"), //16
    ("q", AlacModifiers::Ctrl) => Some("\x11"),      //17
    ("Q", AlacModifiers::Ctrl) => Some("\x11"),      //17
    ("Q", AlacModifiers::CtrlShift) => Some("\x11"), //17
    ("r", AlacModifiers::Ctrl) => Some("\x12"),      //18
    ("R", AlacModifiers::Ctrl) => Some("\x12"),      //18
    ("R", AlacModifiers::CtrlShift) => Some("\x12"), //18
    ("s", AlacModifiers::Ctrl) => Some("\x13"),      //19
    ("S", AlacModifiers::Ctrl) => Some("\x13"),      //19
    ("S", AlacModifiers::CtrlShift) => Some("\x13"), //19
    ("t", AlacModifiers::Ctrl) => Some("\x14"),      //20
    ("T", AlacModifiers::Ctrl) => Some("\x14"),      //20
    ("T", AlacModifiers::CtrlShift) => Some("\x14"), //20
    ("u", AlacModifiers::Ctrl) => Some("\x15"),      //21
    ("U", AlacModifiers::Ctrl) => Some("\x15"),      //21
    ("U", AlacModifiers::CtrlShift) => Some("\x15"), //21
    ("v", AlacModifiers::Ctrl) => Some("\x16"),      //22
    ("V", AlacModifiers::Ctrl) => Some("\x16"),      //22
    ("V", AlacModifiers::CtrlShift) => Some("\x16"), //22
    ("w", AlacModifiers::Ctrl) => Some("\x17"),      //23
    ("W", AlacModifiers::Ctrl) => Some("\x17"),      //23
    ("W", AlacModifiers::CtrlShift) => Some("\x17"), //23
    ("x", AlacModifiers::Ctrl) => Some("\x18"),      //24
    ("X", AlacModifiers::Ctrl) => Some("\x18"),      //24
    ("X", AlacModifiers::CtrlShift) => Some("\x18"), //24
    ("y", AlacModifiers::Ctrl) => Some("\x19"),      //25
    ("Y", AlacModifiers::Ctrl) => Some("\x19"),      //25
    ("Y", AlacModifiers::CtrlShift) => Some("\x19"), //25
    ("z", AlacModifiers::Ctrl) => Some("\x1a"),      //26
    ("Z", AlacModifiers::Ctrl) => Some("\x1a"),      //26
    ("Z", AlacModifiers::CtrlShift) => Some("\x1a"), //26
    ("@", AlacModifiers::Ctrl) => Some("\x00"),      //0
    ("[", AlacModifiers::Ctrl) => Some("\x1b"),      //27
    ("\\", AlacModifiers::Ctrl) => Some("\x1c"),     //28
    ("]", AlacModifiers::Ctrl) => Some("\x1d"),      //29
    ("^", AlacModifiers::Ctrl) => Some("\x1e"),      //30
    ("_", AlacModifiers::Ctrl) => Some("\x1f"),      //31
    ("?", AlacModifiers::Ctrl) => Some("\x7f"),      //127
    _ => None,
  };
  if let Some(esc_str) = manual_esc_str {
    return Some(Cow::Borrowed(esc_str));
  }

  // Automated bindings applying modifiers
  if modifiers.any() {
    let modifier_code = modifier_code(keystroke);
    let modified_esc_str = match key.as_ref() {
      "up" => Some(format!("\x1b[1;{}A", modifier_code)),
      "down" => Some(format!("\x1b[1;{}B", modifier_code)),
      "right" => Some(format!("\x1b[1;{}C", modifier_code)),
      "left" => Some(format!("\x1b[1;{}D", modifier_code)),
      "f1" => Some(format!("\x1b[1;{}P", modifier_code)),
      "f2" => Some(format!("\x1b[1;{}Q", modifier_code)),
      "f3" => Some(format!("\x1b[1;{}R", modifier_code)),
      "f4" => Some(format!("\x1b[1;{}S", modifier_code)),
      "f5" => Some(format!("\x1b[15;{}~", modifier_code)),
      "f6" => Some(format!("\x1b[17;{}~", modifier_code)),
      "f7" => Some(format!("\x1b[18;{}~", modifier_code)),
      "f8" => Some(format!("\x1b[19;{}~", modifier_code)),
      "f9" => Some(format!("\x1b[20;{}~", modifier_code)),
      "f10" => Some(format!("\x1b[21;{}~", modifier_code)),
      "f11" => Some(format!("\x1b[23;{}~", modifier_code)),
      "f12" => Some(format!("\x1b[24;{}~", modifier_code)),
      "f13" => Some(format!("\x1b[25;{}~", modifier_code)),
      "f14" => Some(format!("\x1b[26;{}~", modifier_code)),
      "f15" => Some(format!("\x1b[28;{}~", modifier_code)),
      "f16" => Some(format!("\x1b[29;{}~", modifier_code)),
      "f17" => Some(format!("\x1b[31;{}~", modifier_code)),
      "f18" => Some(format!("\x1b[32;{}~", modifier_code)),
      "f19" => Some(format!("\x1b[33;{}~", modifier_code)),
      "f20" => Some(format!("\x1b[34;{}~", modifier_code)),
      _ if modifier_code == 2 => None,
      "insert" => Some(format!("\x1b[2;{}~", modifier_code)),
      "pageup" => Some(format!("\x1b[5;{}~", modifier_code)),
      "pagedown" => Some(format!("\x1b[6;{}~", modifier_code)),
      "end" => Some(format!("\x1b[1;{}F", modifier_code)),
      "home" => Some(format!("\x1b[1;{}H", modifier_code)),
      _ => None,
    };
    if let Some(esc_str) = modified_esc_str {
      return Some(Cow::Owned(esc_str));
    }
  }

  if alt_is_meta {
    let is_alt_lowercase_ascii = modifiers == AlacModifiers::Alt && keystroke.key.is_ascii();
    let is_alt_uppercase_ascii =
      keystroke.modifiers.alt && keystroke.modifiers.shift && keystroke.key.is_ascii();
    if is_alt_lowercase_ascii || is_alt_uppercase_ascii {
      let key = if is_alt_uppercase_ascii {
        &keystroke.key.to_ascii_uppercase()
      } else {
        &keystroke.key
      };
      return Some(Cow::Owned(format!("\x1b{}", key)));
    }
  }

  None
}

pub fn to_input_bytes(
  keystroke: &Keystroke,
  mode: &TermMode,
  alt_is_meta: bool,
  keyboard_protocol_flags: u32,
) -> Option<Cow<'static, [u8]>> {
  if let Some(escape) = to_esc_str(keystroke, mode, alt_is_meta, keyboard_protocol_flags) {
    return Some(match escape {
      Cow::Borrowed(escape) => Cow::Borrowed(escape.as_bytes()),
      Cow::Owned(escape) => Cow::Owned(escape.into_bytes()),
    });
  }

  if !keystroke.modifiers.control
    && !keystroke.modifiers.alt
    && !keystroke.modifiers.platform
    && let Some(key_char) = &keystroke.key_char
  {
    return Some(Cow::Owned(key_char.as_bytes().to_vec()));
  }

  None
}

///   Code     Modifiers
/// ---------+---------------------------
///    2     | Shift
///    3     | Alt
///    4     | Shift + Alt
///    5     | Control
///    6     | Shift + Control
///    7     | Alt + Control
///    8     | Shift + Alt + Control
/// ---------+---------------------------
/// from: https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-PC-Style-Function-Keys
fn modifier_code(keystroke: &Keystroke) -> u32 {
  let mut modifier_code = 0;
  if keystroke.modifiers.shift {
    modifier_code |= 1;
  }
  if keystroke.modifiers.alt {
    modifier_code |= 1 << 1;
  }
  if keystroke.modifiers.control {
    modifier_code |= 1 << 2;
  }
  modifier_code + 1
}

#[cfg(test)]
mod tests {
  use super::{to_esc_str, to_input_bytes};
  use gpui::{Keystroke, Modifiers};
  use terminal_kernel::term::TermMode;

  use crate::kitty_graphics::pty_filter::{
    KITTY_KEYBOARD_DISAMBIGUATE_ESCAPE_CODES, KITTY_KEYBOARD_REPORT_ALL_KEYS_AS_ESCAPE_CODES,
    WINDOWS_CONPTY_WIN32_INPUT_MODE,
  };

  fn keystroke(key: &str, modifiers: Modifiers) -> Keystroke {
    Keystroke {
      modifiers,
      key: key.to_string(),
      key_char: None,
    }
  }

  #[test]
  fn ctrl_shift_backspace_uses_modified_backspace_sequence() {
    let escape = to_esc_str(
      &keystroke(
        "backspace",
        Modifiers {
          control: true,
          shift: true,
          ..Modifiers::default()
        },
      ),
      &TermMode::empty(),
      true,
      0,
    );

    assert_eq!(escape.as_deref(), Some("\x1b[127;6u"));
  }

  #[test]
  fn capitalized_backspace_name_is_normalized() {
    let escape = to_esc_str(
      &keystroke(
        "Backspace",
        Modifiers {
          control: true,
          shift: true,
          ..Modifiers::default()
        },
      ),
      &TermMode::empty(),
      true,
      0,
    );

    assert_eq!(escape.as_deref(), Some("\x1b[127;6u"));
  }

  #[test]
  fn ctrl_shift_backspace_uses_kitty_sequence_when_all_keys_are_escaped() {
    let escape = to_esc_str(
      &keystroke(
        "backspace",
        Modifiers {
          control: true,
          shift: true,
          ..Modifiers::default()
        },
      ),
      &TermMode::empty(),
      true,
      KITTY_KEYBOARD_REPORT_ALL_KEYS_AS_ESCAPE_CODES,
    );

    assert_eq!(escape.as_deref(), Some("\x1b[127;6u"));
  }

  #[test]
  fn ctrl_backspace_remains_legacy_backspace() {
    let escape = to_esc_str(
      &keystroke(
        "backspace",
        Modifiers {
          control: true,
          ..Modifiers::default()
        },
      ),
      &TermMode::empty(),
      true,
      0,
    );

    assert_eq!(escape.as_deref(), Some("\x08"));
  }

  #[test]
  fn shift_enter_uses_kitty_sequence_when_disambiguate_mode_is_enabled() {
    let escape = to_esc_str(
      &keystroke(
        "enter",
        Modifiers {
          shift: true,
          ..Modifiers::default()
        },
      ),
      &TermMode::empty(),
      true,
      KITTY_KEYBOARD_DISAMBIGUATE_ESCAPE_CODES,
    );

    assert_eq!(escape.as_deref(), Some("\x1b[13;2u"));
  }

  #[test]
  fn ctrl_j_uses_kitty_sequence_when_disambiguate_mode_is_enabled() {
    let escape = to_esc_str(
      &keystroke(
        "j",
        Modifiers {
          control: true,
          ..Modifiers::default()
        },
      ),
      &TermMode::empty(),
      true,
      KITTY_KEYBOARD_DISAMBIGUATE_ESCAPE_CODES,
    );

    assert_eq!(escape.as_deref(), Some("\x1b[106;5u"));
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn shift_enter_uses_conpty_win32_input_mode_when_enabled() {
    let escape = to_esc_str(
      &keystroke(
        "enter",
        Modifiers {
          shift: true,
          ..Modifiers::default()
        },
      ),
      &TermMode::empty(),
      true,
      WINDOWS_CONPTY_WIN32_INPUT_MODE,
    );

    assert_eq!(escape.as_deref(), Some("\x1b[13;28;13;1;16;1_"));
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn ctrl_j_uses_conpty_win32_input_mode_when_enabled() {
    let escape = to_esc_str(
      &keystroke(
        "j",
        Modifiers {
          control: true,
          ..Modifiers::default()
        },
      ),
      &TermMode::empty(),
      true,
      WINDOWS_CONPTY_WIN32_INPUT_MODE,
    );

    assert_eq!(escape.as_deref(), Some("\x1b[74;36;10;1;8;1_"));
  }

  #[test]
  fn plain_escape_is_disambiguated_in_kitty_mode() {
    let escape = to_esc_str(
      &keystroke("escape", Modifiers::default()),
      &TermMode::empty(),
      true,
      KITTY_KEYBOARD_DISAMBIGUATE_ESCAPE_CODES,
    );

    assert_eq!(escape.as_deref(), Some("\x1b[27u"));
  }

  #[test]
  fn shifted_symbol_recovers_shift_for_kitty_sequences() {
    let escape = to_esc_str(
      &keystroke(
        "!",
        Modifiers {
          control: true,
          ..Modifiers::default()
        },
      ),
      &TermMode::empty(),
      true,
      KITTY_KEYBOARD_DISAMBIGUATE_ESCAPE_CODES,
    );

    assert_eq!(escape.as_deref(), Some("\x1b[49;6u"));
  }

  #[test]
  fn report_all_keys_escapes_plain_text_keys() {
    let escape = to_esc_str(
      &Keystroke {
        key: "a".to_string(),
        key_char: Some("a".to_string()),
        modifiers: Modifiers::default(),
      },
      &TermMode::empty(),
      true,
      KITTY_KEYBOARD_REPORT_ALL_KEYS_AS_ESCAPE_CODES,
    );

    assert_eq!(escape.as_deref(), Some("\x1b[97u"));
  }

  #[test]
  fn plain_character_falls_back_to_key_char_input() {
    let input = to_input_bytes(
      &Keystroke {
        key: "a".to_string(),
        key_char: Some("a".to_string()),
        modifiers: Modifiers::default(),
      },
      &TermMode::empty(),
      true,
      0,
    )
    .unwrap();

    assert_eq!(input.as_ref(), b"a");
  }

  #[test]
  fn shifted_character_falls_back_to_key_char_input() {
    let input = to_input_bytes(
      &Keystroke {
        key: "A".to_string(),
        key_char: Some("A".to_string()),
        modifiers: Modifiers {
          shift: true,
          ..Modifiers::default()
        },
      },
      &TermMode::empty(),
      true,
      0,
    )
    .unwrap();

    assert_eq!(input.as_ref(), b"A");
  }
}
