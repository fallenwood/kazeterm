# OSC 7501 program status

- Implemented revision 0.4 of the Program Status Protocol from the specification supplied by the user.
- Added a shared streaming OSC observer and bounded per-terminal record store. Reports replace records completely; hierarchical IDs support subtree clearing and nearest-ancestor app inheritance. The store retains at most 256 records and evicts the least recently updated record.
- Added feature detection replies, BEL/ST and fragmented-input handling, atomic text/size validation, OSC 133 A and process-exit expiration for working/blocked records, and RIS clearing. Alternate-screen switches and soft resets preserve records.
- Wired the observer into Alacritty's Unix/Windows PTY filters and the VTE read loop. Bash and PowerShell prompt hooks now emit OSC 133 A.
- Horizontal and vertical tabs display status/progress. Tooltips identify the originating tab and pane, show literal labels/messages, remove invisible formatting, and prioritize blocked/error/completed records.
- Added parser/store regression coverage and a fake-session GPUI regression for background-tab updates, prompt handling, exit cleanup, and reset. Added README usage examples.
- Validation passed: `cargo fmt --all`, `cargo fmt --all -- --check`, `git diff --check`, and `cargo test --package terminal program_status --offline` (13 tests). The standalone parser tests also passed (13 tests), and their temporary executable/PDB were removed.
- `cargo test --workspace --offline --config 'profile.dev.package.gpui-component.opt-level=0'` passed all 424 tests, including 106 application tests and 110 terminal unit tests. One existing documentation test remains ignored. GPUI component optimization was disabled only for this command after the default dependency compilation took 19 minutes; repository build settings were not changed.
- The initial application package test compile found an incorrect `Spinner` import. It was corrected to `spinner::Spinner`; the successful workspace run includes the entire application package suite. The queued/default workspace and package rerun were stopped to use the faster test-only dependency profile.
- Native UI/real PTY behavior and Unix builds/runtime were not manually verified. Both backend adapters compiled in the Windows workspace test run; Unix conditional PTY-filter code still requires platform validation.
- The initial linked article was retrieved, but automatic approval review failed to fetch the normative specification with `model_not_supported`; the user supplied the full revision 0.4 text, resolving that blocker.
