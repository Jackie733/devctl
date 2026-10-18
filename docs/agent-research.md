# Agent-assisted troubleshooting: research and assessment

Assessed on **2026-10-18** against the current 1.5.0 source. Status: research and
design complete; implementation proposed. [AG01](requirements.md#agent-assisted-troubleshooting-ag01)
owns scope and acceptance; [DESIGN](../DESIGN.md#agent-assisted-troubleshooting-proposed)
owns proposed mechanisms; [roadmap](roadmap.md#ag01-proposed-delivery-gates) owns sequencing.
This record explains the recommendation and evidence, not another source of runtime rules.

## Recommendation and rationale

Keep devctl responsible for accurate local evidence and leave reasoning in an
external assistant chosen by the user. Start with one selected-service packet
that can be reviewed and handed off manually. Add local read-only MCP access only
if follow-up retrieval is useful in real failures. Launching an assistant from
devctl is an optional convenience after that, not the foundation.

The confirmed product already manages processes and logs. Its missing contribution
to assisted diagnosis is trustworthy context: which attempt failed, what evidence
survives, whether it is current, and what is safe to share. An embedded model does
not solve those gaps. The packet also provides a useful fallback without an
assistant login, model network access or a host-specific integration.

This is an architectural judgment, not a measured claim of better diagnosis,
smaller binaries or lower cost. An assistant's own repository tools may be useful
later, but the proposed first workflow needs no automatic source/environment scan.

## Options considered

| Option | Fit and added responsibility | Assessment |
| --- | --- | --- |
| Existing manual log copying | Works now; developer assembles state, time range and context separately | Baseline for comparison; no new integration required |
| Reviewed local packet | One collection/reduction path; no model/auth/runtime dependency | Recommended AG1; can stand alone if it solves the workflow |
| External assistant reads via local MCP | Reuses an existing assistant conversation/model; adds scoped tool transport and compatibility testing | Recommended conditional AG2; no permanent network server |
| devctl invokes an external CLI | Keeps model/auth outside core; adds child lifecycle, host permissions, output parsing and version support | Optional after a real one-click use case; choose one host first |
| Embedded model API and tool loop | Adds credentials, provider/network errors, model settings, spending controls, conversation/tool orchestration and release maintenance | Defer; not justified by current process/log scope |
| Full embedded assistant client | Also adds threads, approvals, streamed events and chat UI | Defer; turns a service manager into another assistant frontend |

MCP exposes devctl evidence to an assistant; a CLI launcher asks an assistant to
perform a task; a model API asks devctl to own the reasoning/tool loop. They solve
different integration problems and are not interchangeable protocols.

## What official sources establish

Sources were opened during this assessment. They establish available integration
mechanisms, not compatibility with every installed client or the usefulness of a
particular diagnosis. Recheck versions and flags at an implementation gate.

| Source | Established capability | Implication for devctl |
| --- | --- | --- |
| [Codex non-interactive mode](https://learn.chatgpt.com/docs/non-interactive-mode) | `codex exec`, stdin context, JSONL events, schema-constrained final output and ephemeral sessions are documented | A CLI adapter can avoid terminal-screen automation; these features do not enforce a diagnosis-only tool set by themselves |
| [Codex MCP integration](https://learn.chatgpt.com/docs/extend/mcp?surface=cli) | Local stdio servers and configured commands/arguments are supported | A local evidence adapter can be used by an existing Codex client without adding model APIs to devctl |
| [Codex app-server](https://learn.chatgpt.com/docs/app-server) | A rich client interface covers authentication, conversations, approvals and events | Suitable for an embedded assistant frontend; unnecessary lifecycle/UI ownership for initial troubleshooting |
| [Claude Code programmatic execution](https://code.claude.com/docs/en/headless) | Non-interactive execution and structured output are documented; tool auto-approval options are separate controls | A second possible host, not a reason to build a generic provider layer now; verify actual tool restriction semantics |
| [MCP 2025-11-25 transports](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports) | Local stdio exchanges framed JSON-RPC messages through a client-launched child; stdout is protocol-only | Prefer the local transport; keep devctl's existing service IPC private behind an adapter |
| [MCP 2025-11-25 tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools) | Tools have input/output schemas; handlers need access checks and input validation; annotations alone are not authoritative | A narrow tool list still needs enforced scope, limits and reduced outputs |
| [Official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk) | A Rust implementation provides local stdio support with selectable transport features | Evaluate/pin minimal server/stdio features at AG2; avoid both a new Node/Python requirement and a hand-written protocol stack |
| [OpenAI function calling](https://developers.openai.com/api/docs/guides/function-calling) | The application executes requested tools and feeds results back in a multi-step interaction | Direct API integration transfers orchestration responsibility to devctl; it is not merely an HTTP call |
| [Anthropic: Building effective agents](https://www.anthropic.com/engineering/building-effective-agents) | The architectural guidance distinguishes fixed workflows from agent-directed loops and recommends starting simply | Supports evaluating a bounded evidence workflow before autonomous repair; it is guidance, not a current SDK compatibility reference |

The dated MCP revision is a reference baseline, not a claim that it is the newest
revision. The Rust SDK also describes newer draft support. Protocol negotiation and
an actual pinned SDK/client pair must be tested before promising compatibility.
No specific model, pricing, subscription entitlement or cloud data-retention policy
is selected or implied by this assessment.

## Repository capability check

Inspected [control requests](../src/control.rs), [client reads](../src/client.rs),
[service state](../src/manager.rs), [log capture](../src/logs.rs),
[archive queries](../src/archive.rs) and [CLI output](../src/main.rs).

| Available today | Gap relevant to AG01 |
| --- | --- |
| Status reports state, instance, error and cleanup confirmation | Loaded command/cwd and structured exit code/signal are not exposed in the response |
| Responses carry a live workspace identity; logs carry session, instance, sequence, time and stream | No single atomic status/log snapshot; separate reads can cross a restart/recreation |
| Bounded literal search, paging, upper sequence and offline history | Agent scope, total disclosure budgets and evidence packets do not yet exist |
| Offline reads obey archive ownership/locking | Historic logs do not establish current process state or necessarily preserve historic command metadata |
| Capture neutralizes terminal controls | It does not remove passwords, authorization values or private business data |
| CLI prints usable status/log pages | Human text is not a stable machine contract; internal replies may include unrelated service states |
| Existing start/stop/save/update operations | These are user controls, not an approved Agent tool surface |

Do not promote the internal TOML socket protocol into a public Agent SDK. Reuse
typed client operations and project only the selected service into the new packet.
Add a small read-only metadata request only if missing execution context proves
necessary; preserve unknown values and compatibility with older supervisors first.

## Local verification and limits

A temporary two-service fixture on Apple Silicon used the existing debug program:
one command emitted a synthetic missing-module error and exited with code 7; the
other remained running. Direct local IPC queries verified matching workspace IDs,
the selected failure's instance/state/error and log sequence/time/stream fields.
The fixture confirmed that a planted, non-real authorization value remains in raw
logs and that status has no command/cwd fields. This validates the need for a new
reduction path, not the correctness of a future masking implementation.

Normal shutdown removed the surviving process. The same fixture's CLI session/log
queries then read the failed command's retained output without starting services.
The test used an isolated configuration/state directory, not the user's projects,
and made no model requests. Local `codex --version` reported **0.145.0**;
`codex exec --help` confirmed stdin, JSON, output-schema, sandbox and ephemeral
options. No authentication, model execution, MCP host or quality evaluation was run.

This demonstrates reusable evidence paths on this Mac. It does not establish WSL
host integration, secret-proof export, assistant action isolation, latency/cost,
diagnostic accuracy or automatic repair. Those remain future implementation checks.

## Evaluation plan before expanding scope

Use the same manually assembled context as the baseline, then compare a packet and,
only if needed, scoped follow-up reads. Keep a small reviewable corpus of at least
ten cases spanning missing executable, immediate exit, occupied port, missing module,
configuration error, application error while Running, noisy/incomplete history,
offline history, restart/stale evidence and insufficient evidence. Include hostile
instructions and planted secrets in fixtures. Use known causes only where a fixture
or actual investigation establishes them; expected abstention is part of the corpus.

Apply the proposed acceptance gates owned by [AG01](requirements.md#agent-assisted-troubleshooting-ag01).
Record the input packet, expected known/unknown facts, evidence references, reviewer
judgment and actual side effects for each case. Include state errors, metadata and
search results in disclosure checks, rather than testing only log bodies. Record
collection latency, follow-up calls, assistant elapsed time and supplied usage/cost.
Exercise cancellation, host unavailability, expiry and output limits while ordinary
service control continues. Keep Mac and WSL observations separate.

No vector search, multi-agent planning, repair automation or benchmark system is
needed to run this initial evaluation. The next product decision is whether to
implement AG1's local evidence workflow; further gates depend on its results.
