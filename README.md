# devctl

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
- **Bounded storage.** Keep recent logs in memory and save history with configurable
  disk limits and retention. Inspect past sessions without starting services.
- **Verified updates.** Download public release manifests, verify SHA-256 and the
  candidate version, and keep an offline rollback copy. In 1.3.0 and later, follow
  terminal stages and download progress, with plain output when redirected.

Source 1.5.0 includes `devctl import DIR`: discover Node projects, review selected
scripts and exact versions, then save workspace configuration with `--apply`.
Preview preserves files; apply preserves existing edits unless explicitly replaced.
Import does not install dependencies or start services. See
[import usage](docs/interaction.md#reviewed-configuration-import-m73).
For explicitly configured Node services, `doctor` checks declarations and local
tools, and `setup --node /absolute/path/to/node` previews an existing-tool binding.
`setup --apply` confirms that binding; managed starts then use those exact tools.
See [environment usage](docs/interaction.md#local-node-environments-m72).

## Platform support

The supported release target is **Linux x86_64**, including **Windows WSL2**.
Release binaries use musl and do not require Rust or a particular glibc version.
The 1.5.0 source tree adds native **macOS 14+ (Apple Silicon and Intel)**.
The current public 1.4.0 release is Linux-only; use the [Mac source installation](docs/install.md#macos)
until multi-platform artifacts are published. Native Windows remains unsupported.

Commands must run without interactive input. Dependency validation and planning
are available, but runtime dependency orchestration, readiness probes, automatic
restarts, and recovery after a machine reboot are not implemented. See the
[roadmap](docs/roadmap.md) for the distinction between delivered and proposed work.

Planned automation will accept a project directory, derive startup configuration,
check and help install its environment/dependencies, and discover, monitor and
control database/Redis services. Read-only discovery, environment checks and local
tool binding plus reviewed configuration import are implemented so far. Database
creation, migrations and application configuration generation are outside this
increment. M7 now has a [detailed design](DESIGN.md#planned-directory-onboarding-and-environment-preparation-m7)
and [planned command flow](docs/interaction.md#planned-directory-workflow-m7):
read-only discovery, reviewed import, environment checks and separately reviewed
setup. The initial proposal covers Node with npm/pnpm on Mac and Linux/WSL, preserves
existing configuration edits, and applies imports only while the workspace is
stopped. Tool/dependency installation remains pending; current
setup only binds existing tools. See [scope and acceptance](docs/requirements.md#directory-onboarding-and-environment-automation-a01a03).

## Installation

For the current Linux release, get the binary, external installer, and checksums from
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
the selected service, or `a` to start the default group. Services join that group
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

## TUI shortcuts

| Keys | Action |
| --- | --- |
| `j/k`, `↑/↓` | Select a project or move through logs, depending on focus |
| `h/l`, `Tab`, `1/0` | Switch between projects and logs |
| `Enter`, `Esc` | Enter logs or return to projects, subject to the current mode |
| `s/x` | Start or stop the selected project |
| `a`, `X` | Start the default group or stop all services |
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

## License

devctl is licensed under the [MIT License](LICENSE).
