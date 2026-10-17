# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: publish v1.5.0 (X03; A01, W05, X01)

Authorized on 2026-10-17 to publish the new feature version and push GitHub.
Release includes TUI project addition, append-only hot update, Tab directory
completion, lightweight CLI onboarding and native Mac distribution. Cargo package
and lock already use 1.5.0; previous public latest is v1.4.0.

Completion criteria: dated changelog and matching EN/ZH/download docs; clean source
commit pushed without overwriting remote changes; both supported platform build/test/
package checks pass; version tag matches that tested commit; publish complete
binary/installer/source/manifest/checksum assets; verify public downloads, anonymous
updates and rollback on every supported platform. Preserve local user workspaces.

Release scope updated at the user's explicit request: do not build/support Intel
Mac. Cancelled the initial three-platform preflight run `39040877432`; remove Intel
from CI, packaging, native update targets and public platform documentation before
rerunning. No version tag or release has been created.

History fixtures now require a receiver acknowledgement after each 32 records.
Hosted Mac timer coalescing and concurrent load made time-paced exact-count inputs
unreliable (including bounded-queue drops); waiting longer alone was insufficient.
The logs query is ordered behind archive writes, so it drains both memory/disk
queues without changing product backpressure. Preserve all exact-count, paused
anchor, eviction and disk restoration assertions; high-output tests retain overload
coverage. Reverify final source before publishing. The first v1.5.0 tag attempt did
not publish; any tag correction must first confirm no public/draft release exists
and use the exact prior tag identity to protect concurrent changes.

Cross-platform archive parity check confirmed identical Git tar bytes but different
native gzip encodings. Compare decompressed source bytes in the merger while still
verifying each bundle checksum and requiring exact installer/build-note equality;
reject invalid gzip and differing source contents. Recheck real final bundles before
tagging; no release has been published yet.

Preparation in progress. Latest implementation verification passed 310 local tests,
format/clippy and Linux cross-clippy; installed client is from `9f75dfe`. This release
must also pass hosted Linux and Apple Silicon checks. CI does not establish
WSL terminal/user acceptance or acceptance on physical macOS 14 hardware.

## Next steps

1. Push release preparation, run the Linux/Apple Silicon workflow without
   publication, fix any failures, then tag/publish the tested commit and verify the
   public release. Record actual workflow/release links and source identity.
2. Keep real daily-project/WSL and Mac platform acceptance gaps separate. A03/Agent
   diagnosis remains pending assessment; no additional feature implementation.

## Retained Mac deployment and release handoff (X01, X03)

Authorized on 2026-10-16, using the user's existing bogu repository. Historical local validation below predates the 1.5.0 publication work above. Platform scope and acceptance remain owned by
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
