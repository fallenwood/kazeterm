# Smooth UI transitions

## Window resizing

- `ResizeWindow` UI-tree actions now animate the native window to its target size over a shared 180 ms ease-in-out transition instead of resizing immediately.
- In-flight resize tasks are cancelled when a newer target arrives, allowing rapid commands to retarget cleanly.
- Resize actions reject non-finite, zero, and negative dimensions before mutating the UI tree.

## Configuration and sidebar changes

- Configuration and theme hot reloads now notify every registered `MainWindow` and briefly fade the refreshed UI into view.
- The vertical tab sidebar keeps separate preferred and rendered widths so show/hide actions can animate width and opacity without losing the user's drag-adjusted width.
- Sidebar transitions also handle configuration-driven orientation and minimum-width changes.
- Manual sidebar dragging cancels any active width animation and immediately adopts the dragged width.
- Workspace restoration synchronizes the rendered sidebar width with restored visibility to avoid stale hidden or visible states.

## Shared transition infrastructure

- Added shared 12-frame, 15 ms transition timing and eased interpolation helpers for scalar, pixel, and window-size values.
- Window resizing, sidebar animation, and configuration fades use the same transition cadence.
- Stored GPUI tasks make all transitions cancellable when superseded.

## Validation

- Added tests for invalid resize dimensions, exact resize completion, intermediate resize state, sidebar hide/show transitions, and configuration-driven fade/sidebar expansion.
- All 69 `kazeterm` tests and the complete `kazeterm-ui-tree` test suite passed.

## Relevant files

- `crates/kazeterm-ui-tree/src/reducer.rs`
- `crates/kazeterm/src/components/transitions.rs`
- `crates/kazeterm/src/components/main_window_transitions.rs`
- `crates/kazeterm/src/components/main_window.rs`
- `crates/kazeterm/src/components/main_window_render.rs`
- `crates/kazeterm/src/components/main_window_search.rs`
- `crates/kazeterm/src/components/workspace_state.rs`
- `crates/kazeterm/src/components/main_window_e2e_tests.rs`
- `crates/kazeterm/src/config_watcher.rs`
- `crates/kazeterm/src/reconciler.rs`
- `crates/kazeterm/src/window_manager.rs`
