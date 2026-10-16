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

The initial MVP was accepted on 2026-10-15. Controlled tests establish coverage of
subsequent work, not final acceptance of those changes in every target environment.
The 1.2.2 documentation/license release provides the next real version for testing
1.2.1 → 1.2.2 updates without inventing additional runtime features.

## Next steps

| Priority | Work | Acceptance basis |
| --- | --- | --- |
| 1 | Real WSL log and UI feedback | Follow the [WSL checks](wsl.md#verify-logs) with actual project output, search, copy, reconnect and shutdown |
| 2 | Real WSL cross-version update/rollback | Follow [update verification](wsl.md#verify-updates); confirm paths, versions, backups and retained configuration |
| 3 | Fix demonstrated usability or reliability issues | Reproduce the issue and connect the smallest fix to an existing requirement |
| 4 | M7: directory onboarding and environment preparation (A01/A02, W05 package.json import) | Refine open decisions, then implement the [directory workflow](requirements.md#directory-onboarding-and-environment-automation-a01a03); not yet implemented |
| 5 | M8: infrastructure discovery, monitoring and control (A03) | Research actual database/Redis deployment sources and shared-service behavior before implementation |
| Later | Assess candidates when concrete scenarios justify them | Preserve the complete [candidate list](requirements.md#post-mvp-scope); do not schedule automatically |

Dedicated restart shortcuts may be worth assessing if explicit stop/start becomes
frequent. Dependency orchestration, probes, crash restart, reboot recovery, expanded
platforms, workspace profiles, interactive services, aggregation, and AI assistance
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
