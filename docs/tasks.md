# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: v1.4.0 published; WSL visual/update acceptance pending (U01 / X03)

- The user authorized committing and releasing the TUI visual refinements, including
  pushing main and the annotated version tag. Implementation commit: `8633e70`.
- [v1.4.0](https://github.com/Jackie733/devctl/releases/tag/v1.4.0) is public, stable,
  and Latest. The annotated tag points to build commit
  `37758491cb7124a7bbc7c36178dc001ac7a6358d`; release notes record the same identity.
- Package/lock versions and installation examples target 1.4.0. English/Chinese
  overviews and the changelog describe state symbols, focus/shortcut emphasis,
  waiting animations, brief state highlights, and successful stop/copy feedback.
  Behavior refines M5 / U01, L01, P02; lifecycle, diagnostics, clipboard caveats,
  and log reading positions retain their contracts. No new dependency, deployment
  configuration, or key binding was added.

## Publication and validation evidence

- [Release run 38883387229](https://github.com/Jackie733/devctl/actions/runs/38883387229)
  completed build, publish, and public verification successfully. CI passed fmt,
  clippy, all 244 Rust tests (150 unit, 94 integration), eleven installer tests,
  static build/artifact checks, four packaging tests, and ten actual static-binary
  TUI PTY scenarios. Release workflow configuration is unchanged.
- All five attachments are uploaded: Linux x86_64 musl program, external installer,
  exact source archive, update.json, and SHA256SUMS. Anonymous downloads and all
  four digests passed. Manifest size/digest/version matched the binary, which
  reported 1.4.0 and had no INTERP/NEEDED entries. The decompressed source archive
  exactly matched git archive of the tagged build commit.
- The real public 1.3.0 client performed its default online upgrade to the exact
  1.4.0 bytes in a temporary HOME, retaining 1.3.0 as its backup. Latest/specified
  checks, no-op and same-version backup preservation, offline rollback, external
  recovery, and configuration preservation passed. No user installation or active
  workspace was modified. Earlier public versions and attachments are unchanged.
- Logs: `/tmp/devctl-v1.4.0-actions.log`, `/tmp/devctl-v1.4.0-ci.log`, and
  `/tmp/devctl-v1.4.0-public-verification.log`; fixtures:
  `/tmp/devctl-v1.4.0-public-zz42jtfc`. Locked version metadata, local fmt, Markdown
  fences/links/anchors, and diff checks passed. Prior implementation test evidence
  remains in `/tmp/devctl-tui-refinement-tests.log`.

## Next steps and acceptance

- The user upgrades after shutting down each active workspace and leaving its TUI;
  follow [installation](install.md#update) and
  [WSL verification](wsl.md#verify-updates) for the real 1.3.0 → 1.4.0 workflow.
- Verify actual font rendering, contrast, animation cadence, compact layout, and
  real-project log interaction in Windows Terminal/WSL. Automated checks establish
  coverage, not visual or real-project acceptance; U01/X03 user checks remain pending.
