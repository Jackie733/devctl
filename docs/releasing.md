# Release maintenance

Public distribution and self-update implement X03. The
[release workflow](../.github/workflows/release.yml) builds Linux/WSL2 x86_64 and macOS Apple Silicon artifacts. Intel Mac is excluded.
Current releases provide both targets; historical 1.4.0 remains Linux-only. [tasks.md](tasks.md) records publication authorization, actual outcomes,
and pending real-environment acceptance.

## One-command release

From the repository checkout, with your intended changes and reviewed notes under
`## Unreleased` in CHANGELOG.md already committed, run:

```sh
./scripts/release 1.7.0
```

Replace the example with a stable `MAJOR.MINOR.PATCH` higher than Cargo.toml's
version. Running this command authorizes committing and pushing main plus a new
annotated tag, and publishing a public GitHub Release without another prompt.
It requires Python 3.9+, Git, Cargo with rustfmt/Clippy, the normal development
test tools, and an authenticated [GitHub CLI](https://cli.github.com/manual/).
Both Git push credentials and GitHub repository write/Actions read access must
work. Branch protection still applies; no bypass or force push is attempted.

The [script](../scripts/release) checks a clean worktree/index, local main,
the official origin fetch/push URLs, repository permissions, an active release
workflow, unused version/tag, and a nondivergent remote main. Local main may
include committed changes not yet pushed. It computes all edits before writing:
Cargo.toml, only devctl's Cargo.lock entry, current platform versions in both
READMEs, current installation/build examples, and the dated changelog entry.
Historical feature versions and cross-version acceptance records are preserved;
an empty Unreleased section remains for future work.

Local checks run fmt, locked Clippy/all Rust tests, installer tests, bundle-merging
tests, release-command tests and `git diff --check`. No commit/tag/push occurs if
they fail. After verifying the checked files/HEAD/index did not change unexpectedly,
the script commits only the six prepared files and creates an annotated tag. An
atomic push sends the main commit and tag together; an unsupported or rejected
atomic push fails instead of falling back to separate pushes.

### Terminal feedback

The command prints six numbered stages: prerequisites, preparation, local checks,
commit/tag, atomic push, and remote build/publication/verification. Each local
check shows its command and final elapsed time. While a check runs, a plain-text
heartbeat appears every ten seconds with elapsed time and its latest output line.
There is no estimated percentage for tests or builds.

Full local-check output is saved in a temporary log whose path is printed before
the first check; use `tail -f /path/to/log` in another terminal for live detail.
A failure prints the last 30 lines (bounded to 8 KiB) and keeps the full log.
The command disables Git/GitHub pagers regardless of inherited pager settings;
checks have no interactive stdin. Normal execution never waits for `q` at `(END)`.

Ctrl+C during a local check interrupts its process group, waits up to five seconds
and kills it if needed. It returns 130 with an explicit cancellation reason, last
stage/elapsed time, log path and whether commit/tag/push was attempted. Stopping
remote monitoring cannot stop a workflow already running on GitHub.

The script locates the push-triggered release.yml run by tag **and exact build
commit**, then uses [gh run watch](https://cli.github.com/manual/gh_run_watch) to
show progress. Success requires both builds, publication and both native public
verification jobs to pass, plus a public stable Latest release with all six
expected attachments. It prints Release/workflow links. Publication does not
install the release or replace running supervisors. Target-environment and user
acceptance remain separate.

### Failure and recovery

Errors return nonzero and preserve prepared files, commits and tags for inspection.
The script never resets/stashes work, deletes/moves tags, overwrites a public
version or automatically dispatches another publication.

- **Before commit:** inspect `git status`/`git diff`. Fix the failure, restore only
  script-prepared changes after review, commit any intended fix/notes and rerun.
  If the only pending changes are the six exact version-preparation edits from
  the cancelled command, restore those files to the committed baseline:

  ```sh
  git restore -- Cargo.toml Cargo.lock CHANGELOG.md README.md README.zh-CN.md docs/install.md
  ./scripts/release 1.7.0
  ```

  Do not restore files containing additional intended edits. `--resume` monitors
  an existing remote tag; it does not restart interrupted local preparation/tests.
- **Commit/tag failure:** inspect the index, HEAD and local tag before retrying;
  do not discard your changes automatically.
- **Push rejected or connection lost:** inspect origin/main, local/remote tag
  identities and Actions first; a lost response does not prove the push failed.
  If the prepared commit/tag are local only and local main still points at the
  release tag, retry `git push --atomic origin main refs/tags/v1.7.0` after resolving
  the rejection. If main moved, review before pushing; never force or move the tag.
- **Workflow discovery timed out (180 seconds), terminal closed, or monitoring
  failed:** the remote workflow may still publish. Continue watching the same tag:

  ```sh
  ./scripts/release 1.7.0 --resume
  ```

  Resume requires a clean local main checkout and identical local/remote annotated
  tags. It monitors without editing, committing, pushing or retrying jobs.
- **CI failed:** inspect the printed workflow link. For a transient failure,
  `gh run rerun RUN_ID -R Jackie733/devctl --failed`, then resume monitoring.
  A code fix requires a new version/tag. If the tag event never ran, use the manual
  fallback below after inspection; resume automatically discovers only push runs.
- **Public verification failed:** the release is already public. The script reports
  this state and its link; failure never promises that publication was rolled back.
  Investigate/rerun verification, or publish a new version for a code fix.

## Manual preparation and trigger

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

Build has `contents: read`, uses pinned official actions and Rust 1.98.1, and runs
on Ubuntu 22.04 and macos-14 (Apple Silicon). Mac builds set
MACOSX_DEPLOYMENT_TARGET=14.0. Each job checks fmt, clippy, all applicable Rust tests,
installer tests and bundle-merging tests, then builds its native release target.
The actual program must pass native dependency checks (static ELF on Linux;
system-library-only, valid Mach-O code signature on Mac), isolated installation/
reinstallation/bad-digest/rollback, generated-manifest consumption by a real client,
and terminal scenarios including native Mac clipboard behavior. Local acceptance
and CI execution are distinct evidence; see tasks.md.

The [packager](../scripts/package-release.sh) requires a clean worktree, matching
binary/package/tag versions, and a new output directory. `git archive HEAD` produces
source from the same commit, rooted at `devctl-VERSION/`, excluding .git and target.
The new [MIT license](../LICENSE) and documentation are included in normal source archives.

| Public attachment | Purpose |
| --- | --- |
| `devctl-vVERSION-x86_64-unknown-linux-musl` | Static executable |
| `devctl-vVERSION-aarch64-apple-darwin` | Apple Silicon executable, macOS 14+ |
| `devctl-update` | External bootstrap, migration, and recovery installer |
| `devctl-vVERSION-source.tar.gz` | Exact source tree |
| `update.json` | Version/platform/name/size/digest from the actual executable |
| `SHA256SUMS` | SHA-256 for the five attachments above |

Each job uploads its own versioned bundle. The [merger](../scripts/merge-release.py)
requires both distinct targets, validates their bytes and checksums, and rejects
mismatching source contents, installers or build-commit notes before producing one
manifest and checksum file. Source comparison uses decompressed Git tar bytes because
native gzip implementations can differ; each original bundle checksum is still
verified and one verified archive is retained. Missing or failed platform jobs prevent publication.

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
checks all anonymous attachments/checksums and each native executable, no-config latest/explicit
checks, explicit online installation, offline rollback, external recovery, and
configuration preservation in a temporary HOME. It can run manually for an existing
manifest-format release; select `linux_only=true` for historical Linux-only versions. A failure here occurs **after** publication; investigate or
retry without claiming the release stayed private.

## Handoff checklist

- Confirm build, publish, and verify success, tag/build identity, Latest status, and
  all six uploaded attachments.
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
