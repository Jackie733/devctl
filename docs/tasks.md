# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: M7.1 read-only project discovery implemented (A01, W05)

The user authorized implementation of the detailed M7 baseline on 2026-10-16.
Source remains **1.5.0, unreleased**. This coherent slice implements discovery only;
M7.2–M7.5 remain necessary for the complete A01/A02 workflow. Scope/acceptance are in
[requirements](requirements.md#directory-onboarding-and-environment-automation-a01a03),
mechanisms in [DESIGN.md](../DESIGN.md#discovery-and-evidence), operations in
[interaction](interaction.md#read-only-project-discovery-m71), and slice order in
[roadmap](roadmap.md#m7-implementation-slices).

Completion criteria: expose a config-independent read-only preview; identify single
packages, independent directories and declared workspace members; report actual
scripts/sources, cwd and shared dependency roots; preserve ambiguity/conflict and
unsupported evidence; enforce filesystem/size/time bounds; execute no repository
commands or tools and write no files; verify real bogu discovery and legacy behavior.

- Added `devctl import DIR` with bounded manifest/version/lockfile evidence, npm/pnpm
  workspace membership, canonical paths and deterministic ordering. New dependencies
  are globset and serde_yaml_ng, locked to the reviewed versions; no shell or runtime
  is needed for discovery. Scripts are displayed but never executed.
- Root/member command overlap is explicitly advisory; multiple startup candidates
  require later selection. Stop/restart, migration, seed, build and similar action
  components are excluded from startup suggestions, including bogu's `dev:stop`.
  Missing manager/version information remains unknown/unresolved, not fabricated.
- Declared patterns prune unrelated trees and stop below members where no recursive
  member can exist. A first real bogu scan exposed irrelevant-directory exhaustion;
  prefix traversal fixed it without raising limits, with representative regression
  coverage. Symlink directories are not followed; contained declaration links are
  supported, while escaping/broken/special declarations fail clearly.
- Full import selection/saving, doctor/setup, managed runtime binding and startup
  changes are not available. `--apply` is rejected; explicit `--config` is explained
  rather than ignored. Discovery does not load or compare existing workspace config.
- Real bogu root scan passed: root plus API, web, domain, extension and knowledge-agent
  (six candidates); pnpm 10.3.0 inherited from root; shared install root and overlapping
  root/member commands reported. Separate web/API scans passed. Manifest/workspace/
  lockfile hashes and bogu Git state were identical before/after. No business-data,
  dependency, installed-devctl or machine-config changes were made.

## Validation

- Focused discovery checks: 19 unit tests and seven real CLI tests passed, including
  fake executables/scripts that must never run, no config requirement, no generated
  files, declaration conflicts, malformed/unsupported evidence, YAML restrictions,
  symlink/special files, terminal controls and every scan budget.
- `cargo fmt --check`: passed. Native `cargo clippy --locked --all-targets -- -D warnings`:
  passed. Full final `cargo test --locked`: **267 passed**, including 19 discovery
  unit tests and seven discovery CLI tests (output retained at
  `/tmp/devctl-m71-cargo-test.log`).
- Real `target/debug/devctl import` on bogu root/web/API passed (approximately 8/5/5 ms
  in the recorded run); the test inspected output and source hashes/Git status.
- `cargo clippy --locked --all-targets --target x86_64-unknown-linux-gnu -- -D warnings`:
  passed after the final no-Git declaration-boundary refinement.
- New dependency licenses verified through `cargo metadata --locked --format-version 1`:
  globset 0.4.20 is Unlicense OR MIT; serde_yaml_ng 0.10.0 is MIT.
- `git diff --check` and the inline Python documentation checker passed: eight
  Markdown files, 95 local links/anchors, 11 TOML/shell examples (parsed with
  tomllib/bash -n, not executed), and closed fences. Reviewed the final code/diff
  and current-versus-planned feature labels.
- This is agent verification of read-only discovery, not user acceptance of complete
  onboarding/setup. No Linux/WSL runtime acceptance or publication is claimed.

## Next steps

1. Continue M7.2 (environment checks and exact tool binding), then M7.3 config
   import, M7.4 setup and M7.5 complete acceptance. The baseline is authorized; do
   not ask again for routine implementation. Only read-only `import DIR` exists.
   Complete real supplied-project and platform acceptance before calling M7 delivered.
2. Preserve public Mac release/platform gaps and real WSL U01/log and X03 update/
   rollback acceptance. Remote CI, macOS 14/physical Intel verification, and anonymous
   public Mac download/update verification remain outstanding. Public 1.4.0 remains
   Linux-only according to the prior
   handoff; this task did not re-fetch public release status. No push/tag/publication.
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
