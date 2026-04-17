use futures::channel::mpsc::UnboundedReceiver;

pub use alacritty_terminal::Term;

pub mod event {
  pub use alacritty_terminal::event::*;
}

pub mod event_loop {
  pub use alacritty_terminal::event_loop::*;
}

pub mod grid {
  pub use alacritty_terminal::grid::*;
}

pub mod index {
  pub use alacritty_terminal::index::*;
}

pub mod selection {
  pub use alacritty_terminal::selection::*;
}

pub mod sync {
  pub use alacritty_terminal::sync::*;
}

pub mod term {
  pub use alacritty_terminal::term::*;
}

pub mod tty {
  pub use alacritty_terminal::tty::*;
}

pub mod vte {
  pub mod ansi {
    pub use alacritty_terminal::vte::ansi::*;
  }
}

pub type SessionEvents = UnboundedReceiver<event::Event>;

pub fn to_themeing_color(color: &vte::ansi::Color) -> themeing::AnsiColor {
  use themeing::{AnsiColor, AnsiNamedColor, AnsiRgb};

  match color {
    vte::ansi::Color::Named(named) => AnsiColor::Named(match named {
      vte::ansi::NamedColor::Black => AnsiNamedColor::Black,
      vte::ansi::NamedColor::Red => AnsiNamedColor::Red,
      vte::ansi::NamedColor::Green => AnsiNamedColor::Green,
      vte::ansi::NamedColor::Yellow => AnsiNamedColor::Yellow,
      vte::ansi::NamedColor::Blue => AnsiNamedColor::Blue,
      vte::ansi::NamedColor::Magenta => AnsiNamedColor::Magenta,
      vte::ansi::NamedColor::Cyan => AnsiNamedColor::Cyan,
      vte::ansi::NamedColor::White => AnsiNamedColor::White,
      vte::ansi::NamedColor::BrightBlack => AnsiNamedColor::BrightBlack,
      vte::ansi::NamedColor::BrightRed => AnsiNamedColor::BrightRed,
      vte::ansi::NamedColor::BrightGreen => AnsiNamedColor::BrightGreen,
      vte::ansi::NamedColor::BrightYellow => AnsiNamedColor::BrightYellow,
      vte::ansi::NamedColor::BrightBlue => AnsiNamedColor::BrightBlue,
      vte::ansi::NamedColor::BrightMagenta => AnsiNamedColor::BrightMagenta,
      vte::ansi::NamedColor::BrightCyan => AnsiNamedColor::BrightCyan,
      vte::ansi::NamedColor::BrightWhite => AnsiNamedColor::BrightWhite,
      vte::ansi::NamedColor::Foreground => AnsiNamedColor::Foreground,
      vte::ansi::NamedColor::Background => AnsiNamedColor::Background,
      vte::ansi::NamedColor::Cursor => AnsiNamedColor::Cursor,
      vte::ansi::NamedColor::DimBlack => AnsiNamedColor::DimBlack,
      vte::ansi::NamedColor::DimRed => AnsiNamedColor::DimRed,
      vte::ansi::NamedColor::DimGreen => AnsiNamedColor::DimGreen,
      vte::ansi::NamedColor::DimYellow => AnsiNamedColor::DimYellow,
      vte::ansi::NamedColor::DimBlue => AnsiNamedColor::DimBlue,
      vte::ansi::NamedColor::DimMagenta => AnsiNamedColor::DimMagenta,
      vte::ansi::NamedColor::DimCyan => AnsiNamedColor::DimCyan,
      vte::ansi::NamedColor::DimWhite => AnsiNamedColor::DimWhite,
      vte::ansi::NamedColor::BrightForeground => AnsiNamedColor::BrightForeground,
      vte::ansi::NamedColor::DimForeground => AnsiNamedColor::DimForeground,
    }),
    vte::ansi::Color::Spec(rgb) => AnsiColor::Spec(AnsiRgb {
      r: rgb.r,
      g: rgb.g,
      b: rgb.b,
    }),
    vte::ansi::Color::Indexed(index) => AnsiColor::Indexed(*index),
  }
}

pub fn is_default_background(color: &vte::ansi::Color) -> bool {
  matches!(
    color,
    vte::ansi::Color::Named(vte::ansi::NamedColor::Background)
  )
}

pub fn is_default_foreground(color: &vte::ansi::Color) -> bool {
  matches!(
    color,
    vte::ansi::Color::Named(vte::ansi::NamedColor::Foreground)
  )
}
