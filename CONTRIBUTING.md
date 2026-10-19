# Contributing to devctl

Thank you for helping improve devctl. Documentation fixes, reproducible bug reports,
and focused changes are welcome.

## Before you start

Check [existing issues](https://github.com/Jackie733/devctl/issues), the
[requirements](docs/requirements.md), and the [roadmap](docs/roadmap.md).
Open an issue before implementing a substantial feature or changing workspace
behavior. A proposal on the roadmap is not a commitment to implement it.

Use the same feature descriptions, commands, and configuration examples in
[README.md](README.md) and [README.zh-CN.md](README.zh-CN.md). Keep the main user and
maintainer references in English.

## Development setup

Use Linux/WSL2 or macOS 14+, Git, Rust, Python 3, and the usual build tools (Apple Command Line Tools on Mac). The release
workflow is tested with Rust 1.98.1. `sh`, `sleep`, Bash, and Zsh are used by
integration and installation tests. See [source builds](docs/install.md#build-from-source)
for setup commands.

```sh
git clone https://github.com/Jackie733/devctl.git
cd devctl
cargo build --locked
cargo run -- --help
```

`Cargo.lock` is committed. Start without `--offline` so Cargo can fetch dependencies;
use it later only when the dependencies are already cached.

## Checks

For code changes, run:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 tests/support/install_scripts.py
python3 tests/support/merge_release.py
python3 tests/support/release_command.py
python3 tests/support/package_release.py ./target/debug/devctl
```

Tests need Linux or macOS with local Unix sockets, PTYs, process groups, and signals. They use temporary configurations and controlled child
processes rather than your development projects. Clipboard tests use a fake
`clip.exe` or captured OSC 52 output.

For documentation changes, check relative links, heading anchors, code fences,
example syntax, and consistency with `devctl --help` and the configuration parser.
Do not add implementation-mirroring tests for prose edits. Release automation
runs the full test suite and validates the actual static artifact.

## Pull requests

- Keep one clear purpose per PR. Explain the problem, resulting behavior, and checks.
- Follow existing Rust conventions; prefer a small explicit change over a new abstraction.
- Cover meaningful behavior and failure cases. Do not claim a test passed if it could not run.
- Update the relevant reference when changing a public command, configuration field,
  storage format, or ownership boundary. Record unreleased user-facing changes in the changelog.
- Keep secrets, generated build output, machine-specific paths, and unrelated changes out of commits.
- Preserve process cleanup, bounded logs, and the distinction between leaving a client
  and shutting down a workspace. See [DESIGN.md](DESIGN.md) for invariants.

Repository-specific collaboration conventions are in [AGENTS.md](AGENTS.md).
Maintainers handle version tags and publication using the [release guide](docs/releasing.md).

## Reporting bugs

Include the devctl version, OS/WSL version and architecture, terminal and shell,
installation method, reproduction steps, expected result, and full relevant error.
Provide a minimal configuration and command if possible. Remove tokens, credentials,
private source, and sensitive log data before sharing an issue.

For update failures, include the current version, selected release, and error text.
For process or log failures, distinguish the active service state from retained logs,
and mention whether the existing supervisor was restarted after a configuration change.

## License

Contributions are distributed under this project's [MIT License](LICENSE).
