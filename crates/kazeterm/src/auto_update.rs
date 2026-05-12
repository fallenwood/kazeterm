use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as AnyhowContext, anyhow, bail};
use gpui::{AnyWindowHandle, App, AppContext, AsyncApp, WeakEntity};
use semver::Version;
use serde::Deserialize;

use crate::components::MainWindow;

const GITHUB_OWNER: &str = "fallenwood";
const GITHUB_REPO: &str = "kazeterm";
const ONE_DAY_SECS: i64 = 24 * 60 * 60;

static AUTO_UPDATE_STARTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug)]
enum UpdateTrigger {
  Automatic,
  Manual,
}

impl UpdateTrigger {
  fn respects_schedule(self) -> bool {
    matches!(self, Self::Automatic)
  }

  fn no_update_message(self) -> &'static str {
    match self {
      Self::Automatic => "Kazeterm is already up to date",
      Self::Manual => "Manual update check: already up to date",
    }
  }

  fn check_failure_message(self) -> &'static str {
    match self {
      Self::Automatic => "Auto update check failed",
      Self::Manual => "Manual update check failed",
    }
  }

  fn apply_failure_message(self) -> &'static str {
    match self {
      Self::Automatic => "Failed to apply auto update",
      Self::Manual => "Failed to apply manual update",
    }
  }
}

pub(crate) fn start_auto_update(
  main_window: WeakEntity<MainWindow>,
  window_handle: AnyWindowHandle,
  cx: &mut App,
) {
  if crate::build_info::is_local_build() {
    tracing::debug!("Skipping auto update for local build");
    return;
  }

  let auto_update = cx.global::<::config::Config>().auto_update.clone();
  if matches!(auto_update.check, ::config::AutoUpdatePolicy::Never) {
    return;
  }

  if AUTO_UPDATE_STARTED.swap(true, Ordering::AcqRel) {
    return;
  }

  run_update_check(
    UpdateTrigger::Automatic,
    auto_update,
    main_window,
    window_handle,
    cx,
  );
}

/// Prepare a manual update check result, bypassing the time guard and local build check.
pub(crate) fn prepare_manual_update(
  auto_update: ::config::AutoUpdateConfig,
) -> anyhow::Result<Option<PreparedUpdate>> {
  check_and_prepare_update(auto_update, CurrentBuild::current(), UpdateTrigger::Manual)
}

fn run_update_check(
  trigger: UpdateTrigger,
  auto_update: ::config::AutoUpdateConfig,
  main_window: WeakEntity<MainWindow>,
  window_handle: AnyWindowHandle,
  cx: &mut App,
) {
  let current = CurrentBuild::current();
  cx.spawn(async move |cx: &mut AsyncApp| {
    let result =
      smol::unblock(move || check_and_prepare_update(auto_update, current, trigger)).await;

    match result {
      Ok(Some(prepared_update)) => {
        if let Err(error) =
          apply_prepared_update(main_window, window_handle, prepared_update, cx).await
        {
          tracing::error!("{}: {error:#}", trigger.apply_failure_message());
        }
      }
      Ok(None) => {}
      Err(error) => {
        tracing::error!("{}: {error:#}", trigger.check_failure_message());
      }
    }
  })
  .detach();
}

pub(crate) async fn apply_prepared_update(
  main_window: WeakEntity<MainWindow>,
  window_handle: AnyWindowHandle,
  prepared_update: PreparedUpdate,
  cx: &mut AsyncApp,
) -> anyhow::Result<()> {
  let main_window = main_window
    .upgrade()
    .ok_or_else(|| anyhow!("main window was closed before the update could be applied"))?;

  cx.update_window(window_handle, |_root_view, _window, cx| {
    main_window.update(cx, |main_window, cx| {
      main_window.sync_ui_tree(cx);
      main_window.ui_tree.save_workspace();
    });
  })?;

  request_restore_workspace_once()?;
  launch_update_helper(&prepared_update)?;
  cx.update(|cx| cx.quit())?;
  Ok(())
}

fn check_and_prepare_update(
  auto_update: ::config::AutoUpdateConfig,
  current: CurrentBuild,
  trigger: UpdateTrigger,
) -> anyhow::Result<Option<PreparedUpdate>> {
  let now = now_unix_timestamp();
  if trigger.respects_schedule()
    && !should_check(auto_update.check, auto_update.last_check_unix_secs, now)
  {
    return Ok(None);
  }

  let http_client = HttpClient::new(&auto_update, &current);
  let releases = fetch_releases(&http_client)?;
  if trigger.respects_schedule()
    && let Err(error) = ::config::Config::set_auto_update_last_check_unix_secs(now)
  {
    tracing::warn!("Failed to save auto update check timestamp: {error}");
  }

  prepare_update_from_releases(&http_client, &current, &releases, trigger)
}

