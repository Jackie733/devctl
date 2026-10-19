# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: local log times and stable run labels (L01 / U01, M5)

Confirmed user case on 2026-10-19: the TUI capture time should match the client's
local timezone; restarting a service should not relabel historical records as `old`.
Use compact `[out #N]` / `[err #N]` row prefixes, omit ordinary row sequence numbers,
and identify the latest known run in the header. Preserve navigation, search,
copying, retained records and CLI/storage identities.

Implemented without new dependencies: use the client's system local-time conversion
at capture time, keep immutable run labels and preserve existing record anchors.
Tests cover UTC, UTC+08:00, UTC+05:45, DST/date rollover, missing capture times,
narrow panes and stable labels. Existing PTY checks now inspect fixture bodies
instead of relying on removed display sequence numbers.

Validation on Linux:

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked --test tui_cli -- --quiet`: passed, 17 real-terminal tests.
- `cargo test --locked -- --quiet`: passed, all 319 tests. The initial full attempt
  hit an unrelated discovery boundary assertion; its isolated rerun and the complete
  rerun passed. Discovery code was not changed.
- `git diff --check`: passed. Inspected the diffs and checked balanced code fences
  and all 63 local link paths across the four changed reference/task documents.

Real user-environment and macOS acceptance remain pending; no release or installation
is included.

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

1. Verify the new log display in the user's terminal with a newly built client.
2. Continue real-platform acceptance in [roadmap](roadmap.md) and [WSL verification](wsl.md).
3. To switch the actual local development installation to the public 1.6.1 bytes,
   shut down active workspaces/clients first, then use the normal updater.
4. Resume startup-readiness or AG01 implementation only after explicit scope approval.
