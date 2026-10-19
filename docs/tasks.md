# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: one-command maintainer release (X03, public distribution follow-up)

Confirmed on 2026-10-19: supply a target version to one repository command and
automatically prepare versions, test and publish. Implemented
`./scripts/release VERSION`, reusing the existing native release workflow without
changing deployment configuration, runtime CLI, credentials or dependencies.

The command requires clean main and reviewed Unreleased notes, checks the official
origin/permissions/workflow/version/tag/remote-main state, updates six release
files, runs checks, commits and creates an annotated tag, then pushes main and tag
atomically. It follows the exact push tag/build commit through both native builds,
publication and both public verification jobs, and confirms Latest/assets before
reporting success. Existing versions/tags and historical notes are preserved.
`--resume` only monitors an identical existing local/remote annotated tag. Recovery
rules and external publication boundaries are in [releasing.md](releasing.md).

Validation on Linux:

- `cargo fmt --check` and `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked -- --quiet`: passed, all 331 Rust tests, including the new
  release-command integration test and 18 existing PTY scenarios.
- Release-command suite: 18 cases with real Git/temporary bare remotes and fake
  Cargo/GitHub responses. Covers complete commit/tag/atomic push, read-only resume,
  invalid/equal/lower versions, dirty work/index, branch/origin/permission/API
  failures, remote-main drift and tag reuse, malformed metadata/missing notes,
  check/commit/push failure, concurrent edits, public/private workflow failures,
  skipped public checks, missing attachments, tag identity and bounded run discovery.
- Independently prepared 1.7.0 in a temporary checkout; real
  `cargo metadata --locked --offline --no-deps --format-version 1 --manifest-path …`
  accepted the version without modifying the prepared dependency lock. All 18
  release-command tests also passed against that already-bumped checkout, verifying
  that self-tests remain usable after Unreleased is converted into release notes.
- `python3 tests/support/install_scripts.py`: passed, 14 cases.
- `python3 tests/support/merge_release.py`: passed, 8 cases.
- `python3 tests/support/package_release.py ./target/debug/devctl`: passed, 4 cases.
- `cargo test --locked --test release_cli -- --nocapture`: passed after the final
  fixture updates; all 18 Python cases execute through the Rust integration test.
- Read-only invocation of the script's monitor against existing v1.6.1 passed
  using real GitHub CLI 2.45.0: exact build commit/run, all five jobs, public stable
  Latest and all six attachments matched. No new workflow/publication was triggered.
- `git diff --check`: passed. Inline Markdown inspection checked balanced fences
  and all 120 local links/anchors in changed reference/task documents.

No real version bump, tag, push, publication or installation was performed. Native
Mac execution and a real end-to-end publication with this entry point remain
unverified; the existing platform/user acceptance items below remain pending.

## Preserved: mouse log text selection/copy and wheel history (L01 / U01, M5)

Confirmed on 2026-10-19: select parts of log text with the mouse, copy the selection
and scroll up/down through history. Implemented left drag/release, `y` or right-click
copy, and three-record wheel scrolling through existing retained-page requests.
Keyboard complete-line selection and lifecycle controls retain their behavior.

Selections use immutable record IDs and original UTF-8 grapheme ranges; rendered-row
snapshots prevent newly arriving output from shifting a click before the next draw.
Prefixes, timestamps, horizontal offsets, tabs/control escapes, Chinese/emoji and
combining characters map to original copy text. Snippets add only inter-record
newlines. Existing clipboard channels, 64 KiB bounds, failure preservation and
in-flight selection protection remain. Popups/hidden panes block mouse input;
resizing/eviction ends a drag. Terminal exit restores capture. The direct
`unicode-segmentation` dependency reuses the version already locked through Ratatui;
no new package/version was added to the dependency graph.

