# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: M7 detailed design complete; implementation pending (A01/A02, W05)

Requested on 2026-10-16. Source remains **1.5.0, unreleased**; this task changes
documentation only. Requirement scope remains in
[automation requirements](requirements.md#directory-onboarding-and-environment-automation-a01a03).

Completion criteria: specify inputs/results, bounded discovery and monorepo rules,
script/tool/version selection, environment checks and installation/retry behavior,
config preservation and running-workspace boundaries, planned commands, acceptance
cases and ordered implementation slices. Distinguish proposals from available
features and keep the Chinese overview consistent.

- Recorded the detailed baseline in
  [DESIGN.md](../DESIGN.md#planned-directory-onboarding-and-environment-preparation-m7),
  proposed operations in [interaction](interaction.md#planned-directory-workflow-m7),
  [acceptance cases](requirements.md#m7-acceptance-cases), and
  [implementation slices](roadmap.md#m7-implementation-slices).
- Proposed first support: Node with npm 10/11 or pnpm 10 on native Mac and
  Ubuntu/Linux/WSL x64. Package ecosystems beyond that matrix remain assessable.
  Proposed import preserves edits and requires an inactive target workspace;
  installation is separately reviewed, with exact local tool bindings and visible
  script/lockfile policies. These product choices need review before implementation;
  an explicit instruction to implement this baseline supplies that review/authorization
  without a repeated approval question. No candidate feature was implemented.
- Read existing config/CLI/launch/lock behavior and inspected only bogu manifests/
  workspace declarations. Confirmed root/child startup overlap and one shared pnpm
  install root; no Node pin in those manifests. No bogu files, machine configuration,
  tools, dependencies or service states were changed.
- Consulted upstream Node/npm/pnpm/Corepack documentation; references are kept in
  DESIGN.md. A full M8 database/Redis design remains separate and requires source/
  credential/shared-service research; it is not cancelled.

Validation performed for this documentation change:

- `git diff --check`: passed.
- `python3` inline documentation checker: passed for all seven changed Markdown
  files, 92 relative links/anchors, closed code fences and ten TOML/shell examples
  (`tomllib.loads` and `bash -n`; examples were parsed, not executed). Checked
  design topic coverage and explicit not-implemented labels.
- Reviewed the complete new design/operation sections and cross-document scope,
  platform, installation, cancellation and active-workspace behavior.
- No Rust functionality changed; cargo tests/clippy were not rerun. No planned
  command or installation was executed; runtime/real-environment acceptance remains
  future implementation work.

## Next steps

1. Review the concrete M7 baseline, then implement M7.1 through M7.5 in order.
   No discovery/import/doctor/setup command exists yet. Complete real supplied-project
   and platform acceptance before calling M7 delivered.
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
