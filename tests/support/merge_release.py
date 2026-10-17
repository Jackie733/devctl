"""Verify multi-platform publication rejects mixed or damaged build bundles."""

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import shutil
import unittest
import sys

sys.dont_write_bytecode = True

spec = importlib.util.spec_from_file_location('merge_release', Path(__file__).resolve().parents[2] / 'scripts/merge-release.py')
merger = importlib.util.module_from_spec(spec)
spec.loader.exec_module(merger)


class MergeRelease(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / 'bundles'
        self.output = self.root / 'dist'
        for target in sorted(merger.TARGETS):
            bundle = self.source / target
            bundle.mkdir(parents=True)
            name = f'devctl-v1.5.0-{target}'
            data = target.encode()
            (bundle / name).write_bytes(data)
            (bundle / 'update.json').write_text(json.dumps({
                'schema_version': 1, 'version': '1.5.0', 'assets': [{
                    'target': target, 'name': name, 'size': len(data),
                    'sha256': hashlib.sha256(data).hexdigest(),
                }],
            }))
            for shared in ['devctl-update', 'devctl-v1.5.0-source.tar.gz', 'release-notes.md']:
                (bundle / shared).write_bytes(b'same build')
            self.checksums(bundle)

    def checksums(self, bundle):
        (bundle / 'SHA256SUMS').write_text(''.join(
            f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n'
            for path in sorted(bundle.iterdir()) if path.name not in ['SHA256SUMS', 'release-notes.md']
        ))

    def test_merge_preserves_all_assets_and_shared_source(self):
        merger.merge(self.source, self.output)
        manifest = json.loads((self.output / 'update.json').read_text())
        self.assertEqual({asset['target'] for asset in manifest['assets']}, merger.TARGETS)
        self.assertEqual(len((self.output / 'SHA256SUMS').read_text().splitlines()), 5)
        for line in (self.output / 'SHA256SUMS').read_text().splitlines():
            digest, name = line.split()
            self.assertEqual(hashlib.sha256((self.output / name).read_bytes()).hexdigest(), digest)
        with self.assertRaises(FileExistsError):
            merger.merge(self.source, self.output)

    def test_missing_platform_is_rejected_without_output(self):
        next(self.source.glob('*/update.json')).unlink()
        with self.assertRaises(ValueError):
            merger.merge(self.source, self.output)
        self.assertFalse(self.output.exists())

    def test_damaged_binary_is_rejected_without_output(self):
        next(self.source.glob('*/devctl-v1.5.0-*darwin')).write_bytes(b'damaged')
        with self.assertRaises(ValueError):
            merger.merge(self.source, self.output)
        self.assertFalse(self.output.exists())

    def test_duplicate_platform_is_rejected_without_output(self):
        first, second = sorted(self.source.iterdir())
        shutil.rmtree(second)
        shutil.copytree(first, second)
        with self.assertRaises(ValueError):
            merger.merge(self.source, self.output)
        self.assertFalse(self.output.exists())

    def test_unsupported_platform_is_rejected_without_output(self):
        bundle = next(self.source.iterdir())
        path = bundle / 'update.json'
        manifest = json.loads(path.read_text())
        manifest['assets'][0]['target'] = 'x86_64-apple-darwin'
        manifest['assets'][0]['name'] = 'devctl-v1.5.0-x86_64-apple-darwin'
        path.write_text(json.dumps(manifest))
        self.checksums(bundle)
        with self.assertRaises(ValueError):
            merger.merge(self.source, self.output)
        self.assertFalse(self.output.exists())

    def test_mixed_sources_are_rejected(self):
        bundle = next(self.source.iterdir())
        (bundle / 'devctl-v1.5.0-source.tar.gz').write_bytes(b'other commit')
        self.checksums(bundle)
        with self.assertRaises(ValueError):
            merger.merge(self.source, self.output)
        self.assertFalse(self.output.exists())



if __name__ == '__main__':
    unittest.main()
