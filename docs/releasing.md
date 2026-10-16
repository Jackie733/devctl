# Release maintenance

Public distribution and self-update implement X03. The
[release workflow](../.github/workflows/release.yml) produces Linux/WSL2 x86_64
artifacts. [tasks.md](tasks.md) records publication authorization, actual outcomes,
and pending real-environment acceptance.

## Prepare and trigger

1. Update Cargo.toml, Cargo.lock, CHANGELOG, and current installation/version examples.
   Keep English and Chinese READMEs consistent. Run applicable checks and commit.
   Stable tags use `vMAJOR.MINOR.PATCH` and must match the Cargo package version.
2. Confirm the target commit is on main and publication is authorized for this task.
   Create an annotated tag and push main plus that tag. Never move or overwrite a
   published tag or reuse a version to replace program bytes.
3. A `v*` tag push triggers publication. Manual dispatch defaults to build-only,
   including tests and intermediate artifact upload, without creating a Release.
   If a tag event does not run, dispatch on main with the existing `tag` and explicit
   `publish=true`. Manual publication without a version tag is rejected.

Manual retry may use a newer workflow definition, but checkout, compilation, and
source archive come from the selected tag. Release notes record the full build
commit. Intermediate artifacts are isolated by run ID and consumed only by that run.

## Checks and artifacts

Build has `contents: read`, uses pinned official actions, and runs on Ubuntu 22.04
with Rust 1.98.1. It checks fmt, clippy, all Rust tests, and installer tests, then
builds `x86_64-unknown-linux-musl`. The actual static artifact must pass dynamic-loader/
dependency checks, isolated install/reinstall/bad-digest/rollback checks, generated
manifest consumption by a real client, and ten real PTY scenarios.

The [packager](../scripts/package-release.sh) requires a clean worktree, matching
binary/package/tag versions, and a new output directory. `git archive HEAD` produces
source from the same commit, rooted at `devctl-VERSION/`, excluding .git and target.
The new [MIT license](../LICENSE) and documentation are included in normal source archives.

| Public attachment | Purpose |
| --- | --- |
| `devctl-vVERSION-x86_64-unknown-linux-musl` | Static executable |
| `devctl-update` | External bootstrap, migration, and recovery installer |
| `devctl-vVERSION-source.tar.gz` | Exact source tree |
| `update.json` | Version/platform/name/size/digest from the actual executable |
| `SHA256SUMS` | SHA-256 for the four attachments above |

The manifest contract is owned by [DESIGN.md](../DESIGN.md#update-manifest-contract).
Internal `release-notes.md` extracts the current changelog section and appends the
build commit in English. Intermediate workflow artifacts are retained for seven days;
GitHub Releases are the permanent download entry point.

## Publication and public verification

Publish depends on successful build, with `contents: write` and `actions: read`
using the built-in GITHUB_TOKEN; no personal token is required. Download and verify
the same run's artifacts, upload all attachments to a draft, then publish and mark
Latest. Failed testing/build/upload leaves the previous Latest unchanged. A manifest
is never published separately from its program. Clients pin downloads to the
manifest's version even if Latest changes afterward.

Same-tag runs are serialized without cancelling an active publication. Retrying may
replace a same-tag draft, but an already published release is never overwritten.
Fix a published problem with a new version.

After publication, the reusable [public verifier](../.github/workflows/verify-release.yml)
checks anonymous attachments/checksums, static executable, no-config latest/explicit
checks, explicit online installation, offline rollback, external recovery, and
configuration preservation in a temporary HOME. It can run manually for an existing
manifest-format release. A failure here occurs **after** publication; investigate or
retry without claiming the release stayed private.

## Handoff checklist

- Confirm build, publish, and verify success, tag/build identity, Latest status, and
  all five uploaded attachments.
- Download anonymously; verify digests, manifest, binary version/static linking,
  and the source archive against the build commit.
- For a cross-version update claim, install the real prior public binary in a
  temporary HOME, run its default `devctl update`, and verify new program bytes,
  previous-version backup, no-config checks, rollback, and configuration preservation.
  A new binary reinstalling itself is not a cross-version upgrade test.
- Share Release/run links and [installation](install.md) / [WSL verification](wsl.md#verify-updates)
  instructions. Controlled success does not establish target WSL project acceptance.

Clients 1.0/1.1 lack `update`; 1.2.0 still uses anonymous REST API discovery. Use
fixed attachments and the external installer to migrate when needed. New clients
also use the external installer for legacy releases without manifests. Preserve
historical release attachments and records instead of retrofitting published versions.
