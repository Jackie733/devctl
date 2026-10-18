# Logging design decisions

This records the product tradeoffs delivered in 1.1.0. The
[architecture](../DESIGN.md#persistent-history) owns mechanisms and budgets,
[requirements](requirements.md#logging-enhancements-l01l04) own acceptance, and
[usage](interaction.md#logs-and-search) owns operations. Current evidence is in
[tasks.md](tasks.md).

## Product goal

Help users recover recent output after ending/recreating a supervisor, locate a
problem in one project, and copy enough ordinary context to investigate it.

| Capability | Decision | Reason |
| --- | --- | --- |
| Live output, pause, paging and selection copy | Deliver with a bounded client window | Read comfortably without accumulating unlimited client memory |
| Disk history, rotation and expiry | Deliver together | Preserve output while bounding storage growth |
| History beyond memory and across supervisors | Deliver | Memory eviction or normal shutdown should not erase all recent evidence |
| Per-project literal search and context | Deliver | Find retained errors without constant manual scrolling |
| Reading restoration across switches/short reconnects in one TUI | Preserve | Avoid interrupting an active reading task |
| Cross-exit reading bookmarks | Do not implement | A fresh TUI should show current latest output |
| Cross-project aggregation and combined filters | Keep as lower-priority candidates | No confirmed scenario justifies extra UI/indexing yet |
| Export | Selection copy or one CLI page redirected to files | Make scope and missing data clear without a new export UI or format system |

Agent log analysis now has a separate [AG01 planning proposal](requirements.md#agent-assisted-troubleshooting-ag01).
It does not replace trustworthy capture, saving, and querying. This logging decision
does not authorize an Agent implementation.

## Engineering choices

Use an independent Linux/WSL user state directory following the
[XDG Base Directory Specification](https://specifications.freedesktop.org/basedir/latest/),
rather than growing logs inside project directories. Capacity and age independently
trigger cleanup; [Docker's logging guidance](https://docs.docker.com/engine/logging/configure/)
informs the tradeoff, not devctl's specific numeric defaults.

Reuse TOML with bidirectional frame lengths in `.dlog` segments instead of the earlier
JSONL proposal. Reverse paging needs no database or per-record index, at the cost
of reading through the CLI rather than plain `tail`. Preserve format version,
identity, capture time, and missing-data feedback.

The supervisor's storage worker owns writes, cleanup, and online queries; offline
reads hold the same workspace lock. Storage failure preserves memory/control.
Normal shutdown checks storage completion; abnormal exits cannot promise unsynced
tails. Search scans bounded batches with a fixed upper bound, and an empty batch
is not evidence that scanning finished.

Record mechanism tests and actual WSL project acceptance separately. Finishing
this scope does not automatically schedule aggregation, combined filters, or other
candidates for the next increment.
