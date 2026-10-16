# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: update progress implemented, unreleased (X03)

- The user confirmed the three-stage terminal design: checking, downloading, and
  verifying/installing; real bytes/percentage/speed/ETA; compact width, plain redirected
  output, NO_COLOR, and explicit success/failure. Implement locally and commit this
  task; no new publication or push is assumed.
- Added a private progress renderer using existing Crossterm/Tokio dependencies.
  It writes transient status to stderr, preserves stdout results and original child
  diagnostics, animates during waiting, and never changes terminal modes. Known-size
  download progress counts successful file writes. Check/no-op and rollback show
  only their actual stages; installer substeps are not invented.
- Presentation errors do not decide update outcomes. Existing source/size/checksum/
  version/install/backup protection stays in place. Child output is streamed through
  bounded chunks and pauses animation so partial diagnostics are not overwritten.
- Mechanisms are recorded in [DESIGN.md](../DESIGN.md#progress-presentation), the user
  contract in [interaction.md](interaction.md#progress-display-development-version),
  and the confirmed requirement under [X03](requirements.md#public-releases-and-self-update-x03).

## Validation

- All existing update CLI cases passed with the new output path. New real PTY coverage
  passed interactive, NO_COLOR, TERM=dumb, stdout/stderr redirection, narrow terminal, failed
  download/partial diagnostic, check-only, no-update, and offline rollback modes.
- Renderer tests check actual byte/time-derived statistics, unknown-total behavior,
  width bounds, and animation frame changes. A process-output bound test is included
  to verify an exited installer cannot be held indefinitely by an open output pipe.
- Final `cargo test --locked --offline`: 239 passed (145 unit, 94 integration),
  including the ten existing TUI PTY scenarios and ten updater PTY modes. The latter
  covers both stdout-only and stderr-only redirection and a final diagnostic without
  a newline. Log: /tmp/devctl-update-progress-tests.log.
- `cargo fmt --check` and `cargo clippy --locked --offline --all-targets -- -D warnings`
  passed. Musl static build, actual artifact static-link/install/reinstall/bad-digest/
  rollback checks, four packaging/manifest tests, and eleven installer tests passed.
- The actual musl program also passed interactive PTY verification with a controlled
  4 MiB download. A real-byte frame showed 50%, 2.0/4.0 MiB, 4.0 MiB/s, ETA 1s.
  Capture: /tmp/devctl-update-progress-preview-2x2jgpri/terminal-output.txt. Its selected
  version and download source are test fixtures, not a new public release.
- All 15 Markdown documents, 104 local links/anchors, code fences, Python syntax,
  and final diffs passed checks. Changes are recorded under Unreleased. No dependency,
  Cargo version, workflow, remote tag, published asset, or real installation changed.

## Previous release and user handoff

- [v1.2.2](https://github.com/Jackie733/devctl/releases/tag/v1.2.2) remains the published
  Latest; [run 38871736455](https://github.com/Jackie733/devctl/actions/runs/38871736455)
  passed build/publication/anonymous verification. Its build source is
  19d79f024af983d8a236adf6fa7dedcfcb905ed6; the real 1.2.1 → 1.2.2 default upgrade and
  rollback were verified in a temporary HOME. No published artifact/tag is changed.
- The new progress UI requires a future release or a local source build. This task
  does not modify the user's real installation or stop real workspaces. Target WSL
  visual/use acceptance and overall X03 acceptance remain separate from controlled tests.
