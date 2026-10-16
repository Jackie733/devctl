# Development collaboration

## Principles

- Understand the existing code, follow repository conventions, and make the smallest safe, readable change.
- Keep APIs small and explicit. Avoid speculative abstractions, frameworks, dependencies, or refactors.
- Preserve architecture and compatibility; leave unrelated code, secrets, and deployment configuration alone.
- Use idiomatic Rust and explain consequential tradeoffs without prioritizing cleverness.

## Workflow

1. Read the current task record, Git status, and relevant implementation. Locate planning documents by requirement ID and section instead of rereading every plan.
2. Implement the authorized code, tests, and necessary documentation directly. Review correctness, failure paths, boundaries, and readability.
3. Reuse existing tests. Add tests only for meaningful behavior gaps, including important success, failure, and boundary cases. Documentation-only changes generally need documentation checks.
4. For functional changes, run relevant tests, `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings`; choose the full suite according to impact. For documentation, inspect diffs, links, and consistency. Record actual commands, results, and unverified items. Fix failed checks before claiming success.
5. Update `docs/tasks.md` when status, blockers, or next steps change. Inspect the diff; stage and commit only the current task's code, tests, and documentation. Do not automatically push. This incremental local commit workflow is already authorized and does not need repeated confirmation.
6. Report the changes, validation evidence, short commit hash, and remaining work. Address or explain blockers before continuing; do not describe a suggestion as implemented.

Communicate concisely and directly. Explain implementation details when requested or necessary for understanding.

## Documentation ownership

- `docs/requirements.md` owns scope, priorities, and acceptance criteria; `docs/interaction.md` owns operation scope and feedback; `DESIGN.md` owns mechanisms, invariants, and tradeoffs; `docs/roadmap.md` owns phase order and requirement mapping.
- Maintain a rule in one primary location. Link or briefly reference it elsewhere. Acceptance scenarios can verify the same rule without duplicating its full definition.
- `docs/tasks.md` contains the current task, necessary handoff, and next steps, linked to milestone and requirement IDs. Git stores history; do not append a session diary.
- Update a document only when its subject changes. Add tests for behavior gaps, not to meet a per-commit quota.
- Keep public reference documents in English, with a matching Chinese overview in `README.zh-CN.md`.

## Scope and acceptance

- Connect functional changes to the appropriate requirement ID and a confirmed use case. Implementation convenience is not a reason to invent a product feature.
- Confirmed user scenarios take precedence over historical plans. Unmentioned historical features remain pending assessment; do not silently cancel them or remove them from the original scope. Simplifying documentation is not reducing functionality. Candidate features require confirmation before implementation; routine local choices do not require repeated questions.
- Tests verify defined behavior. Users accept complete workflows in their real environment. Passing tests, a partial implementation, or consistent documentation do not establish user acceptance.
