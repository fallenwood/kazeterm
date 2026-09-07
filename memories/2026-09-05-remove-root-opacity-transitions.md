# Remove root opacity transitions

- Removed `fade_start_opacity`, `ui_transition_opacity`, and the root fade task so UI changes never multiply the configured window background opacity.
- Geometry transitions retain `enabled`, `duration_ms`, `frame_interval_ms`, and `easing`.
- Config migration `20260905.1` removes the obsolete key from existing files, and animation tests now cover geometry timing only.
