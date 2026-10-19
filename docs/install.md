# Installation and updates

Current public release: **1.9.1**, for Linux x86_64 including WSL2, macOS 14+
Apple Silicon. Intel Mac is not supported. Use the [WSL guide](wsl.md) for Windows
integration. Releases are public and do not require GitHub authentication.
Historical 1.4.0 attachments are Linux-only. Native Windows is not supported.

## Requirements

For Linux / WSL2:

- Linux x86_64 and a writable, absolute `HOME`.
- Bash, curl 7.76.0 or newer, and standard Linux tools: `flock`, `mktemp`, `cp`,
  `mv`, `chmod`, `timeout`, `sha256sum`, and `cmp`.
- For the TUI, a terminal with interactive stdin/stdout. Service commands must
  run without interactive input; install their language runtimes separately.

The musl binary does not need Rust or host glibc. On macOS, the native program uses
system libraries; installation/update additionally use the system Bash, Perl,
`shasum`, curl and normal BSD file tools. Homebrew GNU tools are not required.
Project language runtimes and dependencies are installed separately.

## Prebuilt binary

Download the program, external installer, and checksums into a new version-specific
directory. This uses fixed release URLs rather than the GitHub REST API:

```sh
mkdir -p "$HOME/Downloads/devctl-v1.9.1"
cd "$HOME/Downloads/devctl-v1.9.1"
curl --disable --fail --fail-early --location \
  --proto '=https' --proto-redir '=https' \
  --connect-timeout 10 --max-time 180 --remote-name-all \
  'https://github.com/Jackie733/devctl/releases/download/v1.9.1/devctl-v1.9.1-x86_64-unknown-linux-musl' \
  'https://github.com/Jackie733/devctl/releases/download/v1.9.1/devctl-update' \
  'https://github.com/Jackie733/devctl/releases/download/v1.9.1/SHA256SUMS'
sha256sum --check --ignore-missing SHA256SUMS
```

