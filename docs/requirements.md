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
on 2026-10-15 and released as 1.0.0. On 2026-10-16 the user authorized native
macOS implementation and supplied the local bogu project for real-project
verification. The source target is macOS 14+ on Apple Silicon and Intel; record
actual platform/build/publication evidence separately in [tasks.md](tasks.md).
Native Windows acceptance is not implied.
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

Logging, public release/self-update, and directory-based onboarding/environment
automation are confirmed follow-up directions. Entries marked candidate
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
| X01 | Native macOS operation | Authorized; 1.5.0 source implementation, native install/update/rollback and real bogu verification; platform/release evidence in tasks.md |
| X02 | Native Windows | Candidate |
| X03 | Public distribution and self-update | Public releases and updater delivered; full target WSL acceptance pending |
| P06 | Service PTYs, stdin forwarding, interactive commands | Candidate |
| W05 | package.json/Procfile import and devctl-managed file-watch restart | package.json import is included in A01; Procfile import and file-watch restart remain candidates |
| A01–A03 | Directory onboarding, environment checks and setup, infrastructure discovery/monitoring/control | M7.1 read-only discovery implemented; saving, setup and infrastructure control pending; see [automation](#directory-onboarding-and-environment-automation-a01a03) |
| Configuration extensions | Environment/tilde path expansion, env_files/env overrides | Candidate; existing literal path/environment behavior remains |

An earlier 20-service performance budget is a proposal, not measured evidence.
Existing configuration and planning code remain; unsupported runtime dependencies
must be rejected before startup rather than silently ignored. Historical detailed
proposals are available in Git. Cross-project aggregation, complex filters, and AI
log assistance remain candidates, with logging foundations preceding AI assessment.

### Directory onboarding and environment automation (A01–A03)

Confirmed on 2026-10-16; M7.1 now implements read-only Node project/script
discovery. Configuration saving, environment setup and A03 remain unimplemented;
this partial delivery does not establish complete-workflow or user acceptance.
The priority scenario is: provide an existing project directory, inspect its environment, derive installation/start commands,
and add it to the workspace without hand-writing `devctl.toml`.

| ID | Capability | Observable acceptance |
| --- | --- | --- |
| A01 | Directory discovery and workspace onboarding | Identify projects and startup candidates from repository declarations; save project path, working directory and selected command; repeat discovery without duplicates or overwriting user edits |
| A02 | Environment checks and setup | Report missing/incompatible runtimes, package managers and project dependencies; offer installation with visible commands/results; recheck before starting the project |
| A03 | Database and Redis discovery, monitoring and control | Identify relevant instances and their management source, show availability, and offer supported start/stop controls; relate instances to projects and preserve shared services when stopping a project |

First workflow:

1. Accept a directory and identify its project, or present candidates when it contains
   multiple projects. Read manifests, lockfiles, version declarations and existing
   Compose configuration. Node/npm projects in WSL are the initial research baseline;
   supported ecosystems and directory traversal limits remain to be decided.
2. Derive installation/start commands and working directories from explicit evidence.
   Present ambiguous entry points for selection; do not execute repository commands
   just to discover them. Existing declarations take precedence over guesses.
3. Report environment checks as satisfied, missing, incompatible or unknown. Infer
   infrastructure needs from explicit connection/service configuration; a client
   library alone does not establish an instance, endpoint or required version.
4. Generate workspace configuration, reuse existing entries and preserve user edits.
   Offer setup actions with logs and failures, then recheck and allow normal project
   startup and log inspection. Selecting a directory alone does not authorize installs
   or service changes. The exact CLI/TUI flow remains to be designed.

A03 remains part of the broader automation scope, not cancelled by prioritizing
A01/A02. Research system services, Docker/Compose, devctl-managed processes,
externally started processes and configured remote endpoints. Discovery must also
consider installed but stopped services. A listening port is a clue, not verified
service identity. Display management source and available control capabilities;
discovery or workspace inclusion alone does not imply ownership of a process.
Distinguish installation, process state and application availability, including
unknown/stale checks and authentication failures. A reachable database is not proof
that the project's credentials work. Shared or pre-existing services must not be
stopped implicitly with an individual project; explicit service controls should
identify affected projects. Stopping a database must preserve its data.

Creating databases, executing migrations and generating application/business
configuration are outside this increment. Runtime/dependency installation is in
scope; installing new database/Redis instances, supported installation methods,
privilege handling and infrastructure version isolation still need assessment. Editor setup is
outside this scenario. Historical D01–D05, P03 and R01 proposals remain available;
this discussion does not authorize automatic restart, cascading shutdown or reboot
recovery. Existing configuration and runtime contracts remain until implemented.

A detailed M7 design baseline is now recorded in
[DESIGN.md](../DESIGN.md#planned-directory-onboarding-and-environment-preparation-m7)
and the [planned interaction](interaction.md#planned-directory-workflow-m7). It
proposes bounded Node/npm/pnpm discovery, explicit monorepo selections, exact local
tool bindings, isolated setup and import only while the target workspace is stopped.
The user authorized implementation of this baseline. Only M7.1 discovery has been
implemented; the full matrix, setup/startup policies and real-environment acceptance
remain future evidence. Routine implementation choices need no repeated approval.
M8 still needs database-type, management-source, credential and cross-host/control
research; the M7 design does not settle those separate decisions.

Acceptance must cover a real supplied directory without manual TOML authoring,
missing/incompatible dependencies, ambiguous commands, repeated import preserving
edits, installation failure/retry, and normal start/log/stop behavior. For A03,
include stopped and unreachable instances, authentication failure, unavailable
management permissions, and two projects sharing one service. Tests alone do not
establish real-environment acceptance.

#### M7 acceptance cases

The following cases make A01/A02 observable; mechanisms and numeric budgets remain
in DESIGN.md. Fixture checks must be paired with the real supplied-project workflow.

| Case | Expected result |
| --- | --- |
| Single npm project with one dev script | Preview the actual command, save without hand-written TOML, set up, start/read logs/stop |
| Bogu-style pnpm workspace | Offer root and member alternatives; independently control web/API with one shared install, avoiding duplicate starts |
| Multiple scripts or independent projects | Require explicit selections; never guess an ambiguous executable entry |
| Existing canonical/symlink project alias | Reimport without duplicates; preserve comments, IDs and user edits; replacement is explicit |
| Root/member or manager declaration conflict | Explain conflicting evidence and block apply until resolved |
| Version pin/range conflict, unsupported syntax or ecosystem | Report incompatible/unknown/unsupported; no silent version or manager substitution |
| Missing or incompatible Node/manager | Offer a reviewed exact-version binding/install and recheck before startup |
| Existing shim hangs or attempts activation | Bounded failure; no probe download, shell initialization or indefinite wait |
| pnpm runtime switching/configuration hooks | Read-only checks cannot execute hooks or fetch tools; setup/start either use proven adapter controls or explain unsupported settings |
| Terminal and supervisor have different PATH | Managed service uses the reviewed tools, including scripts' nested invocations |
| Different projects need different versions | Their bindings coexist and do not change legacy services or shell defaults |
| Existing dependencies, stale metadata or removed files | Do not infer correctness solely from directory presence; explain limits and stale evidence |
| Lockfile absent, stale or frozen install fails | Show writes before apply; no silent lockfile-changing retry |
| Installation scripts/native builds needed | Show suppression and incomplete build evidence; script-enabled action requires explicit selection |
| Network/checksum/disk/permission failure | Existing verified tools remain usable; show partial dependency changes and allow a rechecked retry |
| Cancellation/deadline and child processes | Clean up active setup descendants; do not report rollback or leak the install lock |
| Concurrent imports or config changed after preview | One complete valid file or a conflict; no overwritten user edits or partial file |
| Running supervisor/foreground runner | Preview works, apply refuses without stopping services; explicit shutdown/apply/reopen loads the new config |
| Shared dependency root in two managed workspaces | Installation cannot mutate a root held by live managed services; unrelated roots remain usable |
| Private registry configuration | Normal authentication can be used without saving credentials in config or receipts |
| macOS and WSL2 real workflow | Verify each claimed platform independently; Mac-only tests do not establish WSL acceptance |

Research starting points (not selected implementation mechanisms):

- [Compose startup order and readiness](https://docs.docker.com/compose/how-tos/startup-order/)
- [PostgreSQL connection readiness](https://www.postgresql.org/docs/current/app-pg-isready.html)
- [Redis PING](https://redis.io/docs/latest/commands/ping/)
- [WSL systemd support](https://learn.microsoft.com/en-us/windows/wsl/systemd)

### Public releases and self-update (X03)

Users should obtain a supported precompiled version from `devctl update` on a supported platform,
without cloning source or installing Rust for routine updates.

- Publish distinguishable stable versions with source identity, native Linux/Mac
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
- Install native Linux x86_64 musl or macOS arm64/x86_64 programs in `~/.local/bin`.
  Historical Linux-only release availability is documented in the installation
  guide. Hashes trust the same public HTTPS source. Private authentication,
  package-manager distribution, other platforms and publication authorization
  remain separate concerns.

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
project. The current release does not install project dependencies or manage editor
configuration. Planned environment setup is scoped separately under
[A01–A03](#directory-onboarding-and-environment-automation-a01a03).
Background execution is not boot startup, machine-restart recovery, or adoption of
processes created outside devctl. Target-environment verification remains a user
workflow, distinct from controlled tests on a build runner.
