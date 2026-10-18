# Product requirements

This document owns scope, priorities, and acceptance. See [usage](interaction.md)
for operations, [DESIGN.md](../DESIGN.md) for mechanisms, [roadmap](roadmap.md) for
sequence, and [tasks.md](tasks.md) for current evidence.

## Reading scope and status

This document owns product scope and supported platforms. Other documents describe
operations, mechanisms, sequencing or evidence, and refer here when repeating a
rule is unnecessary. The English and Chinese READMEs summarize the same scope.

- **Delivered** means implementation is available; publication and automated
  checks do not establish acceptance of every target-platform workflow.
- **Acceptance pending** identifies implemented workflows still needing real
  project/platform verification or user acceptance.
- **Candidate / pending assessment** preserves a proposal without scheduling or
  authorizing implementation. Research/design authorization is distinct from
  authorization to implement or release a feature.
- **Removed from core** records an explicit scope decision. It does not silently
  cancel other historical candidates.

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
verification. The supported Mac target is macOS 14+ on Apple Silicon. On 2026-10-17 the
user explicitly excluded Intel Mac builds/support. Record actual platform/build/
publication evidence separately in [tasks.md](tasks.md).
Native Windows acceptance is not implied.
Vim-style navigation, compact terminal layout, complete-line copying, and Windows
Terminal/WSL clipboard behavior refine U01/L01 without expanding platform guarantees.

The confirmed U01 visual refinement adds state symbols, distinct focus/selection,
emphasized shortcuts and log modes, and bounded waiting/completion feedback.
Acceptance checks that state remains identifiable when selected or disconnected,
state changes preserve stable backgrounds without reverse-video flashes,
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

