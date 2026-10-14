# Switch workspace to GPUI Fast

## Summary
- Replaced the workspace `gpui` dependency from `gpui-pre` to `gpui-fast` in the root `Cargo.toml`.
- Enabled `gpui-kit`'s `gpui-fast` feature so the Kit facade selects the matching Fast backend without requiring application import changes.

## Files changed
- `Cargo.toml`

## Validation
- Ran: `cargo build --workspace`
- Ran: `cargo fmt --all -- --check`
- Ran: `cargo test --workspace --quiet`

## Result
The workspace builds and tests successfully with the GPUI Fast backend while keeping the existing `gpui-kit` integration intact.
