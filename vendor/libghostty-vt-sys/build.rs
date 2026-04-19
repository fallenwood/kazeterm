use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pinned ghostty commit. Update this to pull a newer version.
const GHOSTTY_REPO: &str = "https://github.com/ghostty-org/ghostty.git";
const GHOSTTY_COMMIT: &str = "ca7516bea60190ee2e9a4f9182b61d318d107c6e";

fn main() {
    // docs.rs has no Zig toolchain. The checked-in bindings in src/bindings.rs
    // are enough for generating documentation, so skip the entire native
    // build when running under docs.rs.
    if env::var("DOCS_RS").is_ok() {
        return;
    }

    println!("cargo:rerun-if-env-changed=LIBGHOSTTY_VT_SYS_NO_VENDOR");
    println!("cargo:rerun-if-env-changed=GHOSTTY_SOURCE_DIR");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=HOST");
    println!("cargo:rerun-if-changed=crates/libghostty-vt-sys/build.rs");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));
    let target = env::var("TARGET").expect("TARGET must be set");
    let host = env::var("HOST").expect("HOST must be set");

    // Locate ghostty source: env override > fetch into OUT_DIR.
    let ghostty_dir = match env::var("GHOSTTY_SOURCE_DIR") {
        Ok(dir) => {
            let p = PathBuf::from(dir);
            assert!(
                p.join("build.zig").exists(),
                "GHOSTTY_SOURCE_DIR does not contain build.zig: {}",
                p.display()
            );
            p
        }
        Err(_) => fetch_ghostty(&out_dir),
    };

    if target.contains("windows") {
        patch_ghostty_windows_vt_write(&ghostty_dir);
        patch_ghostty_windows_ubsan(&ghostty_dir);
    }

    // Build libghostty-vt via zig.
    let install_prefix = out_dir.join("ghostty-install");

    build_ghostty(&ghostty_dir, &install_prefix, &target, &host);

    let include_dir = install_prefix.join("include");
    let search_dirs = library_search_dirs(&target, &install_prefix);
    let candidates = library_artifact_candidates(&target);

    let found = search_dirs
        .iter()
        .any(|dir| candidates.iter().any(|name| dir.join(name).exists()));
    assert!(
        found,
        "no library artifact found; searched {:?} for {:?}",
        search_dirs, candidates
    );
    assert!(
        include_dir.join("ghostty").join("vt.h").exists(),
        "expected header at {}",
        include_dir.join("ghostty").join("vt.h").display()
    );

    for dir in &search_dirs {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
    println!("cargo:rustc-link-lib=dylib=ghostty-vt");
    println!("cargo:include={}", include_dir.display());
}

fn build_ghostty(ghostty_dir: &Path, install_prefix: &Path, target: &str, host: &str) {
    // Zig 0.15.2 on Windows can assert during Ghostty's configure phase when
    // helper binaries are launched from Ghostty's normal working directory.
    // Running the same build from the cached `uucode` package avoids the bad
    // path conversion while still using Ghostty's real build.zig and cache.
    if host.contains("windows") && target.contains("windows") {
        build_ghostty_from_uucode_cache(ghostty_dir, install_prefix, target, host);
        return;
    }

    let mut build = Command::new("zig");
    configure_ghostty_build(
        &mut build,
        ghostty_dir,
        install_prefix,
        target,
        host,
    );
    run(build, "zig build");
}

fn build_ghostty_from_uucode_cache(
    ghostty_dir: &Path,
    install_prefix: &Path,
    target: &str,
    host: &str,
) {
    let ghostty_build_file = ghostty_dir.join("build.zig");
    let local_cache_dir = ghostty_dir.join(".zig-cache");
    let global_cache_dir = zig_global_cache_dir();
    let is_debug = std::env::var("DEBUG").map_or(false, |v| v == "true");
    let optimize = if is_debug { "Debug" } else { "ReleaseFast" };

    let mut fetch = Command::new("zig");
    fetch
        .arg("build")
        .arg("--fetch=needed")
        .arg("--build-file")
        .arg(&ghostty_build_file)
        .arg("--cache-dir")
        .arg(&local_cache_dir)
        .arg("--global-cache-dir")
        .arg(&global_cache_dir)
        .arg("-Demit-lib-vt")
        .arg(format!("-Doptimize={}", optimize))
        .current_dir(ghostty_dir);
    maybe_add_target_arg(&mut fetch, target, host);
    run(fetch, "zig build --fetch=needed");

    let uucode_dir = global_cache_dir
        .join("p")
        .join(read_zig_dependency_hash(ghostty_dir, "uucode"));

    assert!(
        uucode_dir.join("build.zig").exists(),
        "expected cached uucode package at {}",
        uucode_dir.display()
    );

    let mut build = Command::new("zig");
    build
        .arg("build")
        .arg("--build-file")
        .arg(&ghostty_build_file)
        .arg("--cache-dir")
        .arg(&local_cache_dir)
        .arg("--global-cache-dir")
        .arg(&global_cache_dir)
        .arg("-Demit-lib-vt")
        .arg("--prefix")
        .arg(install_prefix)
        .current_dir(&uucode_dir);
    maybe_add_target_arg(&mut build, target, host);
    run(build, "zig build");
}

