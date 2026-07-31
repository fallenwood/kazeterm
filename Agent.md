# Agent Guide

## Repository

Kazeterm is a Rust 2024, GPUI-based terminal emulator. Keep changes small, preserve cross-platform behavior, and treat source files plus `Cargo.toml` as authoritative when older documentation disagrees.

### Workspace layout

- `crates/kazeterm`: application bootstrap, GPUI windows/components, updater, workspace persistence, and UI-tree reconciliation.
- `crates/kazeterm-ui-tree`: serializable UI state, actions, reducer, and diffs. High-level UI mutations should flow through `UIAction` and `UITreeStore` when an action already exists.
- `crates/terminal`: terminal state, input, rendering, search, selection, kitty graphics, scrollbar, and minimap.
- `crates/terminal-kernel`: shared backend API and Alacritty terminal types.
- `crates/terminal-kernel-alacritty`: Alacritty PTY/session adapter.
- `crates/terminal-kernel-vte`: Linux-only VTE backend.
- `crates/terminal-kernel-unix-bundle`: pulls Unix kernels into workspace builds.
- `crates/config`: nested config types, defaults, imports, validation, themes, profiles, keybindings, and migrations.
- `crates/themeing`: GPUI theme/settings bridge.
- `crates/kazeterm-event-system`: JSON event protocol and event dispatch.

## Working rules

- Read the full implementation and all callers before changing shared behavior; fix root causes at the common boundary.
- Reuse existing helpers and patterns. Do not add dependencies or abstractions for a small local change.
- Do not edit `target/`, generated artifacts, or unrelated code. Change `Cargo.lock` only when dependencies change.
- Preserve `#[cfg(...)]` boundaries. VTE is selectable only on Linux; Alacritty is the default on every platform.
- For GPUI work, read `.github/skills/gpui/SKILL.md`, its relevant reference, and `anti-patterns.md`. Use `.github/skills/gpui-element/SKILL.md` only for low-level `Element` work.
- Prefer `Render`/`RenderOnce`; use `Element` only where layout/prepaint/paint control is required.
- After GPUI state changes, notify the context when a repaint is needed. Keep `Subscription` and cancellable `Task` handles alive on their owner.
- Cross-window tab moves must transfer existing terminal entities/PTYs and rebuild window-bound subscriptions; never recreate a running session.
- When adding persisted config, update the owning config type and `Default`, append a migration step, bump `CURRENT_CONFIG_VERSION`, and add migration coverage. Keep import-overlay behavior intact.
- Workspace/UI-tree format changes must preserve serde compatibility or explicitly migrate/validate old data.

## Style

- Run `cargo fmt`; formatting is 2 spaces with trailing commas.
- Follow Rust 2024 syntax and existing module organization.
- Workspace Clippy denies `dbg_macro`, `todo`, `declare_interior_mutable_const`, and `redundant_clone`.
- Use `#[cfg(target_os = "...")]` for platform code rather than runtime platform branches.

## Build and validation

CI uses Rust 1.97.0. Linux requires Clang/LLVM, XCB, and XKB development packages listed in `README.MD`.

Start with the narrowest useful checks, then run workspace checks for shared changes:

```bash
cargo fmt --all -- --check
cargo test --package <crate> <test_name>
cargo test --workspace
cargo build --workspace       # Linux/macOS full workspace
cargo build                   # Windows/default members
```

Do not use `release-fast` for normal development or debugging; it is for CI/package builds.

Useful focused suites:

```bash
cargo test --package kazeterm --bin kazeterm
cargo test --package config --test default_keybindings
cargo test --package kazeterm-event-system --test json_event_protocol
cargo test --package kazeterm-ui-tree
cargo test --package terminal --test headless_terminal
cargo test --package terminal --test snapshot_grid
cargo test --package terminal --test fake_session
```

For intentional terminal snapshot changes, inspect the diff and use `cargo insta review --workspace`; never accept snapshots blindly.

## Tests

- Put pure logic tests near the implementation with ordinary `#[test]`.
- Use `#[gpui::test]` only when GPUI/window context is required, and initialize app globals with `crate::test_support::init_test_app`.
- Terminal/UI integration tests must use the existing fake session factory rather than spawning a real shell or PTY.
- Follow `.claude/COMPONENT_TEST_RULES.md`: test builders as a group and meaningful branches/state transitions; avoid one test per trivial setter.
- Add the smallest regression test that fails before a non-trivial fix and passes after it.

## After modification

1. Summarize the changes in current iteration, save to `memories/<YYYY-mm-dd>-<title>.md`

## Before finishing

1. Review `git diff` for accidental or platform-specific regressions.
2. Run formatting and the narrow tests for changed crates.
3. Run `cargo test --workspace` when the change crosses crate boundaries or affects shared behavior.
4. Report commands actually run and any validation skipped.
