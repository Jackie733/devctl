# Changelog

User-facing changes are grouped by released version. Download artifacts from
[GitHub Releases](https://github.com/Jackie733/devctl/releases).

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
