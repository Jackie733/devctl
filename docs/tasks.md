# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: publish v1.4.0 (X03; M5 / U01, L01, P02)

- The user authorized committing and releasing the TUI visual refinements, including
  pushing main and the annotated version tag. Implementation commit: `8633e70`.
- Package/lock versions and current installation examples target 1.4.0. The changelog
  and English/Chinese overviews describe state symbols, focus and shortcut emphasis,
  waiting animations, brief state highlights, and successful stop/copy feedback.
- Preserve lifecycle, diagnostics, clipboard caveats, and log reading positions.
  No new dependency, deployment configuration, or key binding is introduced.
- Previous implementation validation passed all 244 tests (150 unit, 94 integration),
  including ten PTY scenarios; fmt, clippy, Python syntax, and documentation checks
  passed. Log: `/tmp/devctl-tui-refinement-tests.log`.

## Release work and next steps

- Validate version/documentation consistency, commit release preparation, then push
  main and a new annotated v1.4.0 tag. Do not replace existing tags or releases.
- Follow the existing [release workflow](releasing.md): all CI checks, static build,
  installer/artifact/packaging and PTY checks, complete draft-first publication,
  then anonymous public verification. Record actual results and build identity.
- Verify the real public 1.3.0 client upgrades to 1.4.0 in a temporary HOME, preserving
  the old binary, configuration, rollback, and recovery behavior. Do not modify the
  user's installation or active workspaces.
- Actual Windows Terminal/WSL font rendering, contrast, animation cadence, compact
  layout, and X03 update acceptance remain user checks; automated verification does
  not establish visual or real-project acceptance. See
  [WSL verification](wsl.md#verify-updates).
