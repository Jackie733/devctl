# Product requirements

This document owns scope, priorities, and acceptance. See [usage](interaction.md)
for operations, [DESIGN.md](../DESIGN.md) for mechanisms, [roadmap](roadmap.md) for
sequence, and [tasks.md](tasks.md) for current evidence.

## Confirmed scenario

Manage independent development projects from one terminal:

| Project | Example development command |
| --- | --- |
| Web application | `npm run dev` |
| Mini-program | `npm run dev:weapp` |
| Node.js API/middleware | `npm run dev` |

These commands may start independently and concurrently; the initial scenario does
not require the API to become ready first. Users must be able to start only one
project from an all-stopped workspace, inspect its logs, and stop it without affecting
others. Leaving the client or closing its terminal keeps managed services running;
ending development is an explicit stop/shutdown action.

The confirmed primary environment is WSL2, Ubuntu 22.04.5 LTS, x86_64, using Linux
runtimes and project directories. The initial complete MVP workflow was accepted
on 2026-10-15 and released as 1.0.0. Native Windows/macOS acceptance is not implied.
Vim-style navigation, compact terminal layout, complete-line copying, and Windows
Terminal/WSL clipboard behavior refine U01/L01 without expanding platform guarantees.

The confirmed U01 visual refinement adds state symbols, distinct focus/selection,
emphasized shortcuts and log modes, and bounded waiting/completion feedback.
Acceptance checks that state remains identifiable when selected or disconnected,
animations preserve log reading positions, and failures remain visible after ordinary
success feedback expires. Actual WSL appearance remains a user acceptance step.

## Initial scope and acceptance

Preserve the original requirement IDs. Post-MVP proposals below do not become
initial-release acceptance criteria merely because their validation code exists.

| ID | Capability | Observable acceptance |
| --- | --- | --- |
| W01 | Multiple project directories and commands | One configuration expresses the independent projects without switching terminals |
| W02 | Path resolution and discovery | Paths resolve from the selected config; absolute/space-containing paths and explicit/default discovery work |
| W03 | Validate before execution | Invalid paths, commands, unknown fields, and references fail before spawning |
| P01 | Individual start/stop | Start one service from all-stopped; repeated start does not duplicate it; stop leaves others alone |
| P07 | Group start/stop | Start independent default-group commands concurrently; stop all includes starting/manual instances |
| P02 | Failure feedback | Identify the project, reason, exit code/signal where available; unrelated projects continue |
| P04 | Reliable stop | Clean the corresponding process group and reap the direct child, escalating after a timeout; leaving the client does not stop services |
| P05 | Background and reconnect | Keep services alive without clients; reconnect to existing state/logs and control it |
| L01 | Read and copy logs | Distinguish stdout/stderr, instances, live state, and retained output; select complete lines and explain copy-channel limits |
| L03 | Bounded logs | High output, long lines, and invalid encoding cannot cause unlimited growth; loss/truncation is visible |
| U01 | Terminal interface | Default entry, project/state/log panels, single/group controls, leave/shutdown distinction, Vim navigation, focus, help, and compact layout |

Engineering constraints:

- R02: one owning runner/supervisor per canonical configuration; concurrent creation
  cannot produce two sets of managed services.
- X01: verify the initial Linux/WSL2 target before claiming other platforms.
- Client exit, disconnect, or crash does not stop services. The client restores its
  terminal; normal supervisor stop signals and handled fatal errors clean services.
- Manage only processes created for the current instance. Deliberately detached
  descendants are outside normal process-group cleanup guarantees.
- Logs, records, channels, and responses are bounded. Concrete limits belong to
  [DESIGN.md](../DESIGN.md), not to a promise of unlimited retention.

## Initial workflow acceptance

1. Open an all-stopped workspace, start only web, then the default group. All commands
   run in the intended directories and web is not duplicated.
2. Read each project's output; do not confuse a live process with application readiness.
3. Stop/restart web while other projects continue, with one new instance only.
4. A missing executable, occupied port, or command failure identifies its source;
   other services keep running and no automatic restart occurs.
5. Leave or close the client terminal; reconnect to the same supervisor and recent
   output without replaying startup commands.
6. Stop all managed processes, including descendants, without leaving this instance's
   ports occupied. Explicit shutdown then ends the background manager.

M1–M6 were accepted for the initial real-project workflow. Automated checks establish
mechanisms; later logging/updater changes still need their own real-environment
acceptance. Do not treat a release or consistent documentation as user acceptance.

## Post-MVP scope

Logging and public release/self-update are confirmed follow-up work. Other entries
remain candidates for assessment; retaining a proposal is not scheduling it, and
simplifying documentation must not silently cancel it.

