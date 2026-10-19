# Changelog

User-facing changes are grouped by released version. Download artifacts from
[GitHub Releases](https://github.com/Jackie733/devctl/releases).

## Unreleased

- Add the selected devctl identity to both READMEs and the TUI header, preserving
  the complete three-service-line/control-rail mark in themes, narrow terminals,
  monochrome output and ASCII mode. Include transparent SVG/PNG assets.

## 1.8.1 — 2026-10-19

- Fix macOS release checks by comparing canonical project paths in the edit/remove test.

## 1.8.0 — 2026-10-19

- Add dedicated single and default-group restart commands (`devctl restart SERVICE...`,
  `devctl restart --all`), and TUI shortcuts (`r` to restart selected service, `R` to
  restart default group, and `c` to reconnect/sync).
- Add external configuration hot-reload via `devctl reload` and TUI `u`, applying
  manifest updates, additions, and stopped-service removals without interrupting running services.
- Add TUI stopped-project editing on `e` and confirmed removal on `d`, saving configuration
  changes atomically.
- Add mouse click and wheel navigation in the Projects panel to focus and cycle selection
  without triggering service actions.

## 1.7.0 — 2026-10-19

- Add mouse text selection in logs, `y` or right-click copying, and wheel paging
  through retained history, preserving original Unicode text and terminal restoration.
- Show local log capture times and stable, compact output/run labels.
- Explain project-addition EOF errors from an older background process and reject
  invalid control requests explicitly.
- Add a maintainer release command that prepares versions, runs checks, commits
  and pushes a new tag, then follows both platform builds and public verification.

## 1.6.1 — 2026-10-18

- Remove reverse-video state flashes during start/stop. Keep selected project
  backgrounds stable when focus moves to logs; preserve status words and colors.
- Add coherent dark/light/terminal TUI palettes, matching dialogs, reduced-motion
  waiting indicators, plain status symbols and nonempty `NO_COLOR` support.
- Fit complete contextual footer actions instead of clipping labels; emphasize
  current search matches with underline while preserving copied log evidence.
- Preserve process-state semantics: Running means the process is alive. Application
  startup confirmation remains deferred and is not part of this release.

## 1.5.0 — 2026-10-17

- Add Tab directory completion to the TUI project-path input, with shared-prefix
  extension, sorted candidate cycling, Unicode/spaces and `~/` support. Keep reads
  bounded and asynchronous; preserve input on failure or stale results.

- Add a TUI project dialog on `a`: paste a directory, choose declared npm/pnpm
  projects/scripts or enter a custom command, review/save and return to the list.
  First use needs no config file. Move default-group start to `A`. Hot-add through
  the existing supervisor while preserving running services, log streams and session;
  the new project stays stopped. Refuse conflicting edits/saves for review.

- Add config-independent, read-only `import DIR` discovery for Node manifests,
  npm/pnpm workspace members, declared scripts and package-manager evidence.
  Explain command overlap, conflicts and bounded/incomplete scans without running
  tools/scripts or writing files during discovery.
- Add reviewed configuration import with project/script selections, terminal
  guidance, preserved existing edits, explicit replacements and atomic saves. Refuse
  active workspaces and changed evidence; synchronize runner reads with import edits.
- Add language-independent `add DIR -- COMMAND ARG...` using the same reviewed,
  comment-preserving transaction. Import selects command names only, without runtime
  versions or installed-tool checks. Commands use the existing environment.
- Remove the unreleased environment-management experiment: doctor/setup, exact tool
  bindings, dependency checks and acknowledgement options. Retired configuration
  fields fail explicitly with migration guidance; automatic installation is out of core scope.

- Add native macOS 14+ operation on Apple Silicon, preserving process-group
  cleanup, background services, reconnect, retained logs and terminal restoration.
- Use pbcopy for local Mac log copying, UTF-16LE clip.exe for local WSL and OSC 52
  over SSH/other terminals, with bounded native clipboard operations.
- Support macOS installation, native-target manifest updates and offline rollback
  with the system Bash, Perl and shasum; preserve locking, checksums and backups.
- Build and verify Linux musl plus Apple Silicon Mac artifacts in the release workflow;
  merge matching platform bundles into one manifest and verify public downloads
  on each platform. No macOS artifact exists in the historical 1.4.0 release.

## 1.4.0 — 2026-10-16

- Add service-state symbols, a distinct selection marker, emphasized panel focus,
  connection status, shortcuts, and FOLLOW/READ/V-LINE modes without requiring
  Nerd Fonts. Preserve readable state colors on selected rows and compact layouts.
- Animate waiting for service operations, clipboard results, log loading, and
  searches. Briefly highlight confirmed Running/Stopped transitions without moving
  log reading positions or treating start acceptance as application readiness.
- Hide ordinary successful stop/copy feedback after two seconds while retaining
  its full text and clipboard caveats in help. Keep failures, unknown completion,
  and independent connection/service/log diagnostics visible.
- Synchronize the copy PTY test with actual help closure before resizing, avoiding
  a race where its background selection marker satisfied the wait prematurely.

## 1.3.0 — 2026-10-16

- Add terminal update stages with a spinner, real download byte progress, adaptive
  width, mean transfer speed and approximate ETA; show check-only/no-op/rollback
  states without fabricated download progress.
- Respect NO_COLOR and use plain stage output when redirected or TERM=dumb. Preserve
  stdout results and child diagnostics, suspend animation around child output, and
  keep the existing checksum, installation, backup, and rollback protections.
- Isolate the updater PTY test controlling terminal so narrow-width checks behave
  consistently when run from an interactive shell or a headless runner.

## 1.2.2 — 2026-10-16

- Rework the public project documentation in English, with a matching Simplified
  Chinese README, feature overview, quick start, command reference, and platform scope.
- Add the MIT license, Cargo package metadata, and contributor guidance. Separate
  general installation/update instructions from WSL integration and hands-on checks.
- Translate architecture, requirements, roadmap, logging decisions, release maintenance,
  and historical changelog entries while preserving delivered/candidate distinctions.
- Publish English release notes and provide a real stable version for exercising
  manifest-based updates from 1.2.1. Service-management behavior is unchanged.

## 1.2.1 — 2026-10-16

- Discover latest or explicit releases through `update.json` attachments instead of
  anonymous GitHub REST API requests. Pin the program to the selected version and
  check byte length, SHA-256, and candidate version before installation.
- Generate the manifest from the actual program, include it in SHA256SUMS, and upload
  it with all other attachments to a draft before publication. Test generated manifests
  using the real client in an isolated environment.
- Preserve published 1.2.0 artifacts. Legacy releases without manifests use the external
  installer; API-limited 1.2.0 users can migrate through fixed release attachments.

## 1.2.0 — 2026-10-16

- Add workspace-independent `devctl update` for stable release checks, explicit/latest
  installation, and offline rollback on Linux/WSL2 x86_64.
- Select the public musl artifact and digest, bound downloads by size/time, check SHA-256
  and candidate version, and reuse installer locking, backup, and replacement rules.
- Diagnose GitHub API rate limits without modifying the installation or backup.
- Add tag-triggered GitHub Actions with complete checks, static builds, actual artifact
  install/rollback and PTY tests, same-commit source archives, and draft-first publication.
- Support manual build-only verification, record build identity, use a read-only build
  job, and restrict write permissions to publication.

Old 1.0/1.1 clients require the external installer for initial migration. Replacing
files does not update active supervisors; shut down workspaces first. Publication
and controlled checks did not establish full target WSL acceptance of these changes.

## 1.1.0 — 2026-10-16

- Save background logs in an independent user state directory with bounded segments,
  capacity/age cleanup, and explicit saving degradation.
- Add bidirectional memory/disk history, pause/follow, reading restoration within one
  TUI across switches/reconnects, and latest-follow behavior for newly opened clients.
- Add per-project literal search, match navigation, ordinary context/highlights, and
  complete-line copy. Add CLI session listings, offline history, fixed-bound search
  continuation, and page redirection.
- Preserve UTC capture timestamps, session identity, gaps, expiry, and incomplete tails;
  historical queries never start services.
- Fix tests affected by slower WSL synchronization and shell startup notices, with
  bounded retention batches and in-shell path/version assertions.
- Improve help/shutdown popups, dimmed backgrounds, spacing, wrapping, action hierarchy,
  and narrow-window confirmation behavior.

From 1.0.0, recreate the supervisor after installation to use persistence/history.
Old memory-only logs cannot be saved retroactively. Controlled verification passed;
the complete three-project WSL logging experience remained pending user acceptance.

## 1.0.0 — 2026-10-15

Initial stable MVP, accepted with three real projects on Ubuntu 22.04.5 LTS,
WSL2 x86_64.

- Open the TUI by default; preserve explicit CLI commands, configuration discovery,
  and validation before execution.
- Start/stop selected or default-group services, prevent duplicates, isolate failures,
  and clean process groups.
- Keep services running after leaving/closing a client; reconnect and explicitly end
  the background workspace.
- Provide Vim-style navigation, adaptive layout, content-sized help, and client version.
- Read recent per-project logs and copy complete lines from any cached position with
  `v/y`, using `clip.exe` in local WSL.

The original distribution provided a Linux musl binary, external installer, source
archive, and checksums while the repository was private. Logs were memory-only and
lost at supervisor shutdown; disk history, search, and self-update were follow-up work.
Native Windows and macOS were not accepted platforms. Current installation instructions
are in [docs/install.md](docs/install.md); historical release files remain unchanged.
