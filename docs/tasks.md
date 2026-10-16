# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: v1.3.0 published; WSL acceptance pending (X03)

- The user explicitly authorized the full v1.3.0 release, including pushing commits
  and the annotated tag. They reported passing `cargo fmt --check`,
  `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked`
  after the PTY fix, and requested that those local checks not be repeated.
- Version/package lock and current installation examples now target 1.3.0. English
  and Chinese overviews describe staged update progress. The changelog includes
  real byte/percentage/speed/ETA display, terminal/plain-output behavior, preserved
  installation protections, and the isolated PTY controlling-terminal fix.
- The old client controls the first upgrade's presentation. Upgrading from 1.2.2
  uses its existing output; the animated display is available when running 1.3.0.
- Release workflow configuration is unchanged. Its build checks remain enabled,
  followed by static-artifact/installer/PTY checks, draft-first publication, and
  anonymous public-release verification. These CI checks are distinct from the
  user's completed local checks.

## Publication and validation evidence

- [v1.3.0](https://github.com/Jackie733/devctl/releases/tag/v1.3.0) is public, stable,
  and Latest. [Release run 38879434253](https://github.com/Jackie733/devctl/actions/runs/38879434253)
  completed build, publish, and verify successfully. The annotated tag points to
  build commit `7e289b6bfbd9a4044794ee8516c40910f3583ac3`; notes record that same commit.
- CI passed fmt, clippy, all 239 Rust tests (145 unit, 94 integration), eleven
  installer tests, static build and actual-artifact checks, four packaging tests,
  and ten static-binary TUI PTY scenarios. No local fmt/clippy/test rerun was made.
- All five assets are uploaded: the Linux x86_64 musl binary, external installer,
  source archive, update.json, and SHA256SUMS. Anonymous downloads and all four
  digests passed. Manifest size/digest/version matched the actual executable;
  the binary reported 1.3.0 and had no INTERP/NEEDED entries. Published source files
  and Markdown/license/package/workflow files matched the tagged build commit.
- The actual public 1.2.2 binary performed its default online upgrade to the exact
  1.3.0 bytes in a temporary HOME. Its backup retained 1.2.2. Latest/specified checks,
  same-version installation, no-op backup preservation, offline rollback, external
  recovery, and configuration preservation passed. The user's installation and
  workspaces were not touched.
- Logs: `/tmp/devctl-v1.3.0-actions.log` and
  `/tmp/devctl-v1.3.0-public-verification.log`; downloaded fixtures are under
  `/tmp/devctl-v1.3.0-public-c1g4cubk`. Local locked metadata/version agreement,
  all 15 Markdown documents and 101 local links/anchors/fences, and diff checks passed.
- Next: the user upgrades and checks the actual WSL experience using
  [installation](install.md#update) and [WSL verification](wsl.md#verify-updates).
  From 1.3.0, `devctl update --check` shows checking animation; a same-version
  `devctl update --version 1.3.0` exercises download/install presentation after
  workspace shutdown. No additional release is needed to inspect that display.

## Prior validation and handoff

- Progress implementation: 239 Rust tests passed (145 unit, 94 integration), plus
  static-artifact, installer, packaging, and controlled progress checks. Details
  are retained in the implementation commit and previous task record in Git.
- PTY fix: reproduced the narrow-width failure under a 120-column outer controlling
  terminal. All 11 update integration tests then passed with and without an outer
  controlling terminal; original assertions were preserved. fmt and clippy passed.
- [v1.2.2](https://github.com/Jackie733/devctl/releases/tag/v1.2.2) was the previous
  Latest. Its source commit is 19d79f024af983d8a236adf6fa7dedcfcb905ed6; published
  tags/assets remain unchanged. No real installation or active workspace is touched.
- Controlled checks do not establish target WSL visual/use acceptance. That remains
  with the user; requirement X03 acceptance stays distinct from release completion.
