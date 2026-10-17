# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: TUI directory completion (A01, W05)

Authorized by the user's request for Tab path completion in the add-project dialog.
Scope: [requirements](requirements.md#directory-onboarding-and-environment-automation-a01a03);
operations: [adding projects](interaction.md#adding-projects);
mechanisms: [TUI design](../DESIGN.md#tui-project-addition).

Completion criteria: Tab completes directories only; unique matches append `/`;
multiple matches extend the common prefix and allow deterministic Tab/Shift-Tab
cycling with visible candidates; support absolute/relative/`~/`, quoted, spaced and
Unicode paths. Preserve input on failure, ignore stale completion results after
editing, and keep typing/cancel responsive. Complete off the UI loop with bounded
single-directory reads and no shell execution/config writes. Enter immediately
after Tab must inspect the completed path. Existing save/hot-add/lifecycle behavior
must pass regression checks. Synchronize docs/Chinese overview, verify installed
Mac client without interrupting the current background, commit locally; no push.

Implemented and agent-verified on 2026-10-17. Reuses the onboarding worker and
dialog, with no new runtime dependency. Results only replace unchanged path input;
typing cancels any queued inspection and clears cached alternatives. Enter during
a completion queues inspection, including initially empty input. Actual discovery,
reviewed save and hot addition retain their existing behavior. The PTY renderer now
accounts for wide Unicode characters so Chinese paths can be inspected accurately.

Validation:

- `cargo test --locked`: 310 tests passed across 15 suites. New checks cover unique
  and shared Unicode prefixes, spaces, sorted forward/reverse cycling, `~/`, relative
  and quoted paths, child completion, files/hidden entries/directory symlinks, missing
  directories/HOME, excessive matches, stale input and queued Enter. Existing TUI,
  hot-add, background, logging, import, update and ownership regressions passed.
- `cargo fmt --check`; `cargo clippy --locked --all-targets -- -D warnings`;
  Linux cross-target clippy with `--target x86_64-unknown-linux-gnu`: passed.
  Linux runtime/user acceptance was not performed on this Mac.
- Real PTY completion-to-save workflow passed with debug and installed release:
  blank Tab/Enter, home/relative/quoted paths, Chinese/spaces, candidate switching,
  failed completion followed by editing, save with the added project stopped.
  Installed candidate display/cancel was inspected; before confirmation no config
  was created. Captured screen: `/tmp/devctl-tab-screen-kvp23406/completion-screen.txt`.
- Markdown links/anchors, shell-example syntax, Python AST, fences and
  `git diff --check`: passed.
- `MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --release --locked`: passed. Native
  ARM64 release installed via `scripts/devctl-update` to `~/.local/bin/devctl`;
  strict signature and macOS 14 minimum checks passed. Installed SHA-256:
  `b8503e4edcd8956c825c78a4408f32957e8c897c4ecbc6c2330858829297cf55`.
  Previous installed `a59be5f` code is retained as `devctl.previous`. Additional
  installation receipt/backups: `/tmp/devctl-tab-install-m0tnixi2`.
  Existing config bytes, service states and background workspace identity were
  unchanged across installation; no workspace restart was needed for this client
  change. Version remains 1.5.0, unreleased.

## Next steps

1. User try the updated TUI: leave an old client with `q`, run `devctl`, then `a`,
   type a partial directory and Tab; Tab/Shift-Tab cycles alternatives. Daily-project
   user acceptance of the complete onboarding/hot-add workflow remains separate.
2. Retain WSL runtime and Mac release/platform gaps below. A03/Agent diagnosis is
   pending assessment; no public push/tag/release.

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
