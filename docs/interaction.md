# Configuration and usage reference

This document owns command behavior, operation scope, keyboard controls, and user
feedback. Mechanisms and numeric resource budgets are defined in [DESIGN.md](../DESIGN.md).
For installation and version switching, see [install.md](install.md).

## Configuration

With no subcommand, `devctl` opens the TUI. `devctl tui` is the explicit equivalent.
Other subcommands perform only their named operation; an unknown command is an error.
Help and version exit without loading workspace configuration, as do the independent
`update` path. Import scans its supplied directory before validating a selected or
proposed configuration destination.

Workspace commands select the first existing configuration, without merging:

1. Explicit `--config PATH`.
2. `devctl.toml` in the current directory.
3. `~/devctl.toml`.
4. `~/.config/devctl/devctl.toml`.

Home candidates use an absolute `HOME`. Missing files are skipped; directories,
broken symlinks, inspection errors, or an invalid selected file are errors. An explicit
path never falls back. Non-TUI commands report searched paths when configuration is missing. The TUI
opens an empty view for first-project addition, without creating files before save. Connecting to an existing supervisor does not reload
the file automatically; use `devctl reload` or `u` in the TUI to apply changes to stopped services, or shut down and recreate the supervisor to change log archive settings.

Configuration version is `1`. The [README](../README.md#quick-start) has a minimal
multi-project example. Supported fields are:

| Table | Fields and defaults |
| --- | --- |
| Root | `version = 1`; `workspace`; optional `projects`, `services`, `logs` |
| `[workspace]` | Nonempty `name` |
| `[projects.ID]` | Nonempty existing directory `path` |
| `[services.ID]` | Known `project`; nonempty argument array `cmd`; optional `cwd`; `autostart = true`; optional `depends_on` for validation/planning |
| `[logs]` | `enabled`, `max_mib`, `retain_days`; see [storage budgets](../DESIGN.md#storage-identity-format-and-budgets) |

Unknown fields, invalid references, empty executable names, and invalid directories
fail validation. Project paths resolve from the selected configuration's directory;
relative service `cwd` resolves from its project. The default cwd is the project
directory. TOML paths do not expand `~` or environment variables. Commands execute
as argument arrays without an implicit shell, inheriting the supervisor's original
environment for every service. Environment preparation belongs to the project or
its chosen tools.

`autostart = true` means membership in the default start group. Opening or reattaching
the TUI does not automatically start that group. The current runtime supports one
service per project and independent commands. Dependency graphs can be validated and
planned; unsupported runtime dependencies are rejected before any selected command
starts. They are not silently ignored. See [dependency examples](../examples/dependencies/README.md).

## Commands and service lifecycle

Add `--config /path/to/devctl.toml` to select a workspace explicitly.

| Command | Behavior |
| --- | --- |
| `devctl` / `devctl tui` | Open the workspace or first-project empty view; requires a TTY |
| `devctl validate` | Validate fields, paths, and references without executing commands |
| `devctl plan [SERVICE...]` | Preview dependency-first order; defaults to the default group; does not execute commands |
| `devctl start SERVICE...` | Start selected services; create the supervisor if needed |
| `devctl start --all` | Start the default group; do not duplicate already running services |
| `devctl restart SERVICE...` | Stop selected services, confirm process-group cleanup, and start a fresh instance; create the supervisor if needed |
| `devctl restart --all` | Stop all active services, confirm cleanup, and start the default group; create the supervisor if needed |
| `devctl stop SERVICE...` | Stop selected services and confirm process-group cleanup |
| `devctl stop --all` | Stop every active service, including manually started non-default services; keep the supervisor |
| `devctl reload` | Reload `devctl.toml` into the running supervisor for stopped/cleanup-confirmed failed services without interrupting running services |
| `devctl status` | Inspect existing service states and reasons; do not create a supervisor |
| `devctl shutdown` | Stop all managed processes, finish log saving, and end the supervisor |
| `devctl run SERVICE...` / `devctl run --all` | Run explicit services or the default group in the foreground |

Explicit selection and `--all` are mutually exclusive; `run` requires one of them.
An empty default group exits successfully. Selection is validated as a batch before
starting any service. A start response means the request was accepted, not that the
application is ready. `Running` only means the process was observed alive. A failed
service does not stop or restart others. Starting a running service is a no-op;
a stopping or cleanup-unconfirmed instance cannot be started again.

A foreground runner and background supervisor cannot manage the same configuration
at the same time. Foreground Ctrl-C/SIGTERM stops selected process groups. Natural
completion or explicit stop ends the foreground run; any failure makes its exit
status 1, otherwise 0. Background `stop`, `reload`, and `shutdown` do not create a missing
supervisor. Unconfirmed cleanup reports failure and preserves ownership and diagnostics
rather than pretending shutdown completed. Reconnection never replays a control request.

## Terminal interface

The wide layout shows projects on the left and selected-project logs on the right.
The selected project is independent of panel focus. Service controls always name
that selected project; workspace controls explicitly affect all services or the
default group. There is no `:` command mode or stdin forwarding to services.

### Keys

These keys describe the main view outside popups, search input, and line selection.

| Scope | Keys | Action |
| --- | --- | --- |
| Focus | `Tab` / `Shift-Tab` | Switch between the two panels |
| Focus | `h/l`, `1/0` | Focus projects/logs; h/l stop at the boundary |
| Projects | `j/k`, `↓/↑` | Select next/previous service; stop at list boundaries |
| Projects | `gg/G` | Select first/last service; sorted by service ID |
| Projects | `Enter` | Focus selected-project logs without starting it |
| Projects | `e` / `d` | Edit directory/command or confirm removal of the selected stopped (or cleanup-confirmed failed) project |
| Logs | `j/k`, `↓/↑` | Move the reading cursor, scrolling at the viewport edge |
| Logs | `gg/G` | Read oldest retained history / resume following the latest output |
| Logs | `Esc` | Return to projects after handling active selection/search modes |
| Logs | `v` / `V` | Begin complete-line selection at the reading cursor or first visible line |
| Either panel | `Ctrl-u/d` | Scroll logs half a page without changing focus |
| Either panel | `PageUp/PageDown` | Scroll logs a page with one overlapping line |
| Either panel | `Home/End` | Oldest retained history / latest output and follow |
| Either panel | `p` | Pause/resume following, outside line selection |
| Either panel | `H/L`, `←/→` | Scroll long log lines horizontally by eight columns |
| Service | `s/x/r` | Start, stop, or restart the selected service (`r` reconnects when disconnected) |
| Add project | `a` | Open path/script/command/save dialog |
| Workspace | `A`, `R`, `X` | Start the default group, restart the default group after stopping all active services, or stop all and keep the supervisor |
| Workspace | `u` | Reload `devctl.toml` for stopped services while keeping running services alive |
| Workspace | `Q` | Open shutdown confirmation |
| Connection | `c` | Reconnect/sync to an existing supervisor; do not create or reload it (`r` also reconnects while disconnected) |
| Help | `?` | Open contextual help |
| Leave | `q`, `Ctrl-C` | Leave the client, including from popups; keep services running |

The first `g` waits for a second `g` and shows a pending prefix; `Esc` cancels it.
Other valid keys cancel the prefix and retain their normal meaning. Focus changes,
popups, disconnection, and workspace changes clear it. There is no timing requirement.
Control requests are not queued while another is pending; navigation, help, and leaving
remain responsive. Disconnected service states are `Unknown`, and controls are disabled.

### Line selection and copying

`j/k` first positions a reading cursor. `v` anchors complete-line selection there;
use `j/k` to extend or shrink it in either direction. `gg/G`, `Home/End`, half-page,
and page keys move its endpoint within the current client window. `H/L` changes only
the horizontal view; complete records remain selected.

`y` requests copy; success exits the unchanged selection, while failure or exceeding
the copy limit preserves it. `v/Esc` cancels selection. `?` opens help and returns to
the selection afterward. Service/focus keys do not pass through selection mode.
`q/Ctrl-C` still leaves. Evicting either endpoint cancels with `SELECTION EVICTED`;
project, connection, and workspace changes also clear selection.

Copy includes selected bodies in order, with a newline after each, including text
outside the horizontal viewport and `[truncated]` where applicable. It excludes
source/instance prefixes, UI padding, and status labels. The limit is
64 KiB; partial copies are not sent. A later copy completion cannot clear a new
selection. Only one copy is in flight, and cancelling selection does not retract
an already issued clipboard request.

Mouse input is enabled while the TUI is open. Over the Projects panel, left-clicking
a project row focuses Projects and selects that project; wheel up/down focuses
Projects and moves selection without starting or stopping services. Over the log body:

- Left-button drag selects text, including parts of lines, in either direction.
  Release keeps the selection, shown as `V-TEXT`. A fresh click clears the old
  selection and pauses the view; it does not copy automatically.
- Press `y` or right-click to copy the current selection. Text copies contain only
  the selected original bodies, with newlines between records and no added trailing
  newline, prefixes, padding or truncation labels. Tabs/control characters retain
  their original bytes; visual endpoints follow whole graphemes, including wide
  characters and combining marks, after horizontal scrolling.
- Wheel up/down scrolls three records per event and loads earlier/later retained
  pages at the client-window edges. Scrolling pauses following. Reaching the latest
  output resumes following when no selection or active drag remains; scrolling
  preserves an existing selection. Cancel with `Esc`, then use `G` to follow.

Mouse text selection shares the 64 KiB bound, eviction handling and copy feedback
above. Keyboard endpoint movement changes it to complete-line selection. Logs keep
capturing while reading/selecting. Mouse events do not pass through help, search
input, project addition/edit, removal or shutdown confirmation, hidden panes or unusably small
windows. Resizing ends a drag while preserving its current selection. Terminals
can reserve mouse shortcuts; use `y` when right-click is handled by the terminal.
Normal/error/signal exits restore mouse capture along with other terminal settings.

Local WSL uses `clip.exe` through Windows interoperability; local macOS uses
`pbcopy`. SSH and other Linux sessions use OSC 52 to the display terminal, subject
to terminal/multiplexer support. A failed native clipboard operation is reported
without channel fallback. Copy never reads the clipboard and occurs only on `y`
or an explicit right-click over the logs with a selection.
Success reports a completed native process or an OSC 52 send; paste manually to
verify the actual clipboard.

### Layout, help, and confirmation

The header shows client version, connection state, and supervisor identity in
one row for wide layouts. Compact layouts retain the original second row for the
selected project and panel hints. The 36 × 10 minimum remains supported.
The TUI, including Help, uses no decorative logo or abbreviated branding symbol.
Help reserves its scrollable content area for instructions and diagnostics; the
[graphical identity](../assets/branding/README.md) is used in documentation.
Projects show `Stopped`, `Starting`, `Running`, `Stopping`, `Failed`, or disconnected
`Unknown`.
Log state is displayed separately from `FOLLOW`/`READ` and saving status. Stopped
services retain output labeled as retained logs; stale “ready” text does not imply
current readiness. The log header identifies the latest known run as `latest #N`;
each record keeps its own fixed run number across restarts and reconnects.
Unconfirmed cleanup and unknown control completion retain their own diagnostics.

Service states pair text with symbols: `○ Stopped`, `● Running`, `× Failed`, and
`? Unknown`; `Starting`/`Stopping` use a spinner. No Nerd Font is required. The
focused panel has a cyan, bold title; `▸` marks the selected project independently
of focus. Selection keeps its background when focus moves to logs and preserves
readable state colors. State transitions update the symbol and label without
flashing or reversing the background. Running still means process alive, not
application readiness.

Spinners advance while connecting, awaiting a control or clipboard result, loading
logs, or searching. `FOLLOW`, `READ`, and `V-LINE` use distinct emphasis without
animating log text. Shortcut keys use the focus accent; the footer fits complete
actions in priority order and reserves help/leave access at every usable size.
Successful stop/copy feedback leaves the bottom row after two seconds; its full
text, including clipboard caveats, remains in help until replaced. Start acceptance
is not treated as confirmed startup. Failures and unknown completion do not expire;
connection, service, and log-loss diagnostics remain visible independently.

Version 1.6.1 adds these appearance options (read once at TUI startup):

| Environment variable | Values and behavior |
| --- | --- |
| `DEVCTL_TUI_THEME` | `dark` (default), `light`, `terminal`; explicit selection, no background detection |
| `DEVCTL_TUI_REDUCED_MOTION` | `1` keeps waiting text and a static `~`; `0`/unset enables waiting spinners |
| `DEVCTL_TUI_ASCII` | `1` uses plain status/selection symbols; `0`/unset uses Unicode; log text and pane borders remain unchanged |
| `NO_COLOR` | Any nonempty value removes TUI foreground/background colors; selection/search keep text and underline emphasis |

For example, `DEVCTL_TUI_THEME=light devctl` selects the light palette.
Dark/light themes style the main view and dialogs consistently. Terminal mode
inherits the terminal background and uses its ANSI palette. Unknown theme names or
invalid `0`/`1` options fail before entering terminal mode or connecting a workspace.
Theme/symbol/motion choices do not change service behavior or copied log contents.

The bottom is always two rows: feedback and relevant shortcuts. Full IDs, errors,
saving paths, and operation details are available in scrollable help. Logs do not
wrap; one record occupies one row. The viewport remains anchored while reading.

| Terminal size | Layout |
| --- | --- |
| At least 80 columns and 12 rows | Two panels, with a roughly 30% project panel constrained to 24–32 columns |
| At least 36 columns and 10 rows, below the wide threshold | One active panel; focus and controls retain their meanings |
| Smaller | Size/connection/leave feedback; service and shutdown controls disabled until resized |

Help groups panel controls, service/workspace actions, logs, search/copy, leaving,
and diagnostics. Navigate with `j/k`, arrows, pages, `gg/G`; close with `?/Esc`.
Background service keys do not pass through popups. Popups dim the view, preserve
text styles, and wrap according to terminal columns and grapheme boundaries.

`Q` names the workspace-wide effect: all services stop and the supervisor ends.
Only `y` confirms; `n/Esc` cancels, and `q/Ctrl-C` only leaves. Enter does not confirm.
If the connection, current control request, or available space prevents safe
confirmation, no shutdown request is sent. A successful TUI shutdown exits only
after cleanup and supervisor termination are confirmed.

## Logs and search

Scrolling, moving the reading cursor, or entering selection pauses following.
While paused, the supervisor keeps capturing; the client polls boundaries and counts
without shifting the text. `+N new` counts newly numbered records, not capture-queue
drops. At a window edge, adjacent pages load one at a time while preserving sequence
anchors. Old pages evict the opposite end rather than expanding memory indefinitely.
`G/End`, `p`, or forward paging to the actual tail resumes following outside selection.

`Home/gg` can request the retained start. Selection-mode endpoints stay within the
current window. Feedback distinguishes loading, earlier available, retained start,
snapshots older than retained history, evicted reading positions, and missing records.
The retained start does not mean no earlier output ever existed.

Reading positions survive project switches and short reconnects within the same TUI
and supervisor identity. Expired anchors report a gap and move to available history.
A new supervisor clears bookmarks; a new TUI starts at the current session's tail.
Selections do not survive switching or reconnecting. A running service with no output
does not silently borrow an older session's text.

### Saved sessions and literal search

Background output is saved by default; foreground `run` is not archived. Change
saving settings only after shutting down and recreating the supervisor. The
`save=saving/disabled/degraded` indicator is independent of process and follow state.
`saving` is not a per-record durability acknowledgment. Help shows the directory,
session, and failure reason; `SAVE DEGRADED` signals potentially incomplete storage.

Normal TUI history combines current-session memory and disk. Historical sessions
remain explicit CLI queries rather than being mixed into current output. Log panes
at least 100 columns wide show capture time as `HH:MM:SS` in the TUI client's local
timezone, following the system timezone or `TZ` override, including the offset at
each record's capture time. Missing timestamps from older supervisors are omitted;
failed local-time conversion displays `??:??:??`. Application timestamps in the log
body remain as emitted; CLI records retain UTC Unix capture milliseconds.

Rows use `[out #N]` for stdout and `[err #N]` for stderr. The run number is permanent:
compare it with `latest #N` in the header to distinguish earlier runs, without
relabeling existing lines on restart. The header's process state remains authoritative
for whether that latest run is alive; while disconnected its current state is unknown.
Per-record sequence numbers remain internal navigation/search anchors and in CLI
output, and are omitted from ordinary TUI rows.

| Log-focused keys | Behavior |
| --- | --- |
| `/` | Enter a case-sensitive literal query, up to 256 UTF-8 bytes; preview highlights while typing |
| `Enter` | Search forward after the reading cursor, or from the retained start; pause and fix the search upper bound |
| `n/N` | Next/previous matching record within the same fixed bound; do not wrap |
| `Esc` | Cancel input, or cancel search/highlights while keeping ordinary context; help/selection close first |
| `G/End`, resume follow | Cancel scanning and reload the tail; keep visible highlights; a new `/` refreshes the bound |

During input, q/s and other printable keys are query text; Ctrl-C still leaves.
Search scans only the selected service and current session. Matches are positioned
in ordinary context rather than hiding nonmatching lines; long-line matches scroll
into view. Selection highlighting takes precedence, and copying still uses original
bodies. Searches are cancellable between bounded batches. Expiry, switching, and
reconnection invalidate stale results. “No further match” is shown only after the
available range in that direction was scanned, with any known gaps reported.

```sh
devctl log-sessions
devctl logs web --session SESSION_ID --limit 100
devctl logs web --after 0 --contains 'Error' --limit 100
devctl logs web --after SCANNED_TO --contains 'Error' --upper UPPER
devctl logs web --before SEQUENCE --limit 100
```

Replace uppercase placeholders with reported values. With a supervisor, queries
default to its current session; without one, they use the selected service's latest
retained session. `--session` selects a listed historical session in either case.
Session listings contain at most 100 retained sessions with ID, creation time,
closed/unclosed status, and services. Unclosed is not a current service state.
No log query creates a supervisor or starts services. If a writer holds the workspace
lock but IPC is unavailable, fail instead of bypassing the writer with offline reads.

Both sequence bounds are exclusive; `--after` and `--before` cannot be combined. A page is
returned in ascending sequence order; no direction means the latest page. Limits
are 1–500 records and 64 KiB of bodies. Continue using actual returned cursors,
not the supervisor's highest sequence. Search may return no matches in an incomplete
scan; `complete=false` requires continuation using `scanned_to` and the same `--upper`.
Fix `--session` too when reading historical pages across supervisor recreation.

For one-page export:

```sh
devctl logs web --session SESSION_ID --after 0 --contains 'Error' > errors-page.log 2> errors-page.info
```

Stdout contains this page, including record identity/time prefixes; stderr contains
session, saving state, gaps, and continuation information. It is not a full-history
export. Save subsequent pages explicitly. A normal shutdown waits for saving to
finish; a storage failure is reported even if process cleanup succeeded.

## Disconnection and failures

A disconnected client shows unknown state, not cached state as live evidence.
Polling and `r` connect only to an existing supervisor and do not replay controls.
Reenter the TUI to create an idle supervisor if the old one ended, or create one
with `start` from another client. Slow or absent clients do not stop pipe capture.
Client exit signals restore the terminal and disconnect; supervisor stop signals
request managed-process cleanup. No old PID is treated as proof of ownership or
used to adopt processes after a crash or reboot.

## Agent-assisted troubleshooting (proposed)

These screens and keys are a design for [AG01](requirements.md#agent-assisted-troubleshooting-ag01),
not current commands. The user-confirmed primary experience keeps analysis,
follow-up questions and results inside devctl. Existing service/log controls remain
as documented above; this section defines only the proposed additions.

### Notice a problem without interrupting work

A process failure retains its Failed state. A log pattern suggesting an application
error adds a separate `!` hint and a short excerpt; it never changes process state.
Writing to stderr alone does not qualify. Hints are text plus symbols, not color-only.
Repeated matching output updates a count without opening dialogs, moving selection,
scrolling logs or making model calls. The count means matching records, not unique
root causes. Reading/dismissing a hint is not proof of recovery.

Normal workspace sketch (illustrative content, not a running screen):

```text
 devctl / shop                         Agent: idle
 Projects              | Logs / api / attempt 12
 > api   Failed        | 14:02:11 Error: Cannot find module 'pg'
   web   Running       | 14:02:11 Process exited
                       |
 api: process failed   | e Troubleshoot selected evidence
 Tab Switch panel   / Search   v Select   e Troubleshoot   ? Help
```

Press `e` in Projects to use the selected service's latest current-attempt hint or
failure. In Logs, use selected lines first, otherwise the visible cursor record;
without a record/hint, use a recent tail and the user's problem description. A
manual entry remains available for business errors that detection cannot identify.
The preparation sheet adds an explicit retained-session selector backed by existing
archive reads. Current TUI browsing covers only the current session; historical
session browsing is currently a CLI feature, so this selector is new UI work.

### Prepare and start

`e` opens a local preparation sheet; it does not contact the engine. The sheet shows
service, attempt/session, captured time, trigger excerpt, engine identity and access
scope. Preview opens the exact reduced initial payload, with omissions and masked
values visible. Default and optional data rules are owned by
[DESIGN](../DESIGN.md#context-and-access-boundary).

```text
 Diagnose api / attempt 12                         LOCAL PREVIEW
 Trigger: Error: Cannot find module 'pg'
 Engine: configured local engine / readiness not checked
 Included: service state, loaded command, working directory
           selected log window / 61 records / captured 14:02:12
 Follow-up reads: api / attempt 12 / through record 72
 Missing: structured exit code
 [ ] Allow selected project's source files (read only)
 Problem: [What were you trying to do?                         ]
 [Preview evidence]            [Cancel]       [Start analysis]
 Tab Move focus   Enter Activate focused item   Esc Cancel
```

The problem field receives initial focus; Enter there moves to the action row and
does not send. Start analysis is a separate focused action. Optional source access
opens its directory/exclusion summary before enabling it. Preview must remain
scrollable, with an obvious return action; it is not silently truncated to fit.
Editing the problem or scope invalidates the old preview and rebuilds it locally.
The prepared capture stays fixed until the user chooses Refresh evidence. A session
selector lists retained IDs/times and marks them Historical; changing it rebuilds
the preview and never implies that the historical process is still running.

An unconfigured engine shows setup guidance in this sheet and disables Start.
Start checks availability/authentication locally, then begins the model turn. If
login is needed, show the engine's supported login instruction and a Retry action;
never initiate installation, browser login or authentication without user action.
Service controls remain available by closing the sheet. Display that approved data
will go to the configured engine/provider; local process transport does not imply
local model processing or a particular provider retention policy.

### Analysis workspace and results

At 120 columns by 24 rows or larger, use two working panes: logs and diagnosis.
Projects collapse into the service header; do not squeeze three full columns into
the terminal. Each pane has a visible focus border. The Agent header always names
its pinned service/attempt, even when the workspace selection changes.

```text
 devctl / shop          selected: api        diagnosis: api / attempt 12
 Logs / api                              | Agent / api / analyzing
 14:02:11 Error: Cannot find module 'pg'   | Reading selected log window...
 14:02:11 Process exited                  | Evidence: 61 lines, revision 1
                                         |
                                         | Finding
                                         | Startup could not resolve pg.
                                         | Evidence: [L42: module error]
                                         | Unknown: dependency installation
                                         |
                                         | Suggested check
                                         | Inspect the project's dependencies.
                                         | No changes have been made.
                                         |
                                         | [Add details] [Refresh / verify]
                                         | [Copy result]
 1 Projects   0 Logs   2 Agent   Tab Focus   i Ask   Ctrl-G Stop analysis
```

Show coarse activities, elapsed time and supplied usage when available. Do not show
invented percentages, raw internal reasoning, protocol messages or an indefinitely
spinning indicator after failure. Streaming output follows the tail until the user
scrolls away; show a new-output indicator without forcing them back down.

A completed result distinguishes observations, possible causes, missing evidence,
suggested checks and verification conditions. Evidence references are focusable:
Enter opens the exact service/session/attempt/record in Logs, and `2` returns to the
same diagnosis position. Expired evidence shows its captured excerpt plus a missing
history notice; never jump to a different record with the same sequence number.
Malformed references remain plain text. Commands in suggestions can be copied but
have no Run action.

Follow-up questions and the user's answers stay in this workspace. While analysis
runs, the user may draft a question, but sending waits for completion or explicit
cancellation; the first version does not silently queue or steer another turn.
An Agent question marks `Needs your input`; a scope request marks `Needs access`.
The latter shows the reason, precise extra data and Allow/Decline actions inline.
Decline is initially focused. Hiding the panel does not grant access; ordinary
service control remains usable while the request waits.

### Compact terminals

From 50 columns by 14 rows up to the wide-layout threshold, Logs and Agent share
the body as separate views. Keep the diagnosis service and status visible.

```text
 devctl / shop
 Diagnosis: api / attempt 12 / complete
 [0 Logs] [2 Agent]
 Finding: Startup could not resolve pg.
 Evidence: [L42: module error]
 Unknown: dependency installation

 [Add details] [Refresh / verify] [Copy]
 0 Logs  1 Projects  i Ask  Esc Back
```

Below 50 columns or 14 rows, show a compact status/read-only result and a resize
notice; disable starting, sending and approving when the full controls cannot fit.
`Ctrl-G` can still cancel and Esc returns to the ordinary workspace, whose existing
36-by-10 minimum is unchanged. Resize preserves scroll, focus and drafts; it does
not restart a turn or approve an action. Layouts use ordinary terminal characters.

### Keyboard and focus contract

| Context | Keys and behavior |
| --- | --- |
| Ordinary Projects/Logs | `e` prepares diagnosis; `2` opens the current diagnosis or prepares one if none exists; existing `a`, `A`, `r`, `1`, `0` retain their functions |
| Log selection | `e` prepares from the selection; `y` copies; Esc cancels selection. Service controls remain blocked |
| Preparation sheet | Tab/Shift-Tab move fields/actions; Enter activates the focused action; Esc closes without disclosure |
| Agent reading | `j/k`, arrows and paging scroll; Tab/Shift-Tab traverse actions/references; Enter activates; `i` composes; `1/0` open Projects/Logs; `2` returns |
| Agent composer | Printable keys including `q/s/x/a/e/1/0/2` are text. Enter sends only when ready; Esc leaves editing and preserves the draft. Paste never submits |
| Agent panel | `s/x/A/X/Q` do not pass through to service controls; return to Projects/Logs to use those controls |
| Diagnosis active | Ctrl-G interrupts only this client's diagnosis, including from the composer; Esc hides the panel without cancelling |
| Leave devctl | `q` outside text entry and Ctrl-C retain existing client-exit semantics; the client-owned Agent is cancelled, managed services continue |

The composer initially accepts one paragraph up to 4 KiB UTF-8; pasted line breaks
become spaces with visible feedback, and excess input is rejected with a message.
Search and add-project forms retain precedence over Agent shortcuts. Help includes
the current mode's keys. No key is allowed to act as both text input and a service
operation. Closing a scope sheet and denying access are distinct actions.

### Scope, freshness and lifecycle feedback

Switching projects never retargets an existing conversation. Show `Diagnosis: api`
while the user views web logs. Starting another case while one is active offers
Return to active diagnosis or Stop and prepare new; no silent cancellation. A new
draft does not discard a completed result until Start is activated. The initial
version retains one case in client memory, not a browsable conversation archive.

Restart/disconnect/new supervisor marks the affected evidence stale or historical.
The existing answer remains readable. Refresh / verify previews a new evidence
revision and its scope before the user sends it to the same conversation. A restart
requires explicitly selecting the new attempt; it is never an automatic substitution.
Normal output, completion of analysis and a Running state do not mark the problem
fixed. The result can only describe what its actual evidence establishes.

On cancellation show Cancelled and retain partial output. On missing engine, auth
failure, timeout or engine crash show a specific reason and a deliberate Retry;
never retry model requests automatically. Leaving devctl ends the local case; state
this in help and the Agent footer. Provider-side retention is separately governed
by the selected engine/provider. Runtime state, limits and enforcement are owned by
[DESIGN](../DESIGN.md#agent-assisted-troubleshooting-proposed).

## Self-update

Available since 1.2.0; clients 1.2.1 and later use Release manifests. Available on the
[supported platforms](requirements.md#confirmed-scenario), fixed to `Jackie733/devctl` public releases, with no private-source
authentication or alternate source URL. Workspace configuration is never loaded,
even with an explicit nonexistent `--config`.

| Command | Behavior |
| --- | --- |
| `devctl update --check` | Read the latest stable manifest and report current/release versions without downloading the program |
| `devctl update` | Install only a newer stable version than the running client; preserve installation/backup otherwise |
| `devctl update --version VERSION` | Install an explicit stable version, optionally prefixed with v; allow reinstall or downgrade |
| `devctl update --check --version VERSION` | Validate the selected manifest and compare versions |
| `devctl update --rollback` | Exchange installation and backup offline; conflicts with check/version |

Installation is fixed at `~/.local/bin/devctl`, with one `devctl.previous` backup,
shared locks, and the external installer's protection rules. “Current” means the
executing client's compiled version; check PATH if running from another directory.
Identical reinstallation keeps the previous backup. `--check` validates the manifest,
not the actual existence or content of the program attachment; installation validates
length, SHA-256, and candidate version. Exit codes: 0 for success or no update, 2 for
invalid arguments, and 1 for download, validation, or installation failure.

All updates require an absolute HOME; checks need writable `~/.local/bin` for
private temporary files. Dependencies are listed in the [installation guide](install.md#requirements).
Downloads have fixed size/time limits and clean up their own staging directory on
normal/error return. A failure never silently changes sources or compiles code.
Missing legacy manifests require the external installer rather than an API fallback.
See the [manifest contract](../DESIGN.md#update-manifest-contract).

Follow [version-switching steps](install.md#before-switching-versions) before an
installation or rollback. The updater does not stop projects, replace running
supervisors, or rewrite configuration and saved logs.

### Progress display

The updater in 1.3.0 and later shows checking, downloading, and verifying/installing stages
on stderr. With stdout and stderr attached to terminals and TERM other than `dumb`,
a single line animates at roughly ten frames per second; completed stages remain
in scrollback. Download percentage comes from bytes actually written and the manifest's
size. Wide terminals also show transferred/total MiB, mean transfer speed including
connection time, and an approximate ETA. Before enough data/time is available,
speed and ETA show placeholders. Narrow terminals omit details and shorten the line.

`NO_COLOR` removes colors while keeping terminal animation. Redirecting either stdout
or stderr, or using `TERM=dumb`, produces plain stage lines without terminal controls.
Existing version/result output stays on stdout. No raw mode, hidden cursor, alternate
screen, new dependency, or extra command flag is used.

Check-only and no-newer-version results stop after checking. Rollback shows only its
offline stage. A failed stage is labeled before the normal error report, and the
original curl/installer diagnostics remain visible. Child output clears and suspends
the current animation so even partial diagnostics are not overwritten. The installer
still performs checksum/candidate/backup/replacement work as one reported stage;
substeps are not claimed complete before the installer confirms its result.

The client currently running the update controls presentation. An initial upgrade
from 1.2.2 uses the older output; subsequent updates from 1.3.0 use this display.

## Adding projects

Open `devctl` (or select a workspace with `--config PATH`) and press **a**:

1. Paste or type a project directory, then Enter. `~/` and quoted pasted paths are
   accepted; Ctrl-U clears input. Tab completes directory paths: one match appends
   `/`; multiple matches extend the common prefix and show candidates, then Tab /
   Shift-Tab cycles them in name order. Hidden directories require a dot prefix.
   Absolute, relative and `~/` paths work with spaces and Unicode. No matches or a read
   failure leave the input unchanged. Completion reads one directory in the background, with
   limits; narrow the path or enter it manually if a limit is reached. No repository
   command runs during inspection.
2. npm/pnpm declarations supply project and script choices. One runnable project
   and one startup suggestion can go directly to review. Otherwise choose with
   Up/Down and Enter; workspace root/member alternatives remain separate.
3. If no usable script is found, a basic marker can label Python/Rust/Go/Java;
   enter the command you normally run. This text is explicitly saved as
   `/bin/sh -c COMMAND`, so ordinary shell quoting/operators work. Other languages
   use the same custom-command input. No runtime versions or installation choices.
4. Review the directory, type, command and destination; Enter saves and returns to
   the project list. The added service remains stopped; press `s` to start it.
   Press `e` on review to edit the command. Long reviews can be scrolled; reach the
   end before confirming. A clipped view
   cannot confirm. Esc edits the path, then Esc cancels; Ctrl-C leaves the client.

An empty first-use view creates the normal default config only after save. Existing
canonical projects are preserved without duplicate entries or command replacement.
Background scanning/planning keeps controls and rendering responsive. Bracketed paste
is confined to the active input; service shortcut letters in typed text are literal.
Saving hot-adds the project to the existing workspace: running services, their logs
and the workspace identity remain intact; the new project stays stopped. Other open
clients see it on their next status refresh. While a project is `Stopped` (or `Failed`
with confirmed cleanup), press **e** in the Projects panel to review and edit its
command or directory (`Review project update`), or **d** to confirm removing it from
`devctl.toml` and the running workspace without interrupting other running services.
After editing `devctl.toml` externally, press **u** in the TUI or run `devctl reload`
to apply additions, removals, and updates for stopped services while keeping running
services alive. A changed source/configuration or a
competing save requires review/retry instead of overwriting edits. First-use save creates and opens the new idle workspace. An older
supervisor that cannot accept additions must be shut down and reopened under the
new binary. Updating the executable does not replace an already-running supervisor.
If saving reports an unexpected end of file, inspect the configuration and
`devctl status` before retrying: an older supervisor may have rejected the request,
but a lost reply can also hide a completed save. To replace an old supervisor, leave
the dialog and use `Q` then `y` in the main view, or `devctl shutdown`, then reopen
with the current binary. Shutdown stops all workspace services; start the desired
services again after reopening. CLI save commands retain their existing
owned-workspace refusal.

### Explicit command addition

Use an explicit directory and command for any language. Preview first, then save:

```sh
devctl --config /path/to/workspace.toml add /path/to/project --name api -- python server.py
devctl --config /path/to/workspace.toml add /path/to/project --name api --apply -- python server.py
```

The command and its arguments follow `--`; option-looking arguments after it belong
to the service. There is no implicit shell, interpolation or executable/version
validation. Arguments, including spaces, are saved exactly. Supply an explicit shell
command only when the project requires one. Omit `--name` to derive a stable ID from
the directory. Commands can use the project's existing environment-tool entry point.

Existing canonical projects retain their command and edits; no duplicate is added.
A requested name already used by a different project/service is rejected. Change an
explicit command by editing configuration while the workspace is stopped. Invalid
paths and empty executables are rejected; a missing executable is reported when
starting. Saving configuration is not proof that an application can run.

### Optional script import

For npm/pnpm repositories, import can fill the same path/command fields from declared
scripts. No installed Node or package manager is needed to import:

```sh
devctl --config /path/to/workspace.toml import /path/to/repository
devctl --config /path/to/workspace.toml import /path/to/repository \
  --project apps/web --project apps/api \
  --script apps/web=dev --script apps/api=dev --apply
```

Repeat `--project RELATIVE_PATH` to select packages relative to the supplied directory;
`.` selects its root. Repeat `--script RELATIVE_PATH=SCRIPT` for actual declared
scripts. One runnable project/one suggested script can be selected automatically;
otherwise use flags or terminal choices. Arrow/j/k navigation, Space selects projects,
Enter continues, Esc/Ctrl-C cancels. Libraries without scripts remain visible but
disabled. Resize a clipped screen before making selections. If manager evidence is
missing, choose its command in the terminal or pass `--package-manager npm|pnpm`.
This is a command-name selection, with no version option or version checks.

Discovery displays sources, scripts, workspace roots and possible root/member command
overlap. It ignores environment constraints and runtime version files. A conflicting
manager/layout or incomplete scan fails with guidance to use explicit `add`. Other
languages and unsupported repository layouts can always use `add` or direct config.
Scanner limits and path containment belong to [DESIGN](../DESIGN.md#command-configuration-and-discovery).

Repeat import preserves existing IDs, comments, commands, cwd and autostart. Use
`--replace PROJECT_ID` to review replacement of that discovered project's command
and cwd. Multiple existing services for one project require manual resolution.
Removed manifests do not remove entries. An unchanged apply preserves bytes/inode.

### Saving and feedback

Both commands use the normal config discovery order; `--config PATH` can select a
new file. Without an existing candidate, propose `~/.config/devctl/devctl.toml`.
Without absolute HOME, require explicit `--config`. Never replace an invalid chosen
config or silently fall back. Preview prints the destination and additions/explicit
replacements without creating files, checking tools, installing or starting services.

`--apply` asks for confirmation in a terminal; redirected use requires `--apply --yes`.
`--yes` skips final confirmation only; ambiguous projects/scripts still require flags.
Changed sources/config or lock contention refuse apply. CLI commands also refuse
a running/idle managed workspace: shut it down, apply, then reopen it. The TUI uses
the append-only hot update described above. Saving is atomic, preserves
user edits, and applies only the reviewed additions in the TUI. CLI saving never
reloads a running supervisor. See the
[transaction design](../DESIGN.md#configuration-transaction-and-supervisor-boundary).

After saving, use the printed TUI/start commands. Start uses the existing supervisor
environment and normal lifecycle checks; failures are reported through status/logs.
The configured command may perform its own preparation/network actions; devctl adds
no environment checks or installation. A complete preview/save exits 0; unresolved
choices, invalid sources/configs, conflicts and cancellation exit 1; invalid CLI
usage exits 2.

### Earlier unreleased environment configurations

The environment-management experiment has been removed. `doctor`, `setup`,
`--node`, `--node-version` and `--allow-unverified-dependencies` are unavailable;
TUI starts use the ordinary `s`/`A` controls. Earlier unreleased configurations with
`node_environments`, service `node_environment`, or project `dependency_root` fail
explicitly rather than silently changing execution.

For those configurations, first shut down the old supervisor using the old binary.
Choose the command/environment entry point you normally use, remove the three
managed fields, run `validate` and reopen the workspace. The tool does not rewrite
these files or delete private bindings automatically. Released ordinary command
configs require no migration. Updating the devctl program itself remains available
through `update`; project runtime version management has been removed.
