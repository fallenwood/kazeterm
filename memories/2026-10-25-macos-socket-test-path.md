# macOS Unix socket test path limit

## Root cause

`private_socket_permissions_collision_and_owned_cleanup` built its socket path under `std::env::temp_dir()`. macOS CI's long per-user `TMPDIR`, combined with the unique directory name and `control.sock`, can exceed the 104-byte `sockaddr_un.sun_path` buffer. `RemoteControlServer::bind` canonicalizes the parent, so path sizing must include that canonical prefix too.

## Change

- Updated only the Unix-only test in `crates/kazeterm-event-system/src/remote_control/socket.rs` to use a uniquely named private directory under `/tmp`, independent of `TMPDIR`.
- Canonicalize the test directory and assert that the final socket path is shorter than 104 bytes, accounting for macOS resolving `/tmp` to `/private/tmp` and reserving the terminating NUL.
- Retained the existing directory/socket permission, collision, replacement-file protection, and cleanup checks. Production socket binding behavior and dependencies are unchanged.

## Validation

- Before the fix, ran the failing test with a deliberately long `TMPDIR` on Linux and reproduced the exact `InvalidInput: path must be shorter than SUN_LEN` panic at the bind.
- After the fix, `cargo test --package kazeterm-event-system --all-features --tests` with the same long-`TMPDIR` setup passed all 16 unit/integration tests.
- `cargo fmt --all -- --check`: passed.
- `pwsh -NoProfile -File scripts/check-workspace.ps1`: passed.
- `git diff --check`: passed; reviewed the test-only diff.
- Actual macOS/aarch64 execution was not available (only Linux and wasm Rust targets are installed). Full workspace tests were not rerun for this isolated test-fixture change.
