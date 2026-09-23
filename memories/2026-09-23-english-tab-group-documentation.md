# Rewrite Tab Group Docs and Memories in English

- Rewrote `.github/docs/tab-groups-adr.md` and `.github/docs/tab-groups-glossary.md` in English, retaining the per-window domain rules, interactions, persistence requirements, and current Edge-inspired group-color treatment.
- Rewrote the seven tab-group implementation/design/visual-change memories in English. Preserved each iteration's historical validation claims and added links where later UI changes superseded an earlier appearance.
- Clarified in the ADR that the implemented UI-tree stores window `groups` and optional tab `group_id` with serde defaults for compatibility with older workspaces. No application source or configuration was changed.

## Validation

- `git diff --check` passed; reviewed the documentation-only diff.
- Confirmed there are no Han characters left in `.github/docs` or `memories` and all relative `.md` links there resolve.
- Rust formatting, compilation, and tests were not rerun for this documentation-only change; workspace tests passed before the preceding commit.
