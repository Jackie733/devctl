# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: M7.2 environment checks and local Node binding complete (A02)

Authorized by the user's 2026-10-17 instruction to execute the next M7 slice.
Source remains **1.5.0, unreleased**; M7.1 is committed as `e3a4b73`.
Scope/acceptance: [A02](requirements.md#directory-onboarding-and-environment-automation-a01a03);
operations: [M7.2 usage](interaction.md#local-node-environments-m72).

Completion criteria met: strict optional Node configuration; npm-compatible ranges
including unselected workspace members; separate four-state declarations/tool/binding/
dependency checks; bounded non-activating probes; explicit adoption without downloads;
service-specific tools including nested script calls; complete-batch preflight;
responsive stop/status; CLI and real-terminal confirmation; legacy compatibility;
public English references and Chinese overview updated.

Implemented `doctor`, local-only `setup` preview/apply, private exact-path/version
receipts and launchers, binding/root lifetime locks and managed startup. Existing
dependency directories remain unknown until inventory support; explicit per-start
acknowledgement never bypasses missing dependencies or tool/config failures.
Configuration import, tool/dependency installation and inventory remain pending.
This implementation slice does not establish full M7 or user acceptance.

### Recorded M7.2 validation

- `cargo test --locked`: **286 passed**, including legacy background, lifecycle,
  logs, updates and terminal tests. `cargo test --locked --test environment_cli`:
  **15 passed** again after final checks and repairs.
- `cargo fmt --check`; `cargo clippy --locked --all-targets -- -D warnings`;
  `cargo clippy --locked --all-targets --target x86_64-unknown-linux-gnu -- -D warnings`:
  passed. No Linux/WSL runtime or new physical Intel acceptance is claimed.
- Native fixtures cover incompatible/unknown declarations, shared-root dependencies,
  independent versions, missing/corrupt/changed bindings, live refresh exclusion,
  shim refusal, bounded stalled/flooding probes, interruption/descendant cleanup,
  whole-batch refusal, responsive status/stop and launcher quoting/tool shadowing.
  These cold-start fixtures serialize unrelated tests to avoid competing with
  deadline assertions; multi-service operation is exercised within a fixture.
- A real PTY verifies apply/cancel, declarations changed during review, terminal
  restoration, normal start refusal for unknown dependencies, visible `S`/`y`
  confirmation, clipped-dialog refusal and fresh acknowledgement after stop.
- Disposable projects/private HOME using actual Node **24.18.0**, npm **11.16.0**
  and pnpm **10.3.0** passed doctor, binding preview/apply, managed startup, nested
  Node/npm/pnpm versions, status, stop, restart and shutdown. Project `.bin` spies
  were not executed; manifests stayed unchanged. Corepack's exact cached pnpm CLI
  was used without activation/download; no actual application/data service ran.
- Read-only bogu doctor for web/API found declared pnpm 10.3.0, correctly reported
  unknown dependency evidence and missing local bindings. Manifest/workspace/lockfile
  hashes and Git status were unchanged. No bogu config, dependencies, database or
  running services were changed. The installed devctl binary was not replaced.
- `git diff --check`, 139 local Markdown links/anchors, managed TOML example,
  generated launcher shell syntax and Python helper parsing passed.

## Next steps

1. M7.3: reviewed configuration import/save; then M7.4 tool/dependency installation
   and inventory, and M7.5 complete acceptance. The baseline is authorized; routine
   implementation needs no repeated approval. `import DIR` still only previews.
   Complete real directory-to-running-project and platform acceptance before calling
   M7 delivered. Do not install into bogu implicitly.
2. Preserve public Mac release/platform gaps and real WSL U01/log and X03 update/
   rollback acceptance. Remote CI, macOS 14/physical Intel verification, and anonymous
   public Mac download/update verification remain outstanding. Public 1.4.0 remains
   Linux-only according to the prior handoff; this task did not re-fetch public
   release status. No push/tag/publication.
3. Continue M8 discovery/control research after M7; native Windows and reboot
   recovery remain candidates.

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
