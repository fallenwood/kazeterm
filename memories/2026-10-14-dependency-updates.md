# Dependency updates

Updated the workspace dependency set to current published versions and refreshed the lockfile.

## Changed
- Bumped `gpui-pre` from `0.3.1` to `0.3.9`
- Bumped `gpui-kit` from `0.7.0` to `0.7.3`
- Bumped `windows-core` from `0.62.2` to `0.100.0`
- Bumped `rust-embed` from `8.12.0` to `8.13.0`
- Bumped `toml` from `1.1.5` to `1.1.8`
- Bumped `notify-rust` from `4.18.0` to `4.18.2`
- Bumped `clap` from `4.6.6` to `4.6.7`
- Bumped `insta` from `1.48.0` to `1.49.0`
- Bumped `serde_json` from `1.0.151` to `1.0.154`
- Bumped `winresource` from `0.1.31` to `0.1.34`
- Kept `gpui-pre` and related workspace-provided dependencies aligned with the latest compatible versions resolved by `cargo update`

## Validation
- Ran: `cargo update`
- Ran: `cargo fmt --all -- --check`
- Ran: `cargo test --workspace --quiet`

Result: all workspace tests passed.
