# Tab Group Domain Model and Requirements

- Confirmed the per-window grouping boundary, exclusive membership, nonempty and contiguous groups, pinned-tab exclusion, cross-window moves, and interactions with existing close and split operations with the user over several rounds.
- Recorded first-version rules for group creation, joining, removal, whole-group reordering within a window, delete confirmation, naming, and preset colors. Explicitly excluded collapsing, merging whole groups, and moving whole groups across windows.
- Added `.github/docs/tab-groups-adr.md` and `.github/docs/tab-groups-glossary.md` as agreed design documents before implementation. This planning iteration did not change application source, dependencies, or configuration.

## Validation

- The documentation diff and workspace state were reviewed in that iteration's final report.
- This documentation-only planning iteration did not run Rust formatting, compilation, or tests. The feature had not yet been implemented at this stage; see [implementation](2026-09-23-tab-groups-implementation.md).
