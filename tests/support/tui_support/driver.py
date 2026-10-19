"""Drive a real TUI while checking terminal and mouse-mode restoration."""

import fcntl
import os
import pty
import select
import signal
import struct
import subprocess
import termios
import time

from .screen import Screen


class Tui:
    def __init__(
        self,
        workspace,
        discover=False,
        windows_clipboard=False,
        explicit_tui=False,
        mac_clipboard=False,
        wait_for="Connected",
        appearance=None,
    ):
        self.workspace = workspace
        self.master, self.slave = pty.openpty()
        self.screen = Screen()
        self.raw = bytearray()
        self.original = termios.tcgetattr(self.slave)
        self._winsize(120, 30)
        args = (
            [workspace.binary]
            if discover
            else [workspace.binary, "--config", str(workspace.config)]
        )
        if explicit_tui:
            args.append("tui")
        environment = os.environ.copy()
        # Never write to the real host clipboard, even when the suite runs in WSL.
        for key in [
            "DEVCTL_TUI_THEME",
            "DEVCTL_TUI_ASCII",
            "DEVCTL_TUI_REDUCED_MOTION",
            "NO_COLOR",
        ]:
            environment.pop(key, None)
        environment.update(appearance or {})
        environment["SSH_CONNECTION"] = "devctl-pty-test"
        environment["XDG_STATE_HOME"] = str(workspace.root / "state")
        if windows_clipboard:
            environment.pop("SSH_CONNECTION")
            environment["WSL_DISTRO_NAME"] = "devctl-pty-test"
            environment["PATH"] = (
                str(workspace.root) + os.pathsep + environment.get("PATH", "")
            )
        if mac_clipboard:
            environment["LC_ALL"] = "C"
            environment.pop("SSH_CONNECTION", None)
            environment.pop("WSL_DISTRO_NAME", None)
            environment["PATH"] = (
                str(workspace.root) + os.pathsep + environment.get("PATH", "")
            )
        directory = workspace.root / "unrelated-directory"
        directory.mkdir(exist_ok=True)
        if discover:
            environment["HOME"] = str(workspace.root)
        self.process = subprocess.Popen(
            args,
            cwd=directory,
            env=environment,
            stdin=self.slave,
            stdout=self.slave,
            stderr=subprocess.PIPE,
            start_new_session=True,
            preexec_fn=lambda: fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0),
        )
        workspace.clients.append(self)
        self.wait(wait_for)

    def _winsize(self, width, height):
        fcntl.ioctl(
            self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0)
        )

    def pump(self, duration=0.05):
        deadline = time.monotonic() + duration
        while time.monotonic() < deadline:
            if select.select(
                [self.master], [], [], min(0.05, max(0, deadline - time.monotonic()))
            )[0]:
                data = os.read(self.master, 65536)
                self.raw.extend(data)
                self.screen.feed(data)

    def wait(self, expected, timeout=8):
        predicate = expected if callable(expected) else lambda text: expected in text
        deadline = time.monotonic() + timeout
        while not predicate(self.screen.text()):
            self.pump()
            if self.process.poll() is not None:
                raise AssertionError(
                    f"TUI exited: {self.process.returncode}, {self.process.stderr.read()!r}"
                )
            assert time.monotonic() < deadline, (
                f"Waiting for {expected!r}:\n{self.screen.text()}"
            )
        return self.screen.text()

    def send(self, value):
        os.write(self.master, value)

    def mouse(self, button, column, row, release=False):
        self.send(
            f"\x1b[<{button};{column + 1};{row + 1}{'m' if release else 'M'}".encode()
        )

    def choose(self, service):
        self.send(b"1gg")
        self.wait("Selected: middle |")
        steps = ["middle", "weapp", "web"].index(service)
        self.send(b"\x1b[B" * steps)
        self.wait(f"Selected: {service} |")

    def resize(self, width, height):
        self.screen.resize(width, height)
        self._winsize(width, height)
        self.process.send_signal(signal.SIGWINCH)

    def exit(self, key=b"q", expected=0):
        if key is not None:
            self.send(key)
        deadline = time.monotonic() + 10
        while self.process.poll() is None:
            self.pump()
            assert time.monotonic() < deadline, "TUI did not exit"
        self.pump()
        error = self.process.stderr.read()
        assert self.process.returncode == expected, error
        assert not error, error
        assert termios.tcgetattr(self.master) == self.original, (
            "Terminal modes not restored"
        )
        assert b"\x1b[?1049l" in self.raw and b"\x1b[?25h" in self.raw
        for mode in (1000, 1002, 1003, 1015, 1006):
            assert f"\x1b[?{mode}h".encode() in self.raw, (
                f"Mouse mode {mode} was not enabled"
            )
            assert f"\x1b[?{mode}l".encode() in self.raw, (
                f"Mouse mode {mode} was not restored"
            )

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
            deadline = time.monotonic() + 3
            # Darwin can wait for terminal output to drain during process exit.
            while self.process.poll() is None and time.monotonic() < deadline:
                self.pump()
            if self.process.poll() is None:
                self.process.kill()
                os.close(self.master)
                self.master = None
                self.process.wait(timeout=3)
        self.process.stderr.close()
        if self.master is not None:
            os.close(self.master)
        os.close(self.slave)
