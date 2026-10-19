"""Temporary service configuration, CLI polling and cleanup for TUI checks."""

import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path


class Workspace:
    def __init__(self, binary, mode="normal", suffix=""):
        self.binary = str(Path(binary).resolve())
        self.directory = tempfile.TemporaryDirectory(
            prefix="devctl-tui-", dir="/tmp" if sys.platform == "darwin" else None
        )
        self.root = Path(self.directory.name)
        self.config = self.root / "devctl.toml"
        self.clients = []
        worker = self.root / "worker.py"
        worker.write_text(Path(__file__).with_name("worker.py").read_text())
        text = f'version = 1\n[logs]\nenabled = {str(mode == "disk-history").lower()}\n[workspace]\nname = "tui-test"\n[projects.demo]\npath = {json.dumps(str(self.root))}\n'
        for service in ["middle", "weapp", "web"]:
            worker_mode = (
                "slow"
                if mode == "slow" and service == "middle"
                else "failure"
                if mode == "failure" and service == "weapp"
                else "high"
                if mode == "high" and service == "web"
                else "history"
                if mode in ["history", "disk-history"] and service == "web"
                else "normal"
            )
            args = [
                sys.executable,
                "-u",
                str(worker),
                str(self.root),
                service,
                worker_mode,
                suffix,
            ]
            text += f'\n[services.{service}]\nproject = "demo"\ncmd = {json.dumps(args, ensure_ascii=False)}\nautostart = true\n'
        self.config.write_text(text)

    def cli(self, *args, success=True):
        result = subprocess.run(
            [self.binary, "--config", str(self.config), *args],
            capture_output=True,
            timeout=22,
            env={**os.environ, "XDG_STATE_HOME": str(self.root / "state")},
        )
        if success:
            assert result.returncode == 0, result.stderr.decode()
        return result

    def pump_clients(self):
        # Keep acting as a terminal while polling CLI state. A full PTY output
        # queue otherwise blocks a redraw before the client can read control keys.
        for client in self.clients:
            if client.master is not None and client.process.poll() is None:
                client.pump(0.01)

    def states(self, **expected):
        deadline = time.monotonic() + 10
        while True:
            self.pump_clients()
            result = self.cli("status").stdout.decode()
            if all(
                f"{service}: {state}" in result for service, state in expected.items()
            ):
                return
            assert time.monotonic() < deadline, (expected, result)
            time.sleep(0.03)

    def starts(self, service):
        file = self.root / (service + ".starts")
        return file.read_text().splitlines() if file.exists() else []

    def wait_starts(self, service, count):
        deadline = time.monotonic() + 8
        while len(self.starts(service)) != count:
            self.pump_clients()
            assert time.monotonic() < deadline, (service, self.starts(service))
            time.sleep(0.02)

    def close(self):
        for client in self.clients:
            client.close()
        self.cli("shutdown", success=False)
        self.directory.cleanup()


def log_bodies(text, cursor=False):
    pointer = r"> " if cursor else ""
    return [
        body.rstrip()
        for body in re.findall(pointer + r"\[(?:out|err) #\d+\] ([^│\n]*)", text)
    ]