Continue only when download and verification succeed. `--ignore-missing` skips the
source archive and `update.json` if you did not download them. You can also download
the same files from the [release page](https://github.com/Jackie733/devctl/releases/tag/v1.9.1).

For an existing installation, first [shut down active workspaces](#before-switching-versions)
and leave their TUIs. Then install:

```sh
binary_sha=$(awk '$2 == "devctl-v1.9.1-x86_64-unknown-linux-musl" { print $1 }' SHA256SUMS)
bash ./devctl-update ./devctl-v1.9.1-x86_64-unknown-linux-musl "$binary_sha"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
hash -r
command -v devctl
devctl --version
```

In Zsh use `rehash` instead of `hash -r`. The result should be
`~/.local/bin/devctl` and `devctl 1.9.1`. The installer preserves an existing
program as `~/.local/bin/devctl.previous`; a first installation has no backup.
Installing identical bytes again preserves the useful previous version.

To persist PATH, add `export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"` to your
shell's startup file, or run `bash scripts/setup-path.sh` from a source checkout.
Open a new terminal and verify `command -v devctl`. Keep existing project
configuration; installation does not create or replace it.

## macOS

Download the native 1.9.1 program, installer and checksums into a new directory.
Use `aarch64-apple-darwin` for Apple Silicon; no Intel Mac artifact is published:

```sh
mkdir -p "$HOME/Downloads/devctl-v1.9.1"
cd "$HOME/Downloads/devctl-v1.9.1"
devctl_target=aarch64-apple-darwin
curl --disable --fail --fail-early --location \
  --proto '=https' --proto-redir '=https' \
  --connect-timeout 10 --max-time 180 --remote-name-all \
  "https://github.com/Jackie733/devctl/releases/download/v1.9.1/devctl-v1.9.1-$devctl_target" \
  'https://github.com/Jackie733/devctl/releases/download/v1.9.1/devctl-update' \
  'https://github.com/Jackie733/devctl/releases/download/v1.9.1/SHA256SUMS'
shasum -a 256 --check --ignore-missing SHA256SUMS
# For an existing installation, complete the shutdown step below first.
devctl_binary="devctl-v1.9.1-$devctl_target"
devctl_sha=$(awk -v name="$devctl_binary" '$2 == name { print $1 }' SHA256SUMS)
bash ./devctl-update "./$devctl_binary" "$devctl_sha"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
rehash
command -v devctl
devctl --version
```

Continue only after checksum verification succeeds. The same shutdown, backup and
rollback rules apply on both systems. Do not substitute a Linux artifact. Local Mac
copying uses pbcopy; over SSH it uses the displaying terminal's OSC 52 support.

For a source build, use a checkout of v1.9.1. Source builds require Rust
and Apple's Command Line Tools (`xcode-select --install` if absent); development
checks also require Python 3. The deployment baseline is macOS 14. Local acceptance
was performed on Apple Silicon/macOS 26.5.1; see [current evidence](tasks.md) for
platform and publication verification status.

```sh
MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --release --locked
./target/release/devctl --version
# For an existing installation, complete the shutdown step below first.
devctl_build_sha=$(shasum -a 256 target/release/devctl | awk '{print $1}')
bash scripts/devctl-update ./target/release/devctl "$devctl_build_sha"
bash scripts/setup-path.sh zsh
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
rehash
command -v devctl
devctl --version
```

For Bash, pass `bash` to setup-path.sh and use `hash -r`. Open a new terminal to
verify persistent command discovery. These steps preserve existing configuration;
create a workspace using the [quick start](../README.md#quick-start) and run
`devctl validate` before starting services. Commands inherit the supervisor's
original PATH; ensure it resolves the intended Node/package-manager version before
launching the workspace. Configuration does not expand `~` or environment variables.

Keep the configuration in a short, writable local path, such as
`~/.config/devctl/devctl.toml`. Its canonical path plus `.devctl/control.sock` must
fit the OS Unix-socket limit (104 bytes on macOS). A deeply nested checkout or the
long default macOS temporary directory can exceed it; moving the configuration
changes workspace/log identity, so shut down the old workspace first.

The current release manifest contains the Apple Silicon Mac target, so public `update --check` and `update`
select the native artifact. The historical 1.4.0 manifest has no Mac asset.

## Before switching versions

An installed binary and a running supervisor are separate. Replacing the file does
not update existing supervisors or their loaded configuration and environment.
When it is convenient to stop your services, run this for **each active configuration**:

```sh
devctl --config /path/to/devctl.toml shutdown
```

Replace the path with your configuration. Confirm cleanup succeeds and leave any
remaining TUI clients before installing or rolling back. `q` only leaves the client;
`stop --all` stops services but keeps the supervisor. Neither reloads configuration.
Do not continue through an unconfirmed cleanup error.

## Update

Clients **1.2.1 and later** use public Release manifests without calling the GitHub
REST API:

```sh
devctl update --check
# After shutting down active workspaces and leaving their TUIs:
devctl update
devctl --version
```

To test the real **1.5.0 → 1.6.1** upgrade, check that your initial version is 1.5.0,
check for an available update, complete the shutdown step, run `devctl update`, and
confirm version 1.6.1. The old binary should be retained as `devctl.previous`.
A subsequent check should report that the running version is current. Terminal
progress is provided by clients 1.3.0 and later; the initial upgrade from 1.2.2
still uses the older client's output.

A specific stable release can be checked or installed explicitly:

```sh
devctl update --check --version 1.9.1
devctl update --version 1.9.1
```

Explicit selection permits same-version installation and downgrades. The selected
release must contain `update.json`; use the external installer for older releases
without a manifest. The complete command contract is in the
[update reference](interaction.md#self-update).

Versions 1.0/1.1 have no `update` command. Version 1.2.0 uses the anonymous REST API
and may hit its shared IP quota. Use [fixed attachments](#prebuilt-binary) to migrate
those clients; configuring a GitHub token is not required or supported by the updater.

## Rollback

After shutting down active workspaces and leaving the TUI:

```sh
devctl update --rollback
hash -r
devctl --version
```

Rollback exchanges the installed program and its single backup without networking.
If there is no backup, it fails without changing the current program. Repeated
rollback swaps the two versions; it is not an unlimited history.

If the restored client has no update command or still uses the old API, restore the
new program using the verified installer you downloaded earlier:

```sh
bash "$HOME/Downloads/devctl-v1.9.1/devctl-update" --rollback
hash -r
devctl --version
```

Configuration and retained logs are preserved. Newly installed code applies to
supervisors started afterward.

## Build from source

Use Linux or WSL2 with Git, Python 3, and a Rust toolchain. Mac users can use the [Mac instructions](#macos). CI is tested with Rust
1.98.1; install Rust through the [official instructions](https://rust-lang.org/tools/install/).
On Ubuntu, development tests also need Bash, Zsh, and the normal build tools:

```sh
sudo apt update
sudo apt install -y build-essential curl git python3 zsh
git clone https://github.com/Jackie733/devctl.git
cd devctl
git switch --detach v1.9.1
cargo build --release --locked
./target/release/devctl --version
```

Do not use `--offline` for a fresh checkout until dependencies are cached. Rust is
needed to build, not to run a downloaded release. A source archive can be used
instead of Git; extract it into a separate directory and omit Git commands.

After the [shutdown step](#before-switching-versions), install your local build:

```sh
devctl_build_sha=$(sha256sum target/release/devctl | awk '{print $1}')
bash scripts/devctl-update ./target/release/devctl "$devctl_build_sha"
```

This keeps the same installation path and rollback rules as release installation.
`cargo install --path . --locked` normally writes to `~/.cargo/bin`; it does not
replace a higher-priority `~/.local/bin/devctl`. Development checks are documented
in [CONTRIBUTING.md](../CONTRIBUTING.md).

## Troubleshooting

- Check `command -v devctl`, `devctl --version`, and `type -a devctl` for multiple
  installations, aliases, or a stale shell command cache.
- For HTTP errors, verify access to GitHub and release downloads. Retry after
  temporary service limits; the updater does not silently switch sources or compile.
- For old API quota errors, migrate with fixed attachments as described above.
- For a checksum, length, or candidate-version error, keep the current installation
  and obtain a fresh matching set of release files.
- For Linux execution errors, check `uname -m` and that the file is a Linux artifact.
- For lock or symlink errors, inspect the reported path instead of deleting locks
  or replacing links while another update is active.

If the problem persists, follow the [bug reporting guide](../CONTRIBUTING.md#reporting-bugs).
