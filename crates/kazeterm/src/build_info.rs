use std::sync::OnceLock;

use config::TerminalKernel;

static ALACRITTY_TERM_PROGRAM_VERSION: OnceLock<String> = OnceLock::new();
static GHOSTTY_TERM_PROGRAM_VERSION: OnceLock<String> = OnceLock::new();
static VTE_TERM_PROGRAM_VERSION: OnceLock<String> = OnceLock::new();

static ALACRITTY_XTVERSION_RESPONSE: OnceLock<String> = OnceLock::new();
static GHOSTTY_XTVERSION_RESPONSE: OnceLock<String> = OnceLock::new();
static VTE_XTVERSION_RESPONSE: OnceLock<String> = OnceLock::new();

pub(crate) fn app_version() -> &'static str {
  env!("CARGO_PKG_VERSION")
}

pub(crate) fn commit_hash() -> &'static str {
  option_env!("KAZETERM_COMMIT_SHA").unwrap_or("unknown")
}

pub(crate) fn short_commit_hash() -> &'static str {
  let commit_hash = commit_hash();
  commit_hash.get(..7).unwrap_or(commit_hash)
}

pub(crate) fn terminal_program_version(kernel: TerminalKernel) -> &'static str {
  match kernel {
    TerminalKernel::Alacritty => ALACRITTY_TERM_PROGRAM_VERSION
      .get_or_init(|| format_terminal_program_version(TerminalKernel::Alacritty))
      .as_str(),
    TerminalKernel::Ghostty => GHOSTTY_TERM_PROGRAM_VERSION
      .get_or_init(|| format_terminal_program_version(TerminalKernel::Ghostty))
      .as_str(),
    TerminalKernel::Vte => VTE_TERM_PROGRAM_VERSION
      .get_or_init(|| format_terminal_program_version(TerminalKernel::Vte))
      .as_str(),
  }
}

pub(crate) fn xtversion_response(kernel: TerminalKernel) -> &'static str {
  match kernel {
    TerminalKernel::Alacritty => ALACRITTY_XTVERSION_RESPONSE
      .get_or_init(|| {
        format!(
          "kazeterm {}",
          terminal_program_version(TerminalKernel::Alacritty)
        )
      })
      .as_str(),
    TerminalKernel::Ghostty => GHOSTTY_XTVERSION_RESPONSE
      .get_or_init(|| {
        format!(
          "kazeterm {}",
          terminal_program_version(TerminalKernel::Ghostty)
        )
      })
      .as_str(),
    TerminalKernel::Vte => VTE_XTVERSION_RESPONSE
      .get_or_init(|| format!("kazeterm {}", terminal_program_version(TerminalKernel::Vte)))
      .as_str(),
  }
}

fn format_terminal_program_version(kernel: TerminalKernel) -> String {  format!(
    "{} ({}, commit {})",
    app_version(),
    kernel,
    short_commit_hash()
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn terminal_program_version_only_includes_app_version_and_commit() {
    let version = terminal_program_version(TerminalKernel::Alacritty);

    assert_eq!(
      version,
      format!("{} (commit {})", app_version(), short_commit_hash())
    );
    assert!(!version.contains("alacritty"));
    assert!(!version.contains("ghostty"));
    assert!(!version.contains("vte"));
  }

  #[test]
  fn xtversion_response_keeps_commit_without_kernel_details() {
    let response = xtversion_response(TerminalKernel::Ghostty);

    assert_eq!(
      response,
      format!(
        "kazeterm {} (commit {})",
        app_version(),
        short_commit_hash()
      )
    );
  }
}