fn prepare_update_from_releases(
  http_client: &HttpClient,
  current: &CurrentBuild,
  releases: &[GitHubRelease],
  trigger: UpdateTrigger,
) -> anyhow::Result<Option<PreparedUpdate>> {
  let Some(release) = select_update_release(current, releases) else {
    tracing::info!("{}", trigger.no_update_message());
    return Ok(None);
  };

  let Some(asset) = select_asset_for_target(&release, &current.target_triple) else {
    bail!(
      "release '{}' has no asset for target '{}'",
      release.tag_name,
      current.target_triple
    );
  };

  tracing::info!(
    "Preparing Kazeterm update from release '{}' using asset '{}'",
    release.tag_name,
    asset.name
  );

  prepare_update_package(http_client, &release, &asset)
}

#[derive(Debug, Clone)]
struct HttpClient {
  proxy: Option<String>,
  user_agent: String,
}

impl HttpClient {
  fn new(auto_update: &::config::AutoUpdateConfig, current: &CurrentBuild) -> Self {
    Self {
      proxy: auto_update
        .proxy
        .as_deref()
        .map(str::trim)
        .filter(|proxy| !proxy.is_empty())
        .map(ToOwned::to_owned),
      user_agent: format!("kazeterm/{} ({})", current.version, current.target_triple),
    }
  }

  fn get_bytes(&self, url: &str, headers: &[(&str, &str)]) -> anyhow::Result<Vec<u8>> {
    let mut command = self.curl_command();
    for (name, value) in headers {
      command.arg("--header").arg(format!("{name}: {value}"));
    }
    command.arg(url);

    let output = command
      .output()
      .with_context(|| "failed to start curl for GitHub release request")?;
    if !output.status.success() {
      bail!("curl failed for {url}: {}", command_stderr(&output.stderr));
    }
    Ok(output.stdout)
  }

  fn download_to_path(&self, url: &str, destination: &Path) -> anyhow::Result<()> {
    let mut command = self.curl_command();
    command.arg("--output").arg(destination).arg(url);
    run_command(&mut command, &format!("download {url}"))
  }

  fn curl_command(&self) -> Command {
    let mut command = Command::new("curl");
    command
      .arg("--fail")
      .arg("--location")
      .arg("--silent")
      .arg("--show-error")
      .arg("--connect-timeout")
      .arg("20")
      .arg("--max-time")
      .arg("600")
      .arg("--user-agent")
      .arg(&self.user_agent);
    if let Some(proxy) = &self.proxy {
      command.args(["--proxy", proxy]);
    }
    command
  }
}

fn fetch_releases(http_client: &HttpClient) -> anyhow::Result<Vec<GitHubRelease>> {
  let endpoint =
    format!("https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases?per_page=50");
  let response = http_client.get_bytes(
    &endpoint,
    &[
      ("Accept", "application/vnd.github+json"),
      ("X-GitHub-Api-Version", "2022-11-28"),
    ],
  )?;

  serde_json::from_slice(&response).context("failed to parse GitHub releases")
}

fn download_asset(
  http_client: &HttpClient,
  asset: &GitHubAsset,
  destination: &Path,
) -> anyhow::Result<()> {
  http_client.download_to_path(&asset.browser_download_url, destination)
}

