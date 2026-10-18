# TUI refinement research and proposal

Date: 2026-10-18. Status: initial visual refinement included in version 1.6.1; real-platform user acceptance remains outstanding. Scope: [U01](requirements.md#initial-scope-and-acceptance), M5 in the
[roadmap](roadmap.md), with existing L01/L03 log behavior and A01 onboarding.
This document records design rationale and candidate specifications. Approved
interaction rules belong in [interaction.md](interaction.md); mechanisms belong in
[DESIGN](../DESIGN.md). It does not replace those contracts or the proposed AG01 flow.

## Goal and completion criteria

Make selected service, keyboard focus, service status and log reading mode immediately
distinguishable without distracting from logs. This research is complete when current
behavior is checked, primary third-party references are compared, a concrete palette,
icon and motion proposal is specified, and delivery/verification boundaries are clear.
Implementation and real WSL/macOS visual acceptance are separate work.

## Baseline before this refinement

Source inspection: `src/tui/view.rs`, `app.rs`, `mod.rs`, and existing interaction rules.
The current implementation already provides:

- Two panes at 80 columns / 12 rows and above; a single focused pane below that;
  minimum usable size 36 by 10. The project pane is 24–32 columns wide.
- State symbols and labels, cyan focus borders, gray selection backgrounds, yellow
  shortcut emphasis, separate follow/pause/selection indicators, and contextual help.
- A ten-frame spinner advancing every 100 ms while busy, 300 ms confirmed state
  emphasis, and two-second success feedback. Presentation does not advance service
  state or log reading position. A 33 ms draw timer does not mean every tick redraws.
- Persistent failure indicators and an explicit leave-versus-end-workspace distinction.

Improvement opportunities inferred from the rendering rules (not user-study findings):

- Yellow covers shortcuts, search, waiting and warnings; cyan covers both focus and
  other information. These roles can compete for attention.
- A service's state occurs in several places; long log prefixes contain source,
  instance and sequence information on every line. Useful evidence costs reading space.
- Main panes inherit terminal colors while dialogs force white-on-black. This makes
  a coherent light appearance difficult. Named terminal colors vary with user themes.
- Long contextual shortcut strings can be clipped. The global help/leave area already
  has reserved space; improve contextual prioritization rather than replacing it.
- Existing symbols and restrained motion are a good foundation, not missing features.

## Implementation status

The user authorized implementation and explicitly rejected the white state flash.
Version 1.6.1 removes that effect and its timer entirely. It adds coherent dark,
light and terminal palettes, stable project selection across focus changes, complete
contextual footer actions, reduced-motion waiting symbols, ASCII status symbols and
monochrome selection/search emphasis. See the current
[interaction contract](interaction.md#layout-help-and-confirmation) for options.

The specification below records the researched direction; implemented contracts take
precedence. Delaying spinners by 150 ms, compacting log metadata and AG01 integration
remain later work. Ordinary waiting retains its existing 100 ms animation cadence.
No comparative CPU benchmark or physical WSL/macOS glyph acceptance is claimed.

## Reference products

Sources inspected on the date above are official documentation/repositories. Product
judgments below are design interpretations, not comparative usability measurements.
No third-party application was installed or benchmarked.

| Reference | Documented pattern | Apply to devctl | Avoid copying |
| --- | --- | --- | --- |
| [Lazydocker](https://github.com/jesseduffield/lazydocker) | Centralized service management with common commands a keypress away | Keep selected service and its logs together; prioritize frequent operations | Docker-specific panels and resource graphs without a confirmed need |
| [Lazygit configuration](https://github.com/jesseduffield/lazygit/blob/master/docs/Config.md) | Separate focused/unfocused borders, selected-line styles, dark/light theme overrides | Distinguish focus, selection and state; style dialogs consistently | Its number of panels and extensive customization surface |
| [K9s skins](https://k9scli.io/topics/skins/) | Separate body, border focus, menu key, status and view colors; terminal-default background | Define colors by meaning; provide terminal-background compatibility | Cluster-oriented information density and additional navigation concepts |
| [Gum](https://github.com/charmbracelet/gum) and [Bubbles](https://github.com/charmbracelet/bubbles) | Prompts, selection, spinners, status messages and compact/expanded help | Consistent add-project steps, local waiting feedback, short contextual help | Decorative motion or switching the existing Rust UI stack |
| [bottom](https://github.com/ClementTsang/bottom), [theme configuration](https://github.com/ClementTsang/bottom/blob/main/sample_configs/default_config.toml) | Built-in light/dark themes, basic mode, individual widget expansion | Clear appearance options and deliberate compact layouts | Monitoring charts unrelated to the current service workflow |

Recommended direction: Lazydocker's task focus, Lazygit's focus hierarchy, K9s's
semantic styling, and Charm's small local feedback components. Their libraries are
not dependencies proposed for devctl.

## Proposed layout and interaction

Preserve current keyboard meanings and wide/compact breakpoints initially.

1. Header: product/workspace identity and connection state. Keep version and opaque
   identity secondary; do not imply that a connected supervisor means a healthy app.
2. Project pane: selection marker, name, status symbol and status word. Focus uses
   the pane border/title; selection uses a row background/marker. Moving focus to logs
   must not erase which project is selected. No animation on cursor movement.
3. Log pane: selected service in the title; a stable mode label for FOLLOW, PAUSED,
   SEARCH or SELECT. Keep history/loading position visible without changing row height.
   Treat a running process as Running, never Ready/Healthy without readiness evidence.
4. Reserve one feedback row: persistent failures and unknown state outrank transient
   success. Truncation must retain severity and a route to complete diagnostics in help.
5. Footer: show a small set of context-relevant actions, hide lower-priority hints as
   whole items when space is tight, preserve help and leave access. In Projects prioritize
   start/stop, enter logs and add; in Logs prioritize search, selection/copy and follow.
   Keep all existing actions available through help and preserve input-mode precedence.
6. Add-project and confirmation dialogs use the same surfaces and action hierarchy.
   Preserve directory → command → review/save, cancellation, edit protection and
   save-without-start semantics. Highlight scope before destructive confirmation.
7. Optional later refinement: compact the visual log prefix while retaining source,
   instance transitions, old-output markers and access to full record metadata. Never
   change copied evidence or hide data-loss notices. Requires a separate interaction
   review; do not silently remove currently visible diagnostic information.
8. Leave space for the already proposed AG01 log/analysis split only when invoked;
   do not add a permanently empty Agent panel in this visual update.

## Proposed color system

Recommended default candidate: Calm Dark. Offer Light and Terminal as coherent
alternatives, not an extensive theme gallery. Explicit selection is preferable to
unreliable automatic detection for the first delivery. Configuration placement is
an implementation decision to settle within existing conventions.

| Role | Calm Dark | Light | Use |
| --- | --- | --- | --- |
| Background | `#111827` | `#F8FAFC` | Main canvas |
| Surface | `#1F2937` | `#FFFFFF` | Dialogs / secondary surfaces |
| Text | `#E5E7EB` | `#0F172A` | Logs and primary labels |
| Secondary text | `#9CA3AF` | `#475569` | Metadata, never essential warnings |
| Focus/accent | `#67E8F9` | `#0E7490` | Active pane and shortcut keys |
| Selected background | `#374151` | `#E2E8F0` | Preserve state foreground colors |
| Running/success | `#86EFAC` | `#166534` | Process running / completed request |
| Waiting/warning | `#FCD34D` | `#92400E` | Busy and actionable caution, with distinct words |
| Failure | `#FDA4AF` | `#9F1239` | Failure and important diagnostic labels |
| Selection mode | `#D8B4FE` | `#6B21A8` | Explicit multi-line selection mode |

Search matches use dark text on amber in the dark theme and dark text on pale amber
in the light theme; current match additionally uses bold/underline. Selection and
search overlap need an explicit combined style, not whichever paint occurs last.
Terminal mode inherits foreground/background, uses limited ANSI role colors and
retains textual/shape distinctions. Provide a monochrome fallback; do not assume
existing installer `NO_COLOR` behavior already covers the TUI. Validate actual
terminal rendering; hexadecimal contrast alone cannot verify font or terminal output.

## Proposed icon vocabulary

Keep symbols paired with words. Default to common Unicode without Nerd Font or emoji;
include an ASCII alternative for missing glyphs or inconsistent character width.

| Meaning | Unicode | ASCII alternative |
| --- | --- | --- |
| Running | `● Running` | `* Running` |
| Stopped | `○ Stopped` | `- Stopped` |
| Starting / stopping | Existing braille spinner + state word | Rotating `| / -` + state word |
| Failed | `× Failed` | `x Failed` |
| Unknown / disconnected | `? Unknown` | `? Unknown` |
| Completed request | `✓ Saved` / `✓ Copied` | `OK Saved` / `OK Copied` |
| Selected project | `▸` + row treatment | `>` + row treatment |

Use words for log modes; avoid a second decorative icon family. Technology logos,
colored emoji, and a required special font add little to identifying local services.
Unicode is a compatibility target, not a guarantee across every installed font.

## Proposed motion rules

| Trigger | Behavior | Boundary |
| --- | --- | --- |
| Immediate key action | Update focus/selection immediately | No slide, spring, fade or animated scrolling |
| Background request | Immediate text acknowledgement; spinner if still pending after ~150 ms | Roughly 100 ms per frame; fixed-width slot |
| Confirmed state change | Update status symbol and label; preserve background | User rejected transition flashes; no temporary emphasis |
| Success | Check mark + action result for 2 seconds | Persistent failures always outrank success |
| Failure / disconnect | Stable text and symbol | Never vanish on a success timer or pulse indefinitely |
| Reduced motion | Static busy symbol + state text; no transition emphasis | Preserve acknowledgements and final results |

The delay and durations are proposed starting values, not validated perceptual limits.
No fake percentage for unknown-duration startup, no animation that implies readiness,
and no motion that changes log anchors or selection. Reuse the existing presentation
clock and render-on-change behavior; measure idle/busy overhead before optimizing it.

## Delivery order and verification

1. First: coherent color roles, dialogs, focus/selection, state symbols and footer
   prioritization. Preserve controls and service behavior. Verify dark/light/terminal
   appearances and monochrome distinctions before choosing the final default.
2. Second: refine feedback timing, local state emphasis, reduced motion and symbol
   fallback. Keep animations bounded and disabled when idle.
3. Later, subject to scope agreement: compact log metadata and AG01 integration.
   No command palette, resource dashboard or theme editor is implied by this proposal.

Acceptance scenarios for implementation:

- 120×30, 80×24, 60×18 and 36×10; resize during search, selection and dialogs;
  no clipped critical actions or invisible confirmation, no changed log reading anchor.
- Start one service, observe confirmed running, stop it, simulate failure and disconnect;
  selected and unfocused states remain identifiable without color.
- Search, pause, copy selected lines, return to follow; failure and log-loss notices
  remain discoverable throughout. Success messages do not suppress unresolved problems.
- Add a project with long/Chinese/spaced paths, edit/review/cancel/save; retain existing
  input handling and avoid treating input characters as global shortcuts.
- Default fonts in macOS terminals and Windows Terminal/WSL, light/dark backgrounds,
  ASCII/Unicode, reduced motion and color fallback. Document actual tested environments.
- Check palette contrast for essential text on every surface including selection;
  assess glyph width visually; measure idle and busy repaint/CPU behavior relative to
  the current version. Define performance acceptance from that baseline, not invented data.
- Reuse existing TUI unit and PTY tests; add only behavior gaps. Functional delivery
  must also pass formatting, clippy and impact-appropriate integration tests.

Initial research validation: 70 baseline TUI unit tests and the existing navigation PTY scenario
passed; wide/compact text frames were inspected. At 60×18 the footer visibly truncated
`a add` to `a ad`, supporting whole-item hint prioritization. These checks establish the current
baseline, not the proposed design's appearance or user acceptance. No pixel screenshot,
new theme rendering, third-party runtime comparison or actual WSL visual test was done.


## Follow-up: startup state and complexity

Investigated on 2026-10-18 after the user observed immediate Running followed by a
startup failure, then asked how mainstream tools handle readiness without growing
language-specific complexity. These are comparisons and recommendations. The user explicitly deferred this
feature on 2026-10-18 to release the visual refinement first; no readiness behavior
is implemented or accepted.

| Official source | Mechanism | What it establishes |
| --- | --- | --- |
| [Supervisor](https://supervisord.org/configuration.html#program-x-section-settings) | `startsecs`: STARTING until a process survives the interval (default one second) | Process startup stability; not application readiness |
| [Docker Compose](https://docs.docker.com/compose/how-tos/startup-order/) | Default startup waits for running; explicit `healthcheck` and `service_healthy` can gate dependencies | Running and ready are separate; project supplies a meaningful check |
| [Kubernetes](https://kubernetes.io/docs/concepts/workloads/pods/probes/) | Configured startup/readiness/liveness checks via HTTP, TCP, commands or gRPC | Distinct checks have distinct meanings and lifecycle effects |
| [PM2](https://pm2.keymetrics.io/docs/usage/signals-clean-restart/) | Optional `wait_ready` with an application ready message; also documents a Node HTTP listen hook | Application cooperation or runtime-specific integration supplies readiness information |

Design inference: complexity grows most when devctl tries to infer every framework's
log messages and lifecycle. A small set of explicit, language-independent checks
bounds the implementation, although selecting the right check remains per-project
work. A TCP connection proves only that something accepts a connection; an HTTP
response proves only the specified endpoint condition. Neither guarantees every
feature works or that the application will not fail later.

Recommended staged scope:

- Optional small first improvement: a short process-start observation interval with
  a spinner, then Running if the process survives. Label this as process state;
  do not call it readiness or claim it catches all delayed startup failures. The
  interval value and whether to adopt this behavior remain unconfirmed.
- For projects needing explicit readiness, provide an optional startup confirmation
  independent of implementation language. Web/API projects can use a designated
  application URL; other projects may need an explicit success log. Choose the
  smallest supported mechanism from confirmed real project needs before coding.
- Waiting for configured evidence shows a spinner; evidence success establishes
  startup confirmation. Process exit and check timeout remain distinct outcomes.
  Without a configured check, show that application readiness was not verified;
  do not keep an indefinite readiness spinner or fabricate success.
- Keep continuous health monitoring, automatic restart, dependency ordering and
  framework adapters outside this immediate UI refinement. Historical candidate
  scope is preserved for later assessment, not cancelled.

Current-code verification: a local isolated application that only printed a preparing
message, waited 1.2 seconds and exited with code 1 was observed as Running → Failed.
Cleanup succeeded. This reproduces the user's symptom and confirms the spawn/readiness
boundary; no real project or installed binary was changed by this investigation.
