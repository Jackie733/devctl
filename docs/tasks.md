# Current task

Keep the current task, necessary handoff, and next step here. Git retains history;
collaboration rules are in [AGENTS.md](../AGENTS.md).

## Current: public documentation and 1.2.2 release (X03)

- The user authorized English public documentation, a Chinese README, version
  publication/push, and a real self-update exercise. They explicitly selected MIT.
- Prepared an English README and matching README.zh-CN, contributor guidance, MIT
  license and Cargo metadata. Reworked user/maintainer references in English,
  replaced personal project paths with generic examples, and kept confirmed scope,
  engineering invariants, and candidate features distinct.
- Installation now lives in [install.md](install.md); [wsl.md](wsl.md) owns Windows
  integration and hands-on checks. Architecture, usage, requirements, roadmap,
  logging decisions, changelog, and release notes use English.
- Version 1.2.2 is a documentation/license patch, with no service-management behavior
  change. The new public artifact enables a real 1.2.1 → 1.2.2 default update test.
  Previous tags/releases must not be modified. Publishing, checks, and final public
  upgrade evidence are pending; preparation is not publication.

## Validation evidence and release next step

- Local `cargo test --locked --offline`: 235 passed (142 unit, 93 integration,
  including ten real PTY scenarios). Log: /tmp/devctl-v1.2.2-tests.log.
- `cargo fmt --check` and `cargo clippy --locked --offline --all-targets -- -D warnings`
  passed. Installer tests: 11 passed. The 1.2.2 musl build, four real-artifact
  packaging/manifest tests, and static-link/install/reinstall/bad-digest/rollback/
  configuration-preservation checks passed.
- All 15 Markdown documents passed 104 local-link/anchor checks and fence checks.
  Both READMEs contain matching TOML, validated/planned by the real 1.2.2 parser
  from a different working directory. All 25 shell blocks passed Bash syntax checks;
  documented update/log flags agree with CLI help. English content, MIT text,
  package metadata, Bash packaging syntax, and actionlint checks passed.
- Next: commit, inspect the clean-source bundle and English release notes, then push
  main and a new annotated v1.2.2 tag. Monitor build/publication/anonymous verification.
- After publication, download all five public assets, verify checksums/source identity,
  and use the real 1.2.1 binary in a temporary HOME for default online upgrade to
  1.2.2, backup/config preservation, offline rollback, and external recovery.

## Previous release and handoff

- [v1.2.1](https://github.com/Jackie733/devctl/releases/tag/v1.2.1) is the previous stable
  manifest-format release. Its build commit is 9f76c8da2d1fe562608387009e8bc5c6e96a40ce;
  [run 38868608804](https://github.com/Jackie733/devctl/actions/runs/38868608804) passed
  build, publication, and anonymous verification.
- Complete target WSL experience remains user acceptance, separate from controlled
  tests. This task does not alter real installations, stop real projects, or mark
  X03 fully accepted. Follow [WSL update checks](wsl.md#verify-updates) after release.
