# Current task

Keep the current task, necessary handoff, and next step here. Git retains history;
collaboration rules are in [AGENTS.md](../AGENTS.md).

## Current: 1.2.2 released; target WSL upgrade acceptance pending (X03)

- The user authorized English public documentation, a Chinese README, publication/push,
  and a real self-update exercise, and explicitly selected MIT. The repository now has
  English/Chinese READMEs, MIT LICENSE, CONTRIBUTING, Cargo package metadata, and English
  user/maintainer references. GitHub recognizes the repository license as MIT.
- Personal project paths were replaced with generic examples. Confirmed behavior,
  engineering invariants, pending acceptance, and candidate features remain distinct.
  This is a documentation/license patch with no service-management behavior change.
- [v1.2.2](https://github.com/Jackie733/devctl/releases/tag/v1.2.2) is public, Latest,
  non-draft, and stable. All five attachments are uploaded: the musl executable,
  external installer, source archive, update.json, and SHA256SUMS.
- The annotated tag points to 19d79f024af983d8a236adf6fa7dedcfcb905ed6. English release
  notes record that build identity. Older tags and published attachments were not modified.
- [Run 38871736455](https://github.com/Jackie733/devctl/actions/runs/38871736455) was
  triggered by the tag push; build, publish, and anonymous verify all succeeded.
  A transient local status-poll network interruption did not restart the workflow.

## Validation evidence

- Local and cloud Rust checks passed: 235 tests (142 unit, 93 integration, including
  ten real PTY scenarios), fmt, and clippy with warnings denied. Installer tests: 11.
  Real-artifact packaging/manifest tests: 4. The final musl program passed static-link,
  install/reinstall/bad-digest/rollback/configuration checks; cloud also passed all
  ten PTY scenarios using the shipped static program.
- All 15 Markdown documents passed local-link/anchor and fence checks. Both READMEs
  use matching TOML, validated/planned by the real 1.2.2 parser from another directory.
  All 25 shell blocks passed Bash syntax checks; documented flags match CLI help.
  MIT text, Cargo metadata, English content, packaging syntax, and actionlint passed.
- Clean-source packaging included both READMEs, LICENSE, CONTRIBUTING, installation
  documentation, correct version metadata, and English notes. Publicly downloaded
  source was compared against the tag for every Markdown file, license, Cargo files,
  updater source, and release workflow; all matched.
- All five release attachments were downloaded anonymously; four SHA-256 entries,
  manifest version/name/size/digest, executable version, and static linking matched.
- In a temporary HOME, installed the real verified public 1.2.1 binary, observed
  1.2.2 availability without workspace config, and invoked that old client's default
  `devctl update`. It installed the exact published 1.2.2 bytes and retained the exact
  1.2.1 binary as backup. Latest/explicit checks, same-version reinstall, no-newer-version
  protection, offline rollback to 1.2.1, external recovery, and configuration preservation
  all passed. This was a cross-version online upgrade, not just a new client reinstalling itself.
- Local logs: /tmp/devctl-v1.2.2-tests.log, /tmp/devctl-v1.2.2-actions.log, and
  /tmp/devctl-v1.2.2-public-verification.log. Preflight bundle:
  /tmp/devctl-v1.2.2-preflight-arz_3eff/bundle. Temporary files are not permanent evidence;
  the Actions link and committed results are the durable handoff.

## User handoff

1. From an installed 1.2.1 client, follow [WSL update checks](wsl.md#verify-updates):
   check for 1.2.2, shut down each active workspace and leave its TUI, run the default
   update, and verify the executable path/version and previous-version backup.
2. New users and legacy API-limited clients use [fixed attachments](install.md#prebuilt-binary).
   User documentation starts at [README.md](../README.md) or [README.zh-CN.md](../README.zh-CN.md).
3. Verify logs, controls, reconnect, and rollback in the real target environment.
   This task did not modify real installations or stop real projects; controlled
   success does not establish full X03 acceptance. Future publication requires its
   corresponding task authorization; [release maintenance](releasing.md) describes the process.
