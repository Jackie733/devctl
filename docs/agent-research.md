# Agent-assisted troubleshooting: research and assessment

Assessed on **2026-10-18** against the current 1.5.0 source. Status: research and
design complete; implementation proposed. [AG01](requirements.md#agent-assisted-troubleshooting-ag01)
owns scope and acceptance; [DESIGN](../DESIGN.md#agent-assisted-troubleshooting-proposed)
owns proposed mechanisms; [roadmap](roadmap.md#ag01-proposed-delivery-gates) owns sequencing.
This record explains the recommendation and evidence, not another source of runtime rules.

## Recommendation and rationale

The user confirmed that analysis, follow-up questions and results should stay inside
devctl. This supersedes the initial export-first recommendation: its technical
sequencing omitted the complete product experience. Evidence preparation remains
necessary infrastructure, but exporting a packet is not the first product milestone.

Recommend a focused diagnosis workspace, with logs and analysis side by side on wide
terminals and separate views on compact terminals. A user-triggered case carries
stable service/attempt identity, explicit disclosure and linked evidence throughout
its conversation. The terminal interaction is specified in
[interaction](interaction.md#agent-assisted-troubleshooting-proposed).

Evaluate one local engine adapter first, provisionally Codex app-server, for its
conversation/event/interruption interface. Selection is conditional on proving that
all evidence access can be restricted to the host broker. If native tools cannot be
restricted, a direct API with host-owned tools is the alternative to assess. Keeping
a general assistant's unrestricted tools would violate the proposed product boundary.
This is a reasoned recommendation, not a measured quality, cost or compatibility claim.

## Options considered

| Option | Fit for the confirmed experience | Assessment |
| --- | --- | --- |
| Manual copying / reviewed packet | Requires leaving devctl for conversation and results | Baseline or optional support artifact; insufficient as the first delivery |
| Public read-only MCP server | Supplies evidence to another assistant UI; does not provide the required TUI conversation | Optional future consumer, not an architectural prerequisite |
| One-shot CLI execution | Can stream a single answer, but follow-ups and cancellation need additional lifecycle work | Useful experiment; not the default conversation contract |
| Local engine client interface | Supports turns, streamed messages and interruption while devctl owns the UI | First feasibility candidate; verify exact version and native-tool isolation |
| Direct model API with narrow host tools | Gives devctl direct control over tool dispatch; adds authentication and turn-loop maintenance | Alternative if the engine cannot enforce the evidence boundary |
| General embedded coding assistant | Adds unrestricted editing, execution, sessions, providers and plugins | Exceeds the diagnosis use case; unnecessary for the agreed TUI workflow |

The UI requirement justifies a small conversation state model; it does not justify
a general chat product, an Agent platform or automatic repair. An external engine
process can perform reasoning while devctl owns the complete interaction.

## What official sources establish

Sources were opened during this assessment. They establish available integration
mechanisms, not compatibility with every installed client or the usefulness of a
particular diagnosis. Recheck versions and flags at an implementation gate.

| Source | Established capability | Implication for devctl |
| --- | --- | --- |
| [Codex non-interactive mode](https://learn.chatgpt.com/docs/non-interactive-mode) | `codex exec`, stdin context, JSONL events, schema-constrained final output and ephemeral sessions are documented | A CLI adapter can avoid terminal-screen automation; these features do not enforce a diagnosis-only tool set by themselves |
| [Codex MCP integration](https://learn.chatgpt.com/docs/extend/mcp?surface=cli) | Local stdio servers and configured commands/arguments are supported | A local evidence adapter can be used by an existing Codex client without adding model APIs to devctl |
| [Codex app-server](https://learn.chatgpt.com/docs/app-server) | A rich client interface covers authentication, conversations, approvals and events | Fits the confirmed integrated TUI; custom dynamic tools are experimental and do not disable built-in tools |
| [Claude Code programmatic execution](https://code.claude.com/docs/en/headless) | Non-interactive execution and structured output are documented; tool auto-approval options are separate controls | A second possible host, not a reason to build a generic provider layer now; verify actual tool restriction semantics |
| [MCP 2025-11-25 transports](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports) | Local stdio exchanges framed JSON-RPC messages through a client-launched child; stdout is protocol-only | Prefer the local transport; keep devctl's existing service IPC private behind an adapter |
| [MCP 2025-11-25 tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools) | Tools have input/output schemas; handlers need access checks and input validation; annotations alone are not authoritative | A narrow tool list still needs enforced scope, limits and reduced outputs |
| [Official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk) | A Rust implementation provides local stdio support with selectable transport features | Relevant only if a future external MCP consumer is justified; not needed for the proposed first workflow |
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
Add a narrow read-only loaded-metadata request for diagnosis; preserve unknown values
and compatibility with older supervisors. Never pass unrelated response fields through.

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

For the revised design, `codex app-server --help` and
`codex app-server generate-json-schema --experimental --out /tmp/devctl-agent-appserver-schema`
were run with local version 0.145.0. Generated schemas include thread/turn start,
turn interrupt and dynamic tool callbacks; thread start includes dynamic tools,
ephemeral mode and permission/sandbox settings. Help labels the app-server interface
experimental. No server, login or model turn was started. Generated schemas establish
message shapes, not effective permissions or a working integration.

The official app-server documentation describes multi-turn events and interruption,
and labels dynamic tools experimental. Its shell/process escape interfaces must not
be exposed by this diagnosis adapter. Merely supplying custom tools or selecting a
read-only sandbox does not establish the required scope isolation. A compatibility
spike must verify that claim before the adapter can be chosen.

This demonstrates reusable evidence paths on this Mac. It does not establish WSL
host integration, secret-proof export, assistant action isolation, latency/cost,
diagnostic accuracy or automatic repair. Those remain future implementation checks.

## Evaluation plan before expanding scope

Use manual copying and an external conversation as the baseline, then compare the
complete integrated workflow, including follow-up questions and cited-log navigation. Keep a small reviewable corpus of at least
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

Also exercise terminal sizes around 50-by-14 and 120-by-24, text-input shortcuts,
selection-to-diagnosis, denial of extra access, pasted input, late engine events,
project switching and service restarts during analysis. Measure time to a useful
answer and context switches, not just whether a packet was produced.

No vector search, multi-agent planning, repair automation or benchmark system is
needed. Implementation remains proposed; engine feasibility is the first engineering
gate within one complete in-TUI product increment.
