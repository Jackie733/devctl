# Current task

Keep current work and necessary handoff here; Git retains history. Collaboration
rules are in [AGENTS.md](../AGENTS.md).

## Current: automation scope recorded; implementation deferred (M7/M8, A01–A03)

- Recorded the confirmed directory onboarding scenario, environment checks/setup,
  and database/Redis discovery, monitoring and control in
  [requirements](requirements.md#directory-onboarding-and-environment-automation-a01a03).
- [Roadmap](roadmap.md#next-steps) sequences M7 (A01/A02 and W05 package.json import)
  before M8 (A03). English/Chinese overviews distinguish planned from shipped work.
- This task is discussion/documentation only. No automation code, configuration
  schema, command or installation behavior has been implemented.

## Next development tasks

1. Refine the open decisions in A01–A03 against representative user repositories
   and actual database/Redis installation sources; retain the WSL Node/npm baseline.
2. Design M7 discovery, checks, command selection, configuration persistence and
   setup feedback; connect validation to the specified acceptance scenarios.
3. Design M8 service identity, monitoring and supported control boundaries, including
   shared instances. Keep historical candidate requirements available for assessment.

## Existing release handoff

- v1.4.0 is published; implementation `8633e70`, tagged build
  `37758491cb7124a7bbc7c36178dc001ac7a6358d`. The
  [release run](https://github.com/Jackie733/devctl/actions/runs/38883387229)
  passed CI and public artifact/update verification; detailed history is in Git.
- U01/X03 real WSL visual/log/update acceptance remains pending. Follow
  [WSL verification](wsl.md) and [installation](install.md#update), shutting down
  active workspaces and leaving their TUIs before switching versions.

## Validation

- Documentation-only validation passed: `git diff --check`; Python local Markdown
  link/anchor checks for all five changed files; manual diff and English/Chinese
  scope consistency review. External research links were retained from discussion,
  not re-fetched during this documentation task.
- No Rust tests are required for this task; automation behavior remains unverified
  because it is not implemented.
