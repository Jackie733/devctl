"""Exercise release preparation and recovery with real local Git and fake Cargo/gh.

All pushes go to a temporary bare repository; no GitHub mutation or Rust build.
"""

import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import pty
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
loader = importlib.machinery.SourceFileLoader('release', str(ROOT / 'scripts/release'))
spec = importlib.util.spec_from_loader(loader.name, loader)
release = importlib.util.module_from_spec(spec)
loader.exec_module(release)
GIT = shutil.which('git')


class ReleaseCommand(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='devctl release ')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / 'checkout'
        self.repo.mkdir()
        self.remote = self.root / 'remote.git'
        self.mock = self.root / 'bin'
        self.mock.mkdir()
        self.log = self.root / 'commands.jsonl'
        self.state = self.root / 'published'
        self.env = {**os.environ, 'PATH': f'{self.mock}:{os.environ["PATH"]}',
                    'REAL_GIT': GIT, 'RELEASE_REMOTE': str(self.remote),
                    'RELEASE_LOG': str(self.log), 'RELEASE_STATE': str(self.state),
                    'GIT_CONFIG_GLOBAL': os.devnull, 'GIT_CONFIG_NOSYSTEM': '1',
                    'GIT_TERMINAL_PROMPT': '0', 'TMPDIR': str(self.root)}
        for key in list(self.env):
            if key.startswith('GIT_CONFIG_KEY_') or key.startswith('GIT_CONFIG_VALUE_') or key in (
                    'GIT_CONFIG_COUNT', 'GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE',
                    'GIT_OBJECT_DIRECTORY', 'GIT_ALTERNATE_OBJECT_DIRECTORIES'):
                self.env.pop(key)
        for name in release.FILES:
            path = self.repo / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / name, path)
        # These tests also run after the real release command has bumped its own
        # checkout and emptied Unreleased. Keep fixture versions/notes independent
        # of the source checkout's current or future release version.
        current = re.search(r'^version = "([^"]+)"',
                            (ROOT / 'Cargo.toml').read_text(), re.M)[1]
        for name in ('README.md', 'README.zh-CN.md', 'docs/install.md'):
            path = self.repo / name
            path.write_text(path.read_text().replace(current, '1.6.1'))
        (self.repo / 'Cargo.toml').write_text(
            '[package]\nname = "devctl"\nversion = "1.6.1"\nedition = "2024"\n')
        (self.repo / 'CHANGELOG.md').write_text(
            '# Changelog\n\n## Unreleased\n\n- Reviewed fixture notes.\n\n'
            '## 1.6.1 — 2026-10-18\n\n- Historical notes.\n')
        (self.repo / 'scripts').mkdir()
        shutil.copy2(ROOT / 'scripts/release', self.repo / 'scripts/release')
        for name in ('install_scripts', 'merge_release', 'release_command'):
            path = self.repo / f'tests/support/{name}.py'
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('pass\n')
        # Make a minimal Cargo graph; another package sharing the old version must
        # not be changed by release preparation.
        (self.repo / 'Cargo.lock').write_text(
            'version = 4\n\n[[package]]\nname = "other"\nversion = "1.6.1"\n\n'
            '[[package]]\nname = "devctl"\nversion = "1.6.1"\n')
        self.git('init', '-q', '--initial-branch=main')
        self.git('config', 'user.name', 'Release fixture')
        self.git('config', 'user.email', 'release@example.invalid')
        self.git('config', 'commit.gpgsign', 'false')
        self.git('config', 'tag.gpgsign', 'false')
        self.git('add', '.')
        self.git('commit', '-qm', 'fixture')
        self.git('init', '-q', '--bare', str(self.remote))
        self.git('remote', 'add', 'origin', 'git@github.com:Jackie733/devctl.git')
        self.git('push', '-q', str(self.remote), 'main')
        self.base = self.git('rev-parse', 'HEAD').stdout.strip()
        self.originals = {name: (self.repo / name).read_bytes() for name in release.FILES}
        self.write_mock('git', '''
if args[:2] == ['remote', 'get-url']:
    print(os.environ.get('MOCK_ORIGIN', 'git@github.com:Jackie733/devctl.git'))
    sys.exit(0)
if args and args[0] in ('fetch', 'push', 'ls-remote'):
    args = [os.environ['RELEASE_REMOTE'] if arg == 'origin' else arg for arg in args]
    if args[0] == 'push' and '--dry-run' not in args and os.environ.get('MOCK_PUSH_FAIL'):
        sys.exit(1)
sys.exit(subprocess.run([os.environ['REAL_GIT'], *args]).returncode)
''')
        self.write_mock('cargo', '''
if args[0] == 'metadata':
    version = re.search(r'^version = "([^"\\n]+)"', pathlib.Path('Cargo.toml').read_text(), re.M)[1]
    print(json.dumps({'packages': [{'id': 'devctl', 'name': 'devctl', 'version': version}],
                      'workspace_members': ['devctl']}))
elif args[0] == os.environ.get('MOCK_CHECK_FAIL'):
    print('controlled local check failure', file=sys.stderr)
    sys.exit(1)
elif args[0] == 'test' and os.environ.get('MOCK_CANCEL_CHECK'):
    print('controlled waiting test', flush=True)
    os.kill(os.getppid(), signal.SIGINT)
    time.sleep(60)
elif args[0] == 'test' and os.environ.get('MOCK_CHANGE_DURING_CHECK'):
    pathlib.Path(os.environ['MOCK_CHANGE_DURING_CHECK']).write_text('concurrent edit')
''')
        self.write_mock('gh', '''
state = pathlib.Path(os.environ['RELEASE_STATE'])
commit = subprocess.check_output([os.environ['REAL_GIT'], 'rev-parse', 'HEAD'], text=True).strip()
tag = 'v1.7.0'
assets = ['devctl-v1.7.0-x86_64-unknown-linux-musl', 'devctl-v1.7.0-aarch64-apple-darwin',
          'devctl-v1.7.0-source.tar.gz', 'devctl-update', 'update.json', 'SHA256SUMS']
data = {'tag_name': tag, 'draft': False, 'prerelease': False,
        'html_url': 'https://example.invalid/release', 'assets': [{'name': name} for name in assets]}
if os.environ.get('MOCK_BAD_ASSETS'):
    data['assets'].pop()
if args[0] == 'api':
    endpoint = args[1]
    if os.environ.get('MOCK_API_FAIL'):
        print('controlled API failure', file=sys.stderr)
        sys.exit(1)
    if endpoint.endswith('/release.yml'):
        print(json.dumps({'state': 'active'}))
    elif '/releases?' in endpoint:
        print(json.dumps([data] if state.exists() else []))
    elif endpoint.endswith('/latest'):
        print(json.dumps(data))
    else:
        print(json.dumps({'permissions': {'push': not os.environ.get('MOCK_NO_PERMISSION')}}))
elif args[:2] == ['run', 'list']:
    print(json.dumps([{'databaseId': 42, 'headSha': commit, 'headBranch': tag,
                       'url': 'https://example.invalid/run/42'}]))
elif args[:2] == ['run', 'watch']:
    if not os.environ.get('MOCK_WATCH_FAIL') or os.environ.get('MOCK_PUBLISHED_FAILURE'):
        state.touch()
    sys.exit(1 if os.environ.get('MOCK_WATCH_FAIL') else 0)
elif args[:2] == ['run', 'view']:
    names = ['build (ubuntu-22.04)', 'build (macos-14)', 'publish',
             'verify / verify (ubuntu-22.04)', 'verify / verify (macos-14)']
    print(json.dumps({'headSha': commit, 'status': 'completed', 'conclusion': 'success',
                      'jobs': [{'name': name, 'conclusion':
                                'skipped' if os.environ.get('MOCK_SKIPPED') and name.startswith('verify')
                                else 'success'} for name in names]}))
else:
    sys.exit('Unexpected gh invocation')
''')

    def write_mock(self, name, body):
        path = self.mock / name
        path.write_text('#!' + sys.executable + '\n'
                        'import json, os, pathlib, re, signal, subprocess, sys, time\n'
                        'args = sys.argv[1:]\n'
                        + ('if args[:1] == ["--no-pager"]: args.pop(0)\n' if name == 'git' else '') +
                        'with open(os.environ["RELEASE_LOG"], "a") as log:\n'
                        f'    log.write(json.dumps([{name!r}, *args]) + "\\n")\n' + body)
        path.chmod(0o755)

    def git(self, *args):
        return subprocess.run([GIT, *args], cwd=self.repo, env=self.env,
                              text=True, capture_output=True, check=True, timeout=30)

    def command(self, *args, ok=True, **env):
        result = subprocess.run([sys.executable, self.repo / 'scripts/release', *args],
                                cwd=self.root, env={**self.env, **env}, text=True,
                                capture_output=True, timeout=30)
        self.assertEqual(result.returncode == 0, ok, result.stdout + result.stderr)
        self.result_code = result.returncode
        return result.stdout + result.stderr

    def assert_no_release(self):
        self.assertEqual(self.git('rev-parse', 'HEAD').stdout.strip(), self.base)
        self.assertEqual(self.git('tag', '--list').stdout, '')
        self.assertEqual(self.git('ls-remote', '--tags', str(self.remote)).stdout, '')

    def assert_unmodified(self):
        self.assert_no_release()
        for name, contents in self.originals.items():
            self.assertEqual((self.repo / name).read_bytes(), contents, name)

    def test_complete_release_and_read_only_resume(self):
        text = self.command('1.7.0')
        self.assertIn('Released 1.7.0', text)
        self.assertIn('[3/6] Run local checks', text)
        self.assertIn('[6/6] Wait for GitHub', text)
        self.assertIn('[6/6] git diff --check: OK', text)
        commit = self.git('rev-parse', 'HEAD').stdout.strip()
        self.assertNotEqual(commit, self.base)
        self.assertEqual(self.git('cat-file', '-t', 'v1.7.0').stdout.strip(), 'tag')
        self.assertEqual(self.git('rev-parse', 'v1.7.0^{commit}').stdout.strip(), commit)
        self.assertIn(commit, self.git('ls-remote', str(self.remote), 'refs/heads/main').stdout)
        self.assertEqual(self.git('status', '--porcelain').stdout, '')
        names = self.git('diff-tree', '--no-commit-id', '--name-only', '-r', 'HEAD').stdout.splitlines()
        self.assertEqual(set(names), set(release.FILES))
        lock = (self.repo / 'Cargo.lock').read_text()
        self.assertIn('name = "other"\nversion = "1.6.1"', lock)
        self.assertIn('name = "devctl"\nversion = "1.7.0"', lock)
        changelog = (self.repo / 'CHANGELOG.md').read_text()
        original_history = self.originals['CHANGELOG.md'].decode().split('## 1.6.1', 1)[1]
        self.assertEqual(changelog.split('## 1.6.1', 1)[1], original_history)
        self.assertIn('## Unreleased\n\n## 1.7.0 — ', changelog)
        self.assertIn('Version 1.6.1 adds', (self.repo / 'README.md').read_text())
        self.assertIn('Release **1.7.0** provides', (self.repo / 'README.md').read_text())
        install = (self.repo / 'docs/install.md').read_text()
        self.assertIn('**1.5.0 → 1.6.1**', install)
        self.assertIn('--version 1.7.0', install)
        commands = [json.loads(line) for line in self.log.read_text().splitlines()]
        pushes = [cmd for cmd in commands if cmd[:2] == ['git', 'push']]
        self.assertEqual(len(pushes), 2)
        self.assertTrue(all('--atomic' in cmd for cmd in pushes))
        self.log.write_text('')
        self.assertIn('Released 1.7.0', self.command('1.7.0', '--resume'))
        resumed = [json.loads(line) for line in self.log.read_text().splitlines()]
        self.assertFalse(any(cmd[:2] in (['git', 'push'], ['git', 'commit'], ['cargo', 'test']) for cmd in resumed))
        self.assertEqual(self.git('rev-parse', 'HEAD').stdout.strip(), commit)

    def test_invalid_equal_and_lower_versions_leave_no_changes(self):
        for version in ('v1.7.0', '01.7.0', '1.7.0-rc.1', '1.7.0+build', '1.6.1', '1.5.0'):
            with self.subTest(version=version):
                self.command(version, ok=False)
                self.assert_unmodified()

    def test_dirty_index_and_wrong_branch_are_rejected(self):
        (self.repo / 'unrelated').write_text('keep')
        self.command('1.7.0', ok=False)
        self.assert_unmodified()
        self.git('add', 'unrelated')
        self.command('1.7.0', ok=False)
        self.assert_unmodified()
        self.git('commit', '-qm', 'unrelated')
        self.base = self.git('rev-parse', 'HEAD').stdout.strip()
        self.git('switch', '-qc', 'feature')
        self.command('1.7.0', ok=False)
        self.assert_unmodified()

    def test_existing_remote_tag_is_not_reused(self):
        self.git('tag', '-a', 'v1.7.0', '-m', 'existing')
        self.git('push', '-q', str(self.remote), 'refs/tags/v1.7.0')
        self.git('tag', '-d', 'v1.7.0')
        before = self.git('ls-remote', '--tags', str(self.remote)).stdout
        self.assertIn('already exists', self.command('1.7.0', ok=False))
        self.assertEqual(self.git('ls-remote', '--tags', str(self.remote)).stdout, before)
        self.assertEqual(self.git('status', '--porcelain').stdout, '')

    def test_preflight_api_permission_and_origin_failures_do_not_edit(self):
        for env in ({'MOCK_API_FAIL': '1'}, {'MOCK_NO_PERMISSION': '1'},
                    {'MOCK_ORIGIN': 'git@github.com:other/devctl.git'}):
            with self.subTest(env=env):
                self.command('1.7.0', ok=False, **env)
                self.assert_unmodified()

    def test_behind_remote_main_is_rejected(self):
        self.git('commit', '--allow-empty', '-qm', 'remote advance')
        self.git('push', '-q', str(self.remote), 'main')
        self.git('reset', '--hard', self.base)
        self.assertIn('behind or diverges', self.command('1.7.0', ok=False))
        self.assert_unmodified()

    def test_missing_notes_and_malformed_version_fields_fail_before_writing(self):
        for name, contents in (('CHANGELOG.md', '# Changelog\n\n## Unreleased\n\n'),
                               ('Cargo.lock', 'version = 4\n')):
            with self.subTest(name=name):
                (self.repo / name).write_text(contents)
                self.git('add', name)
                self.git('commit', '-qm', 'incomplete release metadata')
                before = {item: (self.repo / item).read_bytes() for item in release.FILES}
                self.command('1.7.0', ok=False)
                self.assertEqual({item: (self.repo / item).read_bytes() for item in release.FILES}, before)
                self.assertEqual(self.git('tag', '--list').stdout, '')
                self.git('reset', '--hard', self.base)

    def test_failed_checks_preserve_edits_without_commit_tag_or_push(self):
        text = self.command('1.7.0', ok=False, MOCK_CHECK_FAIL='test')
        self.assertIn('controlled local check failure', text)
        self.assertIn('Last stage: [3/6] Run local checks', text)
        self.assertIn('No release push was attempted', text)
        log = re.search(r'^Full check output: (.+)$', text, re.M)[1]
        self.assertIn('controlled local check failure', Path(log).read_text())
        self.assert_no_release()
        self.assertIn('version = "1.7.0"', (self.repo / 'Cargo.toml').read_text())
        self.assertEqual(self.git('diff', '--cached', '--name-only').stdout, '')

    def test_ctrl_c_reports_stage_preserves_edits_and_stops_check(self):
        text = self.command('1.7.0', ok=False, MOCK_CANCEL_CHECK='1')
        self.assertEqual(self.result_code, 130)
        self.assertIn('Cancelled by Ctrl+C', text)
        self.assertIn('Last stage: [3/6] Run local checks', text)
        self.assertIn('No release commit or tag was created', text)
        self.assertIn('No release push was attempted', text)
        self.assert_no_release()
        self.assertIn('version = "1.7.0"', (self.repo / 'Cargo.toml').read_text())

    def test_rejected_push_preserves_local_commit_tag_and_remote_main(self):
        self.command('1.7.0', ok=False, MOCK_PUSH_FAIL='1')
        self.assertNotEqual(self.git('rev-parse', 'HEAD').stdout.strip(), self.base)
        self.assertEqual(self.git('cat-file', '-t', 'v1.7.0').stdout.strip(), 'tag')
        self.assertIn(self.base, self.git('ls-remote', str(self.remote), 'refs/heads/main').stdout)
        self.assertEqual(self.git('ls-remote', '--tags', str(self.remote)).stdout, '')
        self.assertIn('remote tag is absent', self.command('1.7.0', '--resume', ok=False))

    def test_commit_failure_preserves_staged_files_without_tag_or_push(self):
        hook = self.repo / '.git/hooks/pre-commit'
        hook.write_text('#!/bin/sh\nexit 1\n')
        hook.chmod(0o755)
        self.command('1.7.0', ok=False)
        self.assert_no_release()
        staged = self.git('diff', '--cached', '--name-only').stdout.splitlines()
        self.assertEqual(set(staged), set(release.FILES))

    def test_concurrent_edits_are_not_committed(self):
        text = self.command('1.7.0', ok=False, MOCK_CHANGE_DURING_CHECK='README.md')
        self.assertIn('changed unexpectedly during checks', text)
        self.assert_no_release()
        self.assertEqual((self.repo / 'README.md').read_text(), 'concurrent edit')

    def test_ci_failure_before_publication_is_not_reported_as_success(self):
        text = self.command('1.7.0', ok=False, MOCK_WATCH_FAIL='1')
        self.assertIn('publication is not confirmed', text)
        self.assertNotIn('Released 1.7.0', text)
        self.assertFalse(self.state.exists())

    def test_public_verification_failure_reports_already_published(self):
        text = self.command('1.7.0', ok=False, MOCK_WATCH_FAIL='1', MOCK_PUBLISHED_FAILURE='1')
        self.assertIn('Already published:', text)
        self.assertIn('https://example.invalid/release', text)
        self.assertNotIn('Released 1.7.0', text)

    def test_skipped_public_checks_and_missing_assets_are_not_success(self):
        self.command('1.7.0')
        for env in ({'MOCK_SKIPPED': '1'}, {'MOCK_BAD_ASSETS': '1'}):
            with self.subTest(env=env):
                text = self.command('1.7.0', '--resume', ok=False, **env)
                self.assertNotIn('Released 1.7.0', text)

    def test_resume_rejects_mismatched_tag_identity(self):
        self.command('1.7.0')
        self.git('tag', '-fa', 'v1.7.0', self.base, '-m', 'different tag')
        self.assertIn('identity differs', self.command('1.7.0', '--resume', ok=False))


