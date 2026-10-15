"""Isolated Linux checks; never modify the real user's HOME or stop services."""

import fcntl
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
PATH_LINE = 'export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"'


class InstallScripts(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="devctl install ")
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name)
        self.env = {**os.environ, "HOME": str(self.home), "SHELL": "/bin/bash"}
        for key in ("BASH_ENV", "ENV", "ZDOTDIR"):
            self.env.pop(key, None)
        self.bin = self.home / ".local/bin"
        self.target = self.bin / "devctl"
        self.previous = self.bin / "devctl.previous"

    def run_script(self, name, *args, ok=True):
        result = subprocess.run(
            ["bash", str(SCRIPTS / name), *map(str, args)],
            env=self.env, capture_output=True, text=True, timeout=15,
        )
        self.assertEqual(result.returncode == 0, ok, result.stdout + result.stderr)
        return result

    def candidate(self, name, body):
        path = self.home / name
        path.write_text("#!/bin/sh\n" + body + "\n")
        return path

    def install_version(self, version):
        path = self.candidate(version, f"echo 'devctl {version}'")
        self.run_script("devctl-update", path, hashlib.sha256(path.read_bytes()).hexdigest())
        return path.read_bytes()

    def test_install_update_repeat_and_rollback_preserve_config(self):
        config = self.home / ".config/devctl/devctl.toml"
        config.parent.mkdir(parents=True)
        config.write_text("my existing workspace\n")
        first = self.install_version("first")
        self.assertFalse(self.previous.exists())
        second = self.install_version("second")
        self.assertEqual(self.target.read_bytes(), second)
        self.assertEqual(self.previous.read_bytes(), first)
        self.install_version("second")
        self.assertEqual(self.previous.read_bytes(), first)
        self.run_script("devctl-update", "--rollback")
        self.assertEqual(self.target.read_bytes(), first)
        self.assertEqual(self.previous.read_bytes(), second)
        self.assertTrue(os.access(self.target, os.X_OK))
        self.assertEqual(config.read_text(), "my existing workspace\n")

    def test_bad_checksum_is_not_executed_and_preserves_both_versions(self):
        first = self.install_version("first")
        second = self.install_version("second")
        marker = self.home / "executed"
        path = self.candidate("untrusted", f"touch '{marker}'\necho 'devctl bad'")
        result = self.run_script("devctl-update", path, "0" * 64, ok=False)
        self.assertIn("SHA-256 mismatch", result.stderr)
        self.assertFalse(marker.exists())
        self.assertEqual(self.target.read_bytes(), second)
        self.assertEqual(self.previous.read_bytes(), first)
        self.assertEqual(list(self.bin.glob(".devctl-update.*")), [self.bin / ".devctl-update.lock"])

    def test_unrunnable_or_wrong_program_does_not_replace_install(self):
        installed = self.install_version("first")
        for name, body in [("broken", "exit 1"), ("other", "echo other-tool")]:
            path = self.candidate(name, body)
            self.run_script("devctl-update", path, hashlib.sha256(path.read_bytes()).hexdigest(), ok=False)
            self.assertEqual(self.target.read_bytes(), installed)

    def test_missing_backup_bad_arguments_and_http_are_rejected(self):
        self.run_script("devctl-update", "--rollback", ok=False)
        self.run_script("devctl-update", "file", "bad-hash", ok=False)
        self.run_script("devctl-update", "http://example.invalid/devctl", "0" * 64, ok=False)
        self.assertFalse(self.target.exists())

    def test_concurrent_update_and_custom_symlink_are_rejected(self):
        installed = self.install_version("first")
        with (self.bin / ".devctl-update.lock").open("w") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            result = self.run_script("devctl-update", "--rollback", ok=False)
            self.assertIn("Another update", result.stderr)
        self.assertEqual(self.target.read_bytes(), installed)
        self.target.unlink()
        actual = self.home / "custom"
        actual.write_bytes(installed)
        self.target.symlink_to(actual)
        self.run_script("devctl-update", "--rollback", ok=False)
        self.assertTrue(self.target.is_symlink())
        self.assertEqual(actual.read_bytes(), installed)

    def test_failed_download_keeps_existing_install(self):
        installed = self.install_version("first")
        mock_dir = self.home / "mock"
        mock_dir.mkdir()
        curl = mock_dir / "curl"
        curl.write_text("#!/bin/sh\nexit 22\n")
        curl.chmod(0o755)
        self.env["PATH"] = str(mock_dir) + os.pathsep + self.env["PATH"]
        self.run_script("devctl-update", "https://example.invalid/devctl", "0" * 64, ok=False)
        self.assertEqual(self.target.read_bytes(), installed)

    def test_https_download_uses_restricted_protocols_and_checks_content(self):
        path = self.candidate("download", "echo 'devctl downloaded'")
        mock_dir = self.home / "mock"
        mock_dir.mkdir()
        curl = mock_dir / "curl"
        curl.write_text(
            "#!/usr/bin/env python3\n"
            "import os, pathlib, shutil, sys\n"
            "args = sys.argv[1:]\n"
            "assert args[0] == '--disable'\n"
            "assert args[args.index('--proto') + 1] == '=https'\n"
            "assert args[args.index('--proto-redir') + 1] == '=https'\n"
            "assert '--fail' in args and '--location' in args\n"
            "shutil.copyfile(os.environ['TEST_DOWNLOAD'], args[args.index('--output') + 1])\n"
        )
        curl.chmod(0o755)
        self.env["PATH"] = str(mock_dir) + os.pathsep + self.env["PATH"]
        self.env["TEST_DOWNLOAD"] = str(path)
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        self.run_script("devctl-update", "https://example.invalid/devctl", digest.upper())
        self.assertEqual(self.target.read_bytes(), path.read_bytes())

    def test_path_setup_is_repeatable_and_prefers_new_install(self):
        rc = self.home / ".bashrc"
        rc.write_text("# existing settings without final newline")
        self.run_script("setup-path.sh", "bash")
        self.run_script("setup-path.sh", "bash")
        self.assertEqual(rc.read_text().count(PATH_LINE), 1)
        self.assertTrue(rc.read_text().startswith("# existing settings without final newline\n"))
        self.install_version("first")
        cargo_bin = self.home / ".cargo/bin"
        cargo_bin.mkdir(parents=True)
        old = cargo_bin / "devctl"
        old.write_text("#!/bin/sh\necho old\n")
        old.chmod(0o755)
        result = subprocess.run(
            ["bash", "--noprofile", "-ic", "command -v devctl"],
            env=self.env, capture_output=True, text=True, timeout=10,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), str(self.target))

    def test_zsh_custom_startup_directory_and_unknown_shell(self):
        custom = self.home / "zsh config"
        custom.mkdir()
        self.env["ZDOTDIR"] = str(custom)
        self.run_script("setup-path.sh", "zsh")
        self.assertIn(PATH_LINE, (custom / ".zshrc").read_text())
        self.run_script("setup-path.sh", "fish", ok=False)
        self.assertFalse((self.home / ".bashrc").exists())


if __name__ == "__main__":
    unittest.main()
