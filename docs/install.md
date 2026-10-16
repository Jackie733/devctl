# Installation and updates

The supported release target is Linux x86_64, including WSL2. Use the
[WSL guide](wsl.md) for Windows integration. Releases are public and do not require
GitHub authentication. Current release: **1.2.2**.

## Requirements

- Linux x86_64 and a writable, absolute `HOME`.
- Bash, curl 7.76.0 or newer, and standard Linux tools: `flock`, `mktemp`, `cp`,
  `mv`, `chmod`, `timeout`, `sha256sum`, and `cmp`.
- For the TUI, a terminal with interactive stdin/stdout. Service commands must
  run without interactive input; install their language runtimes separately.

The musl binary does not need Rust or host glibc. Native Windows and macOS release
artifacts are not provided.

## Prebuilt binary

Download the program, external installer, and checksums into a new version-specific
directory. This uses fixed release URLs rather than the GitHub REST API:

```sh
mkdir -p "$HOME/Downloads/devctl-v1.2.2"
cd "$HOME/Downloads/devctl-v1.2.2"
curl --disable --fail --fail-early --location \
  --proto '=https' --proto-redir '=https' \
  --connect-timeout 10 --max-time 180 --remote-name-all \
  'https://github.com/Jackie733/devctl/releases/download/v1.2.2/devctl-v1.2.2-x86_64-unknown-linux-musl' \
  'https://github.com/Jackie733/devctl/releases/download/v1.2.2/devctl-update' \
  'https://github.com/Jackie733/devctl/releases/download/v1.2.2/SHA256SUMS'
sha256sum --check --ignore-missing SHA256SUMS
```

Continue only when download and verification succeed. `--ignore-missing` skips the
source archive and `update.json` if you did not download them. You can also download
the same files from the [release page](https://github.com/Jackie733/devctl/releases/tag/v1.2.2).

For an existing installation, first [shut down active workspaces](#before-switching-versions)
and leave their TUIs. Then install:

```sh
binary_sha=$(awk '$2 == "devctl-v1.2.2-x86_64-unknown-linux-musl" { print $1 }' SHA256SUMS)
bash ./devctl-update ./devctl-v1.2.2-x86_64-unknown-linux-musl "$binary_sha"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
hash -r
command -v devctl
devctl --version
```

In Zsh use `rehash` instead of `hash -r`. The result should be
`~/.local/bin/devctl` and `devctl 1.2.2`. The installer preserves an existing
program as `~/.local/bin/devctl.previous`; a first installation has no backup.
Installing identical bytes again preserves the useful previous version.

To persist PATH, add `export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"` to your
shell's startup file, or run `bash scripts/setup-path.sh` from a source checkout.
Open a new terminal and verify `command -v devctl`. Keep existing project
configuration; installation does not create or replace it.

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

To test the real **1.2.1 → 1.2.2** upgrade, check that your initial version is 1.2.1,
check for an available update, complete the shutdown step, run `devctl update`, and
confirm version 1.2.2. The old binary should be retained as `devctl.previous`.
A subsequent check should report that the running version is current.

A specific stable release can be checked or installed explicitly:

```sh
devctl update --check --version 1.2.2
devctl update --version 1.2.2
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
bash "$HOME/Downloads/devctl-v1.2.2/devctl-update" --rollback
hash -r
devctl --version
```

Configuration and retained logs are preserved. Newly installed code applies to
supervisors started afterward.

## Build from source

Use Linux or WSL2 with Git, Python 3, and a Rust toolchain. CI is tested with Rust
1.98.1; install Rust through the [official instructions](https://rust-lang.org/tools/install/).
On Ubuntu, development tests also need Bash, Zsh, and the normal build tools:

```sh
sudo apt update
sudo apt install -y build-essential curl git python3 zsh
git clone https://github.com/Jackie733/devctl.git
cd devctl
git switch --detach v1.2.2
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
