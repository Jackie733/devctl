# devctl

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/branding/devctl-mark-on-dark.svg">
  <img src="assets/branding/devctl-mark.svg" alt="devctl: three independent service lines and one control rail" width="96" height="96">
</picture>

A terminal workspace manager for local development services, written in Rust.
Start projects, inspect their logs, and stop them from one place. Leave the terminal
interface without stopping your services, then reconnect when you need it.

[![Release](https://img.shields.io/github/v/release/Jackie733/devctl)](https://github.com/Jackie733/devctl/releases/latest)
[![Release workflow](https://github.com/Jackie733/devctl/actions/workflows/release.yml/badge.svg)](https://github.com/Jackie733/devctl/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**English** | [简体中文](README.zh-CN.md)

## Features

- **One workspace, multiple projects.** Run independent development commands from
  their own directories; start or stop selected services or your default group.
- **Persistent background services.** Closing the TUI or its terminal keeps services
  running. Reconnect to the same supervisor instead of starting duplicate processes.
- **A keyboard-driven TUI.** Navigate with Vim-style keys, switch between projects
  and logs, and use contextual help in wide or compact terminals.
  Since 1.4.0, state symbols, focus/shortcut emphasis, waiting animations, and brief
  completion feedback make operations clearer without requiring Nerd Fonts.
- **Logs for daily debugging.** Read stdout/stderr, pause and page through retained
  history, search literal text, and copy complete lines with their surrounding context (local Mac uses pbcopy).
  Drag to select log text, press `y` or right-click to copy, and use the mouse wheel
  to read earlier/later retained output. See [selection and copying](docs/interaction.md#line-selection-and-copying).
  Wide log panes use local capture times and compact, stable output/run labels.
- **Bounded storage.** Keep recent logs in memory and save history with configurable
  disk limits and retention. Inspect past sessions without starting services.
- **Verified updates.** Download public release manifests, verify SHA-256 and the
  candidate version, and keep an offline rollback copy. In 1.3.0 and later, follow
  terminal stages and download progress, with plain output when redirected.

Version 1.5.0 lets you press **a** in the TUI, paste a project directory, choose an
existing npm/pnpm script or enter your usual command, then review and save. The new
project stays stopped until you start it. First use needs no handwritten config.
`A` starts the default group. CLI `add`/`import` remain available. No environment
versions, tool bindings or dependency checks are required. See
[onboarding usage](docs/interaction.md#adding-projects).

## Platform support

Release **1.8.1** provides native programs for **Linux x86_64** (including **WSL2**),
**macOS 14+ Apple Silicon**. The Linux musl program needs
neither Rust nor a particular glibc version. Intel Mac and native Windows are not supported.
See the [installation guide](docs/install.md) for the matching download.

Commands must run without interactive input. Dependency validation and planning
are available, but runtime dependency orchestration, readiness probes, automatic
restarts, and recovery after a machine reboot are not implemented. See the
[roadmap](docs/roadmap.md) for the distinction between delivered and proposed work.

The core focuses on service lifecycle, logs and lightweight configuration. Project
environment diagnosis, runtime version management and automatic installation have
been removed from the unreleased experiment. Existing managed-field configurations
need [explicit migration](docs/interaction.md#earlier-unreleased-environment-configurations).
Agent-assisted troubleshooting has a [research and architecture proposal](docs/agent-research.md):
notice a problem, preview scoped evidence, analyze and ask follow-up questions inside
the TUI, then inspect cited logs and refresh evidence to verify a user-performed check.
No Agent integration is shipped; service management works offline without it.
Database/Redis automation is pending reassessment. See
[scope](docs/requirements.md#directory-onboarding-and-environment-automation-a01a03).

## Installation

Get your platform binary, external installer, and checksums from
[GitHub Releases](https://github.com/Jackie733/devctl/releases/latest), then follow
the [installation guide](docs/install.md#prebuilt-binary) to verify and install them
into `~/.local/bin/devctl`. No GitHub account, token, or local compilation is needed.

Already running **1.2.1 or later**? Check for an update:

```sh
devctl update --check
```

Before installing or rolling back a version, shut down each active workspace and
leave its TUI. Then run `devctl update`. Updating a file does not replace an already
running supervisor. For 1.0/1.1 installations, or a 1.2.0 client affected by API rate
limits, use the [external installer](docs/install.md#prebuilt-binary) first.

## Quick start

Run `devctl`, press `a`, and enter your project directory (Tab completes paths).
Choose a declared script or enter your normal command, then confirm saving. The new project appears immediately
and stays stopped; existing services keep running. Press `s` to start, `q` to leave
services running, or `Q` to end the workspace. The manual configuration alternative follows.

Put a `devctl.toml` beside your project directories:

```text
workspace/
├── devctl.toml
├── web/
└── api/
```

```toml
version = 1

[workspace]
name = "local-dev"

[projects.web]
path = "./web"

[projects.api]
path = "./api"

[services.web]
project = "web"
cmd = ["npm", "run", "dev"]

[services.api]
project = "api"
cmd = ["npm", "run", "dev"]
```

Replace the directories and commands with your own. Commands are argument arrays;
no shell is inserted. Relative project paths resolve from the configuration file,
not the directory from which you invoke devctl.

From `workspace/`, run:

```sh
devctl validate
devctl
```

The TUI connects to an idle workspace without starting services. Press `s` to start
the selected service, or `A` to start the default group. Services join that group
unless configured with `autostart = false`.

With no `--config`, devctl looks for `./devctl.toml`, `~/devctl.toml`, then
`~/.config/devctl/devctl.toml`, using the first existing file. To select a workspace
explicitly, use `devctl --config /path/to/devctl.toml`.

## Everyday commands

| Command | Purpose |
| --- | --- |
| `devctl` | Open or reconnect the TUI; do not start services automatically |
| `devctl start web` | Start one service, creating the background workspace if needed |
| `devctl start --all` | Start the default group (`autostart = true`) |
| `devctl restart web` | Restart one service after confirmed process-group cleanup |
| `devctl restart --all` | Stop all active services and restart the default group |
| `devctl reload` | Reload `devctl.toml` for stopped services without interrupting running services |
| `devctl status` | Inspect the existing background workspace |
| `devctl logs web --limit 100` | Read a bounded page of service output |
| `devctl log-sessions` | List retained log sessions |
| `devctl logs web --after 0 --contains 'Error'` | Search retained output for literal text |
| `devctl stop web` | Stop one service and clean up its process group |
| `devctl stop --all` | Stop all services while keeping the supervisor available |
| `devctl shutdown` | Stop all services and end the background workspace |
| `devctl update --rollback` | Exchange the installed program and its previous version offline |

All workspace commands accept `--config PATH`. `update`, `--help`, and `--version`
do not need workspace configuration. For foreground operation, use
`devctl run web` or `devctl run --all`; Ctrl-C cleans up the selected services.

Version 1.6.1 adds a [visual refinement](docs/tui-design-research.md):
coherent dark/light/terminal palettes, stable selection without state flashes,
complete contextual hints and motion/symbol fallbacks. See
[appearance options](docs/interaction.md#layout-help-and-confirmation), including
`DEVCTL_TUI_THEME=light devctl` for light terminals.

## TUI shortcuts

| Keys | Action |
| --- | --- |
| `j/k`, `↑/↓`, mouse click/wheel | Select a project or move through logs, depending on focus/pane |
| `h/l`, `Tab`, `1/0` | Switch between projects and logs |
| `Enter`, `Esc` | Enter logs or return to projects, subject to the current mode |
| `s/x/r` | Start, stop, or restart the selected project (`r` reconnects when disconnected) |
| `a`, `e/d` | Add a project, or edit/remove the selected stopped project |
| `u` | Reload `devctl.toml` for stopped services while keeping running services alive |
| `A`, `R`, `X` | Start or restart the default group, or stop all services |
| `c` | Reconnect/sync to the existing background workspace |
| `gg/G`, `PageUp/PageDown` | Navigate logs; `G` resumes following outside selection mode |
| `p` | Pause or resume following logs |
| `/`, `n/N` | Search literal text and jump between matches |
| `v`, `j/k`, `y` | Select complete log lines and request clipboard copy |
| `?` | Open contextual help |
| `q`, `Ctrl-C` | Leave the client; services keep running |
| `Q` | Confirm stopping all services and ending the workspace |

Clipboard copy uses `clip.exe` in local WSL sessions and OSC 52 elsewhere, including
SSH. OSC 52 requires terminal support. See the [usage guide](docs/interaction.md)
for selection modes, search boundaries, pagination, and failure feedback.

## Documentation

- [Logo/icon assets and usage](assets/branding/README.md): the complete service-line
  and control-rail mark for documentation and project identity. The TUI uses a
  compact text header and text-only Help.
- [Installation, updates, rollback, and source builds](docs/install.md)
- [Configuration, CLI, TUI, and log reference](docs/interaction.md)
- [WSL setup and hands-on verification](docs/wsl.md)
- [Architecture and resource limits](DESIGN.md)
- [Requirements and acceptance criteria](docs/requirements.md)
- [Roadmap](docs/roadmap.md) and [changelog](CHANGELOG.md)
- [Release maintenance](docs/releasing.md)

## Contributing

Bug reports, documentation fixes, and focused pull requests are welcome. Read
[CONTRIBUTING.md](CONTRIBUTING.md) for setup, checks, and how to report a problem.
Please discuss substantial behavior changes before implementing them.

Maintainers can run `./scripts/release 1.7.0` from a clean main checkout after
committing reviewed `Unreleased` notes. It updates versions, tests, commits/tags,
pushes and follows publication/public verification. See [release maintenance](docs/releasing.md).

## License

devctl is licensed under the [MIT License](LICENSE).
