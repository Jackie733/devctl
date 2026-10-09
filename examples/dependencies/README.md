# Dependency validation examples

These configurations are for validation and planning. Their `echo` commands are
placeholders, not long-running services. Validation does not execute commands.
Project paths resolve relative to each configuration file.

| File | Expected result |
| --- | --- |
| `chain.toml` | Valid: web depends on api, which depends on db |
| `shared.toml` | Valid: api and worker share db as a dependency |
| `cycle.toml` | Invalid: api -> db -> api |
| `separate-cycle.toml` | Invalid: queue -> worker -> queue in an independent branch |

Run from the repository root:

```sh
cargo run -- --config examples/dependencies/chain.toml validate
cargo run -- --config examples/dependencies/shared.toml validate
cargo run -- --config examples/dependencies/cycle.toml validate
cargo run -- --config examples/dependencies/separate-cycle.toml validate
```

The invalid examples intentionally exit with code 1 and print the cycle to stderr.

Preview a startup plan without executing service commands:

```sh
cargo run -- --config examples/dependencies/chain.toml plan web
cargo run -- --config examples/dependencies/chain.toml plan
cargo run -- --config examples/dependencies/shared.toml plan web
```

The chain example selects only web by default and includes its disabled-by-default
dependencies, producing `db -> api -> web`. The shared example includes db once,
producing `db -> api -> worker -> web`. Plans require a validated, acyclic configuration.

Run the automated checks:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```
