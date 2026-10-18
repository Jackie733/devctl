# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## In progress: publish 1.6.1 (U01 / M5, X03)

Authorized on 2026-10-18: defer startup-readiness work and publish the existing TUI
visual refinement. No startup observation interval, readiness detection, framework
adapter or new service-state meaning is included.

Scope: remove start/stop state flashes; coherent dark/light/terminal palettes;
stable selection through focus changes; whole-action footer hints; current-search
underline; reduced motion, ASCII status symbols and nonempty `NO_COLOR` support.
No new runtime dependencies. Native release checks now exercise the appearance
scenario against each optimized binary as well as the existing terminal scenarios.

Completion criteria: version/doc consistency, relevant local checks, clean committed
source on main, a new annotated v1.6.1 tag, successful Linux/Apple Silicon build and
publication, both anonymous public-verification jobs, exact tag/build/asset identity,
and a real 1.5.0 → 1.6.1 update/rollback in an isolated home. Preserve prior releases.

### Validation and release state

- Prepared Cargo 1.6.1, matching lockfile, dated changelog and current install/version
  examples. Researched startup-readiness remains explicitly deferred by the user.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` and
  `cargo test --locked`: passed on local Apple Silicon; 315 tests total.
- `python3 tests/support/install_scripts.py`: 14 passed;
  `python3 tests/support/merge_release.py`: 8 passed.
- `MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --release --locked`: passed; strict
  signature verification passed. `release_artifact.py ... 1.6.1` passed native
  dependencies/install/reinstall/bad-checksum/rollback/config preservation.
- `python3 tests/support/package_release.py target/release/devctl`: 4 passed;
  `python3 tests/support/tui_pty.py target/release/devctl appearance`: passed.
- `git diff --check` and Markdown link/fence checks passed: 17 documents, 172 local
  links/anchors, zero errors.
- Initial v1.6.0 tag `bf22375` failed before publication in
  [run 39135559760](https://github.com/Jackie733/devctl/actions/runs/39135559760):
  Linux passed; Mac optimized lifecycle fixture stopped consuming terminal output
  while polling CLI state and could block its own stop key. Neither release nor
  anonymous verification ran. Preserve that tag; it was not published as a Release.
- Reproduced output backpressure locally: a large high-output redraw kept the stop
  key pending until the fixture drained the PTY. Fix state/start polling to drain live
  clients, preserving deadlines/assertions. Extend the existing high-output scenario
  to exercise stopping after a redraw exceeding the terminal queue.
- v1.6.1 includes the test-harness correction; runtime behavior is unchanged beyond
  the original visual refinement. Local optimized appearance, high-output/large-redraw
  stop and lifecycle scenarios passed, alongside native artifact and package checks.

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

1. Complete the authorized 1.6.1 release and record actual results before claiming
   publication or public update compatibility.
2. Continue real-platform acceptance in [roadmap](roadmap.md) and [WSL verification](wsl.md).
3. Resume startup-readiness or AG01 implementation only after explicit scope approval.