class RunDiscovery(unittest.TestCase):
    def test_ignores_unrelated_runs_and_waits_for_exact_tag_and_commit(self):
        unrelated = [{'headSha': 'wrong', 'headBranch': 'v1.7.0'},
                     {'headSha': 'right', 'headBranch': 'main'}]
        expected = {'headSha': 'right', 'headBranch': 'v1.7.0', 'databaseId': 42}
        with patch.object(release, 'gh_json', side_effect=[unrelated, [expected]]), \
                patch.object(release.time, 'sleep') as sleep:
            self.assertEqual(release.find_run('v1.7.0', 'right'), expected)
            sleep.assert_called_once_with(5)

    def test_discovery_timeout_does_not_dispatch_publication(self):
        with patch.object(release, 'gh_json', return_value=[]) as api, \
                patch.object(release.time, 'monotonic', side_effect=[0, 181]):
            with self.assertRaisesRegex(RuntimeError, 'No manual publication was triggered'):
                release.find_run('v1.7.0', 'commit')
            self.assertEqual(api.call_args.args[:2], ('run', 'list'))

    def test_release_lookup_paginates_and_does_not_hide_api_failure(self):
        with patch.object(release, 'gh_json', side_effect=[
                [{'tag_name': 'other'}] * 100, [{'tag_name': 'v1.7.0'}]]) as api:
            self.assertEqual(release.release_state('v1.7.0')['tag_name'], 'v1.7.0')
            self.assertIn('page=2', api.call_args.args[1])
        with patch.object(release, 'gh_json', side_effect=RuntimeError('API unavailable')):
            with self.assertRaisesRegex(RuntimeError, 'API unavailable'):
                release.release_state('v1.7.0')


