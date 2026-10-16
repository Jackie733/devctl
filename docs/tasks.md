# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: publish v1.3.0 (X03)

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
- Next: commit preparation, push main plus v1.3.0, track build/publication/verification,
  verify Latest/assets/tag identity and actual 1.2.2 → 1.3.0 migration in a temporary
  HOME. Publication and migration results remain pending until observed.

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
