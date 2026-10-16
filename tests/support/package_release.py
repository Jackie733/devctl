"""Check packaging and consume its generated manifest with a real update client."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest


BINARY = Path(sys.argv.pop(1)).resolve()
kind = subprocess.check_output(['file', '-b', BINARY], text=True)
TARGET = ('aarch64-apple-darwin' if 'arm64' in kind else 'x86_64-apple-darwin') if 'Mach-O' in kind else 'x86_64-unknown-linux-musl'
SCRIPTS = Path(__file__).resolve().parents[2] / 'scripts'


class PackageRelease(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='devctl package ')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / 'repo'
        (self.repo / 'src').mkdir(parents=True)
        (self.repo / 'scripts').mkdir()
        self.version = subprocess.check_output([BINARY, '--version'], text=True).strip().split()[1]
        (self.repo / 'Cargo.toml').write_text(
            f'[package]\nname = "devctl"\nversion = "{self.version}"\nedition = "2024"\n'
        )
        (self.repo / 'Cargo.lock').write_text(
            f'version = 4\n[[package]]\nname = "devctl"\nversion = "{self.version}"\n'
        )
        (self.repo / 'src/main.rs').write_text('fn main() {}\n')
        (self.repo / 'CHANGELOG.md').write_text(f'## {self.version}\n\nManifest update.\n')
        for name in ('devctl-update', 'package-release.sh'):
            shutil.copy2(SCRIPTS / name, self.repo / 'scripts' / name)
        self.env = {**os.environ, 'RELEASE_TAG': f'v{self.version}'}
        for key in ('BASH_ENV', 'ENV', 'DEVCTL_UPDATE_EXPECTED_VERSION'):
            self.env.pop(key, None)
        self.run_command(['git', 'init', '-q'])
        self.run_command(['git', 'add', '.'])
        self.run_command(['git', '-c', 'user.name=Release test', '-c', 'user.email=test@example.invalid',
                          '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture'])

    def run_command(self, args, env=None, ok=True):
        result = subprocess.run(args, cwd=self.repo, env=env or self.env,
                                capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode == 0, ok, result.stdout + result.stderr)
        return result.stdout

    def package(self, output, env=None, ok=True):
        return self.run_command(['bash', 'scripts/package-release.sh', BINARY, output], env, ok)

    def test_complete_bundle_is_consumed_by_the_real_client(self):
        bundle = self.root / 'bundle'
        self.package(bundle)
        name = f'devctl-v{self.version}-{TARGET}'
        manifest = json.loads((bundle / 'update.json').read_text())
        self.assertEqual(manifest, {
            'schema_version': 1, 'version': self.version,
            'assets': [{'target': TARGET, 'name': name,
                        'size': BINARY.stat().st_size,
                        'sha256': hashlib.sha256(BINARY.read_bytes()).hexdigest()}],
        })
        entries = [line.split(maxsplit=1) for line in (bundle / 'SHA256SUMS').read_text().splitlines()]
        self.assertEqual(len(entries), 4)
        sums = {asset: digest for digest, asset in entries}
        self.assertEqual(set(sums), {name, 'devctl-update', f'devctl-v{self.version}-source.tar.gz', 'update.json'})
        for asset, digest in sums.items():
            self.assertEqual(hashlib.sha256((bundle / asset).read_bytes()).hexdigest(), digest)
        revision = self.run_command(['git', 'rev-parse', 'HEAD']).strip()
        self.assertIn(revision, (bundle / 'release-notes.md').read_text())
        with tarfile.open(bundle / f'devctl-v{self.version}-source.tar.gz') as archive:
            source = archive.extractfile(f'devctl-{self.version}/Cargo.toml').read()
            self.assertEqual(source, (self.repo / 'Cargo.toml').read_bytes())

        # Serve only the packaged files. Reject API calls and unpinned binary URLs.
        mock = self.root / 'mock'
        mock.mkdir()
        curl = mock / 'curl'
        curl.write_text('''#!/usr/bin/env python3
import os, pathlib, shutil, sys
root = pathlib.Path(os.environ['BUNDLE_FIXTURE'])
version = os.environ['BUNDLE_VERSION']
base = 'https://github.com/Jackie733/devctl/releases/'
url = sys.argv[-1]
name = f'devctl-v{version}-' + os.environ['BUNDLE_TARGET']
if url in (base + 'latest/download/update.json', base + f'download/v{version}/update.json'):
    asset = 'update.json'
elif url == base + f'download/v{version}/' + name:
    asset = name
else:
    sys.exit(22)
with (root / asset).open('rb') as source:
    shutil.copyfileobj(source, sys.stdout.buffer)
''')
        curl.chmod(0o755)
        home = self.root / 'home'
        target = home / '.local/bin/devctl'
        target.parent.mkdir(parents=True)
        original = b"#!/bin/sh\necho 'devctl 0.0.0'\n"
        target.write_bytes(original)
        target.chmod(0o755)
        config = home / '.config/devctl/keep'
        config.parent.mkdir(parents=True)
        config.write_text('unchanged')
        env = {**self.env, 'HOME': str(home), 'PATH': f'{mock}:{os.environ["PATH"]}',
               'BUNDLE_FIXTURE': str(bundle), 'BUNDLE_VERSION': self.version, 'BUNDLE_TARGET': TARGET}
        self.run_command([BINARY, '--config', '/missing.toml', 'update', '--check'], env)
        self.assertEqual(target.read_bytes(), original)
        self.run_command([BINARY, 'update', '--version', self.version], env)
        self.assertEqual(target.read_bytes(), BINARY.read_bytes())
        self.assertEqual(target.with_name('devctl.previous').read_bytes(), original)
        self.run_command([target, 'update', '--rollback'], env)
        self.assertEqual(target.read_bytes(), original)
        self.assertEqual(config.read_text(), 'unchanged')

    def test_wrong_tag_is_rejected(self):
        output = self.root / 'bundle'
        error = self.package(output, env={**self.env, 'RELEASE_TAG': 'v999.0.0'}, ok=False)
        self.assertFalse(output.exists(), error)

    def test_dirty_source_is_rejected(self):
        (self.repo / 'src/main.rs').write_text('fn main() { println!("dirty"); }\n')
        output = self.root / 'bundle'
        self.package(output, ok=False)
        self.assertFalse(output.exists())

    def test_existing_bundle_is_not_overwritten(self):
        output = self.root / 'bundle'
        output.mkdir()
        marker = output / 'update.json'
        marker.write_text('keep')
        self.package(output, ok=False)
        self.assertEqual(marker.read_text(), 'keep')


if __name__ == '__main__':
    unittest.main()
