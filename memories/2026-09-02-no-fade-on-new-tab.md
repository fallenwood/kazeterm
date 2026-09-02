# New tabs should not fade the terminal

- Root opacity animation was applied to every visible UI-tree diff, so adding a tab faded the whole window including terminal contents.
- `TabAdded` and `ActiveTabChanged` now skip the shared fade. Direct `insert_new_tab` no longer calls `animate_ui_change()`.
- Config, search, splits, dialogs, and tab-bar changes still fade.
