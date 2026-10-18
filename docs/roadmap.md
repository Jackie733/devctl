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
| Native macOS | 1.5.0 adds native lifecycle, clipboard, installation/update/rollback and a Linux/Apple Silicon release pipeline; publication checks and platform acceptance remain distinct in tasks.md | X01, X03 |

The initial MVP was accepted on 2026-10-15. Controlled tests establish coverage of
subsequent work, not final acceptance of those changes in every target environment.
The historical 1.2.2 documentation/license release provided a distinct version for
testing 1.2.1 → 1.2.2 updates. Current outstanding acceptance is listed below;
that historical test opportunity is not evidence of a completed upgrade workflow.

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
| Published, user acceptance pending | v1.5.0 multi-platform distribution | Linux/Apple Silicon build and public download/update verification passed; separate Mac/WSL user acceptance remains pending |
| Pending assessment | A03 database/Redis discovery/control | Reassess concrete need and core boundary before research/design; no longer an automatic next milestone |
| Design assessed; implementation proposed | AG01 Agent-assisted troubleshooting | Evidence-first external-assistant design; staged gates below; no Agent integration implemented |
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
Windows, workspace profiles, interactive services and aggregation remain candidates.
AG01 has a separate proposed sequence below. Existing planning code does not
establish runtime dependency support. A candidate's presence is neither a promise
nor authorization to implement it.

## How work advances

Tie functional changes to confirmed scenarios and requirement IDs. Preserve existing
configuration/planning behavior unless a change is explicitly agreed. Discuss
scope/behavior changes before implementing them; make routine local choices using
existing conventions. Keep current state in the task record and history in Git.

Logs were prioritized before Agent assessment. The AG01 planning request authorizes
research/design; the proposed implementation gates below need a scope decision.
Historical candidates and outstanding X03 acceptance remain intact.

## AG01 proposed delivery gates

These are conditional increments, not scheduled releases. Scope and acceptance are
owned by [AG01](requirements.md#agent-assisted-troubleshooting-ag01); mechanisms by
[DESIGN](../DESIGN.md#agent-assisted-troubleshooting-proposed).

| Gate | Deliverable if authorized | Evidence needed before advancing |
| --- | --- | --- |
| AG0: assessment | Completed research, option comparison, proposed architecture and code-capability checks | Current planning artifacts; no model-quality or end-to-end integration claim |
| AG1: local evidence | One selected-service context packet, preview/masking, stable references and manual assistant handoff | Provenance, limits, stale/offline/error cases and no process side effects; compare with today's manual log copying on real failures |
| AG2: read-only assistant access | Local stdio MCP adapter reusing AG1 collection and disclosure rules | A real assistant can retrieve scoped evidence; malformed/hostile/oversized requests, expiry and host-side permissions verified on Mac and WSL |
| Conditional: launch from devctl | One supported external CLI adapter and cancellable client UI, only if manual handoff is a demonstrated obstacle | Explicit host permissions, version/auth compatibility, no service effects, output validation and useful diagnosis; no general provider framework |
| Separate future assessment | Controlled fixes or service actions | Confirm action scenarios and enforceable authorization/ownership checks; never infer authority from a model response |

AG1 may be sufficient. Advance to AG2 only if follow-up evidence retrieval materially
improves the workflow; a launch button is optional and not a prerequisite for MCP.
Do not schedule autonomous repair, a model SDK inside the supervisor, multi-agent
orchestration, a vector database or remote hosting as part of this proposal.
