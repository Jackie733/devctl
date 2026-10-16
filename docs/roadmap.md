# Roadmap

The main goal is one terminal for independent local development projects. Scope is
owned by [requirements.md](requirements.md); current work and evidence live in
[tasks.md](tasks.md). Delivered, awaiting acceptance, and candidate work are distinct.

## Delivered milestones

| Milestone | Result | Requirements |
| --- | --- | --- |
| M1: configuration | Project/command schema, path resolution, discovery, preflight validation | W01–W03 |
| M2: single-service lifecycle | Process-group stop, descendant cleanup, timeout escalation and exit feedback | P01, P02, P04 |
| M3: multiple services | Concurrent independent commands, selected/group control and run lock | P01, P07, P04, R02 |
| M4: background manager | Detached supervisor, bounded local IPC, discovery, reconnect and background logs | P05, R02, P04, L01, L03 |
| M5: terminal interface | Project/state/log panels, controls, leave/shutdown distinction and terminal restoration | U01, P01, P07, P05, L01, L03 |
| M6: real-project acceptance | Three-project WSL2 workflow accepted; MVP released as 1.0.0 | X01, initial scope |
| Logging follow-up | 1.1.0 delivered history, persistence, pause, literal search, context and page/selection export | L01–L04 |
| Public distribution | Public Releases, static artifacts, checksums, source identity and tested publication workflow | X03 |
| Self-update | 1.2.0 introduced independent update/rollback; 1.2.1 switched to Release manifests | X03 |
| Native macOS source | 1.5.0 adds native lifecycle, clipboard, installation/update/rollback and a three-platform release pipeline; publication and platform evidence remain in tasks.md | X01, X03 |

The initial MVP was accepted on 2026-10-15. Controlled tests establish coverage of
subsequent work, not final acceptance of those changes in every target environment.
The 1.2.2 documentation/license release provides the next real version for testing
1.2.1 → 1.2.2 updates without inventing additional runtime features.

## Next steps

The user requested M7 detailed design on 2026-10-16 after local Mac deployment.
Its design baseline is recorded; functional implementation remains pending. Existing
Mac publication and historical WSL acceptance work remain explicit follow-ups in
[tasks.md](tasks.md), rather than prerequisites that silently block this design.

| Order | Work | Acceptance basis |
| --- | --- | --- |
| Current | M7 design (A01/A02, W05 package.json import) | Detailed [mechanisms](../DESIGN.md#planned-directory-onboarding-and-environment-preparation-m7), [interaction](interaction.md#planned-directory-workflow-m7), acceptance cases and implementation slices recorded; proposed product choices await review |
| Next | M7 implementation | Advance through the slices below; the milestone is complete only after the whole directory-to-running-project workflow |
| Following | M8 (A03) | Research actual database/Redis sources, credentials, stopped instances and shared-service control before detailed implementation design |
| Outstanding | Real WSL log/UI and cross-version update/rollback | Preserve the [log](wsl.md#verify-logs) and [update](wsl.md#verify-updates) checks with real projects |
| Outstanding | Native Mac publication/platform evidence | Preserve the release and platform handoff in tasks.md; no automatic push/tag/publication |
| As demonstrated | Reliability/usability fixes | Reproduce issues and connect the smallest fix to an existing requirement |
| Later | Assess candidates when concrete scenarios justify them | Preserve the [candidate list](requirements.md#post-mvp-scope); do not schedule automatically |

### M7 implementation slices

These are incremental local implementation/commit boundaries, not independent
claims of user acceptance. Each slice preserves legacy behavior and has focused
checks; later slices must not be described as already implemented.

| Slice | Deliverable | Required evidence before advancing |
| --- | --- | --- |
| M7.1: discovery | Bounded manifest/workspace scan, candidates, source/conflict reporting and read-only import preview | Single package, bogu-shaped workspace, ambiguity, exclusion, symlink/limit and no-command-execution fixtures |
| M7.2: environment binding | Optional Node config fields, version semantics, bounded doctor checks and service-specific launch binding | npm-range cases, stalled shims, compatible local tools, different per-project versions, nested script tools and unchanged legacy execution |
| M7.3: configuration import | Selection/replacement flow, stable IDs, comment-preserving atomic writes and active-workspace refusal | New/existing config, repeat/no-op, edits preserved, path aliases, simultaneous import and supervisor-start races |
| M7.4: setup | Isolated verified tool install, shared-root dependency actions, progress/cancellation/retry and post-checks | Temporary HOME and disposable projects; checksum/network/permission failures, script policy, lockfile policy, shared-root locks and cleanup |
| M7.5: complete acceptance | Full CLI onboarding into ordinary TUI start/log/stop/leave/reconnect | All [M7 cases](requirements.md#m7-acceptance-cases), actual supplied bogu directory and separate Mac/WSL evidence; no business-data mutations |

Run relevant tests, `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings`
for functional changes; broaden the suite for launch/config integration. Re-run
legacy lifecycle/background/TUI checks when touching their boundaries. Record real
commands/results and limitations in tasks.md, commit each coherent slice, and do
not push automatically.

Dedicated restart shortcuts may be worth assessing if explicit stop/start becomes
frequent. Dependency orchestration, probes, crash restart, reboot recovery, native
Windows, workspace profiles, interactive services, aggregation, and AI assistance
remain candidates. Existing planning code does not establish runtime dependency
support. A candidate's presence is neither a promise nor authorization to implement it.

## How work advances

Tie functional changes to confirmed scenarios and requirement IDs. Preserve existing
configuration/planning behavior unless a change is explicitly agreed. Discuss
scope/behavior changes before implementing them; make routine local choices using
existing conventions. Keep current state in the task record and history in Git.

Logs were prioritized before AI assessment. This documentation work does not expand
AI scope, silently remove historical requirements, or equate a published release
with acceptance of the full X03 user workflow.
