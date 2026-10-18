# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: documentation boundaries and Agent assessment (AG01 planning)

Documentation boundary corrections cover the two supported platforms, append-only
live additions and delivered/acceptance-pending/candidate/removed status definitions.
Agent research and architecture planning were requested on 2026-10-18; implementation
and publication have not been authorized by that planning request. Scope is owned by
[requirements](requirements.md#reading-scope-and-status). Research/design and their
validation are the next task step; existing platform acceptance below remains open.
Boundary check: `git diff --check` passed; a Python local Markdown link/anchor check
validated 131 links with no failures. No runtime behavior changed.

## Completed: publish v1.5.0 (X03; A01, W05, X01)

Authorized and published on 2026-10-17. The user explicitly excluded Intel Mac:
release builds and update targets are Linux x86_64 / WSL2 and Apple Silicon macOS
14+ only. No Intel build is included in the final workflow or public assets.

- [v1.5.0 Release](https://github.com/Jackie733/devctl/releases/tag/v1.5.0) is public,
  stable and Latest. Tag/build source: `2574a7febd6075a03e4320d307e48eb59218a3aa`.
- [Final release workflow](https://github.com/Jackie733/devctl/actions/runs/39043197453)
  passed both builds, publication and both public-verification jobs.
- Six assets: Linux musl executable, Apple Silicon executable, external installer,
  exact source archive, `update.json` and `SHA256SUMS`. The manifest has exactly two
  supported native targets; the checksum file covers the other five assets.
- Feature scope: TUI `a` directory/script/custom-command review/save, append-only
  live addition without restarting existing services, Tab directory completion,
  first-use configuration, lightweight CLI add/import and native Mac distribution.
  Default-group start is `A`; new projects stay stopped. Environment preparation
  remains outside core scope. See [interaction](interaction.md#adding-projects).

### Verification

- Final hosted Rust suites: 310 passed on Apple Silicon/macOS 14; 313 on Linux.
  Fmt/clippy, 14 installer checks, 8 merge checks, 4 package checks, native artifact
  verification and release-binary terminal/lifecycle/history scenarios passed.
- Public verification on both platforms passed anonymous downloads and checksums,
  native execution, latest/explicit manifest checks without workspace config,
  installation, offline rollback, external recovery and configuration preservation
  in isolated HOME directories. This is not a claim of a real 1.4.0-to-1.5.0 client
  upgrade test or WSL terminal/user acceptance.
- Independent local anonymous download checked manifest/source digests, Latest and
  the complete decompressed source tar against `git archive` of the tag commit.
  Receipt/downloads: `/tmp/devctl-150-public-final-92qhu3gy`.
- Local final full suite: 310 passed; directory-completion/hot-add/concurrent-window
  scenarios also passed using the verified Mac preflight binary. Local docs links,
  shell examples, Python AST and diff whitespace checks passed.
- Publishing exposed fixture-only Mac timing/overload assumptions. History input
  now waits for backend acknowledgements per 32 records, including queued archive
  writes; exact counts/anchors/eviction/restore checks remain. High-output tests
  separately cover bounded overload. Merger comparison uses decompressed Git tar
  bytes because native gzip encodings differ, retaining per-bundle checksums and
  exact installer/build-note equality; invalid/mixed source bundles are rejected.

The existing local installed client remains the earlier 1.5.0 development build
from `9f75dfe` (receipt in Git history); this publication did not reinstall it or
restart/change user workspaces. To replace it with exact published bytes later,
follow the normal shutdown step and explicitly select `update --version 1.5.0`.
A same-version development build need not appear as an available upgrade.

## Next steps

1. User acceptance of daily-project onboarding/hot addition and real WSL terminal,
   clipboard/log reading and update workflows remain distinct from hosted checks.
   Existing Mac Bogu lifecycle verification is retained in Git history; it does not
   establish every business flow or physical macOS 14 hardware acceptance.
2. Keep scope/priorities in [requirements](requirements.md) and [roadmap](roadmap.md).
   A03 infrastructure control and Agent diagnosis remain pending assessment, not
   automatic next milestones. Future fixes require a new version; preserve published
   v1.5.0 tag/artifacts.
