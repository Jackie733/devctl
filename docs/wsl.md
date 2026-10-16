# WSL setup and verification

Use the **Linux** binary inside WSL2, not a Windows executable. The initial MVP
workflow was accepted on Ubuntu 22.04.5 LTS, WSL2, x86_64. Later logging and updater
changes have controlled test coverage; their complete real-project experience
still needs target-environment acceptance. See [tasks.md](tasks.md).

## Install

In PowerShell, inspect the installed distributions:

```powershell
wsl -l -v
```

In your WSL terminal, check `uname -m` is `x86_64` and follow the
[installation guide](install.md#prebuilt-binary). It covers verified release
installation, PATH, source builds, updates, and rollback.

Use Linux Node.js and your projects' required package manager inside WSL. The
supervisor inherits PATH when it starts; an interactive shell alias is not a service
executable. Prefer project files in the WSL filesystem when using Linux tools,
as described in [Microsoft's filesystem guidance](https://learn.microsoft.com/en-us/windows/wsl/filesystems).

## Configure projects

Keep your existing configuration when upgrading. For a new workspace, follow the
[README example](../README.md#quick-start) and
[configuration reference](interaction.md#configuration). A typical setup has a web
application, a mini-program, and an API, with `npm run dev`, `npm run dev:weapp`, and
`npm run dev` respectively. Replace all directories and commands with your own.

Paths in TOML do not expand `~` or `$HOME`. Use absolute Linux paths or paths relative
to the configuration directory. Shell expansion in a command that writes a config
is separate from devctl path resolution.

Before starting devctl, check each project's scripts and runtime in an ordinary
terminal. For example, from a project directory:

```sh
node --version
npm --version
command -v node npm
npm pkg get scripts engines packageManager
npm run dev
```

Check pre/post scripts, indirect package-manager calls, and interactive prompts.
Install dependencies and perform any project-specific build-script approvals in an
ordinary terminal; devctl does not forward stdin to managed services.

```sh
devctl --config "$HOME/.config/devctl/devctl.toml" validate
devctl --config "$HOME/.config/devctl/devctl.toml" plan
```

Validation does not install dependencies or prove a development server can start.
A plan previews disk configuration; it does not reload an existing supervisor.

## Verify the everyday workflow

1. Open the TUI without starting services. Select one project and press `s`; confirm
   the others stay stopped, then press `a` to start the default group.
2. Inspect each project's stdout/stderr and errors. Stop one service with `x`; the
   others should keep running. Start it again without duplicate processes.
3. Press `q`, confirm services remain active with `devctl status`, and reconnect.
4. Press `X` to stop all services while retaining the supervisor. Use `Q` with
   confirmation, or `devctl shutdown`, to stop services and end the workspace.
5. Repeat with the actual terminal sizes, runtime paths, and commands you use.

`wsl --shutdown` and `wsl --terminate` stop the WSL environment. Background operation
does not promise recovery across WSL shutdown or a Windows reboot.

## Verify logs

- Confirm live output and the `save=saving` indicator. This indicates saving is
  enabled, not that every visible record has already been synchronized to disk.
- Use `p` to pause, check that the new-record count grows without moving the text,
  then page through history. Switch projects and return to the same reading position.
- Search real output with `/`, `Enter`, and `n/N`. Check that ordinary context stays
  visible and that `v/y` copies the expected complete lines. `Esc` clears search.
- Open help and the shutdown confirmation while output continues; check wrapping,
  scrolling, focus, and cancel behavior in wide and small windows.
- Leave with `q` and reopen: a new TUI should follow the latest current-session
  output rather than restore the previous client's old position.
- After a normal shutdown, run `devctl log-sessions` and `devctl logs web --limit 20`.
  Query an actual listed ID with `--session SESSION_ID` after creating a new supervisor.
  Historical queries must not start projects or mix old and new sessions.

Use the [log reference](interaction.md#logs-and-search) for pagination and search
continuation. `.dlog` files are structured segments, not text files for `tail`.
Do not fill disks or corrupt real log files to test failure behavior; controlled
tests cover those paths. Check for save degradation and missing-history feedback
in your normal workload.

## Verify updates

To exercise **1.2.1 → 1.2.2**, begin with the installed 1.2.1 client:

```sh
command -v devctl
devctl --version
devctl update --check
```

The check should find 1.2.2 without requiring workspace configuration. Follow the
[shutdown step](install.md#before-switching-versions), then run:

```sh
devctl update
rehash
devctl --version
devctl update --check
```

Expect 1.2.2 and a current-version result. Bash users can use `hash -r` instead of
`rehash`. Verify `devctl.previous` retains 1.2.1. To test rollback, shut down any
new workspaces first, run `devctl update --rollback`, and confirm 1.2.1. You may then
upgrade again using `devctl update`, or recover using the external installer as
shown in the [rollback guide](install.md#rollback).

For 1.2.0 clients affected by API limits, or 1.0/1.1 clients without `update`, use
[fixed attachments](install.md#prebuilt-binary) for the initial migration. A same-version
reinstallation is useful for integrity checks but does not establish cross-version
upgrade acceptance. Report the actual paths, versions, online checks, rollback,
and real-project results.

## Troubleshooting

### Changed commands or environment do not take effect

Leaving the TUI, stopping one service, or `stop --all` keeps the existing supervisor
and its original configuration and PATH. Shut down the workspace, validate the
intended file, and start a new supervisor. If npm output mentions pnpm, inspect the
project's scripts: an npm script may call a different package manager internally.
Project dependency approvals belong in the project's normal terminal workflow.

### The client still looks like an older version

Check `devctl --version`, `command -v devctl`, and `type -a devctl`. In Zsh,
`whence -va devctl` also shows aliases and all executable candidates. A Cargo install
does not update a higher-priority binary in `~/.local/bin`; restarting the supervisor
alone does not replace the TUI executable. Refresh the shell cache after installation.

### Clipboard copy does not paste

In the same local WSL environment:

```sh
command -v clip.exe
printf 'devctl-copy-test' | clip.exe
```

Paste into a Windows editor to verify interoperability. Local WSL copy uses
`clip.exe`; SSH uses OSC 52 and depends on terminal/multiplexer support. A failed
or timed-out `clip.exe` request keeps the selection and reports the error instead
of silently switching channels. See [line selection and copying](interaction.md#line-selection-and-copying).

### Development checks fail

Use a fresh source checkout and the checks in [CONTRIBUTING.md](../CONTRIBUTING.md).
Filesystem synchronization can be slower on WSL, and interactive shells can print
startup notices. Include full assertion output, Git revision, toolchain, filesystem,
and OS details when reporting a failure. Runtime saving degradation still indicates
potentially incomplete history; inspect the reported I/O or overload reason.
