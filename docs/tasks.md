# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Completed: AG01 integrated TUI architecture revision

Requested on 2026-10-18. The user confirmed analysis, follow-up questions and results
inside devctl. This replaces the earlier export-first recommendation, not the
published runtime. Agent implementation remains proposed under
[AG01](requirements.md#agent-assisted-troubleshooting-ag01).

- [Interaction](interaction.md#agent-assisted-troubleshooting-proposed) now defines
  error hints, local preparation, wide/compact screen sketches, focus/keys, follow-up,
  evidence navigation, scope requests, cancellation and freshness feedback.
- [DESIGN](../DESIGN.md#agent-assisted-troubleshooting-proposed) defines identities,
  read boundaries, budgets, lifecycle and a conditional engine choice. Engine-native
  tool isolation is an unresolved feasibility gate, not a claimed capability.
- [Research](agent-research.md) revises the comparison using the confirmed experience;
  [roadmap](roadmap.md#ag01-proposed-delivery-gates) treats the first delivery as one
  complete workflow with internal engineering gates. README summaries agree.
- Documentation ownership remains unchanged. No runtime code, dependencies, model
  calls, automatic repair or release artifacts were added.

### Validation

- `git diff --check`: passed after removing trailing blank lines.
- `python3 /tmp/devctl-doc-check.py /Users/chuliu/Documents/Code/devctl`: passed;
  16 Markdown documents and 165 local links/anchors, with closed code fences.
- Reviewed keyboard precedence against `src/tui/mod.rs`, layout against the current
  TUI, and historical-session scope against the existing interaction contract.
  Corrected the proposal to identify the historical-session selector as new UI.
- Measured all four ASCII screen sketches: widths 65/64/78/40 columns, within
  their illustrative 80/80/120/50-column layouts. This is not interactive UI testing.
- `codex --version`: 0.145.0. `codex app-server --help` and experimental JSON-schema
  generation passed; inspected thread/turn, interruption and dynamic-tool shapes.
  No engine server, authentication or model turn was run.
- Prior isolated two-service fixture established reusable state/log/history paths,
  missing loaded command/cwd response fields and raw fixture-secret exposure.
  Receipt: `/private/tmp/dv-ag-lyd09_i1`; this revision did not rerun that runtime test.
- No Rust behavior changed; a Rust-suite rerun is not needed for this documentation task.

Unverified: actual engine isolation, TUI implementation/usability, masking, diagnosis
quality, costs/performance and real Mac/WSL integration. Screen sketches are proposed
layouts, not screenshots of working features.

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

1. When implementation is authorized, begin AG1.1 engine feasibility against the
   documented data boundary, then implement and accept the complete AG1 TUI workflow.
   Do not substitute an export-only release or silently broaden engine access.
2. Continue outstanding real-project/platform acceptance in [roadmap](roadmap.md)
   and [WSL verification](wsl.md). A03 infrastructure control remains pending assessment.
3. Preserve published v1.5.0 tag/artifacts; runtime changes require a new version.
