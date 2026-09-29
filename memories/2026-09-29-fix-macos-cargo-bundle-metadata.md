# Fix macOS cargo-bundle metadata

- Moved the macOS minimum system version in `crates/kazeterm/Cargo.toml` from the unsupported top-level `osx_minimum_system_version` key to `[package.metadata.bundle.osx] minimum_system_version`, retaining `10.15`.
- Verified with `cargo metadata --no-deps --format-version 1`, `cargo fmt --all -- --check`, and `git diff --check`.
- Could not run the macOS `cargo bundle --target aarch64-apple-darwin --profile=release-fast --format=osx -p kazeterm` command on the Windows host; cargo-bundle is not installed here. No tests were run because this is a packaging metadata-only change.
