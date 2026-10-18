# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Completed: U01 / M5 TUI research and refinement proposal

Requested on 2026-10-18: research third-party TUI products before proposing UI,
motion, icon and palette improvements. [Research and proposal](tui-design-research.md)
records the existing baseline, five reference groups, concrete visual rules, staged
work and acceptance scenarios. No runtime code or dependency changed.

### Validation

- `cargo test tui:: -- --quiet`: 70 unit tests passed; other suites filtered out.
- `cargo build --quiet`: passed.
- `python3 tests/support/tui_pty.py target/debug/devctl navigation`: passed.
- Inspected isolated current-version 120×30 and 60×18 terminal text frames.
- `git diff --check`: passed.
- `python3 /tmp/devctl-doc-check.py /Users/chuliu/Documents/Code/devctl`: passed;
  17 Markdown documents, 168 local links/anchors, zero errors.
- No new UI was implemented. Proposed appearance/motion, physical terminal glyphs,
  performance and real WSL/macOS user acceptance remain unverified. No visual screenshot
  audit or third-party runtime benchmark is claimed.

### Preserved handoff

The [AG01 integrated proposal](agent-research.md) remains proposed. Its interaction,
architecture and delivery gates remain in the owning documents. Engine isolation,
masking, diagnosis quality, costs/performance and real Mac/WSL integration are still
unverified. This U01 proposal neither implements nor cancels that workflow.

## Published baseline and outstanding acceptance

- [v1.5.0 Release](https://github.com/Jackie733/devctl/releases/tag/v1.5.0) was published
  on 2026-10-17 from `2574a7febd6075a03e4320d307e48eb59218a3aa`.
  [Final workflow](https://github.com/Jackie733/devctl/actions/runs/39043197453)
  passed builds/publication/public verification for both supported targets.
- Hosted suites passed 310 tests on Apple Silicon/macOS 14 and 313 on Linux, plus
  installer/package/merge/native artifact checks. Anonymous downloads, manifests,
  isolated installation/rollback and configuration preservation passed. Detailed
  release receipts remain in Git; local download receipt: `/tmp/devctl-150-public-final-92qhu3gy`.
- The previously verified local installed client was a 1.5.0 development build from
  `9f75dfe`; this task did not inspect or replace that installation. To select exact
  published bytes later, follow normal shutdown and `update --version 1.5.0`.
- Daily-project onboarding/hot addition, actual WSL terminal/clipboard/log experience,
  real cross-version update/rollback and user acceptance remain distinct from hosted
  checks. Earlier Mac Bogu verification does not establish every workflow or physical
  macOS 14 hardware acceptance.

## Next steps

1. Review the U01 visual proposal before implementation; prioritize coherent colors,
   focus, status and contextual hints, then motion and compatibility.
2. When implementation is authorized, begin AG1.1 engine feasibility against the
   documented data boundary, then implement and accept the complete AG1 TUI workflow.
   Do not substitute an export-only release or silently broaden engine access.
3. Continue outstanding real-project/platform acceptance in [roadmap](roadmap.md)
   and [WSL verification](wsl.md). A03 infrastructure control remains pending assessment.
4. Preserve published v1.5.0 tag/artifacts; runtime changes require a new version.
