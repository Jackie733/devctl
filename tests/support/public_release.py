"""Verify public release downloads and the real anonymous update client."""

import hashlib
import json
import os
import platform
from pathlib import Path
import re
import subprocess
import sys
import tempfile


tag = sys.argv[1]
assert re.fullmatch(r'v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', tag), 'Stable tag required'
version = tag[1:]
base = f'https://github.com/Jackie733/devctl/releases/download/{tag}/'
assert sys.platform != 'darwin' or platform.machine() == 'arm64', 'Mac releases support Apple Silicon only'
target_platform = 'aarch64-apple-darwin' if sys.platform == 'darwin' else 'x86_64-unknown-linux-musl'
name = f'devctl-{tag}-{target_platform}'


def run(args, env=None):
    result = subprocess.run(args, env=env, capture_output=True, text=True, timeout=200)
    assert result.returncode == 0, result.stdout + result.stderr
    return result.stdout


with tempfile.TemporaryDirectory(prefix='devctl-public-release-') as tmp:
    root = Path(tmp)
    def download(asset):
        run(['curl', '--disable', '--fail', '--silent', '--show-error', '--location',
             '--proto', '=https', '--proto-redir', '=https', '--connect-timeout', '10',
             '--max-time', '180', '--max-filesize', str(128 * 1024 * 1024),
             '--output', str(root / asset), '--', base + asset])
    download('update.json')
    download('SHA256SUMS')
    manifest = json.loads((root / 'update.json').read_text())
    assert manifest['schema_version'] == 1 and manifest['version'] == version
    assets = manifest['assets']
    targets = {asset['target'] for asset in assets}
    assert len(targets) == len(assets) and target_platform in targets
    assert targets <= {'x86_64-unknown-linux-musl', 'aarch64-apple-darwin'}
    for asset in assets:
        assert asset['name'] == f'devctl-{tag}-{asset["target"]}'
    expected = {asset['name'] for asset in assets} | {'devctl-update', f'devctl-{tag}-source.tar.gz', 'update.json'}
    entries = [line.split(maxsplit=1) for line in (root / 'SHA256SUMS').read_text().splitlines()]
    assert len(entries) == len(expected) and {asset for _, asset in entries} == expected
    for asset in expected - {'update.json'}:
        download(asset)
    for digest, asset in entries:
        assert hashlib.sha256((root / asset).read_bytes()).hexdigest() == digest
    for asset in assets:
        data = (root / asset['name']).read_bytes()
        assert len(data) == asset['size'] and hashlib.sha256(data).hexdigest() == asset['sha256']
    print('PASS public downloads and SHA-256', flush=True)
    binary = root / name
    binary.chmod(0o755)
    assert run([binary, '--version']).strip() == f'devctl {version}'
    if sys.platform == 'darwin':
        dependencies = run(['otool', '-L', binary]).splitlines()[1:]
        assert dependencies and all(line.strip().startswith(('/usr/lib/', '/System/Library/')) for line in dependencies)
        run(['codesign', '--verify', binary])
    else:
        assert 'INTERP' not in run(['readelf', '-l', binary])
        assert 'NEEDED' not in run(['readelf', '-d', binary])

    home = root / 'home'
    target = home / '.local/bin/devctl'
    target.parent.mkdir(parents=True)
    original = b"#!/bin/sh\nprintf 'devctl 0.0.0\\n'\n"
    target.write_bytes(original)
    target.chmod(0o755)
    config = home / '.config/devctl/keep'
    config.parent.mkdir(parents=True)
    config.write_text('unchanged')
    env = {**os.environ, 'HOME': str(home)}
    for key in ('GH_TOKEN', 'GITHUB_TOKEN', 'BASH_ENV', 'ENV', 'DEVCTL_UPDATE_EXPECTED_VERSION'):
        env.pop(key, None)
    run([binary, '--config', '/missing.toml', 'update', '--check'], env=env)
    output = run([binary, 'update', '--check', '--version', version], env=env)
    assert f'Release: {version}' in output
    print('PASS anonymous latest and specified-version checks without workspace config', flush=True)
    run([binary, 'update', '--version', version], env=env)
    assert target.read_bytes() == binary.read_bytes()
    previous = target.with_name('devctl.previous')
    assert previous.read_bytes() == original
    print('PASS anonymous command download, checksum and installation', flush=True)
    run([target, 'update', '--rollback'], env=env)
    assert target.read_bytes() == original
    run(['bash', root / 'devctl-update', '--rollback'], env=env)
    assert target.read_bytes() == binary.read_bytes()
    assert config.read_text() == 'unchanged'
    print('PASS offline rollback, external recovery and config preservation', flush=True)
