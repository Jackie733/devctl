# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Completed: documentation boundaries and AG01 assessment

Requested on 2026-10-18. This task changes documentation only; Agent implementation
and publication remain proposed under [AG01](requirements.md#agent-assisted-troubleshooting-ag01).

- Corrected stale platform/update/build descriptions and the loaded-configuration
  invariant. [Requirements](requirements.md#reading-scope-and-status) own scope/status
  terminology; append-only live additions remain the only supported live config change.
- [Research](agent-research.md) records official sources, alternatives, current code
  gaps and the evaluation method. [DESIGN](../DESIGN.md#agent-assisted-troubleshooting-proposed)
  owns proposed evidence collection, disclosure, identity, limits and optional access.
- [Roadmap](roadmap.md#ag01-proposed-delivery-gates) proposes local evidence first,
  conditional read-only MCP next, then an optional external CLI launcher. No Agent
  command, SDK, dependency, model call or automated repair was implemented.
- English and Chinese README summaries and the proposed interaction agree. Historical
  candidates remain recorded; removed A02 environment management is not restored.

### Validation

- `git diff --check`: passed.
- `python3 /tmp/devctl-doc-check.py /Users/chuliu/Documents/Code/devctl`: passed;
  checked 16 Markdown documents, code-fence closure and 161 local links/anchors.
  The earlier boundary commit checked 131 links without failures.
- `python3` isolated two-service IPC/history fixture: passed after using canonical
  paths for the Mac `/tmp` alias. Verified selected failure/state/log identities,
  raw fixture-secret exposure, absent command/cwd metadata, surviving-service cleanup,
  and offline retained history. Receipt: `/private/tmp/dv-ag-lyd09_i1`.
- `codex --version` / `codex exec --help`: local CLI 0.145.0 supports the documented
  stdin/JSON/schema/sandbox/ephemeral options. No login or model run performed.
- No Rust behavior changed; no Rust-suite rerun is required for this documentation
  task. Previous 310/313 test results below are publication evidence, not new tests.

Unverified: Agent diagnosis quality, export/masking, MCP/launcher compatibility,
performance/cost and real Mac/WSL assistant workflows; those features do not exist yet.

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

1. Decide whether to authorize AG1's local evidence workflow using the proposed scope
   and acceptance; AG2 and launching an assistant depend on demonstrated need.
2. Continue the outstanding real-project/platform acceptance in [roadmap](roadmap.md)
   and [WSL verification](wsl.md). A03 infrastructure control remains pending assessment.
3. Preserve published v1.5.0 tag/artifacts; any runtime fix requires a new version.
