# crates/kazeterm-ui-tree/

## Responsibility

Defines a UI-framework-independent, JSON-serializable model of windows, tabs, split panes, search/tab-bar state, and overlays, together with validated actions and structural diffs for reconciling that model into a concrete UI.

## Design

- `UITree` is the aggregate root; `WindowNode`, `TabNode`, and recursive `PaneNode` represent the hierarchy and use monotonic string IDs.
- `UIAction` is a serializable Command algebra for all mutations, including atomic-looking batches, window/tab/pane operations, search, overlays, and UI toggles.
- `UITree::apply` is a Reducer: it validates identifiers, dimensions, indices, split ratios, and state invariants while mutating the tree in place.
- `diff_trees` compares snapshots and emits semantic `TreeDiff` values. `Reconciler` is an Adapter port for framework-specific consumers such as GPUI.
- The crate has no GPUI dependency, keeping state serialization and transition logic portable and deterministic.

## Flow

1. A caller creates or deserializes a `UITree` snapshot.
2. UI or external commands become `UIAction`s and pass through `UITree::apply`; invalid transitions return `anyhow::Error`.
3. The caller retains the previous tree and invokes `diff_trees(old, new)`.
4. Diffing reports semantic additions, removals, reordering, focus/content changes, and visibility/configuration changes.
5. A `Reconciler` implementation consumes the diffs and updates concrete UI entities while the tree remains the source of truth.

## Integration

- Depends on: Serde/Serde JSON for state and action interchange, and `anyhow` for reducer validation errors.
- Consumed by: `kazeterm` workspace state, application event handlers, and its GPUI `TreeReconciler` implementation.
- Entry point: `src/lib.rs` exposes `action`, `node`, `reducer`, and `diff` modules.
