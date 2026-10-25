//! Shared shell environment and prompt integration for terminal session factories.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::tty;

/// Prepared PTY options and the path used by shell prompts to report their CWD.
/// Preparation does not create a file, spawn a process, or open a PTY.
pub struct ShellLaunch {
  pub pty_options: tty::Options,
  pub cwd_file: PathBuf,
}

impl ShellLaunch {
  /// Prepare the common terminal environment and shell-specific prompt hooks.
  ///
  /// User environment overrides take precedence over terminal and backend
  /// defaults. Integration variables and supported shells' prompt hooks are
  /// installed afterwards, matching the session factories' existing behavior.
  pub fn new(
    program: String,
    args: Vec<String>,
    working_directory: Option<PathBuf>,
    env_overrides: &HashMap<String, String>,
    term_program_version: &str,
    backend_env: &[(&str, &str)],
  ) -> Self {
    let env = terminal_environment(
      term_program_version,
      backend_env,
      env_overrides,
      std::env::var("LANG").is_ok(),
    );
    let cwd_file = std::env::temp_dir().join(format!(
      "kazeterm-cwd-{}-{}",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
    ));

    prepare_shell(
      program,
      args,
      working_directory,
      env,
      cwd_file,
      &std::env::var("PROMPT_COMMAND").unwrap_or_default(),
    )
  }
}

fn terminal_environment(
  term_program_version: &str,
  backend_env: &[(&str, &str)],
  env_overrides: &HashMap<String, String>,
  has_inherited_lang: bool,
) -> HashMap<String, String> {
  let mut env = HashMap::new();
  if !has_inherited_lang {
    env.insert("LANG".to_string(), "en_US.UTF-8".to_string());
  }
  env.insert("TERM_PROGRAM".to_string(), "kazeterm".to_string());
  env.insert(
    "TERM_PROGRAM_VERSION".to_string(),
    term_program_version.to_string(),
  );
  env.insert("TERM".to_string(), "xterm-256color".to_string());
  env.insert("COLORTERM".to_string(), "truecolor".to_string());
  env.extend(
    backend_env
      .iter()
      .map(|(key, value)| (key.to_string(), value.to_string())),
  );
  for (key, value) in env_overrides {
    env.insert(key.clone(), value.clone());
  }
  env
}

