#!/usr/bin/env bash
set -euo pipefail

if [[ $# != 2 ]]; then
    printf 'Usage: package-release.sh BINARY NEW_OUTPUT_DIRECTORY\n' >&2
    exit 1
fi

repo_root=$(git rev-parse --show-toplevel)
binary=$(realpath -- "$1")
output=$(realpath -m -- "$2")
cd "$repo_root"

[[ -z $(git status --porcelain) ]] || { printf 'Release packaging requires a clean worktree.\n' >&2; exit 1; }
version=$(cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; m=json.load(sys.stdin); print(next(p["version"] for p in m["packages"] if p["id"] in m["workspace_members"] and p["name"] == "devctl"))')
[[ "$version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]] || { printf 'A stable package version is required.\n' >&2; exit 1; }
if [[ -n ${RELEASE_TAG:-} && "$RELEASE_TAG" != "v$version" ]]; then
    printf 'Release tag does not match the Cargo package version.\n' >&2
    exit 1
fi
[[ $(timeout 5 "$binary" --version) == "devctl $version" ]] || { printf 'Binary version does not match the package.\n' >&2; exit 1; }

# Refuse to mix a previous build's files into a new release bundle.
mkdir -- "$output"
name="devctl-v$version-x86_64-unknown-linux-musl"
install -m 755 -- "$binary" "$output/$name"
install -m 755 scripts/devctl-update "$output/devctl-update"
git archive --format=tar --prefix="devctl-$version/" HEAD | gzip -n > "$output/devctl-v$version-source.tar.gz"
revision=$(git rev-parse HEAD)
python3 - "$version" "$revision" "$output" <<'PY'
from pathlib import Path
import hashlib
import json
import re
import sys

version, revision, destination = sys.argv[1:]
output = Path(destination)
target = 'x86_64-unknown-linux-musl'
name = f'devctl-v{version}-{target}'
binary = output / name
manifest = {
    'schema_version': 1,
    'version': version,
    'assets': [{
        'target': target,
        'name': name,
        'size': binary.stat().st_size,
        'sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
    }],
}
(output / 'update.json').write_text(json.dumps(manifest, indent=2) + '\n')
text = Path('CHANGELOG.md').read_text()
match = re.search(r'^## ' + re.escape(version) + r'[^\n]*\n(.*?)(?=^## |\Z)', text, re.M | re.S)
if not match:
    raise SystemExit('CHANGELOG.md has no entry for this release.')
notes = match.group(1).strip() + f'\n\n构建提交：`{revision}`。\n'
(output / 'release-notes.md').write_text(notes)
PY
(
    cd "$output"
    sha256sum "$name" devctl-update "devctl-v$version-source.tar.gz" update.json > SHA256SUMS
    sha256sum --check SHA256SUMS
)
printf 'Release bundle: %s\nCommit: %s\n' "$output" "$revision"
