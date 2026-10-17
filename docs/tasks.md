# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: M7.3 reviewed configuration import implemented (A01, W05)

Authorized by the user's instruction to continue after M7.2 (`e84c3dd`). Source
remains **1.5.0, unreleased**. Scope: [A01](requirements.md#directory-onboarding-and-environment-automation-a01a03);
mechanisms: [configuration transaction](../DESIGN.md#configuration-transaction-and-supervisor-boundary);
operations: [M7.3 usage](interaction.md#reviewed-configuration-import-m73).

Completion criteria met: explicit/guided project and script selections; exact
version choices and source compatibility; stable canonical-directory identities;
new/existing config plans; no-write previews; preserved comments/user edits and
no-op bytes/inode; explicit replacements; source/target rechecks; atomic publication;
active-workspace refusal; import/start race coverage; real-terminal and real-Node
verification; public references and Chinese overview synchronized.

Import saves only reviewed additions/replacements. It never installs, adopts tools,
starts services or edits repository files. Existing legacy commands remain unchanged
without explicit replacement. Tool/dependency installation and inventory remain M7.4;
full M7/platform/user acceptance is not claimed.

### Recorded M7.3 validation

- `cargo test --locked`: **307 passed** across 16 suites. Includes 16 configuration
  import CLI tests, four transaction tests, the existing discovery/legacy lifecycle/
  background/TUI/update suites and a new tool-pair compatibility regression.
- `cargo fmt --check`; `cargo clippy --locked --all-targets -- -D warnings`;
  `cargo clippy --locked --all-targets --target x86_64-unknown-linux-gnu -- -D warnings`:
  passed. No Linux/WSL runtime, physical Intel or macOS 14 acceptance was added.
- Coverage includes new/default destinations, missing versions, ambiguity, shared
  installation roots, unselected member constraints, stable collision IDs, aliases,
  preserved manual fields/inline comments, replacements, no-op inode preservation,
  removed manifests, incompatible selected tools, malformed inputs, special/oversized
  files, hardlinks, sidecar failures and concurrent creators/editors/parent changes.
- Real PTY coverage: project/script selection, disabled libraries, exact-version
  prompts, clipped-screen refusal, confirmation/cancel, signal cleanup, source and
  target changes while reviewing, terminal restoration and no writes on refusal.
- A deterministic gated-read test replaces config during foreground/supervisor
  preflight: only the newly committed command executes. Final declarations are
  reread under ownership; initial invalid configs still fail without sidecars.
- Disposable projects/private HOME with actual Node **24.18.0**, npm **11.16.0** and
  pnpm **10.3.0** passed import preview/apply without manual TOML, quoted doctor/setup
  hints, explicit local binding, start/status/stop/restart, TUI leave/reconnect with
  unchanged instance, shutdown and unchanged repeat import. Spaces/apostrophes in
  project/config paths worked, nested tool versions stayed bound and project `.bin`
  spies were not executed. No package downloads/dependency installation occurred.
- Read-only bogu planning generated web/API additions sharing its declared pnpm
  version/root in a disposable destination preview. A separate preview against the
  user's existing config preserved its web/API IDs, custom commands and autostart.
  Config bytes, manifest/workspace/lockfile hashes and bogu Git status were unchanged.
  No bogu services, dependency files, database or installed devctl were modified.
- Local Markdown links/anchors, balanced fences, managed TOML and Python helper
  parsing, nine shell documentation blocks and `git diff --check` passed.
  `toml_edit` 0.25.17 is MIT OR Apache-2.0; lockfile changes are scoped to this editor
  and its already present dependencies' required features.

## Next steps

1. M7.4: isolated tool/dependency installation, policy, progress/cancellation and
   inventory; then M7.5 complete acceptance. The baseline is authorized; routine
   implementation needs no repeated approval. Import saving/local binding exist;
   downloads and dependency installation do not.
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