fn configure_ghostty_build(
    build: &mut Command,
    ghostty_dir: &Path,
    install_prefix: &Path,
    target: &str,
    host: &str,
) {
    build
        .arg("build")
        .arg("-Demit-lib-vt")
        .arg("--prefix")
        .arg(install_prefix)
        .current_dir(ghostty_dir);
    maybe_add_target_arg(build, target, host);
}

fn maybe_add_target_arg(build: &mut Command, target: &str, host: &str) {
    // Zig's native Windows host detection prefers MSVC even when Cargo is
    // running under the GNU toolchain, so native Windows GNU builds must force
    // the Ghostty target explicitly.
    if should_specify_zig_target(target, host) {
        build.arg(format!("-Dtarget={}", zig_target(target)));
    }
}

fn should_specify_zig_target(target: &str, host: &str) -> bool {
    target != host || is_windows_gnu_target(target)
}

fn is_windows_gnu_target(target: &str) -> bool {
    target.contains("windows-gnu") || target.contains("windows-gnullvm")
}

/// Clone ghostty at the pinned commit into OUT_DIR/ghostty-src.
/// Reuses an existing clone if the commit matches.
fn fetch_ghostty(out_dir: &Path) -> PathBuf {
    let src_dir = out_dir.join("ghostty-src");
    let stamp = src_dir.join(".ghostty-commit");

    // Skip fetch if we already have the right commit.
    if stamp.exists()
        && let Ok(existing) = std::fs::read_to_string(&stamp)
        && existing.trim() == GHOSTTY_COMMIT
    {
        return src_dir;
    }

    // Clean and clone fresh.
    if src_dir.exists() {
        std::fs::remove_dir_all(&src_dir)
            .unwrap_or_else(|e| panic!("failed to remove {}: {e}", src_dir.display()));
    }

    eprintln!("Fetching ghostty {GHOSTTY_COMMIT} ...");

    let mut clone = Command::new("git");
    clone
        .arg("clone")
        .arg("--filter=blob:none")
        .arg("--no-checkout")
        .arg(GHOSTTY_REPO)
        .arg(&src_dir);
    run(clone, "git clone ghostty");

    let mut checkout = Command::new("git");
    checkout
        .arg("checkout")
        .arg(GHOSTTY_COMMIT)
        .current_dir(&src_dir);
    run(checkout, "git checkout ghostty commit");

    std::fs::write(&stamp, GHOSTTY_COMMIT).unwrap_or_else(|e| panic!("failed to write stamp: {e}"));

    src_dir
}

fn run(mut command: Command, context: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to execute {context}: {error}"));
    assert!(status.success(), "{context} failed with status {status}");
}

fn zig_global_cache_dir() -> PathBuf {
    PathBuf::from(
        env::var_os("LOCALAPPDATA")
            .unwrap_or_else(|| panic!("LOCALAPPDATA must be set for Windows Ghostty builds")),
    )
    .join("zig")
}

fn read_zig_dependency_hash(ghostty_dir: &Path, dependency_name: &str) -> String {
    let zon = std::fs::read_to_string(ghostty_dir.join("build.zig.zon"))
        .unwrap_or_else(|error| panic!("failed to read Ghostty build.zig.zon: {error}"));

    let dependency_marker = format!(".{dependency_name} = .{{");
    let dependency_start = zon.find(&dependency_marker).unwrap_or_else(|| {
        panic!(
            "failed to locate dependency {dependency_name} in {}",
            ghostty_dir.join("build.zig.zon").display()
        )
    });

    let dependency_body = &zon[dependency_start..];
    let hash_marker = ".hash = \"";
    let hash_start = dependency_body.find(hash_marker).unwrap_or_else(|| {
        panic!("failed to locate .hash for dependency {dependency_name} in Ghostty build.zig.zon")
    });

    let hash_value = &dependency_body[hash_start + hash_marker.len()..];
    let hash_end = hash_value.find('"').unwrap_or_else(|| {
        panic!("failed to parse .hash for dependency {dependency_name} in Ghostty build.zig.zon")
    });

    hash_value[..hash_end].to_owned()
}

