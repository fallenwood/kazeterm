use std::process::Command;

fn main() {
  println!("cargo:rerun-if-changed=../../.git/logs/HEAD");
  println!(
    "cargo:rustc-env=TARGET={}",
    std::env::var("TARGET").unwrap()
  );
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

    if let Some(explicit_rc_toolkit_path) = std::env::var("KAZETERM_TOOLKIT_PATH").ok() {
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
