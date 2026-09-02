# crates/terminal-kernel-unix-bundle/

## Responsibility

Acts as a build-aggregation crate that pulls the Unix-only VTE terminal kernel into default Linux/macOS workspace builds while remaining dependency-free and buildable on Windows.

## Design

The crate is intentionally a no-op library. Its manifest declares `terminal-kernel-vte` only under the Linux/macOS target condition; inclusion in the root `default-members` therefore selects the backend at Cargo graph construction time without introducing runtime APIs or Windows compilation failures.

## Flow

1. Cargo includes this crate in a default workspace build.
2. On Linux/macOS its target-specific dependency causes `terminal-kernel-vte` to compile.
3. On Windows the dependency edge does not exist and the empty library compiles without Unix-only code.

## Integration

- Depends on: `terminal-kernel-vte` only for Linux and macOS targets.
- Consumed by: the root workspace's `default-members` build policy; it has no runtime consumers.
- Entry points: `Cargo.toml` contains the functional behavior; `src/lib.rs` documents the intent.
