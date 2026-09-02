# crates/config/src/migration/

## Responsibility

Upgrades historical Kazeterm configuration documents in place at the raw TOML boundary so the current typed `Config` only needs to understand schema version `20260901.1`.

## Design

- `mod.rs` owns `CURRENT_CONFIG_VERSION`, the private ordered `Migration { from_version, to_version, migrate }` registry, and the public dispatcher `apply_migrations`.
- `steps/` contains one narrowly scoped `fn(&mut toml::Value)` per adjacent version transition. See [steps/codemap.md](steps/codemap.md).
- **Registry/Command pattern:** migration functions are data entries ordered oldest-to-newest. The dispatcher locates the entry whose `from_version` equals the document version and executes the remaining suffix.
- **Raw document transformation:** steps edit `toml::Value` tables rather than partially deserializing obsolete Rust structs. This supports field insertion/removal/rename, section restructuring, and keybinding format conversion.
- Steps generally preserve explicit user values with `contains_key`/`entry(...).or_insert...`, then stamp their exact target version. Some schema additions intentionally only advance the version because Serde defaults supply the new runtime value.

## Flow

1. `Config::load_from_path` parses the base file and calls `apply_migrations` before imports are merged or typed deserialization begins.
2. Missing `version` is treated as `"0"`; an already-current document returns `false` without mutation.
3. A recognized historical version selects the matching registry index. Every step from that point through the registry tail runs in order and advances the document version.
4. An unknown version logs a warning, is left unchanged, and returns `false`; subsequent deserialization decides whether the document is usable.
5. A `true` result tells `Config` to back up the original base text and persist the upgraded raw base document. Imported overlay files are not migrated by the import path.

## Integration

- Called by the main load pipeline and by raw updater-state mutators before `[auto_update]` is edited.
- Structural/keybinding steps depend on current `KeybindingConfig` defaults and the keybinding canonicalizer when legacy action-first data must be preserved; the latest step materializes the complete default `[animation]` policy before typed deserialization.
- The migration version is re-exported from `crate::lib` and becomes `Config::default().version` and the generated default-file version.
- Adding a schema change requires an adjacent step module, a `steps/mod.rs` re-export, a final registry entry, and a matching `CURRENT_CONFIG_VERSION` update; the order and version chain are invariants.
