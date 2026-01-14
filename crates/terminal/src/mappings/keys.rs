use std::borrow::Cow;

use alacritty_terminal::term::TermMode;
use gpui::Keystroke;

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

pub fn to_esc_str(
  keystroke: &Keystroke,
  mode: &TermMode,
  alt_is_meta: bool,
) -> Option<Cow<'static, str>> {
  let modifiers = AlacModifiers::new(keystroke);

  // Manual Bindings including modifiers
  let manual_esc_str: Option<&'static str> = match (keystroke.key.as_ref(), &modifiers) {
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
    let modified_esc_str = match keystroke.key.as_ref() {
      "up" => Some(format!("\x1b[1;{}A", modifier_code)),
      "down" => Some(format!("\x1b[1;{}B", modifier_code)),
      "right" => Some(format!("\x1b[1;{}C", modifier_code)),
      "left" => Some(format!("\x1b[1;{}D", modifier_code)),
      "f1" => Some(format!("\x1b[1;{}P", modifier_code)),
      "f2" => Some(format!("\x1b[1;{}Q", modifier_code)),
      "f3" => Some(format!("\x1b[1;{}R", modifier_code)),
      "f4" => Some(format!("\x1b[1;{}S", modifier_code)),
      "F5" => Some(format!("\x1b[15;{}~", modifier_code)),
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
