# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: lightweight onboarding simplification implemented (A01/A02, W05)

Authorized by the user's 2026-10-17 instruction to implement the discussed reduction.
Source remains **1.5.0, unreleased**. Scope:
[requirements](requirements.md#directory-onboarding-and-environment-automation-a01a03);
mechanisms: [DESIGN](../DESIGN.md#lightweight-project-onboarding-m7);
operations: [adding projects](interaction.md#adding-projects).

Completion criteria: language-independent directory/argv addition; optional npm/pnpm
script import without version selection or tool/dependency checks; remove doctor,
setup, managed bindings, environment launch preflight and dependency confirmation
controls; preserve reviewed/atomic config saves, comments, identity/no-op behavior,
active-workspace refusal and import/start race protection; explicit rejection of
retired fields/options; verify normal start/log/stop/leave/reconnect; synchronize
English references and Chinese overview; local commit only.

Completion criteria met in source: explicit `add`, command-only optional import,
removed environment subsystem/launch gates/CLI/TUI options, safe configuration
transactions and explicit retired-field migration guidance. The node-semver direct
dependency and its unused transitive dependencies are removed. English reference
documents and the Chinese overview agree. No Agent integration was introduced.
This is implementation/agent verification, not full target-platform/user acceptance.
No installed devctl, user project/configuration or private tool binding was changed.

### Recorded validation

- `cargo test --locked`: **292 passed** across 15 suites, including 20 import/add
  CLI tests and retained lifecycle/background/TUI/logging/update checks. Dedicated
  environment tests were removed with their subsystem; relevant import tests now
  verify the reduced behavior. New checks cover literal argv, absent/broken manifests
  on explicit add, missing local tools/version constraints, inherited command execution,
  name conflicts, retired fields/options and same-manager differing versions.
- Real PTY import/add selection/confirmation/cancel/signal/resize/source/target
  changes passed, including both explicit-add confirmation outcomes. Existing
  import/start race, concurrent save, alias, comment, no-op inode, active/idle
  ownership and transaction failure tests remain green.
- `cargo fmt --check`; `cargo clippy --locked --all-targets -- -D warnings`;
  `cargo clippy --locked --all-targets --target x86_64-unknown-linux-gnu -- -D warnings`:
  passed. Linux/WSL runtime, physical Intel and macOS 14 acceptance were not added.
- `python3 /tmp/devctl-slim-real.py`: actual Python, npm and pnpm projects in a
  private HOME/temp directory passed preview/save/start/idempotence/log viewing,
  real TUI leave/reconnect, normal stop/restart/shutdown and unchanged repeat saves.
  Spaces/apostrophes worked; source files stayed unchanged and no project dependencies
  were installed. A deliberately nonexistent pnpm version in the initial fixture
  produced its own manager error in retained logs; corrected declared versions passed.
  Configured commands retain their own behavior, including possible network actions.
- Read-only bogu web/API import passed with no installed-tool/version selections.
  Source manifests/workspace/lockfile, actual user-config hashes and bogu Git status
  were unchanged; no bogu services or database actions ran.
- `python3 /tmp/devctl-slim-doccheck.py`: **130** local Markdown links/anchors,
  **28** shell examples (`bash -n`), Python AST, balanced fences and `git diff --check`
  passed. No performance improvement is claimed without separate measurements.

## Next steps

1. Gather daily-project/user acceptance for the simplified flow; preserve separate
   real WSL log/UI and update/rollback acceptance plus Mac release/platform gaps below.
   Remote CI, macOS 14/physical Intel verification and anonymous public Mac
   download/update remain outstanding. No automatic push/tag/publication or installed
   binary replacement.
2. A03 database/Redis control and Agent-assisted diagnosis remain pending assessment,
   not automatic next milestones. Native Windows/reboot recovery remain candidates.
   A02 installation/version management is explicitly removed from core scope.

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
