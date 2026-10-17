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

The user revised M7 on 2026-10-17: keep lightweight onboarding and remove
core environment inspection/version management/installation. Earlier discovery and
configuration work is retained where it supports that boundary; the M7.2 bindings
and planned M7.4 installer are superseded. Git retains the original design/history.

| Order | Work | Acceptance basis |
| --- | --- | --- |
| Implemented, acceptance pending | Simplify A01 onboarding and remove A02 core environment management | TUI path/script/command/save, explicit CLI alternatives, no environment management; append-only hot update and lifecycle checks |
| Following | Verify lightweight onboarding with real daily projects | Directory/command to ordinary TUI start/log/stop/leave/reconnect; record separate Mac/WSL and user acceptance |
| Outstanding | Real WSL log/UI and cross-version update/rollback | Preserve [log](wsl.md#verify-logs) and [update](wsl.md#verify-updates) checks |
| Outstanding | Native Mac publication/platform evidence | Keep the release/platform handoff in tasks.md; no automatic push/tag/publication |
| Pending assessment | A03 database/Redis discovery/control | Reassess concrete need and core boundary before research/design; no longer an automatic next milestone |
| Candidate | Agent-assisted diagnostics and solutions | Assess an actual failure workflow using existing status/log/control commands; no new Agent integration implemented |
| As demonstrated | Reliability/usability fixes | Reproduce issues and tie the smallest fix to an existing requirement |

### M7 implementation boundary

A01 includes TUI path/script/custom-command addition, language-independent CLI `add`,
bounded optional npm/pnpm script `import`,
selection, canonical IDs, preserved edits and atomic configuration saves. Commands
run in the existing environment; onboarding does not install or validate versions.
A02 tool binding, dependency inventory and installation are removed from core scope.
See [acceptance](requirements.md#m7-acceptance-cases),
[mechanisms](../DESIGN.md#lightweight-project-onboarding-m7) and
[usage](interaction.md#adding-projects). Tests establish implementation evidence;
real target-platform workflows and user acceptance remain distinct.

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
