# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: macOS deployment assessment complete (X01/X02, X03)

Assessment date: 2026-10-16. Completion criteria: inspect platform scope,
build/runtime portability, distribution/install/update, configuration and test
coverage; attempt native verification; record gaps and ordered next steps without
treating an assessment as authorization to implement a candidate platform.

**Result: macOS installation and operation are not complete or supported.** The
[installation guide](install.md#requirements) and
[platform scope](requirements.md#post-mvp-scope) agree with the implementation:
Linux/WSL2 x86_64 is the release target; native macOS remains a candidate.

| Area | Evidence and remaining gap |
| --- | --- |
| Native build | On Darwin arm64 with Rust 1.97.1, `cargo build --locked` fails at `src/process.rs:11`: the selected nix API does not expose `Id`/`waitid` on macOS. This is a platform API gap, not an observed toolchain-version rejection. |
| Service lifecycle | `src/process.rs` unconditionally reads Linux `/proc` to confirm process-group cleanup. Compilation alone would not establish reliable stop, natural-exit cleanup or restart on macOS. |
| Distribution | The release workflow and packager produce only `x86_64-unknown-linux-musl`; there is no macOS build/test job or native release artifact. Minimum macOS version and Apple Silicon/Intel scope are undecided. |
| Install/update/rollback | `scripts/devctl-update` explicitly rejects Darwin; `src/update.rs` rejects platforms other than Linux x86_64. The installer also uses Linux/GNU tools and shell features that need a macOS compatibility review. |
| Configuration | Existing explicit/default discovery, relative paths and validation are documented in the [configuration reference](interaction.md#configuration). These are reusable foundations, not verified macOS operation. Zsh PATH setup passed an isolated repeatability check. Root `devctl.toml` has no projects/services and is not a usable project setup. |
| Environment preparation | Runtime/dependency installation and generated workspace configuration remain unimplemented A01/A02 work. Database/Redis discovery and control remain unimplemented A03 work. |
| User acceptance | Most lifecycle/background/log/TUI integration suites are Linux-only. Native build failure prevented configuration/runtime/TUI execution here. Terminal appearance, copy support, reconnect, descendants and port cleanup require macOS checks; reboot recovery remains a separate candidate. |

Only this task record changed. No runtime, installer, release configuration or
platform-support promise was changed; the user's real HOME and services were not
modified by the isolated checks.

## Next development tasks

The [existing roadmap](roadmap.md#next-steps) still puts real WSL visual/log and
cross-version update/rollback acceptance first. Automation work remains:

1. Refine the open decisions in [A01–A03](requirements.md#directory-onboarding-and-environment-automation-a01a03) against representative user repositories
   and actual database/Redis installation sources; retain the WSL Node/npm baseline.
2. Design M7 discovery, checks, command selection, configuration persistence and
   setup feedback; connect validation to the specified acceptance scenarios.
3. Design M8 service identity, monitoring and supported control boundaries, including
   shared instances. Keep historical candidate requirements available for assessment.

If macOS is selected as the next implementation priority, use this order; these
are assessment recommendations, not newly scheduled scope:

1. Confirm supported Mac architectures/minimum OS and a representative project.
2. Make native compilation and process lifecycle work, preserving descendant
   cleanup and ownership guarantees; verify normal exit, stop escalation, repeated
   start/stop, background persistence/reconnect and shutdown on the selected Macs.
3. Complete native artifacts and compatible installation/update/rollback, including
   checksums, PATH behavior, backup/configuration preservation and failure recovery.
4. Add macOS automated coverage and installation/usage guidance with a matching
   Chinese overview; accept the complete workflow on a real Mac before claiming
   support. Environment automation remains the separately scoped M7/M8 work.

## Existing release handoff

- v1.4.0 is published; implementation `8633e70`, tagged build
  `37758491cb7124a7bbc7c36178dc001ac7a6358d`. The
  [release run](https://github.com/Jackie733/devctl/actions/runs/38883387229)
  passed CI and public artifact/update verification; detailed history is in Git.
- U01/X03 real WSL visual/log/update acceptance remains pending. Follow
  [WSL verification](wsl.md) and [installation](install.md#update), shutting down
  active workspaces and leaving their TUIs before switching versions.

## Validation

- Passed: `cargo fmt --check`; `bash -n scripts/devctl-update scripts/setup-path.sh scripts/package-release.sh`.
- Isolated Python/subprocess checks on this Mac: `bash scripts/devctl-update
  --rollback` returned 1 with the explicit Linux/WSL2-only message and installed
  no binary; running `bash scripts/setup-path.sh zsh` twice with temporary HOME
  returned 0 both times and wrote the PATH line exactly once.
- `cargo build --locked`, `cargo test --locked`, and
  `cargo clippy --locked --all-targets -- -D warnings` each exited 101 with the
  same macOS import error above. No Rust test success or native runtime acceptance
  is claimed. Repairing platform support is outside this assessment-only task.
- Documentation checks: `git diff --check`, local Markdown links/anchors and manual
  diff review passed. Release publication status above is retained handoff evidence,
  not a fresh check of GitHub. No Linux/WSL test suite was rerun here.