fn prepare_update_package(
  http_client: &HttpClient,
  release: &GitHubRelease,
  asset: &GitHubAsset,
) -> anyhow::Result<Option<PreparedUpdate>> {
  let current_exe = std::env::current_exe().context("failed to determine current executable")?;
  let current_binary_name = current_exe
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or_else(|| anyhow!("current executable path has no file name"))?
    .to_string();

  let package_format = PackageFormat::from_asset_name(&asset.name)
    .ok_or_else(|| anyhow!("unsupported update asset format: {}", asset.name))?;
  let temp_dir = create_update_temp_dir(&release.tag_name)?;
  let download_path = temp_dir.join(sanitize_file_name(&asset.name));
  let extracted_root = temp_dir.join("package");
  fs::create_dir_all(&extracted_root)?;

  download_asset(http_client, asset, &download_path)?;

  match package_format {
    PackageFormat::DirectBinary => {
      fs::copy(&download_path, extracted_root.join(&current_binary_name)).with_context(|| {
        format!(
          "failed to stage direct update asset {}",
          download_path.display()
        )
      })?;
    }
    PackageFormat::Zip => extract_zip(&download_path, &extracted_root)?,
    PackageFormat::TarGz => extract_tar_gz(&download_path, &extracted_root)?,
    PackageFormat::TarZstd => extract_tar_zstd(&download_path, &extracted_root)?,
    PackageFormat::SevenZip => extract_7z(&download_path, &extracted_root)?,
  }

  let replacement_binary = find_file_named(&extracted_root, &current_binary_name)?
    .ok_or_else(|| anyhow!("update package does not contain '{}'", current_binary_name))?;
  let package_dir = replacement_binary
    .parent()
    .ok_or_else(|| anyhow!("replacement binary has no parent directory"))?
    .to_path_buf();

  Ok(Some(PreparedUpdate {
    release_tag: release.tag_name.clone(),
    temp_dir,
    package_dir,
    current_exe,
  }))
}

fn launch_update_helper(prepared_update: &PreparedUpdate) -> anyhow::Result<()> {
  let install_dir = prepared_update
    .current_exe
    .parent()
    .ok_or_else(|| anyhow!("current executable path has no parent directory"))?;
  let backup_path = unique_backup_path(&prepared_update.current_exe)?;
  let current_pid = std::process::id();

  #[cfg(target_os = "windows")]
  {
    let script_path = prepared_update.temp_dir.join("apply-update.ps1");
    fs::write(
      &script_path,
      windows_update_script(
        current_pid,
        &prepared_update.current_exe,
        &backup_path,
        &prepared_update.package_dir,
        install_dir,
      ),
    )?;
    Command::new("powershell.exe")
      .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
      .arg(&script_path)
      .spawn()
      .with_context(|| format!("failed to launch {}", script_path.display()))?;
  }

  #[cfg(not(target_os = "windows"))]
  {
    let script_path = prepared_update.temp_dir.join("apply-update.sh");
    fs::write(
      &script_path,
      unix_update_script(
        current_pid,
        &prepared_update.current_exe,
        &backup_path,
        &prepared_update.package_dir,
        install_dir,
      ),
    )?;
    Command::new("sh")
      .arg(&script_path)
      .spawn()
      .with_context(|| format!("failed to launch {}", script_path.display()))?;
  }

  tracing::info!(
    "Kazeterm update '{}' staged; current binary will be backed up to {}",
    prepared_update.release_tag,
    backup_path.display()
  );
  Ok(())
}

#[derive(Debug, Clone)]
struct CurrentBuild {
  release_tag: String,
  version: String,
  commit_hash: String,
  build_unix_timestamp: Option<i64>,
  target_triple: String,
}

impl CurrentBuild {
  fn current() -> Self {
    Self {
      release_tag: crate::build_info::release_tag().to_string(),
      version: crate::build_info::app_version().to_string(),
      commit_hash: crate::build_info::commit_hash().to_string(),
      build_unix_timestamp: crate::build_info::build_unix_timestamp(),
      target_triple: crate::build_info::target_triple().to_string(),
    }
  }

