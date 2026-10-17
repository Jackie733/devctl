# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: TUI project addition (A01, W05)

Authorized by the user's request to add projects inside the TUI instead of requiring
CLI setup commands. Keep the 2026-10-17 simplified core boundary: environment checks,
version management and installation remain removed; Agent support is a candidate.
Scope: [requirements](requirements.md#directory-onboarding-and-environment-automation-a01a03);
mechanisms: [TUI design](../DESIGN.md#tui-project-addition);
operations: [adding projects](interaction.md#adding-projects).

Completion criteria: `a` path entry/paste; existing npm/pnpm project/script choices;
manual command fallback with a type label; reviewed atomic save and hot addition; first-use
empty workspace; duplicate/user-edit preservation; no command execution before start;
no environment setup; no running-service interruption; visible/cancelable input and
clipped-screen protection; `A` for default start; real PTY and full regression checks;
reference docs/Chinese overview synchronized; local commit only.

Implemented and agent-verified locally; real daily-project user acceptance remains
pending. Confirmed TUI saves append through the existing supervisor, preserving
running tasks, states, log streams and session. Source/target/live-declaration checks
prevent conflicting saves or silent external changes. Scanning/planning/rechecking
run outside the event loop. Existing CLI active-workspace refusal remains. First-use
save creates an idle workspace. Hot update is limited to reviewed additions; editing
existing declarations or external-file watching is not implemented.

Verified on 2026-10-17:

- `cargo test --locked`: 303 passed across 15 suites, including real PTY first-use,
  paste/edit/cancel/clipped views, npm/pnpm member/script choices, duplicate no-op,
  source/target conflicts, live additions, multiple-client visibility/conflicts,
  unchanged old process/log/session identity and new stream archive/reconnect.
- `cargo fmt --check`; `cargo clippy --locked --all-targets -- -D warnings`;
  Linux cross-target clippy with `--target x86_64-unknown-linux-gnu`: passed.
- A real Node 24/npm project exercised directory-only TUI creation, script review,
  start/logs/leave/reconnect/stop/shutdown without installs or source-file changes.
  Repeated this and the live-add/concurrent-window checks using the installed release.
- Local Markdown links/anchors, shell-example syntax, Python AST, fences and
  `git diff --check`: passed. Dialog output inspected in the PTY screen renderer.
- `MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --release --locked`: passed; installed
  native ARM64 build at `~/.local/bin/devctl`, version 1.5.0, SHA-256
  `a4f128e7c7d19893c38001778870009fca140e8c886a6252c1d0fe74088f6a3d`.
  Strict signature, macOS 14 minimum and login Zsh resolution checks passed. Previous
  source `68ab377` binary retained as `devctl.previous`; additional backups/receipts
  are in `/tmp/devctl-hot-install-dwnrrjsa`.
- Existing Mac workspace was idle before its old supervisor was replaced. Installed
  TUI open/add/cancel preserved its configuration bytes; API/web remain stopped and
  the new idle supervisor uses the normal environment. User config SHA-256 remains
  `167c99bdac6678ab36fa3ece1e04cad76fee67c1c2767fd23e703c4e9ad028a4`.
  This verifies preservation, not renewed Bogu business-flow acceptance.

## Next steps

1. User try the installed `devctl`: `a`, directory, select/edit command, review/save,
   then `s`. Verify daily projects and hot addition while another service is running.
2. Retain separate WSL runtime/user acceptance and Mac release/platform gaps below.
   A03 infrastructure control/Agent diagnosis remain pending assessment, not automatic
   next milestones. No public push/tag/release.

## Retained Mac deployment and release handoff (X01, X03)

Authorized on 2026-10-16, using the user's existing bogu repository. Source version
is **1.5.0, unreleased**. Platform scope and acceptance remain owned by
[requirements](requirements.md#confirmed-scenario); installation is in
[install.md](install.md#macos).

Completion criteria: compile native Mac programs; preserve process-group cleanup,
background persistence/reconnect and independent service controls; verify install,
update/rollback protections and terminal copying; exercise a real project; record
actual evidence and public-release limitations.

- Implemented macOS lifecycle handling, inherited-descriptor cleanup before the
  detached supervisor creates Tokio, and local UTF-8 pbcopy support. Linux process
  cleanup and SSH/WSL clipboard behavior are preserved.
- Implemented system-Bash-compatible Mac installation, native-target manifest
  selection, shared update locks, checksums, candidate deadlines, atomic replacement
  and rollback. Added native build/package and multi-platform merge/public-verify
  jobs; no publishing action has been taken.
- Local platform: Apple Silicon, macOS 26.5.1, Rust 1.97.1. Deployment baseline is
  macOS 14. Intel binaries and tests run locally through Rosetta; this is not
  validation on Intel hardware or macOS 14.
- Installed the Apple Silicon release build in `~/.local/bin/devctl`, configured
  Zsh PATH, and saved the two-service bogu workspace in
  `~/.config/devctl/devctl.toml`. These machine-specific files are not committed.
  Commands select the existing Node 24.18.0 and project-declared pnpm 10.3.0 instead
  of the host's stalled Node shim/unavailable default pnpm. Background job processing
  is disabled in this acceptance workspace; existing PostgreSQL remains external.
- Real bogu verification passed for web HTTP, API health, start idempotence,
  TUI/help, leave/reconnect with unchanged listeners, individual web stop/restart
  preserving API, complete shutdown and release of ports 5174/3001. Existing
  PostgreSQL on 5432 was preserved. No migrations, seeds or business-data actions
  were executed. This is agent verification, not a claim of user acceptance of
  every Bogu business flow.
- A repeated concurrent-start check exposed a supervisor retaining an inherited
  caller pipe. Added a deterministic inherited-pipe regression and fixed Mac startup;
  30 subsequent eight-client concurrency checks passed.

### Recorded Mac validation

- `cargo fmt --check`; `cargo clippy --locked --all-targets -- -D warnings`;
  Linux cross-target clippy with `--target x86_64-unknown-linux-gnu`: passed.
- `cargo test --locked` and
  `MACOSX_DEPLOYMENT_TARGET=14.0 cargo test --locked --target x86_64-apple-darwin`:
  241 tests passed per architecture after the inherited-descriptor fix (Intel via
  Rosetta). No Linux runtime suite was executed on this Mac.
- `python3 tests/support/install_scripts.py`: 14 passed, including lock lifetime,
  stalled candidate, checksum/version failures and destination-directory races.
- `python3 tests/support/merge_release.py`: 5 passed, including missing/duplicate
  platforms, damaged bytes and mixed source bundles.
- Native release builds, `release_artifact.py` and `package_release.py` passed on
  both Mac architectures after the final startup fix. The installed binary matches
  the final Apple Silicon release build, and a fresh login Zsh resolves it. Packaging tests exercise real
  manifest consumption, installation, backup and rollback in temporary HOME.
- Final installed bogu workflow and offline retained logs passed again with the
  saved user configuration. Both managed ports are free and PostgreSQL is preserved.
- `git diff --check`, Markdown links/anchors, Bash syntax, Python compilation and
  YAML parsing passed. Remote workflow execution is not claimed.
- Local Mac bundles are in `target/macos-release-bundles/arm64` and
  `target/macos-release-bundles/intel`, built from implementation commit
  `c564d6a652d5195a61e2e1ea7655b10efe759fff`. Both passed checksum, executable
  version/signature and macOS 14 deployment-baseline checks. Their source archives
  match `git archive` of that exact commit; shared files agree, and the installed
  binary matches the packaged Apple Silicon artifact. The subsequent handoff
  record is documentation-only. No local bundle has been published.
- Final cleanup confirmed no verification supervisor or listeners on 5174/3001;
  the original PostgreSQL process remains running. The bogu Git worktree is clean.

## Existing public release handoff

v1.4.0 is published; implementation `8633e70`, tagged build
`37758491cb7124a7bbc7c36178dc001ac7a6358d`. The recorded
[release run](https://github.com/Jackie733/devctl/actions/runs/38883387229) passed CI
and public artifact/update verification. This historical status was not re-fetched
as part of the Mac implementation task.
