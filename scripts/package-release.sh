#!/usr/bin/env bash
set -euo pipefail

if [[ $# != 2 ]]; then
    printf 'Usage: package-release.sh BINARY NEW_OUTPUT_DIRECTORY\n' >&2
    exit 1
fi

repo_root=$(git rev-parse --show-toplevel)
binary=$(python3 -c 'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve())' "$1")
output=$(python3 -c 'import pathlib,sys; print(pathlib.Path(sys.argv[1]).resolve())' "$2")
cd "$repo_root"

[[ -z $(git status --porcelain) ]] || { printf 'Release packaging requires a clean worktree.\n' >&2; exit 1; }
version=$(cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; m=json.load(sys.stdin); print(next(p["version"] for p in m["packages"] if p["id"] in m["workspace_members"] and p["name"] == "devctl"))')
[[ "$version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]] || { printf 'A stable package version is required.\n' >&2; exit 1; }
if [[ -n ${RELEASE_TAG:-} && "$RELEASE_TAG" != "v$version" ]]; then
    printf 'Release tag does not match the Cargo package version.\n' >&2
    exit 1
fi
[[ $(python3 -c 'import subprocess,sys; print(subprocess.check_output([sys.argv[1], "--version"], timeout=5, text=True).strip())' "$binary") == "devctl $version" ]] || { printf 'Binary version does not match the package.\n' >&2; exit 1; }

# Refuse to mix a previous build's files into a new release bundle.
mkdir -- "$output"
kind=$(file -b "$binary")
case "$kind" in
    *'Mach-O 64-bit executable arm64'*) target=aarch64-apple-darwin ;;
    *'Mach-O 64-bit executable x86_64'*) target=x86_64-apple-darwin ;;
    *'ELF 64-bit LSB'*'x86-64'*) target=x86_64-unknown-linux-musl ;;
    *) printf 'Unsupported release binary format/architecture.\n' >&2; exit 1 ;;
esac
name="devctl-v$version-$target"
install -m 755 -- "$binary" "$output/$name"
install -m 755 scripts/devctl-update "$output/devctl-update"
git archive --format=tar --prefix="devctl-$version/" HEAD | gzip -n > "$output/devctl-v$version-source.tar.gz"
revision=$(git rev-parse HEAD)
python3 - "$version" "$revision" "$output" "$target" <<'PY'
from pathlib import Path
import hashlib
import json
import re
import sys

version, revision, destination, target = sys.argv[1:]
output = Path(destination)
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
notes = match.group(1).strip() + f'\n\nBuild commit: `{revision}`.\n'
(output / 'release-notes.md').write_text(notes)
PY
python3 - "$output" "$name" "$version" <<'PY_CHECKSUMS'
import hashlib
from pathlib import Path
import sys
output = Path(sys.argv[1])
names = [sys.argv[2], 'devctl-update', f'devctl-v{sys.argv[3]}-source.tar.gz', 'update.json']
(output / 'SHA256SUMS').write_text(''.join(
    f'{hashlib.sha256((output / name).read_bytes()).hexdigest()}  {name}\n' for name in names
))
PY_CHECKSUMS
printf 'Release bundle: %s\nCommit: %s\n' "$output" "$revision"