fn prepare_shell(
  program: String,
  mut args: Vec<String>,
  working_directory: Option<PathBuf>,
  mut env: HashMap<String, String>,
  cwd_file: PathBuf,
  inherited_prompt_command: &str,
) -> ShellLaunch {
  env.insert(
    "KAZETERM_CWD_FILE".to_string(),
    cwd_file.to_string_lossy().to_string(),
  );

  let shell_name = Path::new(&program)
    .file_stem()
    .and_then(|s| s.to_str())
    .unwrap_or("")
    .to_lowercase();

  let cwd_file_str = cwd_file.to_string_lossy();
  let bash_hook = format!(
    r#"printf '\e]133;A\e\\\e]7;file://%s%s\e\\' "$(hostname)" "$PWD"; printf '%s' "$PWD" >| "{}""#,
    cwd_file_str,
  );
  env.insert("__KAZETERM_OSC7".to_string(), bash_hook.clone());

  if shell_name == "bash" || shell_name == "sh" {
    let prompt_cmd = if inherited_prompt_command.is_empty() {
      bash_hook
    } else {
      format!("{bash_hook};{inherited_prompt_command}")
    };
    env.insert("PROMPT_COMMAND".to_string(), prompt_cmd);
  } else if shell_name == "zsh" {
    env.insert("PROMPT_COMMAND".to_string(), bash_hook);
  }

  let pwsh_init = format!(
    concat!(
      r#"$__kazeterm_orig_prompt = $function:prompt; "#,
      r#"function prompt {{ "#,
      r#"$cwd = (Get-Location).Path; "#,
      r#"[System.IO.File]::WriteAllText('{}', $cwd); "#,
      r#"$esc = [char]27; "#,
      r#"$host_name = [System.Net.Dns]::GetHostName(); "#,
      r#"[Console]::Write("${{esc}}]133;A${{esc}}\"); "#,
      r#"[Console]::Write("${{esc}}]7;file://${{host_name}}${{cwd}}${{esc}}\"); "#,
      r#"if ($__kazeterm_orig_prompt) {{ & $__kazeterm_orig_prompt }} "#,
      r#"else {{ "PS $($executionContext.SessionState.Path.CurrentLocation)$('>' * ($nestedPromptLevel + 1)) " }} "#,
      r#"}}"#,
    ),
    cwd_file_str,
  );
  env.insert("KAZETERM_PWSH_OSC7_INIT".to_string(), pwsh_init.clone());

  if (shell_name == "pwsh" || shell_name == "powershell") && args.is_empty() {
    args = vec!["-NoExit".to_string(), "-Command".to_string(), pwsh_init];
  }

  ShellLaunch {
    pty_options: tty::Options {
      shell: Some(tty::Shell::new(program, args)),
      working_directory,
      drain_on_exit: true,
      env,
      #[cfg(windows)]
      escape_args: true,
    },
    cwd_file,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn environment_defaults_and_override_precedence() {
    for has_inherited_lang in [false, true] {
      let env = terminal_environment("test-version", &[], &HashMap::new(), has_inherited_lang);
      assert_eq!(
        env.get("LANG").map(String::as_str),
        (!has_inherited_lang).then_some("en_US.UTF-8")
      );
      assert_eq!(env["TERM_PROGRAM"], "kazeterm");
      assert_eq!(env["TERM_PROGRAM_VERSION"], "test-version");
      assert_eq!(env["TERM"], "xterm-256color");
      assert_eq!(env["COLORTERM"], "truecolor");
      assert!(!env.contains_key("VTE_VERSION"));

      let backend_env = &[("VTE_VERSION", "7600")];
      let env = terminal_environment(
        "test-version",
        backend_env,
        &HashMap::new(),
        has_inherited_lang,
      );
      assert_eq!(env["VTE_VERSION"], "7600");

      let overrides = HashMap::from([
        ("LANG".into(), "custom-locale".into()),
        ("TERM".into(), "custom-term".into()),
        ("TERM_PROGRAM".into(), "custom-program".into()),
        ("TERM_PROGRAM_VERSION".into(), "custom-version".into()),
        ("COLORTERM".into(), "custom-color".into()),
        ("VTE_VERSION".into(), "custom-vte".into()),
        ("CUSTOM".into(), "value".into()),
      ]);
      assert_eq!(
        terminal_environment("test-version", backend_env, &overrides, has_inherited_lang),
        overrides
      );
    }
  }

  fn shell_launch(program: &str, args: &[&str], prompt_command: &str) -> ShellLaunch {
    let env = HashMap::from([
      ("PROMPT_COMMAND".into(), "configured-prompt".into()),
      ("KAZETERM_CWD_FILE".into(), "configured-cwd".into()),
      ("__KAZETERM_OSC7".into(), "configured-hook".into()),
      ("KAZETERM_PWSH_OSC7_INIT".into(), "configured-init".into()),
    ]);
    prepare_shell(
      program.into(),
      args.iter().map(|arg| arg.to_string()).collect(),
      Some(PathBuf::from("workspace")),
      env,
      PathBuf::from("kazeterm-test-cwd"),
      prompt_command,
    )
  }

  #[test]
  fn posix_hooks_preserve_shell_specific_prompt_behavior() {
    for program in ["shells/bash", "shells/SH", "shells/zsh", "shells/fish"] {
      for inherited_prompt in ["", "existing-prompt"] {
        let launch = shell_launch(program, &["--login"], inherited_prompt);
        let options = launch.pty_options;
        assert_eq!(
          options.shell,
          Some(tty::Shell::new(program.into(), vec!["--login".into()]))
        );
        assert_eq!(options.working_directory, Some(PathBuf::from("workspace")));
        assert!(options.drain_on_exit);
        #[cfg(windows)]
        assert!(options.escape_args);
        assert_eq!(
          options.env["KAZETERM_CWD_FILE"],
          launch.cwd_file.to_string_lossy()
        );
        let hook = &options.env["__KAZETERM_OSC7"];
        assert_eq!(
          hook,
          r#"printf '\e]133;A\e\\\e]7;file://%s%s\e\\' "$(hostname)" "$PWD"; printf '%s' "$PWD" >| "kazeterm-test-cwd""#
        );
        let expected_prompt = match program {
          "shells/bash" | "shells/SH" if !inherited_prompt.is_empty() => {
            format!("{hook};{inherited_prompt}")
          }
          "shells/bash" | "shells/SH" | "shells/zsh" => hook.clone(),
          _ => "configured-prompt".into(),
        };
        assert_eq!(options.env["PROMPT_COMMAND"], expected_prompt);
      }
    }
  }

  #[test]
  fn powershell_injects_prompt_only_without_explicit_arguments() {
    for program in [
      "pwsh",
      "powershell",
      "shells/PWSH.EXE",
      "shells/PowerShell.exe",
    ] {
      for args in [vec![], vec!["-NoProfile", "-File", "script.ps1"]] {
        let launch = shell_launch(program, &args, "existing-prompt");
        let options = launch.pty_options;
        let init = &options.env["KAZETERM_PWSH_OSC7_INIT"];
        assert!(init.contains("[System.IO.File]::WriteAllText('kazeterm-test-cwd', $cwd)"));
        assert!(init.contains("$__kazeterm_orig_prompt = $function:prompt"));
        assert!(init.contains(r#"[Console]::Write("${esc}]133;A${esc}\")"#));
        assert!(init.contains(r#"[Console]::Write("${esc}]7;file://${host_name}${cwd}${esc}\")"#));
        assert_eq!(options.env["PROMPT_COMMAND"], "configured-prompt");
        let expected_args = if args.is_empty() {
          vec!["-NoExit".into(), "-Command".into(), init.clone()]
        } else {
          args.iter().map(|arg| arg.to_string()).collect()
        };
        assert_eq!(
          options.shell,
          Some(tty::Shell::new(program.into(), expected_args))
        );
      }
    }
  }

  #[test]
  fn launch_prepares_options_without_spawning_the_program() {
    let overrides = HashMap::from([("CUSTOM".into(), "value".into())]);
    let launch = ShellLaunch::new(
      "kazeterm-nonexistent-test-shell".into(),
      vec![],
      None,
      &overrides,
      "test-version",
      &[("VTE_VERSION", "7600")],
    );
    assert_eq!(
      launch.cwd_file.parent(),
      Some(std::env::temp_dir().as_path())
    );
    assert!(
      launch
        .cwd_file
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with(&format!("kazeterm-cwd-{}-", std::process::id()))
    );
    let options = launch.pty_options;
    assert_eq!(
      options.env["KAZETERM_CWD_FILE"],
      launch.cwd_file.to_string_lossy()
    );
    assert_eq!(options.env["TERM_PROGRAM_VERSION"], "test-version");
    assert_eq!(options.env["VTE_VERSION"], "7600");
    assert_eq!(options.env["CUSTOM"], "value");
    assert_eq!(
      options.env.contains_key("LANG"),
      std::env::var("LANG").is_err()
    );
    assert_eq!(options.working_directory, None);
    assert_eq!(
      options.shell,
      Some(tty::Shell::new(
        "kazeterm-nonexistent-test-shell".into(),
        vec![]
      ))
    );
  }
}
