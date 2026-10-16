# Configuration and usage reference

This document owns command behavior, operation scope, keyboard controls, and user
feedback. Mechanisms and numeric resource budgets are defined in [DESIGN.md](../DESIGN.md).
For installation and version switching, see [install.md](install.md).

## Configuration

With no subcommand, `devctl` opens the TUI. `devctl tui` is the explicit equivalent.
Other subcommands perform only their named operation; an unknown command is an error.
Help and version exit without loading workspace configuration, as does the independent
`update` path.

Workspace commands select the first existing configuration, without merging:

1. Explicit `--config PATH`.
2. `devctl.toml` in the current directory.
3. `~/devctl.toml`.
4. `~/.config/devctl/devctl.toml`.

Home candidates use an absolute `HOME`. Missing files are skipped; directories,
broken symlinks, inspection errors, or an invalid selected file are errors. An explicit
path never falls back. A missing configuration reports searched paths instead of
creating an empty workspace. Connecting to an existing supervisor does not reload
the file; shutdown and recreate the supervisor to apply changes.

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
environment. Manage runtime versions and dependencies outside devctl.

`autostart = true` means membership in the default start group. Opening or reattaching
the TUI does not automatically start that group. The current runtime supports one
service per project and independent commands. Dependency graphs can be validated and
planned; unsupported runtime dependencies are rejected before any selected command
starts. They are not silently ignored. See [dependency examples](../examples/dependencies/README.md).

## Commands and service lifecycle

Add `--config /path/to/devctl.toml` to select a workspace explicitly.

| Command | Behavior |
| --- | --- |
| `devctl` / `devctl tui` | Connect to or create an idle background workspace; requires a TTY |
| `devctl validate` | Validate fields, paths, and references without executing commands |
| `devctl plan [SERVICE...]` | Preview dependency-first order; defaults to the default group; does not execute commands |
| `devctl start SERVICE...` | Start selected services; create the supervisor if needed |
| `devctl start --all` | Start the default group; do not duplicate already running services |
| `devctl stop SERVICE...` | Stop selected services and confirm process-group cleanup |
| `devctl stop --all` | Stop every active service, including manually started non-default services; keep the supervisor |
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
status 1, otherwise 0. Background `stop` and `shutdown` do not create a missing
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
| Logs | `j/k`, `↓/↑` | Move the reading cursor, scrolling at the viewport edge |
| Logs | `gg/G` | Read oldest retained history / resume following the latest output |
| Logs | `Esc` | Return to projects after handling active selection/search modes |
| Logs | `v` / `V` | Begin complete-line selection at the reading cursor or first visible line |
| Either panel | `Ctrl-u/d` | Scroll logs half a page without changing focus |
| Either panel | `PageUp/PageDown` | Scroll logs a page with one overlapping line |
| Either panel | `Home/End` | Oldest retained history / latest output and follow |
| Either panel | `p` | Pause/resume following, outside line selection |
| Either panel | `H/L`, `←/→` | Scroll long log lines horizontally by eight columns |
| Service | `s/x` | Start/stop the selected service |
| Workspace | `a`, `X` | Start the default group / stop all and keep the supervisor |
| Workspace | `Q` | Open shutdown confirmation |
| Connection | `r` | Reconnect to an existing supervisor; do not create or reload it |
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
source/instance/sequence prefixes, UI padding, and status labels. The limit is
64 KiB; partial copies are not sent. A later copy completion cannot clear a new
selection. Only one copy is in flight, and cancelling selection does not retract
an already issued clipboard request.

Local WSL uses `clip.exe` through Windows interoperability; local macOS uses
`pbcopy`. SSH and other Linux sessions use OSC 52 to the display terminal, subject
to terminal/multiplexer support. A failed native clipboard operation is reported
without channel fallback. Copy never reads the clipboard and occurs only on `y`.
Success reports a completed native process or an OSC 52 send; paste manually to
verify the actual clipboard.

### Layout, help, and confirmation

The header shows client version, connection state, and supervisor identity. Projects
show `Stopped`, `Starting`, `Running`, `Stopping`, `Failed`, or disconnected `Unknown`.
Log state is displayed separately from `FOLLOW`/`READ` and saving status. Stopped
services retain output labeled as retained logs; stale “ready” text does not imply
current readiness. Earlier process instances are marked `old` only when confirmed.
Unconfirmed cleanup and unknown control completion retain their own diagnostics.

Service states pair text with symbols: `○ Stopped`, `● Running`, `× Failed`, and
`? Unknown`; `Starting`/`Stopping` use a spinner. No Nerd Font is required. The
focused panel has a cyan, bold title; `▸` marks the selected project independently
of focus, and selection preserves readable state colors. A confirmed transition
to Running or Stopped briefly highlights the state for 300 ms. Running still means
process alive, not application readiness.

Spinners advance while connecting, awaiting a control or clipboard result, loading
logs, or searching. `FOLLOW`, `READ`, and `V-LINE` use distinct emphasis without
animating log text. Shortcut keys are highlighted within the existing footer.
Successful stop/copy feedback leaves the bottom row after two seconds; its full
text, including clipboard caveats, remains in help until replaced. Start acceptance
is not treated as confirmed startup. Failures and unknown completion do not expire;
connection, service, and log-loss diagnostics remain visible independently.

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
remain explicit CLI queries rather than being mixed into current output. Wide views
show UTC time; CLI records retain UTC Unix capture milliseconds, not parsed application time.

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

## Self-update

Available since 1.2.0; clients 1.2.1 and later use Release manifests. Supported on
Linux/WSL2 x86_64 and macOS 14+ arm64/x86_64, fixed to `Jackie733/devctl` public releases, with no private-source
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