class TerminalOutput(unittest.TestCase):
    def test_git_diff_check_never_opens_pager_on_a_real_terminal(self):
        with tempfile.TemporaryDirectory(prefix='devctl pager ') as directory:
            root = Path(directory)
            marker = root / 'pager-used'
            pager = root / 'pager'
            pager.write_text('#!/bin/sh\nprintf used > "$PAGER_MARKER"\ncat\n')
            pager.chmod(0o755)
            pager_command = shlex.quote(str(pager))
            env = {**os.environ, 'PAGER_MARKER': str(marker), 'GIT_PAGER': pager_command,
                   'PAGER': pager_command, 'GH_PAGER': pager_command, 'LESS': '-R'}
            subprocess.run([GIT, 'init', '-q', directory], env=env, check=True)
            for fixed in (False, True):
                master, slave = pty.openpty()
                code = (f'import runpy; r=runpy.run_path({str(ROOT / "scripts/release")!r}); '
                        'r["run"]("git", "diff", "--check", capture=False)')
                command = [sys.executable, '-c', code] if fixed else [GIT, 'diff', '--check']
                process = subprocess.Popen(command, cwd=root, env=env, stdin=slave,
                                           stdout=slave, stderr=slave, start_new_session=True)
                os.close(slave)
                try:
                    self.assertEqual(process.wait(timeout=5), 0)
                    if fixed:
                        self.assertFalse(marker.exists(), 'release invoked an interactive pager')
                    else:
                        self.assertTrue(marker.exists(), 'control did not reproduce Git paging')
                        marker.unlink()
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait()
                    os.close(master)

    def test_quiet_checks_report_heartbeat_and_keep_full_log(self):
        with tempfile.TemporaryDirectory() as directory:
            progress = release.Progress('1.7.0')
            progress.log = str(Path(directory) / 'checks.log')
            child = Mock()
            child.wait.side_effect = [subprocess.TimeoutExpired(['cargo', 'test'], 10), 0]
            with patch.object(release.subprocess, 'Popen', return_value=child) as popen, \
                    patch('sys.stdout') as stdout:
                progress.check(3, 6, ('cargo', 'test', '--locked'))
            text = ''.join(call.args[0] for call in stdout.write.call_args_list)
            self.assertIn('still running', text)
            self.assertIn('OK', text)
            self.assertIn('$ cargo test --locked', Path(progress.log).read_text())
            self.assertEqual(popen.call_args.kwargs['stdin'], subprocess.DEVNULL)
            self.assertTrue(popen.call_args.kwargs['start_new_session'])


if __name__ == '__main__':
    unittest.main()
