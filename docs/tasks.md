# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Completed: U01 / M5 visual refinement and local installation (unreleased)

Requested on 2026-10-18 after [research](tui-design-research.md): remove the white
start/stop state flash and implement the recommended visual direction.

- Removed reverse-video state emphasis and its expiry timer entirely. Selection
  backgrounds remain stable through state changes and focus switches.
- Added coherent dark/light/terminal palettes, shared dialog styling, quieter stopped
  notices, complete contextual footer actions, and current-search underline.
- Added reduced motion, ASCII status symbols and nonempty `NO_COLOR` support; options
  are documented in the [interaction contract](interaction.md#layout-help-and-confirmation).
- Existing keyboard scopes, log metadata/copy contents and service lifecycle remain.
  No new runtime dependency or public release was made.
- Fixed the PTY screen interpreter's handling of overwriting either half of a wide
  character; verified quoted Chinese/spaced directory completion without weakening
  its assertions. Added four-mode interactive appearance coverage.

### Validation

- `cargo test --all-targets -- --quiet`: all 315 tests passed (186 unit + 129 integration).
- After final visual adjustments, `cargo test tui:: -- --quiet`: 74 passed;
  `cargo test --test tui_cli -- --quiet`: all 18 passed, including appearance,
  lifecycle/failure/reconnect, search/copy/history, onboarding and resize scenarios.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`: passed.
- `python3 tests/support/tui_add_pty.py target/debug/devctl completion`: passed;
  `python3 tests/support/tui_pty.py target/debug/devctl appearance`: passed.
- Rendering checks cover stable start/stop backgrounds at multiple times, complete
  actions at 120×30 / 80×24 / 60×18 / 36×10, dark/light text contrast of at least
  4.5:1 for the exercised search/selection/help screens, and monochrome emphasis.
- Inspected rasterized TUI test-buffer previews for layout/palette coherence. These
  are buffer previews, not native-terminal screenshots or font acceptance evidence.
- `git diff --check` and the local Markdown link/fence checker passed. No temporary
  capture code or generated previews are included in the commit.

Unverified: physical macOS/WSL terminal font/rendering acceptance, comparative CPU
measurements. Spinner-onset delay, compact log prefixes and AG01 remain later work.
Published v1.5.0 artifacts were not replaced; these source changes require a new
release before public distribution.

### Local installation

The user explicitly requested replacing the installed program on 2026-10-18.
Built source commit `12bb89f` with
`MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --release --locked`; build and strict Mac
signature verification passed. The discovered workspace had no running services
(two stopped and one prior failed startup). Normal shutdown confirmed cleanup;
no old client/supervisor remained before replacement.

The standard checksum-verifying installer replaced `~/.local/bin/devctl` and saved
its prior bytes to `~/.local/bin/devctl.previous`. Installed bytes match the local
release build, backup bytes match the previous installation, and the actual workspace
configuration hash is unchanged. The local build still reports `devctl 1.5.0`; it is
an unreleased development build, not the published v1.5.0 artifact.
Installation receipt: `/tmp/devctl-tui-install-a7q1d8kb/receipt.json`.

`python3 tests/support/tui_pty.py /Users/chuliu/.local/bin/devctl appearance`: passed
against the installed program across dark/light/terminal and monochrome/ASCII/reduced
motion modes. Strict signature verification and installed `--version` passed.
Configuration remained unchanged and no test clients/supervisors remained. Actual terminal/font visual
acceptance remains separate. Reopen `devctl` to create an idle workspace using the
new program; no real project services were restarted during replacement.

### Preserved handoff

The [AG01 integrated proposal](agent-research.md) remains proposed. Its interaction,
architecture and delivery gates remain in the owning documents. Engine isolation,
masking, diagnosis quality, costs/performance and real Mac/WSL integration are still
unverified. This U01 proposal neither implements nor cancels that workflow.

## Published baseline and outstanding acceptance

- [v1.5.0 Release](https://github.com/Jackie733/devctl/releases/tag/v1.5.0) was published
  on 2026-10-17 from `2574a7febd6075a03e4320d307e48eb59218a3aa`.
  [Final workflow](https://github.com/Jackie733/devctl/actions/runs/39043197453)
  passed builds/publication/public verification for both supported targets.
- Hosted suites passed 310 tests on Apple Silicon/macOS 14 and 313 on Linux, plus
  installer/package/merge/native artifact checks. Anonymous downloads, manifests,
  isolated installation/rollback and configuration preservation passed. Detailed
  release receipts remain in Git; local download receipt: `/tmp/devctl-150-public-final-92qhu3gy`.
- The local installed client now uses unreleased source commit `12bb89f`, as verified
  above. To select exact published bytes later, follow normal shutdown and
  `update --version 1.5.0`.
- Daily-project onboarding/hot addition, actual WSL terminal/clipboard/log experience,
  real cross-version update/rollback and user acceptance remain distinct from hosted
  checks. Earlier Mac Bogu verification does not establish every workflow or physical
  macOS 14 hardware acceptance.

## Next steps

1. Validate the first U01 refinement in the user's actual macOS/WSL terminals before
   release; tune palette/motion from that evidence. Keep metadata compaction separate.
2. When implementation is authorized, begin AG1.1 engine feasibility against the
   documented data boundary, then implement and accept the complete AG1 TUI workflow.
   Do not substitute an export-only release or silently broaden engine access.
3. Continue outstanding real-project/platform acceptance in [roadmap](roadmap.md)
   and [WSL verification](wsl.md). A03 infrastructure control remains pending assessment.
4. Preserve published v1.5.0 tag/artifacts; runtime changes require a new version.
