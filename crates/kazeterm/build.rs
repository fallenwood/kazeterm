use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
  println!("cargo:rerun-if-changed=../../.git/logs/HEAD");
  println!("cargo:rerun-if-env-changed=KAZETERM_RELEASE_TAG");
  println!("cargo:rerun-if-env-changed=GITHUB_ACTIONS");
  println!("cargo:rerun-if-env-changed=CI");
  println!(
    "cargo:rustc-env=TARGET={}",
    std::env::var("TARGET").unwrap()
  );
  println!(
    "cargo:rustc-env=KAZETERM_BUILD_UNIX_TIMESTAMP={}",
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map(|duration| duration.as_secs())
      .unwrap_or_default()
  );

  let release_tag = std::env::var("KAZETERM_RELEASE_TAG")
    .ok()
    .map(|tag| tag.trim().to_string())
    .filter(|tag| !tag.is_empty());
  let is_ci_build = release_tag.is_some()
    || std::env::var("GITHUB_ACTIONS").is_ok_and(|value| value.eq_ignore_ascii_case("true"))
    || std::env::var("CI").is_ok_and(|value| value.eq_ignore_ascii_case("true"));
  let build_source = if is_ci_build { "ci" } else { "local" };
  println!("cargo:rustc-env=KAZETERM_BUILD_SOURCE={build_source}");

  if is_ci_build {
    println!(
      "cargo:rustc-env=KAZETERM_RELEASE_TAG={}",
      release_tag.as_deref().unwrap_or("wip")
    );
  }

  #[cfg(target_os = "linux")]
  println!("cargo:rustc-link-arg-bin=kazeterm=-Wl,-rpath,$ORIGIN");

  #[cfg(target_os = "macos")]
  println!("cargo:rustc-link-arg-bin=kazeterm=-Wl,-rpath,@executable_path");

  #[cfg(target_os = "macos")]
  println!("cargo:rustc-link-arg-bin=kazeterm=-Wl,-rpath,@executable_path/../Frameworks");

  if let Ok(output) = Command::new("git").args(["rev-parse", "HEAD"]).output()
    && output.status.success()
  {
    let git_sha = String::from_utf8_lossy(&output.stdout);
    let git_sha = git_sha.trim();

    println!("cargo:rustc-env=KAZETERM_COMMIT_SHA={git_sha}");

    if let Ok(build_profile) = std::env::var("PROFILE")
      && build_profile == "release"
    {
      println!("cargo:warning=Info: using '{git_sha}' hash for KAZETERM_COMMIT_SHA env var");
    }
  }

  #[cfg(target_os = "windows")]
  {
    #[cfg(target_env = "msvc")]
    {
      println!("cargo:rustc-link-arg=/stack:{}", 8 * 1024 * 1024);
    }

    let icon = "../../assets/icons/kazeterm.ico";
    let icon = std::path::Path::new(icon);

    let mut res = winresource::WindowsResource::new();

    if let Ok(explicit_rc_toolkit_path) = std::env::var("KAZETERM_TOOLKIT_PATH") {
      res.set_toolkit_path(explicit_rc_toolkit_path.as_str());
    }
    res.set_icon(icon.to_str().unwrap());
    res.set("FileDescription", "Kazeterm");
    res.set("ProductName", "Kazeterm");

    if let Err(e) = res.compile() {
      eprintln!("{}", e);
      std::process::exit(1);
    }
  }
}