fn patch_ghostty_windows_vt_write(ghostty_dir: &Path) {
    let terminal_c = ghostty_dir.join("src").join("terminal").join("c").join("terminal.zig");
    let original = "    wrapper.stream.nextSlice(ptr[0..len]);";
    let patched = "    for (ptr[0..len]) |c| wrapper.stream.next(c);";
    patch_file_once(
        &terminal_c,
        original,
        patched,
        "Windows vt_write",
    );
}

fn patch_ghostty_windows_ubsan(ghostty_dir: &Path) {
    let shared_deps = ghostty_dir.join("src").join("build").join("SharedDeps.zig");
    let original = r#"        // Disable ubsan for MSVC to avoid undefined references to
        // __ubsan_handle_* symbols that require a runtime we don't link
        // and bundle. Hopefully we can fix this one day since ubsan is nice!
        if (target.result.abi == .msvc) try flags.appendSlice(b.allocator, &.{"#;
    let patched = r#"        // Disable ubsan for Windows to avoid undefined references to
        // __ubsan_handle_* symbols when the Ghostty build does not bundle a
        // runtime on that target.
        if (target.result.os.tag == .windows) try flags.appendSlice(b.allocator, &.{"#;
    patch_file_once(
        &shared_deps,
        original,
        patched,
        "Windows UBSan",
    );
}

fn patch_file_once(path: &Path, original: &str, patched: &str, description: &str) {
    let contents = std::fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "failed to read {} for {description}: {error}",
            path.display()
        )
    });

    if contents.contains(patched) {
        return;
    }

    let updated = contents.replacen(original, patched, 1);
    assert!(
        updated != contents,
        "failed to apply {description} patch to {}",
        path.display()
    );

    std::fs::write(path, updated).unwrap_or_else(|error| {
        panic!(
            "failed to write {} for {description}: {error}",
            path.display()
        )
    });
}

/// Returns directories to search for the built library artifact.
/// On Windows, Zig may place the DLL in `bin/` and the import lib in `lib/`,
/// so both are included.
fn library_search_dirs(target: &str, install_prefix: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![install_prefix.join("lib")];
    if target.contains("windows") {
        dirs.push(install_prefix.join("bin"));
    }
    dirs
}

/// Returns candidate filenames for the shared library artifact, ordered by
/// preference. The build assertion succeeds if any one of these exists in any
/// of the search directories.
fn library_artifact_candidates(target: &str) -> &'static [&'static str] {
    if target.contains("darwin") {
        &["libghostty-vt.0.1.0.dylib", "libghostty-vt.dylib"]
    } else if target.contains("windows-gnu") {
        &["libghostty-vt.dll.a", "ghostty-vt.dll", "ghostty-vt.lib"]
    } else if target.contains("windows-msvc") {
        &["ghostty-vt.lib", "ghostty-vt.dll", "libghostty-vt.dll.lib"]
    } else {
        &["libghostty-vt.so.0.1.0", "libghostty-vt.so"]
    }
}

fn zig_target(target: &str) -> String {
    let value = match target {
        "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu",
        "x86_64-unknown-linux-musl" => "x86_64-linux-musl",
        "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu",
        "aarch64-unknown-linux-musl" => "aarch64-linux-musl",
        "aarch64-apple-darwin" => "aarch64-macos-none",
        "x86_64-apple-darwin" => "x86_64-macos-none",
        "x86_64-pc-windows-gnu" => "x86_64-windows-gnu",
        "aarch64-pc-windows-gnullvm" => "aarch64-windows-gnu",
        "x86_64-pc-windows-msvc" => "x86_64-windows-msvc",
        "aarch64-pc-windows-msvc" => "aarch64-windows-msvc",
        other => panic!("unsupported Rust target for vendored build: {other}"),
    };
    value.to_owned()
}
