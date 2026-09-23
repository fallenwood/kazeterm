# ADR: Per-Window Tab Groups

- Status: Implemented; pending hands-on desktop interaction review
- Date: 2026-09-23
- Related terms: [Tab Groups Glossary](tab-groups-glossary.md)

## Context

Each Kazeterm window owns an ordered list of tabs. Tabs can be pinned, reordered within a window, moved across windows, or merged into another tab's split layout. The UI tree stores workspace state and recreates tabs and shells when workspace restoration is enabled. Named, colored groups must fit these existing operations without changing the lifetime of running terminal sessions.

## Decision: Domain Boundaries and Invariants

- A group belongs to exactly one window. A tab belongs to at most one group in that window, or remains ungrouped. Groups cannot contain other groups or have members in another window.
- Single-tab groups are allowed; empty groups are not. A group disappears automatically when its final member is removed, closed, transferred, or merged into a split layout.
- Members of a group occupy a contiguous range in the tab bar and can be reordered within that range. Pinned tabs cannot join groups; pinning a grouped tab removes it from its group first.
- A group has an identity independent of its name. Names may be absent (showing an automatically generated name) or duplicated. Colors come from preset options that follow the light/dark theme. Group identity, membership, name, and color are part of the restorable workspace state.
- Changing membership affects organization and any necessary tab ordering only: it must not recreate or close a terminal that should remain running. Moving a single tab between windows continues to transfer its existing terminal entities and PTY.

## Decision: Interactions and Operations

- The tab context menu can create a single-tab group or move a tab into an existing group. Dropping a tab on a group label also joins that group. Dropping an ungrouped tab on an ordinary tab only reorders it; it does not create or join a group. Dropping a tab from another group onto a group label transfers just that tab, not the entire group.
- Dragging within a group reorders its members. Dragging a tab outside the group removes it and places it at the drop position; using **Remove from Group** in the context menu places it immediately after its old group. Dropping onto an ordinary tab in another group can only place the dragged tab before or after that whole group, without joining it.
- Dragging a group label moves its entire contiguous block within the same window. Moving an entire group across windows is not supported in the first version. A single grouped tab moved to another window becomes ungrouped by default; it joins a destination group only when explicitly dropped on that group's label. An emptied source group disappears.
- Group labels remain visible whenever the tab bar is visible. They show the group's name in a colored capsule and offer rename, color, and delete actions in a context menu. A normal left click on the label does not activate a tab. In the vertical tab bar, a thin colored rail runs from below the capsule alongside the group's members; member tab icons do not get colored dots. Active grouped tabs use a neutral, theme-text-colored border instead of the group color; active ungrouped tabs retain their existing accent border. Both horizontal and vertical tab bars support groups. Collapsing or merging whole groups is outside the first version.
- A newly created tab is ungrouped by default. Duplicating a grouped tab inserts its copy next to the source and into the same group.
- Deleting a group always requires confirmation with three outcomes: **Cancel**, **Remove All Tabs from Group**, or **Close All Tabs in Group**. The safer remove-all choice has initial focus; the close-all choice states the tab count and the risk of ending terminals. Remove-all deletes only the group and membership, preserving tab order, the active tab, and running terminals. Close-all closes the group's tabs and terminals. If this leaves the window empty, the existing `close_on_last` setting decides whether the window closes or a new ungrouped tab is created.
- Existing **Close Other Tabs** and **Close Tabs to the Right** actions still operate on individual tabs and retain their pinned-tab exclusions. Surviving members stay in their group, and an empty group disappears. Merging a tab into another tab's split layout removes only that tab from its source group.

## Workspace and Compatibility

Groups are restored only when the existing workspace-restore mechanism is enabled. The setting is unchanged, and restoration does not keep shell processes alive across restarts: shells are recreated as before. Each window's UI-tree JSON stores `groups`, and each tab has an optional `group_id`. Serde defaults let older workspaces without these fields load as entirely ungrouped. New data must satisfy window ownership, unique membership, nonempty and contiguous groups, and the pinned-tab exclusion.

## Acceptance Scenarios

1. Create a group from one tab, add a second, rename and recolor it, and reorder its members. Verify both tab-bar layouts and restoration when workspace restore is enabled.
2. Remove a tab via the context menu or drag, move it to another group, and transfer it between windows. Empty source groups disappear; terminals that should keep running do so, without recreating a PTY on cross-window moves.
3. Pin, duplicate, close, bulk-close, or merge a grouped tab into a split layout. Each operation preserves the membership and ordering invariants above.
4. Try each delete-group choice: cancel, remove all, and close all. The first two do not close terminals; remove-all preserves order and the active tab. Closing the last group follows `close_on_last`.
5. Load an older workspace without group data. It must not create implicit groups or disturb existing tabs.

This ADR records the approved first-version requirements and boundaries. The feature is implemented, but hands-on desktop interaction review is still pending.
