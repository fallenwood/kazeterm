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

pub(crate) fn release_tag() -> &'static str {
  match option_env!("KAZETERM_RELEASE_TAG") {
    Some(tag) if !tag.is_empty() => tag,
    _ => "local",
  }
}

pub(crate) fn build_source() -> &'static str {
  option_env!("KAZETERM_BUILD_SOURCE").unwrap_or("local")
}

pub(crate) fn is_local_build() -> bool {
  build_source().eq_ignore_ascii_case("local")
}

pub(crate) fn build_unix_timestamp() -> Option<i64> {
  option_env!("KAZETERM_BUILD_UNIX_TIMESTAMP")
    .and_then(|value| value.parse().ok())
    .filter(|timestamp| *timestamp > 0)
}

pub(crate) fn target_triple() -> &'static str {
  option_env!("TARGET").unwrap_or("unknown")
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

fn format_terminal_program_version(kernel: TerminalKernel) -> String {
  format!(
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
  fn terminal_program_version_includes_kernel_and_commit() {
    let version = terminal_program_version(TerminalKernel::Alacritty);

    assert_eq!(
      version,
      format!(
        "{} ({}, commit {})",
        app_version(),
        TerminalKernel::Alacritty,
        short_commit_hash()
      )
    );
    assert!(version.contains("alacritty"));
    assert!(!version.contains("ghostty"));
    assert!(!version.contains("vte"));
  }

  #[test]
  fn xtversion_response_prefixes_kazeterm_and_preserves_kernel_details() {
    let response = xtversion_response(TerminalKernel::Ghostty);

    assert_eq!(
      response,
      format!(
        "kazeterm {} ({}, commit {})",
        app_version(),
        TerminalKernel::Ghostty,
        short_commit_hash()
      )
    );
  }

  #[test]
  fn build_source_is_known_and_matches_local_helper() {
    assert!(matches!(build_source(), "local" | "ci"));
    assert_eq!(is_local_build(), build_source() == "local");
    assert!(!release_tag().is_empty());
  }
}