Validation on Linux:

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked -- --quiet`: passed, all 330 tests, including 18 PTY tests.
  Unit coverage includes forward/reverse/cross-line text selection, Unicode/control
  column mapping, horizontal offsets, search overlap, copy bounds, eviction,
  late-output clicks and overlay isolation. The PTY mouse scenario verifies exact
  clipboard bytes, retained-page wheel navigation, compact layout, service-control
  isolation and restoration of all five mouse modes on exit. Existing native
  clipboard success/failure/in-flight checks also passed.
- `cargo test --locked --test tui_cli -- --quiet`: passed independently, all 18
  scenarios. Earlier runs exposed a tab-offset error and excess per-row layout
  computation; both were corrected. The fixture now waits for copy-completion
  redraw and actual page arrival instead of assuming scheduling speed.
- `git diff --check`: passed. Inspected code/docs and checked balanced fences and
  all 109 local link paths in the seven changed reference/task documents.

Native Mac and actual Windows Terminal/WSL acceptance remain pending. No release
or installation is included.

## Published baseline: 1.6.1 (U01 / M5, X03)

Authorized on 2026-10-18: defer startup-readiness work and publish the existing TUI
visual refinement. No startup observation interval, readiness detection, framework
adapter or new service-state meaning is included.

Scope: remove start/stop state flashes; coherent dark/light/terminal palettes;
stable selection through focus changes; whole-action footer hints; current-search
underline; reduced motion, ASCII status symbols and nonempty `NO_COLOR` support.
No new runtime dependencies. Native release checks now exercise the appearance
scenario against each optimized binary as well as the existing terminal scenarios.

[Release v1.6.1](https://github.com/Jackie733/devctl/releases/tag/v1.6.1) is public
and Latest, from annotated tag/build commit
`e7a484116fe7eeb464eff755bcf5b9e008949fd8`. The
[final workflow](https://github.com/Jackie733/devctl/actions/runs/39136166524) passed
both builds, publication and both anonymous public-verification jobs.

### Validation

- Local `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`
  and `cargo test --locked -- --quiet`: passed, 315 Rust tests.
- Hosted suites: 315 tests on Apple Silicon/macOS 14; 318 on Linux/Ubuntu 22.04.
  Both passed fmt/clippy, 14 installer and 8 merge checks, native artifact checks,
  4 package tests, and optimized terminal scenarios including appearance, lifecycle,
  high-output/large-redraw stop, clipboard, reconnect and history.
- Local `MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --release --locked`, strict code
  signature verification, native install/reinstall/bad-checksum/rollback/config
  checks, 4 package tests and optimized appearance/high/lifecycle scenarios passed.
- Both public jobs passed anonymous downloads/digests, native execution/dependencies,
  latest/explicit version checks, installation, offline rollback/external recovery
  and configuration preservation.
- Independently downloaded all six attachments; verified both binary hashes/sizes,
  manifest targets, native Mac execution/signature and exact decompressed source
  archive identity against the release tag.
- Independent real public 1.5.0 → 1.6.1 default update passed in an isolated HOME:
  downloaded/verified the actual previous public Mac binary, confirmed the current
  release with the old client, performed its default update, matched new/backup bytes,
  preserved configuration, rolled back offline and restored via the external installer.
  Receipt: `/tmp/devctl-161-public-final-a4xofkwi/receipt.json`.
- `git diff --check` and the Markdown link/fence checker passed before tagging and
  for the final handoff: 17 documents, 172 local links/anchors, zero errors.

The initial v1.6.0 tag `bf22375` was not published as a Release.
[Its run](https://github.com/Jackie733/devctl/actions/runs/39135559760) passed Linux
but failed a Mac optimized lifecycle fixture. Reproduced that polling CLI state
without consuming terminal output could block its own queued control key. The
1.6.1 test correction drains live clients during polling, preserves deadlines and
assertions, and extends the high-output case to stop after a redraw exceeding the
terminal queue. The failed tag is preserved; no published artifacts were overwritten.

## Preserved handoff

- Project-addition EOF handling landed in `6dcc7e7` (A01/M7): possible old-background
  guidance, explicit invalid-request errors and verified old/new-binary recovery.
  Linux fmt/clippy, all 321 tests and documentation checks passed. The user's
  actual supervisor version/recovery remain unconfirmed; no release/install.
- Local-time log capture and immutable `[out #N]` / `[err #N]` display landed in
  `28c488d` (L01/U01, M5). Linux fmt/clippy, all 319 tests (including 17 PTY tests)
  and documentation checks passed; the initial unrelated discovery-boundary failure
  passed isolated/full reruns. Real-terminal/Mac acceptance, release and installation
  of those changes remain pending.
- [Startup readiness comparison](tui-design-research.md#follow-up-startup-state-and-complexity):
  deferred on 2026-10-18, not cancelled from historical D01–D05 scope. Running still
  means a live process; readiness checks and even a startup observation interval
  remain unimplemented.
- The [AG01 integrated proposal](agent-research.md) remains proposed; engine isolation,
  masking, diagnosis quality, costs/performance and real Mac/WSL integration remain
  unverified. No Agent integration is included in this release.
- The local installed client uses the unreleased `12bb89f` build and still reports
  1.5.0. Its bytes, backup, unchanged configuration and installed appearance/control
  checks were verified; receipt: `/tmp/devctl-tui-install-a7q1d8kb/receipt.json`.
  Publication does not replace it or an already-running supervisor automatically.
- Previous [v1.5.0](https://github.com/Jackie733/devctl/releases/tag/v1.5.0) was published
  from `2574a7febd6075a03e4320d307e48eb59218a3aa`; its
  [final workflow](https://github.com/Jackie733/devctl/actions/runs/39043197453) passed.
  Keep those tags/artifacts intact.
- Daily-project onboarding/hot addition, physical terminal/font appearance on Mac/WSL,
  real user-environment update/rollback and user acceptance remain distinct from
  controlled tests and public artifact checks. A03 remains pending assessment.

## Next steps

When the user selects a release version, run the [one-command release](releasing.md#one-command-release)
and record the actual CI/public outcome. Executing this implementation task does
not itself select a version or authorize publishing an unspecified release.

1. Verify dragging, actual clipboard paste, wheel paging and terminal restoration
   in the user's Windows Terminal/WSL and native Mac separately, using a new client
   build. The mouse feature requires no supervisor protocol change or service restart.
2. Verify the user's project-addition recovery after an explicit supervisor
   shutdown/reopen; inspect configuration/status before retrying.
3. Verify the new log display in the user's terminal with a newly built client.
4. Continue real-platform acceptance in [roadmap](roadmap.md) and [WSL verification](wsl.md).
5. Resume startup-readiness or AG01 implementation only after explicit scope approval.