  fn is_wip(&self) -> bool {
    is_wip_tag(&self.release_tag)
  }
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubRelease {
  tag_name: String,
  #[serde(default)]
  target_commitish: String,
  #[serde(default)]
  draft: bool,
  published_at: Option<String>,
  #[serde(default)]
  assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubAsset {
  name: String,
  browser_download_url: String,
  updated_at: Option<String>,
  created_at: Option<String>,
}

#[derive(Debug)]
pub(crate) struct PreparedUpdate {
  release_tag: String,
  temp_dir: PathBuf,
  package_dir: PathBuf,
  current_exe: PathBuf,
}

pub(crate) fn request_restore_workspace_once() -> anyhow::Result<()> {
  ::config::Config::set_auto_update_restore_workspace_once(true)
    .map_err(|error| anyhow!("failed to save one-shot workspace restore setting: {error}"))
}

pub(crate) fn take_restore_workspace_once() -> bool {
  match ::config::Config::take_auto_update_restore_workspace_once() {
    Ok(value) => value,
    Err(error) => {
      tracing::warn!("Failed to read one-shot workspace restore setting: {error}");
      false
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackageFormat {
  DirectBinary,
  Zip,
  TarGz,
  TarZstd,
  SevenZip,
}

impl PackageFormat {
  fn from_asset_name(name: &str) -> Option<Self> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".zip") {
      Some(Self::Zip)
    } else if lower.ends_with(".tar.zstd") || lower.ends_with(".tar.zst") {
      Some(Self::TarZstd)
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
      Some(Self::TarGz)
    } else if lower.ends_with(".7z") {
      Some(Self::SevenZip)
    } else if lower.ends_with(".exe") || lower == "kazeterm" {
      Some(Self::DirectBinary)
    } else {
      None
    }
  }
}

fn should_check(
  policy: ::config::AutoUpdatePolicy,
  last_check_unix_secs: Option<i64>,
  now_unix_secs: i64,
) -> bool {
  match policy {
    ::config::AutoUpdatePolicy::Always => true,
    ::config::AutoUpdatePolicy::Never => false,
    ::config::AutoUpdatePolicy::OnceADay => last_check_unix_secs
      .map(|last| now_unix_secs.saturating_sub(last) >= ONE_DAY_SECS)
      .unwrap_or(true),
  }
}

fn select_update_release(
  current: &CurrentBuild,
  releases: &[GitHubRelease],
) -> Option<GitHubRelease> {
  if current.is_wip() {
    let release = releases
      .iter()
      .find(|release| !release.draft && is_wip_tag(&release.tag_name))?;
    return release_is_newer_than_current_wip(release, current).then(|| release.clone());
  }

  let current_version =
    pinned_version(&current.release_tag).or_else(|| pinned_version(&current.version))?;
  let (latest_version, latest_release) = releases
    .iter()
    .filter(|release| !release.draft && !is_wip_tag(&release.tag_name))
    .filter_map(|release| pinned_version(&release.tag_name).map(|version| (version, release)))
    .max_by(|(left, _), (right, _)| left.cmp(right))?;

  (latest_version > current_version).then(|| latest_release.clone())
}

fn release_is_newer_than_current_wip(release: &GitHubRelease, current: &CurrentBuild) -> bool {
  if remote_commit_differs(&release.target_commitish, &current.commit_hash) {
    return true;
  }

  match (
    release_latest_timestamp(release),
    current.build_unix_timestamp,
  ) {
    (Some(remote), Some(current)) => remote > current,
    _ => false,
  }
}

fn remote_commit_differs(remote: &str, current: &str) -> bool {
  if current == "unknown" || !looks_like_commit(remote) {
    return false;
  }

  let remote = remote.to_ascii_lowercase();
  let current = current.to_ascii_lowercase();
  !(current.starts_with(&remote) || remote.starts_with(&current))
}

fn looks_like_commit(value: &str) -> bool {
  let len = value.len();
  (7..=40).contains(&len) && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn release_latest_timestamp(release: &GitHubRelease) -> Option<i64> {
  let release_time = release
    .published_at
    .as_deref()
    .and_then(parse_github_timestamp);
  release
    .assets
    .iter()
    .flat_map(|asset| [asset.updated_at.as_deref(), asset.created_at.as_deref()])
    .flatten()
    .filter_map(parse_github_timestamp)
    .chain(release_time)
    .max()
}

fn parse_github_timestamp(value: &str) -> Option<i64> {
  if value.len() < 20 {
    return None;
  }

  let bytes = value.as_bytes();
  if bytes.get(4) != Some(&b'-')
    || bytes.get(7) != Some(&b'-')
    || bytes.get(10) != Some(&b'T')
    || bytes.get(13) != Some(&b':')
    || bytes.get(16) != Some(&b':')
  {
    return None;
  }

  let year = parse_fixed_i32(value, 0, 4)?;
  let month = parse_fixed_u32(value, 5, 7)?;
  let day = parse_fixed_u32(value, 8, 10)?;
  let hour = parse_fixed_u32(value, 11, 13)?;
  let minute = parse_fixed_u32(value, 14, 16)?;
  let second = parse_fixed_u32(value, 17, 19)?;
  let timezone_start = value[19..]
    .find(|ch| matches!(ch, 'Z' | '+' | '-'))
    .map(|offset| 19 + offset)?;
  let timezone_offset = parse_timezone_offset(&value[timezone_start..])?;

  if !(1..=12).contains(&month)
    || !(1..=31).contains(&day)
    || hour > 23
    || minute > 59
    || second > 60
  {
    return None;
  }

  Some(
    days_from_civil(year, month, day) * 86_400
      + i64::from(hour) * 3_600
      + i64::from(minute) * 60
      + i64::from(second)
      - timezone_offset,
  )
}

fn pinned_version(tag: &str) -> Option<Version> {
  tag
    .trim()
    .trim_start_matches('v')
    .trim_start_matches('V')
    .parse()
    .ok()
}

fn is_wip_tag(tag: &str) -> bool {
  tag.trim().eq_ignore_ascii_case("wip")
}

fn select_asset_for_target(release: &GitHubRelease, target_triple: &str) -> Option<GitHubAsset> {
  let platform_key = platform_asset_key_from_target(target_triple)
    .unwrap_or_else(|| platform_asset_key_from_consts());

  release
    .assets
    .iter()
    .filter_map(|asset| asset_score(&asset.name, &platform_key).map(|score| (score, asset)))
    .max_by_key(|(score, _)| *score)
    .map(|(_, asset)| asset.clone())
}

fn asset_score(name: &str, platform_key: &str) -> Option<i32> {
  let lower = name.to_ascii_lowercase();
  if !lower.contains(platform_key) {
    return None;
  }

  let format = PackageFormat::from_asset_name(&lower)?;
  let mut score = match format {
    PackageFormat::DirectBinary => 100,
    PackageFormat::Zip => 90,
    PackageFormat::TarZstd => 80,
    PackageFormat::TarGz => 70,
    PackageFormat::SevenZip => 60,
  };

  if lower.contains("dmg") {
    score -= 40;
  }

  Some(score)
}

fn platform_asset_key_from_target(target_triple: &str) -> Option<String> {
  let lower = target_triple.to_ascii_lowercase();
  let os = if lower.contains("windows") {
    "windows"
  } else if lower.contains("linux") {
    "linux"
  } else if lower.contains("darwin") || lower.contains("apple") {
    "macos"
  } else {
    return None;
  };

  let arch = if lower.starts_with("x86_64") || lower.starts_with("amd64") {
    "x64"
  } else if lower.starts_with("aarch64") || lower.starts_with("arm64") {
    "arm64"
  } else {
    return None;
  };

  Some(format!("{os}-{arch}"))
}

fn platform_asset_key_from_consts() -> String {
  let os = match std::env::consts::OS {
    "windows" => "windows",
    "linux" => "linux",
    "macos" => "macos",
    other => other,
  };
  let arch = match std::env::consts::ARCH {
    "x86_64" => "x64",
    "aarch64" => "arm64",
    other => other,
  };
  format!("{os}-{arch}")
}

fn create_update_temp_dir(release_tag: &str) -> anyhow::Result<PathBuf> {
  let dir = std::env::temp_dir().join(format!(
    "kazeterm-update-{}-{}-{}",
    sanitize_file_name(release_tag),
    std::process::id(),
    now_unix_timestamp()
  ));
  fs::create_dir_all(&dir)?;
  Ok(dir)
}

fn sanitize_file_name(name: &str) -> String {
  let sanitized = name
    .chars()
    .map(|ch| {
      if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
        ch
      } else {
        '_'
      }
    })
    .collect::<String>();

  if sanitized.is_empty() {
    "package".to_string()
  } else {
    sanitized
  }
}

fn extract_zip(archive_path: &Path, destination: &Path) -> anyhow::Result<()> {
  #[cfg(target_os = "windows")]
  {
    let mut command = Command::new("powershell.exe");
    command
      .args([
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        "Expand-Archive -LiteralPath $args[0] -DestinationPath $args[1] -Force",
      ])
      .arg(archive_path)
      .arg(destination);
    run_command(&mut command, "extract zip update package")
  }

  #[cfg(not(target_os = "windows"))]
  {
    let mut command = Command::new("unzip");
    command
      .arg("-q")
      .arg(archive_path)
      .arg("-d")
      .arg(destination);
    run_command(&mut command, "extract zip update package")
  }
}

fn extract_tar_gz(archive_path: &Path, destination: &Path) -> anyhow::Result<()> {
  let mut command = Command::new("tar");
  command
    .arg("-xzf")
    .arg(archive_path)
    .arg("-C")
    .arg(destination);
  run_command(&mut command, "extract tar.gz update package")
}

fn extract_tar_zstd(archive_path: &Path, destination: &Path) -> anyhow::Result<()> {
  let mut command = Command::new("tar");
  command
    .arg("--zstd")
    .arg("-xf")
    .arg(archive_path)
    .arg("-C")
    .arg(destination);
  run_command(&mut command, "extract tar.zstd update package")
}

fn extract_7z(archive_path: &Path, destination: &Path) -> anyhow::Result<()> {
  let output_arg = format!("-o{}", destination.display());
  let mut last_error = None;

  for command in ["7z", "7za"] {
    let mut command = Command::new(command);
    command
      .arg("x")
      .arg("-y")
      .arg(&output_arg)
      .arg(archive_path);
    match run_command(&mut command, "extract .7z update package") {
      Ok(()) => return Ok(()),
      Err(error) => last_error = Some(error.to_string()),
    }
  }

  bail!(
    "failed to extract .7z update package {}; {}",
    archive_path.display(),
    last_error.unwrap_or_else(|| "7z is not available".to_string())
  )
}

fn run_command(command: &mut Command, description: &str) -> anyhow::Result<()> {
  let output = command
    .output()
    .with_context(|| format!("failed to start command to {description}"))?;
  if output.status.success() {
    return Ok(());
  }

  bail!("{description} failed: {}", command_stderr(&output.stderr))
}

fn command_stderr(stderr: &[u8]) -> String {
  let stderr = String::from_utf8_lossy(stderr).trim().to_string();
  if stderr.is_empty() {
    "process exited with a non-zero status".to_string()
  } else {
    stderr
  }
}

fn find_file_named(root: &Path, file_name: &str) -> io::Result<Option<PathBuf>> {
  let mut stack = vec![root.to_path_buf()];
  while let Some(path) = stack.pop() {
    for entry in fs::read_dir(path)? {
      let entry = entry?;
      let path = entry.path();
      let file_type = entry.file_type()?;
      if file_type.is_dir() {
        stack.push(path);
      } else if file_type.is_file()
        && path.file_name().and_then(|name| name.to_str()) == Some(file_name)
      {
        return Ok(Some(path));
      }
    }
  }
  Ok(None)
}

fn unique_backup_path(current_exe: &Path) -> anyhow::Result<PathBuf> {
  let file_name = current_exe
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or_else(|| anyhow!("current executable path has no file name"))?;
  let timestamp = timestamp_for_file_name();
  let parent = current_exe
    .parent()
    .ok_or_else(|| anyhow!("current executable path has no parent directory"))?;

  for collision_index in 0_u32.. {
    let backup_name = if collision_index == 0 {
      format!("{file_name}.{timestamp}.bak")
    } else {
      format!("{file_name}.{timestamp}.{collision_index}.bak")
    };
    let path = parent.join(backup_name);
    if !path.exists() {
      return Ok(path);
    }
  }

  unreachable!("unbounded backup collision search should always find a free path")
}

fn timestamp_for_file_name() -> String {
  let (year, month, day, hour, minute, second) = unix_timestamp_to_utc(now_unix_timestamp());
  format!(
    "{:04}{:02}{:02}-{:02}{:02}{:02}",
    year, month, day, hour, minute, second
  )
}

fn now_unix_timestamp() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|duration| duration.as_secs() as i64)
    .unwrap_or_default()
}

fn parse_fixed_i32(value: &str, start: usize, end: usize) -> Option<i32> {
  value.get(start..end)?.parse().ok()
}

fn parse_fixed_u32(value: &str, start: usize, end: usize) -> Option<u32> {
  value.get(start..end)?.parse().ok()
}

fn parse_timezone_offset(value: &str) -> Option<i64> {
  if value == "Z" {
    return Some(0);
  }

  let sign = match value.as_bytes().first()? {
    b'+' => 1,
    b'-' => -1,
    _ => return None,
  };
  if value.as_bytes().get(3) != Some(&b':') {
    return None;
  }

  let hour = parse_fixed_u32(value, 1, 3)?;
  let minute = parse_fixed_u32(value, 4, 6)?;
  if hour > 23 || minute > 59 {
    return None;
  }

  Some(i64::from(sign) * (i64::from(hour) * 3_600 + i64::from(minute) * 60))
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
  let year = year - i32::from(month <= 2);
  let era = if year >= 0 { year } else { year - 399 } / 400;
  let year_of_era = year - era * 400;
  let month = month as i32;
  let day = day as i32;
  let month_prime = month + if month > 2 { -3 } else { 9 };
  let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
  let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;

  i64::from(era) * 146_097 + i64::from(day_of_era) - 719_468
}

fn unix_timestamp_to_utc(timestamp: i64) -> (i32, u32, u32, u32, u32, u32) {
  let days = timestamp.div_euclid(86_400);
  let seconds_of_day = timestamp.rem_euclid(86_400);
  let (year, month, day) = civil_from_days(days);
  let hour = (seconds_of_day / 3_600) as u32;
  let minute = ((seconds_of_day % 3_600) / 60) as u32;
  let second = (seconds_of_day % 60) as u32;

  (year, month, day, hour, minute, second)
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
  let days = days + 719_468;
  let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
  let day_of_era = days - era * 146_097;
  let year_of_era =
    (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
  let mut year = year_of_era + era * 400;
  let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
  let month_prime = (5 * day_of_year + 2) / 153;
  let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
  let month = month_prime + if month_prime < 10 { 3 } else { -9 };
  year += i64::from(month <= 2);

  (year as i32, month as u32, day as u32)
}

#[cfg(target_os = "windows")]
fn windows_update_script(
  pid: u32,
  current: &Path,
  backup: &Path,
  package: &Path,
  install_dir: &Path,
) -> String {
  format!(
    r#"$ErrorActionPreference = 'Stop'
$pidToWait = {pid}
$current = {current}
$backup = {backup}
$package = {package}
$installDir = {install_dir}
Wait-Process -Id $pidToWait -ErrorAction SilentlyContinue
Rename-Item -LiteralPath $current -NewName (Split-Path -Leaf $backup)
Get-ChildItem -LiteralPath $package -Force | ForEach-Object {{
  $destination = Join-Path $installDir $_.Name
  if (Test-Path -LiteralPath $destination) {{
    Remove-Item -LiteralPath $destination -Recurse -Force
  }}
  Move-Item -LiteralPath $_.FullName -Destination $destination -Force
}}
Start-Process -FilePath $current -WorkingDirectory $installDir
"#,
    current = powershell_quote(current),
    backup = powershell_quote(backup),
    package = powershell_quote(package),
    install_dir = powershell_quote(install_dir),
  )
}

#[cfg(target_os = "windows")]
fn powershell_quote(path: &Path) -> String {
  format!("'{}'", path.display().to_string().replace('\'', "''"))
}

#[cfg(not(target_os = "windows"))]
fn unix_update_script(
  pid: u32,
  current: &Path,
  backup: &Path,
  package: &Path,
  install_dir: &Path,
) -> String {
  format!(
    r#"#!/bin/sh
set -eu
pid={pid}
current={current}
backup={backup}
package={package}
install_dir={install_dir}
while kill -0 "$pid" 2>/dev/null; do
  sleep 0.2
done
mv "$current" "$backup"
for item in "$package"/* "$package"/.[!.]* "$package"/..?*; do
  [ -e "$item" ] || continue
  destination="$install_dir/$(basename "$item")"
  rm -rf "$destination"
  mv "$item" "$install_dir/"
done
chmod +x "$current" 2>/dev/null || true
"$current" >/dev/null 2>&1 &
"#,
    current = shell_quote(current),
    backup = shell_quote(backup),
    package = shell_quote(package),
    install_dir = shell_quote(install_dir),
  )
}

#[cfg(not(target_os = "windows"))]
fn shell_quote(path: &Path) -> String {
  format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn current(tag: &str) -> CurrentBuild {
    CurrentBuild {
      release_tag: tag.to_string(),
      version: "0.1.0".to_string(),
      commit_hash: "1111111111111111111111111111111111111111".to_string(),
      build_unix_timestamp: Some(1_700_000_000),
      target_triple: "x86_64-pc-windows-msvc".to_string(),
    }
  }

  fn release(tag: &str, target_commitish: &str, published_at: &str) -> GitHubRelease {
    GitHubRelease {
      tag_name: tag.to_string(),
      target_commitish: target_commitish.to_string(),
      draft: false,
      published_at: Some(published_at.to_string()),
      assets: vec![],
    }
  }

  fn asset(name: &str) -> GitHubAsset {
    GitHubAsset {
      name: name.to_string(),
      browser_download_url: format!("https://example.invalid/{name}"),
      updated_at: None,
      created_at: None,
    }
  }

  #[test]
  fn check_policy_respects_frequency() {
    assert!(should_check(
      ::config::AutoUpdatePolicy::Always,
      Some(10),
      11
    ));
    assert!(!should_check(::config::AutoUpdatePolicy::Never, None, 11));
    assert!(should_check(::config::AutoUpdatePolicy::OnceADay, None, 11));
    assert!(!should_check(
      ::config::AutoUpdatePolicy::OnceADay,
      Some(1_000),
      1_000 + ONE_DAY_SECS - 1
    ));
    assert!(should_check(
      ::config::AutoUpdatePolicy::OnceADay,
      Some(1_000),
      1_000 + ONE_DAY_SECS
    ));
  }

  #[test]
  fn auto_update_config_state_defaults_and_roundtrips() {
    let legacy: ::config::AutoUpdateConfig = toml::from_str(
      r#"
check = "never"
last_check_unix_secs = 42
"#,
    )
    .unwrap();
    assert_eq!(legacy.last_check_unix_secs, Some(42));
    assert!(!legacy.restore_workspace_once);

    let state = ::config::AutoUpdateConfig {
      check: ::config::AutoUpdatePolicy::Never,
      proxy: None,
      last_check_unix_secs: Some(42),
      restore_workspace_once: true,
    };
    let toml = toml::to_string(&state).unwrap();
    let parsed: ::config::AutoUpdateConfig = toml::from_str(&toml).unwrap();

    assert_eq!(parsed.last_check_unix_secs, Some(42));
    assert!(parsed.restore_workspace_once);
  }

  #[test]
  fn pinned_build_ignores_wip_and_uses_latest_pinned_tag() {
    let releases = vec![
      release("wip", "2222222", "2026-05-12T00:00:00Z"),
      release("0.2.0", "3333333", "2026-05-10T00:00:00Z"),
      release("0.1.5", "4444444", "2026-05-01T00:00:00Z"),
    ];

    let selected = select_update_release(&current("0.1.0"), &releases).unwrap();

    assert_eq!(selected.tag_name, "0.2.0");
  }

  #[test]
  fn pinned_build_does_not_update_when_latest_pinned_is_current() {
    let releases = vec![
      release("wip", "2222222", "2026-05-12T00:00:00Z"),
      release("0.1.0", "3333333", "2026-05-10T00:00:00Z"),
    ];

    assert!(select_update_release(&current("0.1.0"), &releases).is_none());
  }

  #[test]
  fn wip_build_updates_when_remote_commit_differs() {
    let releases = vec![release(
      "wip",
      "2222222222222222222222222222222222222222",
      "2026-05-12T00:00:00Z",
    )];

    let selected = select_update_release(&current("wip"), &releases).unwrap();

    assert_eq!(selected.tag_name, "wip");
  }

  #[test]
  fn wip_build_updates_when_remote_release_is_newer_than_build() {
    let mut current = current("wip");
    current.commit_hash = "unknown".to_string();
    let releases = vec![release("wip", "master", "2026-05-12T00:00:00Z")];

    let selected = select_update_release(&current, &releases).unwrap();

    assert_eq!(selected.tag_name, "wip");
  }

  #[test]
  fn asset_selection_prefers_current_platform_package_over_dmg() {
    let mut release = release("0.2.0", "3333333", "2026-05-10T00:00:00Z");
    release.assets = vec![
      asset("kazeterm-macos-arm64-0.2.0-dmg.tar.gz"),
      asset("kazeterm-windows-x64-0.2.0.7z"),
      asset("kazeterm-windows-x64-0.2.0.zip"),
    ];

    let selected = select_asset_for_target(&release, "x86_64-pc-windows-msvc").unwrap();

    assert_eq!(selected.name, "kazeterm-windows-x64-0.2.0.zip");
  }

  #[test]
  fn platform_asset_key_maps_known_targets() {
    assert_eq!(
      platform_asset_key_from_target("x86_64-pc-windows-msvc").as_deref(),
      Some("windows-x64")
    );
    assert_eq!(
      platform_asset_key_from_target("aarch64-unknown-linux-gnu").as_deref(),
      Some("linux-arm64")
    );
    assert_eq!(
      platform_asset_key_from_target("x86_64-apple-darwin").as_deref(),
      Some("macos-x64")
    );
  }

  #[test]
  fn github_timestamp_parser_handles_utc_and_offsets() {
    assert_eq!(parse_github_timestamp("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse_github_timestamp("1970-01-01T01:00:00+01:00"), Some(0));
    assert_eq!(parse_github_timestamp("1969-12-31T19:00:00-05:00"), Some(0));
  }
}
