"""Verify a real Linux release artifact using only a temporary HOME."""

import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile


binary = Path(sys.argv[1]).resolve()
version = sys.argv[2]
installer = Path(__file__).resolve().parents[2] / 'scripts/devctl-update'


def run(args, env=None, ok=True):
    result = subprocess.run(args, env=env, capture_output=True, text=True, timeout=20)
    assert (result.returncode == 0) == ok, result.stdout + result.stderr
    return result


assert run([binary, '--version']).stdout.strip() == f'devctl {version}'
assert '--rollback' in run([binary, 'update', '--help']).stdout
assert 'INTERP' not in run(['readelf', '-l', binary]).stdout
assert 'NEEDED' not in run(['readelf', '-d', binary]).stdout

with tempfile.TemporaryDirectory(prefix='devctl-release-smoke-') as tmp:
    home = Path(tmp)
    env = {**os.environ, 'HOME': str(home)}
    for key in ('BASH_ENV', 'ENV', 'DEVCTL_UPDATE_EXPECTED_VERSION'):
        env.pop(key, None)
    config = home / '.config/devctl/devctl.toml'
    config.parent.mkdir(parents=True)
    config.write_text('keep this workspace configuration\n')
    target = home / '.local/bin/devctl'
    previous = target.with_name('devctl.previous')
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    run(['bash', installer, binary, digest], env=env)
    assert not previous.exists()
    assert target.read_bytes() == binary.read_bytes()
    assert run([target, '--version'], env=env).stdout.strip() == f'devctl {version}'

    previous.write_text("#!/bin/sh\nprintf 'devctl 0.0.0\\n'\n")
    previous.chmod(0o755)
    old_bytes = previous.read_bytes()
    run(['bash', installer, binary, digest], env=env)
    assert previous.read_bytes() == old_bytes, 'Repeat install discarded the backup'
    run(['bash', installer, binary, '0' * 64], env=env, ok=False)
    assert target.read_bytes() == binary.read_bytes()
    assert previous.read_bytes() == old_bytes

    run([target, '--config', '/missing.toml', 'update', '--rollback'], env=env)
    assert target.read_bytes() == old_bytes
    assert previous.read_bytes() == binary.read_bytes()
    run(['bash', installer, '--rollback'], env=env)
    assert target.read_bytes() == binary.read_bytes()
    assert config.read_text() == 'keep this workspace configuration\n'
    for entry in target.parent.iterdir():
        assert not entry.name.startswith(('.devctl-download-', '.devctl-update.')) or entry.name == '.devctl-update.lock'

print('PASS: static artifact, fresh/repeat install, bad checksum, rollback, config preservation')
