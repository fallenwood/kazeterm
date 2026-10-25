# Use PowerShell for workspace automation

## Changes
- Replaced the workspace-checking prototype with `scripts/check-workspace.ps1` and dependency-free regression tests in `scripts/test-workspace.ps1`; removed the Python scripts/cache and their ignore rules.
- Preserved workspace membership, headless dependency, and centralized path checks. Cargo supplies validated metadata; a scoped TOML lexer locates explicit member dependency paths while ignoring comments, string values, and source-target paths.
- Added regression cases for normal/dev/build and target-specific dependency tables, per-dependency tables, dotted/quoted/multiline declarations, and false-positive cases.
- PR/release CI explicitly uses `shell: pwsh` for both checks; no Pester or other module installation is required.
- Updated README, architecture/codemap references, cartography state, the workspace iteration summary, and contributor guidance. Future repository scripts/tests must use PowerShell 7.

## Validation
- `pwsh -NoProfile -File scripts/test-workspace.ps1` passed all five regression groups on PowerShell 7.6.4/Linux.
- `pwsh -NoProfile -File scripts/check-workspace.ps1` passed against the actual workspace, including invocation from outside the repository.
- Verified Cargo command failures propagate and the caller's working directory is restored.
- Checked the CLI's nonzero exit status for invalid dependency graphs.
- Confirmed no Python script references remain in current automation/docs/cartography entries.
- `git diff --check` passed.
- No Rust sources or dependency versions changed in this iteration; the full Rust test suite was not rerun. Native Windows/macOS execution remains for CI.