Logging, public release/self-update, and lightweight project onboarding are
confirmed follow-up directions. Entries marked candidate
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
| D04, D05 | HTTP probes, one-shot tasks, sustained health checks, cascaded control | Candidate; standalone startup-state refinement explicitly deferred on 2026-10-18 |
| U01, P07 | Multiple services per project and aggregate workspace/project/service UI | Candidate; current one-command-per-project scope remains |
| U02 | Expanded non-TUI foreground presentation | Candidate; existing foreground run and control CLI remain available |
| X01 | Native macOS operation | Delivered in 1.5.0; native install/update/rollback and real bogu verification; separate platform/user acceptance evidence in tasks.md |
| X02 | Native Windows | Candidate |
| X03 | Public distribution and self-update | Public releases and updater delivered; full target WSL acceptance pending |
| P06 | Service PTYs, stdin forwarding, interactive commands | Candidate |
| W05 | package.json/Procfile import and devctl-managed file-watch restart | package.json import is included in A01; Procfile import and file-watch restart remain candidates |
| A01–A03 | Lightweight onboarding and historical environment/infrastructure proposals | A01 retained and simplified; A02 removed from core; A03 pending reassessment; see [scope](#directory-onboarding-and-environment-automation-a01a03) |
| AG01 | Agent-assisted troubleshooting | Research/design requested on 2026-10-18; implementation remains proposed; see [boundary and proposed acceptance](#agent-assisted-troubleshooting-ag01) |
| Configuration extensions | Environment/tilde path expansion, env_files/env overrides | Candidate; existing literal path/environment behavior remains |

An earlier 20-service performance budget is a proposal, not measured evidence.
Existing configuration and planning code remain; unsupported runtime dependencies
must be rejected before startup rather than silently ignored. Historical detailed
proposals are available in Git. Cross-project aggregation and complex filters remain
candidates. Agent assessment follows the delivered logging foundations; see AG01.

### Directory onboarding and environment automation (A01–A03)

The user revised the 2026-10-16 baseline on 2026-10-17 to prioritize a small,
responsive service/log manager. Lightweight onboarding remains; environment
inspection, runtime/package-manager version management, tool bindings, dependency
inventory, automatic installation and repair are removed from core scope. This
supersedes the earlier A02/M7 setup plan. Agent-assisted problem assessment follows
the separate [AG01 planning boundary](#agent-assisted-troubleshooting-ag01).

| ID | Scope | Observable acceptance |
| --- | --- | --- |
| A01 | TUI path/script/command addition; CLI add/import remain available | Add a project from the TUI without writing TOML/CLI setup commands or selecting environment versions; hot-add without restarting running services; preserve edits/duplicates and normal lifecycle |
| A02 | Removed from core | No doctor/setup, managed tool bindings, installation or dependency-readiness gate; unknown retired fields/options fail explicitly |
| A03 | Infrastructure discovery/monitoring/control pending reassessment | Historical scope retained for a later concrete scenario; not an automatic next milestone |

The primary onboarding workflow accepts a directory inside the TUI, offers explicit
script choices, or asks for the command the user normally runs. It works across languages without ecosystem-specific installers or mandatory
scanning. Optional script discovery reads explicit repository declarations, offers
ambiguous entry points for selection and saves the chosen directory/command.
Discovery/save must not execute repository commands. Onboarding and service startup
must not add devctl-managed version probes or environment preparation; an explicitly
configured command retains its own behavior, including any environment preparation.

Basic execution checks remain: valid config, directories, command arguments,
references and lifecycle ownership. Report missing executables, process failures,
exit results and logs. Environment preparation belongs to the project or its chosen
tools; no automatic diagnosis or readiness promise is required. Existing status,
logs and control commands may support future Agent workflows, while ordinary
operation remains fully usable offline without an Agent.

A03's historical proposal includes system services, Docker/Compose, managed/external
processes and remote endpoints, including installed but stopped instances. Any later
implementation must distinguish installation/process/availability/authentication,
show control ownership and preserve data and shared services. Creating databases,
migrations/seeds, installation methods and privilege handling were never settled.
This simplification neither implements nor silently cancels A03; its priority and
boundary need reassessment before design. Crash restart, cascading controls and
reboot recovery remain separate candidates.

Mechanisms live in [DESIGN](../DESIGN.md#lightweight-project-onboarding-m7);
commands and migration guidance live in [interaction](interaction.md#adding-projects).

#### M7 acceptance cases

| Case | Expected result |
| --- | --- |
| Directory with an explicit command, any language | Preview/save without reading manifests, checking versions or installing; preserve argv exactly |
| Already runnable npm/pnpm project | Import with no runtime/version choices; normal service startup uses existing environment |
| Bogu-style workspace | Offer independent web/API scripts and warn about root overlap; save paths/commands only |
| Missing or ambiguous manager/scripts | Explicit choice or direct add; no guessed startup command |
| Conflicting/unsupported/incomplete discovery | Explain and offer explicit add; scanner support never gates ordinary command use |
| Environment constraints, absent dependencies or unavailable runtime | Import/save are allowed; actual command execution reports its own failures |
| Repeat import/add, aliases, edits, explicit replacement | No duplicates; preserve comments/IDs/commands/cwd/autostart and no-op bytes/inode |
| Concurrent saves, source/target changes and active/idle runner | No partial file, overwritten edits or stale commands; CLI refuses owned workspaces, TUI appends through the existing supervisor without restart; externally changed existing declarations require reopening |
| Earlier unreleased managed fields/options | Clear rejection/migration guidance; never silently switch tool execution |
| First-use/cancel/paste/clipped TUI | Add without existing config; no files before confirmation; typed shortcut letters stay input; cancellation/resize cannot save invisibly |
| Tab directory completion | Complete unique/shared directory prefixes; cycle visible alternatives; preserve Unicode/spaces, `~/` and relative paths; no-match, stale results and cancellation never change files or block input |
| Other client starts during review | Saving succeeds with its process/state/logs/workspace identity preserved; new project stays stopped |
| Normal service lifecycle | Start, logs, stop, repeat start, leave/reconnect and complete cleanup stay correct |
| macOS and WSL2 | Record evidence independently; build checks are not platform/user acceptance |

Research starting points (not selected implementation mechanisms):

- [Compose startup order and readiness](https://docs.docker.com/compose/how-tos/startup-order/)
- [PostgreSQL connection readiness](https://www.postgresql.org/docs/current/app-pg-isready.html)
- [Redis PING](https://redis.io/docs/latest/commands/ping/)
- [WSL systemd support](https://learn.microsoft.com/en-us/windows/wsl/systemd)

### Agent-assisted troubleshooting (AG01)

**Design proposed; not implemented.** On 2026-10-18 the user confirmed that analysis,
follow-up questions and results belong inside devctl. This supersedes the earlier
export-first recommendation. Planning does not authorize runtime implementation or
release. [Research](agent-research.md) owns comparisons and source evidence;
[interaction](interaction.md#agent-assisted-troubleshooting-proposed) owns screens,
keys and feedback; [DESIGN](../DESIGN.md#agent-assisted-troubleshooting-proposed)
owns mechanisms and limits; [roadmap](roadmap.md#ag01-proposed-delivery-gates) owns sequencing.

The first product increment must complete one in-TUI workflow: notice a problem,
select its evidence, preview disclosure, start analysis, follow progress, answer or
ask questions, inspect cited logs, and refresh evidence after a user-performed check.
Manual copying is a comparison baseline and optional escape hatch, not a prerequisite
product phase. A running service with an application problem is a valid starting point.

Proposed boundaries:

- Local error hints never initiate a model request or steal focus. Users can also
  initiate diagnosis from arbitrary selected logs or a description without an error hint.
  Process failure and suspected application errors remain distinct.
- Ordinary start/stop/logging works offline without an Agent. Diagnosis failure,
  cancellation and client exit cannot stop or restart managed services.
- Default disclosure covers one selected service and execution attempt: reduced
  state, loaded execution metadata and bounded log evidence. Users can preview what
  will be sent. Source access is optional and scoped; other projects, older attempts
  and newly arriving logs need an explicit scope change. Raw environment, credentials,
  `.env` and whole source trees are never collected by default. Masking is not a
  guarantee that all sensitive business data has been removed.
- The initial Agent observes and explains. It cannot execute commands, edit files,
  install dependencies, control services or operate infrastructure. Users perform
  suggested checks through their ordinary tools; a suggestion is not a completed fix.
- One active diagnosis per TUI client is sufficient initially. General chat, a session
  library, provider marketplaces, autonomous repair and multi-agent coordination are
  outside this increment. Future repair/control remains a separate assessment; AG01
  does not restore removed A02 or silently cancel pending A03.

Proposed acceptance (these are gates, not evidence of delivery):

| Case | Required result |
| --- | --- |
| Spawn failure or unexpected exit | Show the existing failure and a diagnosis entry; bind evidence to the failed attempt, including when no logs exist |
| Application error while Running | Show a separate advisory hint; stderr alone and ordinary mentions of errors do not make a process Failed |
| Manual log selection | Selected lines become the diagnosis anchor; selection/copy still work without an Agent |
| Open and cancel preparation | No model request or external disclosure occurs before Start analysis |
| Complete diagnosis | Analysis, questions, answers and results remain inside devctl; cited evidence opens the corresponding logs |
| Typing, resizing and project switching | Typing cannot invoke service controls; evidence stays pinned to its original service; narrow layouts remain usable |
| Restart, reconnect, edited config or offline history | Preserve provenance, expose missing data and mark stale evidence; do not silently replace it with newer output |
| Sensitive or hostile evidence | Preview reduced fields, mask planted secrets and reject tool/scope bypasses even when log text requests them |
| Extra evidence | Show precisely what additional access is requested; denial allows an uncertain result without bypass |
| Unavailable engine, cancellation or malformed output | Preserve available partial results, report the failure and keep service management responsive |
| User checks a suggested remedy | Refresh is explicit and labelled with a new evidence revision; Running alone never proves recovery |
| Target workflow | Exercise the complete integrated workflow with an actual supported engine on Mac and inside WSL separately |

Use the [reviewable corpus and baseline](agent-research.md#evaluation-plan-before-expanding-scope).
All cases require complete evidence provenance and enforced limits, masking of all
planted fixture secrets across released fields, and zero unintended project,
configuration or service changes. At least 8 of the initial 10 diagnoses should be
useful or appropriately uncertain according to a recorded human rubric; this is
not a general accuracy claim. Measure user effort, latency and available usage/cost.
Passing automated checks does not establish acceptance in the user's real environment.

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
- Install programs for the [supported platforms](#confirmed-scenario) in `~/.local/bin`.
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
configuration. Project environment checks, version management and installation
were explicitly removed from core scope; see
[A01–A03](#directory-onboarding-and-environment-automation-a01a03).
Background execution is not boot startup, machine-restart recovery, or adoption of
processes created outside devctl. Target-environment verification remains a user
workflow, distinct from controlled tests on a build runner.
