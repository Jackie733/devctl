# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: Python terminal-check cleanup complete (U01 / L01 / A01; X03 tooling boundary)

The maintainer authorized organizing existing Python support code. No product
feature, runtime dependency, release command or workflow configuration changes.
The [contributor guide](../CONTRIBUTING.md#python-support-code) owns the Python
code map and check commands; both READMEs summarize the development-only role.

Split the former 1,039-line TUI runner into reusable screen/driver/workspace/service
fixtures and lifecycle, navigation/appearance, clipboard/mouse and history modules.
The entry point keeps every existing scenario name and binary argument. Binary
paths are passed explicitly instead of being captured from another script's argv.
Memory/disk history share navigation setup while retaining their distinct eviction,
reconnect, search and copy assertions. Unicode fixture output uses an explicit
suffix instead of rewriting Python source text.

The four terminal entry points execute only under their main guards. Onboarding
checks use readable statements and clean their temporary directories after closing
clients and shutting down the isolated workspace. A new Rust integration check
verifies imports with no CLI arguments cannot launch processes or create workspaces.
Release tools remain in their existing locations with their own isolated tests.

Validation on native macOS:

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked -- --quiet`: passed, 335 tests (201 unit, 134 integration),
  including all 20 existing TUI checks and the new import-isolation check.
- `python3 tests/support/install_scripts.py`: 14 passed;
  `python3 tests/support/merge_release.py`: 8 passed;
  `python3 tests/support/release_command.py`: 21 passed;
  `python3 tests/support/package_release.py ./target/debug/devctl`: 4 passed.
- `uvx ruff check tests/support/tui_support tests/support/{tui_pty,tui_add_pty,import_pty,update_pty}.py --select F,I`
  and `uvx ruff format --check tests/support/tui_support tests/support/{tui_pty,tui_add_pty,import_pty,update_pty}.py`:
  passed. Ruff was used as an external check tool; no project dependency was added.
- All Python sources compile. Compared the screen interpreter and 11 unchanged
  scenario/helper bodies against their original syntax trees; statements match
  after accounting for explicit binary parameters. TUI runner `--help` passed.
- `git diff --check`: passed. Four changed Markdown documents have balanced
  fences and 72 valid local links/anchors.

Next: no cleanup work remains. Physical-terminal/WSL acceptance and the preserved
release repair handoff below remain separate; this cleanup ran no WSL/Linux CI,
installed-client replacement, push or publication.

## Preserved: graphical identity outside the TUI (U01 visual identity)

The maintainer authorized keeping the complete service-line/control-rail mark in
README/documentation and project identity, while removing decorative branding
from the TUI, including Help. Removed the terminal brand renderer and its layout
reservations. Help again dedicates its content area to instructions/diagnostics.
The original compact main header, name/version/status, selection/navigation
symbols and 36 × 10 minimum remain intact. No abbreviated logo is introduced.

Full SVG/PNG assets and both README artworks are unchanged. The
[brand usage rule](../assets/branding/README.md) owns the placement; reference
documents and the [updated preview](../assets/branding/tui-preview.png) match the
text-only terminal build. No service behavior or new dependency is involved.

Validation on native macOS:

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked -- --quiet`: passed, 334 tests (201 unit, 133 integration;
  includes 20 real-PTY TUI scenarios).
- Appearance PTY checks cover the compact header in dark/light/terminal/monochrome
  modes, 120-column, 60-column and 36 × 10 layouts, and logo-free Help.
  Unit coverage verifies original header height, absence of the decorative mark,
  Help navigation/footer and beginning/end content across themes and sizes.
  Existing lifecycle,
  log reading/copying, mouse selection, project edit/remove/reload, help diagnostic
  reachability and terminal restoration scenarios pass.
- Ran isolated dark/light/ASCII-monochrome main views and light Help. Reconstructed
  actual PTY text/colors with Menlo and inspected the preview; capture workspaces
  cleaned up. This is not a native-window screenshot.
- Changed Markdown links/fences, image references and `git diff --check` passed.
  No installed binary replacement, push, release or publication.

Next: physical-terminal/font acceptance on the user's Mac and Windows Terminal/
WSL remains separate from controlled native PTY checks. Existing release repair
and real-platform acceptance handoffs below remain as recorded; this task does
not trigger or replace that release work.

## Preserved: repair macOS release checks (X03 / A01)

The [v1.8.0 workflow](https://github.com/Jackie733/devctl/actions/runs/39235652469)
failed in the macOS edit/remove unit test: production code canonicalizes project
paths to `/private/tmp/...`, but two assertions expected the uncanonicalized
`/tmp/...` fixture paths. Linux passed; publication and public verification were
skipped. `gh release view v1.8.0 -R Jackie733/devctl` returned `release not found`.

Both assertions now canonicalize their expected paths. Runtime behavior and
workflow configuration are unchanged. A Linux run with `TMPDIR` pointing through
a directory symlink reproduced the original failure; all four hot-add tests passed
with that same setup after the fix.

Validation on Linux: `cargo fmt --check`,
`cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked -- --quiet` (336 tests), and `git diff --check` passed.
Inspected the diff and verified the recovery document link/anchor.

Next: publish a new version with `./scripts/release 1.8.1`.
Per [release recovery](releasing.md#failure-and-recovery),
keep `v1.8.0` intact: rerunning or resuming it cannot include this source fix.
Native macOS validation and publication of the repaired version remain pending.
No push, workflow rerun, or publication was triggered during this repair.

## Implementation handoff: dedicated restart, hot config reload, TUI project edit/remove, and Projects mouse navigation (P01, P07, A01, U01)

Authorized on 2026-10-19: implement three high-frequency daily workflow enhancements:

1. **Dedicated single and default-group restart (`P01`, `P07`)**:
   - CLI `devctl restart SERVICE...` and `devctl restart --all` (creates the supervisor if needed after preflighting targets).
   - TUI `r` restarts the selected service when connected (or reconnects when disconnected); `R` stops all active services, confirms process-group cleanup, and starts the default group (`autostart = true`); `c` reconnects/syncs to the existing background workspace.
2. **External configuration hot-reload and TUI stopped-project edit/remove (`A01`)**:
   - CLI `devctl reload` and TUI `u` reload `devctl.toml` into the running supervisor, applying additions, removals, and updates to `Stopped`/`Failed` (`cleanup_confirmed`) services without interrupting running services.
   - TUI `e` in the Projects panel opens the review/edit dialog (`Edit project` / `Review project update`) for a stopped service and atomically saves updated command/directory changes via `Request::UpdateProject`.
   - TUI `d` in the Projects panel opens a confirmation modal (`Remove project`, `y`/`n`/`Esc`) for a stopped service and atomically removes it (and its `projects.<id>` entry when unreferenced) via `Request::RemoveProject`.
3. **Projects panel mouse click and wheel navigation (`U01`)**:
   - Left-clicking a row in the Projects panel focuses `Projects` and selects that service.
   - Scrolling the mouse wheel over the Projects panel focuses `Projects` and moves selection up/down without triggering service start/stop/restart/remove actions.

Validation on Linux:

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked -- --quiet`: passed, all 336 tests (204 unit tests and 132 integration tests, including 19 TUI PTY scenarios).
- `git diff --check`: passed.

Real-environment WSL/macOS acceptance of the new commands and TUI interactions remains pending.

## Preserved: 1.7.0 published; CI Action maintenance follow-up (X03)

The user successfully ran the corrected one-command release on 2026-10-19.
[Release v1.7.0](https://github.com/Jackie733/devctl/releases/tag/v1.7.0) is public,
stable and Latest, from annotated tag/build commit
`f169de83fdc0778e5f2f78697c495f8bf4a90485`. The
[release workflow](https://github.com/Jackie733/devctl/actions/runs/39221442679)
completed successfully: both native builds, publication and both anonymous public
verification jobs passed. All six expected public attachments are present.

The release includes mouse log selection/copy/wheel history, local-time/stable log
labels, project-addition EOF guidance and the maintainer release command. The
pager/feedback correction is included from `a197129`: disable paging, numbered
stages, timed checks/heartbeat, persistent check logs and explicit interrupt state.
Local validation before publication passed fmt, locked all-target Clippy and all
331 Rust tests, including 21 release-command cases and 18 TUI PTY scenarios.
A positive-control real-PTY regression reproduced the original empty less pager;
the corrected runner completed supervisor tests and diff checks without paging.

### Publication evidence and annotations

Read-only verification after the user's run:

- `gh run view 39221442679 -R Jackie733/devctl --json headSha,status,conclusion,url,jobs`:
  exact tag/build commit, completed/success, all five jobs successful.
- `gh release view v1.7.0 -R Jackie733/devctl --json tagName,isDraft,isPrerelease,url,assets`:
  public stable release and exactly six expected attachments.
- `gh api repos/Jackie733/devctl/releases/latest --jq .tag_name`: `v1.7.0`.
- Local tag resolves to the same full build commit; worktree was clean after the
  user's release command. The previously prepared six-file changes are committed.
- Documentation-only handoff: `git diff --check` passed; checked fences and all
  32 local links/anchors in the three changed documents. No Rust code changed.

Two Node 20 annotations identify the pinned upload-artifact v4 and download-artifact
v5 Actions. GitHub forced those Actions to Node 24 and both completed successfully.
[GitHub's migration notice](https://github.blog/changelog/2025-09-19-deprecation-of-node-20-on-github-actions-runners/)
recommends versions explicitly targeting Node 24. Plan a separate, compatible
Action-pin update retaining full commit SHAs and validate artifact layout/build-only
execution before the next release. No workflow/deployment configuration was changed
in this handoff, and v1.7.0 needs no replacement or overwrite for these warnings.
The macOS arm64 capacity annotation concerns possible queue delays, not a failed job;
no project-code change is needed for that notice.

Publication through the entry point is now verified in the user's environment.
Actual installed-client/supervisor versions, physical Mac/WSL mouse/clipboard/log
acceptance and user-project acceptance remain unconfirmed. No new installation,
workflow run or publication was triggered while inspecting these annotations.

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

Native Mac and actual Windows Terminal/WSL acceptance remain pending. This work
is now published in 1.7.0; installation in the user environment is unconfirmed.

## Previous published baseline: 1.6.1 (U01 / M5, X03)

Authorized on 2026-10-18: defer startup-readiness work and publish the existing TUI
visual refinement. No startup observation interval, readiness detection, framework
adapter or new service-state meaning is included.

Scope: remove start/stop state flashes; coherent dark/light/terminal palettes;
stable selection through focus changes; whole-action footer hints; current-search
underline; reduced motion, ASCII status symbols and nonempty `NO_COLOR` support.
No new runtime dependencies. Native release checks now exercise the appearance
scenario against each optimized binary as well as the existing terminal scenarios.

[Release v1.6.1](https://github.com/Jackie733/devctl/releases/tag/v1.6.1) is public
and was Latest at publication, from annotated tag/build commit
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
  actual supervisor version/recovery remain unconfirmed; included in 1.7.0,
  installation unconfirmed.
- Local-time log capture and immutable `[out #N]` / `[err #N]` display landed in
  `28c488d` (L01/U01, M5). Linux fmt/clippy, all 319 tests (including 17 PTY tests)
  and documentation checks passed; the initial unrelated discovery-boundary failure
  passed isolated/full reruns. Included in 1.7.0; real-terminal/Mac acceptance and
  installation of those changes remain pending.
- [Startup readiness comparison](tui-design-research.md#follow-up-startup-state-and-complexity):
  deferred on 2026-10-18, not cancelled from historical D01–D05 scope. Running still
  means a live process; readiness checks and even a startup observation interval
  remain unimplemented.
- The [AG01 integrated proposal](agent-research.md) remains proposed; engine isolation,
  masking, diagnosis quality, costs/performance and real Mac/WSL integration remain
  unverified. No Agent integration is included in this release.
- The last verified local installed client used the unreleased `12bb89f` build and
  reported 1.5.0; its current version is unconfirmed. Its bytes, backup, unchanged
  configuration and installed appearance/control checks were verified; receipt:
  `/tmp/devctl-tui-install-a7q1d8kb/receipt.json`.
  Publication does not replace it or an already-running supervisor automatically.
- Previous [v1.5.0](https://github.com/Jackie733/devctl/releases/tag/v1.5.0) was published
  from `2574a7febd6075a03e4320d307e48eb59218a3aa`; its
  [final workflow](https://github.com/Jackie733/devctl/actions/runs/39043197453) passed.
  Keep those tags/artifacts intact.
- Daily-project onboarding/hot addition, physical terminal/font appearance on Mac/WSL,
  real user-environment update/rollback and user acceptance remain distinct from
  controlled tests and public artifact checks. A03 remains pending assessment.

## Next steps

Schedule the Node-24 Action-pin maintenance above without changing the published
v1.7.0 tag/assets. Continue the real-environment acceptance below; publication and
CI verification do not establish physical-terminal or user-project acceptance.

1. Verify dragging, actual clipboard paste, wheel paging and terminal restoration
   in the user's Windows Terminal/WSL and native Mac separately, using a new client
   build. The mouse feature requires no supervisor protocol change or service restart.
2. Verify the user's project-addition recovery after an explicit supervisor
   shutdown/reopen; inspect configuration/status before retrying.
3. Verify the new log display in the user's terminal with a newly built client.
4. Continue real-platform acceptance in [roadmap](roadmap.md) and [WSL verification](wsl.md).
5. Resume startup-readiness or AG01 implementation only after explicit scope approval.
