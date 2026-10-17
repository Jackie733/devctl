"""Combine verified per-platform bundles without overwriting shared release files."""

import hashlib
import json
import re
from pathlib import Path
import shutil
import sys

TARGETS = {'x86_64-unknown-linux-musl', 'aarch64-apple-darwin'}


def merge(source, output):
    bundles = sorted(source.glob('*/update.json'))
    if len(bundles) != len(TARGETS):
        raise ValueError('Exactly two platform bundles are required.')
    assets = []
    shared = {}
    version = None
    for path in bundles:
        manifest = json.loads(path.read_text())
        if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', manifest['version']):
            raise ValueError('A stable release version is required.')
        if manifest['schema_version'] != 1 or len(manifest['assets']) != 1:
            raise ValueError('Invalid platform manifest.')
        if version is None:
            version = manifest['version']
        if manifest['version'] != version:
            raise ValueError('Platform versions differ.')
        asset = manifest['assets'][0]
        if asset['target'] not in TARGETS or asset['name'] != f'devctl-v{version}-{asset["target"]}':
            raise ValueError('Invalid platform asset.')
        if any(item['target'] == asset['target'] for item in assets):
            raise ValueError('Duplicate platform target.')
        data = (path.parent / asset['name']).read_bytes()
        if len(data) != asset['size'] or hashlib.sha256(data).hexdigest() != asset['sha256']:
            raise ValueError('Platform binary checksum/size mismatch.')
        sums = dict((name, digest) for digest, name in
                    (line.split() for line in (path.parent / 'SHA256SUMS').read_text().splitlines()))
        names = [asset['name'], 'devctl-update', f'devctl-v{version}-source.tar.gz', 'update.json']
        if set(sums) != set(names):
            raise ValueError('Incomplete platform checksums.')
        for name in names:
            if hashlib.sha256((path.parent / name).read_bytes()).hexdigest() != sums[name]:
                raise ValueError('Platform bundle checksum mismatch.')
        for name in ['devctl-update', f'devctl-v{version}-source.tar.gz', 'release-notes.md']:
            data = (path.parent / name).read_bytes()
            if name in shared and shared[name] != data:
                raise ValueError('Platform bundles do not share the same source/installer/notes.')
            shared[name] = data
        assets.append(asset)
    output.mkdir()  # Refuse to reuse any output, even after a failed previous attempt.
    for path, asset in zip(bundles, assets):
        shutil.copy2(path.parent / asset['name'], output / asset['name'])
    for name, data in shared.items():
        (output / name).write_bytes(data)
    (output / 'devctl-update').chmod(0o755)
    assets.sort(key=lambda item: item['target'])
    (output / 'update.json').write_text(json.dumps({
        'schema_version': 1, 'version': version, 'assets': assets,
    }, indent=2) + '\n')
    names = sorted([asset['name'] for asset in assets] +
                   ['devctl-update', f'devctl-v{version}-source.tar.gz', 'update.json'])
    (output / 'SHA256SUMS').write_text(''.join(
        f'{hashlib.sha256((output / name).read_bytes()).hexdigest()}  {name}\n' for name in names
    ))


if __name__ == '__main__':
    if len(sys.argv) != 3:
        raise SystemExit('Usage: merge-release.py BUNDLE_DIRECTORY NEW_OUTPUT_DIRECTORY')
    merge(Path(sys.argv[1]), Path(sys.argv[2]))