| ID/scope | Capability | Status |
| --- | --- | --- |
| P05 | Background, leave/reconnect, independent CLI control | Included in the initial release |
| P01, P07 | Dedicated single/group restart shortcuts | Assess after MVP; existing explicit stop/start remains |
| D01–D03 | Dependency execution order, TCP/exec readiness, blocking chains and degradation | Assess after MVP; current independent commands need no readiness dependency |
| P03 | Automatic crash restart, backoff, retry limits | Candidate |
| R01 | Restore workspace/service choices after supervisor exit or reboot | Candidate; reconnecting to a still-live supervisor is already P05 |
| L01, L02 | Paging, in-client reading restoration, pause, literal search and export | Delivered in 1.1.0; target WSL experience pending; no cross-exit bookmarks |
| L04 | Persistent logs, rotation, expiry, cross-supervisor history | Delivered in 1.1.0; target WSL experience pending |
| W04 | Workspace aliases/listing and profiles | Candidate |
| D04, D05 | HTTP probes, one-shot tasks, sustained health checks, cascaded control | Candidate |
| U01, P07 | Multiple services per project and aggregate workspace/project/service UI | Candidate; current one-command-per-project scope remains |
| U02 | Expanded non-TUI foreground presentation | Candidate; existing foreground run and control CLI remain available |
| X01, X02 | Native macOS and Windows | Candidate |
| X03 | Public distribution and self-update | Public releases and updater delivered; full target WSL acceptance pending |
| P06 | Service PTYs, stdin forwarding, interactive commands | Candidate |
| W05 | package.json/Procfile import and devctl-managed file-watch restart | Candidate |
| Configuration extensions | Environment/tilde path expansion, env_files/env overrides | Candidate; existing literal path/environment behavior remains |

An earlier 20-service performance budget is a proposal, not measured evidence.
Existing configuration and planning code remain; unsupported runtime dependencies
must be rejected before startup rather than silently ignored. Historical detailed
proposals are available in Git. Cross-project aggregation, complex filters, and AI
log assistance remain candidates, with logging foundations preceding AI assessment.

### Public releases and self-update (X03)

Users should obtain a supported precompiled version from `devctl update` in WSL,
without cloning source or installing Rust for routine updates.

- Publish distinguishable stable versions with source identity, executable Linux
  artifacts, checksums, and notes in the existing public `Jackie733/devctl` repository.
- Test/build before publication. Generate the update manifest from the artifact;
  upload a complete draft before exposing it as Latest. Normal client discovery
  and downloads must not depend on anonymous REST API quotas.
- Present truthful checking/download/install progress, including known-size download
  percentage and optional speed/ETA. Preserve plain output for redirected/dumb terminals,
  honor NO_COLOR, and retain stage-specific errors without changing update protection.
- Support check-only, latest stable installation, explicit stable versions, and one
  offline rollback. Select the supported artifact and digest without user-supplied URLs.
- Preserve the current program on download/validation failure, retain workspace
  configuration, and work without any workspace config for updater operations.
- Keep a separate bootstrap/external installer. Version 1.0/1.1 lacks the updater;
  1.2.0 API discovery may need external migration; 1.2.1 introduced manifests.
- Clearly separate file replacement from already-running supervisors. Initially users
  explicitly shut down each workspace before switching versions. Automatic discovery
  or restart of all workspaces requires separate assessment.
- Current target is Linux x86_64 musl installed in `~/.local/bin`. Hashes trust the
  same public HTTPS source. Private authentication, package-manager distribution,
  additional platforms, and new publication authorization are separate concerns.

Commands are defined in [self-update](interaction.md#self-update); release mechanics
in [releasing.md](releasing.md); actual results in [tasks.md](tasks.md). WSL acceptance
covers HTTPS access, permissions, installation, rollback, and real project behavior.

### Logging enhancements (L01–L04)

Delivered in 1.1.0 after the initial MVP, with scope refined around daily debugging:

| Scope | Capability | Observable acceptance |
| --- | --- | --- |
| L01 | History paging and reading restoration | Load retained earlier pages; preserve position across project switches/short reconnects within one TUI; a new TUI follows the latest output |
| L01, L02 | Independent pause | Keep capturing/counting new output while a reader's position stays fixed; explicitly resume following |
| L04 | Disk history and old sessions | Query persisted output after supervisor shutdown/recreation without starting services or mixing sessions |
| L03, L04 | Rotation and retention | Bound memory/disk growth; make expired history and invalid cursors explicit |
| L02 | Literal text search and context | Find/highlight retained per-service text without hiding ordinary surrounding records |
| L02 | Export | Copy a selection or redirect a CLI page with clear session, scope, and missing-history feedback |

Users confirmed new TUI clients should follow current output, not restore old
cross-exit bookmarks. Cross-project aggregation and complex combined filters are
lower-priority candidates, not part of this delivered increment. No independent
export panel or multi-format exporter is required.

Verify pause under continuous output, past-memory history, session isolation,
rotation/expiry feedback, search continuation/bounds/context/copy, missing data,
and degraded saving while controls remain responsive. Records may be lost before
capture or before durable sync on abnormal exit; persistence does not guarantee
complete recovery or restore selected/running services. Storage budgets and failure
semantics are owned by [DESIGN.md](../DESIGN.md#persistent-history), and operation
feedback by [interaction.md](interaction.md#logs-and-search).

## Environment and boundaries

The accepted scenario uses Linux Node/npm commands inside WSL; exact versions,
package managers, dependencies, editors, and mini-program tooling belong to each
project. devctl does not install project dependencies or manage editor configuration.
Background execution is not boot startup, machine-restart recovery, or adoption of
processes created outside devctl. Target-environment verification remains a user
workflow, distinct from controlled tests on a build runner.
