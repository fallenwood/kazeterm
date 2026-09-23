# Tab Groups Glossary

Related decision: [Per-Window Tab Groups ADR](tab-groups-adr.md). This document describes the agreed domain model. The feature is implemented, but hands-on desktop interaction review is still pending.

| Term | Definition |
| --- | --- |
| Window | The ownership boundary for a group; all of a group's members belong to one window. |
| Tab | The smallest grouping unit. A tab can contain multiple split terminal panes. It belongs to at most one group, or none. |
| Group | A nonempty set of tabs with its own identity, name, and preset color. Members are contiguous in tab-bar order. Groups cannot be nested, span windows, or contain pinned tabs. |
| Group label | The visible name-and-color marker in the tab bar. It accepts individual tab drops, supports moving the whole group within a window, and exposes group actions. It is not itself a tab. |
| Group name | A user-provided name, or no name with an automatically generated display name. Names need not be unique and are not group identifiers. |
| Group color | A theme-aware preset used for the group label's capsule background and the continuous rail beside members in the vertical tab bar. It is restored with the workspace, does not signal the active tab, and does not add a colored dot before member tab icons. |
| Member order | The order of a group's tabs in its contiguous block, adjustable by dragging. |
| Join group | Assign one tab to a target group and move it into that group's contiguous block. An emptied source group disappears. The terminal is not recreated. |
| Remove from group | Remove a single tab's membership without closing the tab or its terminal. A dragged tab stays at its drop position; a context-menu removal places it after its former group. |
| Remove all / Disband group | Delete the group and leave all its former members ungrouped, preserving their order, the active tab, and running terminals. |
| Close all / Close grouped tabs | Delete the group and close every member tab and its terminals. If no tabs remain, follow the existing `close_on_last` setting. |
| Automatic removal | A group ceases to exist immediately when its last member leaves through removal, closing, cross-window transfer, or a split-layout merge. |
| Pinned tab | An existing pinned tab, which cannot belong to a group. Pinning a grouped tab first removes it from the group. |
| Workspace restoration | Restore group identity, membership, name, and color only when the existing workspace-restore mechanism is enabled. Shells are recreated as before; their processes do not survive a restart. |

**Out of scope for the first version:** nested groups, cross-window groups, merging whole groups, dragging a whole group across windows, collapsing groups, or implicitly creating/joining a group by dropping onto an ordinary tab.
