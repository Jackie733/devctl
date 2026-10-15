"""Black-box TUI checks. Only Python's standard library and a real PTY are used."""
import codecs
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

BINARY = str(Path(sys.argv[1]).resolve())


class Screen:
    """Interpret the cursor/erase sequences emitted by the Crossterm backend."""
    def __init__(self, width=120, height=30):
        self.resize(width, height)
        self.decoder = codecs.getincrementaldecoder('utf-8')('replace')
        self.pending = ''

    def resize(self, width, height):
        self.width, self.height = width, height
        self.rows = [[' '] * width for _ in range(height)]
        self.x = self.y = 0

    def feed(self, data):
        self.pending += self.decoder.decode(data)
        while self.pending:
            if self.pending.startswith('\x1b['):
                match = re.match(r'\x1b\[([0-9;?]*)([ -/]*)([@-~])', self.pending)
                if not match:
                    return
                args, _, command = match.groups()
                self.pending = self.pending[match.end():]
                values = [int(value) if value else 0 for value in args.lstrip('?').split(';')]
                count = values[0] or 1
                if command in ('H', 'f'):
                    self.y = max(0, min(self.height - 1, count - 1))
                    self.x = max(0, min(self.width - 1, (values[1] if len(values) > 1 else 1) - 1))
                elif command == 'G': self.x = min(self.width - 1, count - 1)
                elif command == 'A': self.y = max(0, self.y - count)
                elif command == 'B': self.y = min(self.height - 1, self.y + count)
                elif command == 'C': self.x = min(self.width - 1, self.x + count)
                elif command == 'D': self.x = max(0, self.x - count)
                elif command == 'J' and values[0] in (2, 3):
                    self.rows = [[' '] * self.width for _ in range(self.height)]
                elif command == 'K':
                    self.rows[self.y][self.x:] = [' '] * (self.width - self.x)
                continue
            if self.pending[0] == '\x1b':
                if len(self.pending) < 2: return
                raise AssertionError(f'Unexpected escape: {self.pending[:30]!r}')
            character, self.pending = self.pending[0], self.pending[1:]
            if character == '\r': self.x = 0
            elif character == '\n': self.y = min(self.height - 1, self.y + 1)
            elif character >= ' ':
                if self.x < self.width and self.y < self.height:
                    self.rows[self.y][self.x] = character
                self.x += 1

    def text(self):
        return '\n'.join(''.join(row) for row in self.rows)


class Tui:
    def __init__(self, workspace):
        self.workspace = workspace
        self.master, self.slave = pty.openpty()
        self.screen = Screen()
        self.raw = bytearray()
        self.original = termios.tcgetattr(self.slave)
        self._winsize(120, 30)
        self.process = subprocess.Popen(
            [BINARY, '--config', str(workspace.config), 'tui'],
            stdin=self.slave, stdout=self.slave, stderr=subprocess.PIPE,
            start_new_session=True,
            preexec_fn=lambda: fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0),
        )
        workspace.clients.append(self)
        self.wait('Connected')

    def _winsize(self, width, height):
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack('HHHH', height, width, 0, 0))

    def pump(self, duration=0.05):
        deadline = time.monotonic() + duration
        while time.monotonic() < deadline:
            if select.select([self.master], [], [], min(0.05, max(0, deadline - time.monotonic())))[0]:
                data = os.read(self.master, 65536)
                self.raw.extend(data)
                self.screen.feed(data)

    def wait(self, expected, timeout=8):
        predicate = expected if callable(expected) else lambda text: expected in text
        deadline = time.monotonic() + timeout
        while not predicate(self.screen.text()):
            self.pump()
            if self.process.poll() is not None:
                raise AssertionError(f'TUI exited: {self.process.returncode}, {self.process.stderr.read()!r}')
            assert time.monotonic() < deadline, f'Waiting for {expected!r}:\n{self.screen.text()}'
        return self.screen.text()

    def send(self, value):
        os.write(self.master, value)

    def choose(self, service):
        self.send(b'\x1b[A\x1b[A')
        self.wait('Selected: middle |')
        steps = ['middle', 'weapp', 'web'].index(service)
        self.send(b'\x1b[B' * steps)
        self.wait(f'Selected: {service} |')

    def resize(self, width, height):
        self.screen.resize(width, height)
        self._winsize(width, height)
        self.process.send_signal(signal.SIGWINCH)

    def exit(self, key=b'q', expected=0):
        if key is not None: self.send(key)
        deadline = time.monotonic() + 10
        while self.process.poll() is None:
            self.pump()
            assert time.monotonic() < deadline, 'TUI did not exit'
        self.pump()
        error = self.process.stderr.read()
        assert self.process.returncode == expected, error
        assert not error, error
        assert termios.tcgetattr(self.slave) == self.original, 'Terminal modes not restored'
        assert b'\x1b[?1049l' in self.raw and b'\x1b[?25h' in self.raw

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
            try: self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
        self.process.stderr.close()
        if self.master is not None: os.close(self.master)
        os.close(self.slave)


WORKER = r'''
import os, signal, sys, time
from pathlib import Path
root, service, mode = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
with (root / (service + '.starts')).open('a') as stream:
    stream.write(str(os.getpid()) + '\n')
if mode == 'failure': sys.exit(7)
if mode == 'slow':
    def stop(*_):
        time.sleep(1.5)
        sys.exit(0)
    signal.signal(signal.SIGTERM, stop)
number = 0
while True:
    if mode == 'high':
        for _ in range(300):
            print(service + '-line-' + str(number) + '-' + 'x' * 400)
            number += 1
    else:
        print(service + '-line-' + str(number))
        if number % 3 == 0: print(service + '-stderr-' + str(number), file=sys.stderr)
        number += 1
    time.sleep(.02)
'''


class Workspace:
    def __init__(self, mode='normal'):
        self.directory = tempfile.TemporaryDirectory(prefix='devctl-tui-')
        self.root = Path(self.directory.name)
        self.config = self.root / 'devctl.toml'
        self.clients = []
        worker = self.root / 'worker.py'
        worker.write_text(WORKER)
        text = f'version = 1\n[workspace]\nname = "tui-test"\n[projects.demo]\npath = {json.dumps(str(self.root))}\n'
        for service in ['middle', 'weapp', 'web']:
            worker_mode = ('slow' if mode == 'slow' and service == 'middle' else
                           'failure' if mode == 'failure' and service == 'weapp' else
                           'high' if mode == 'high' and service == 'web' else 'normal')
            args = [sys.executable, '-u', str(worker), str(self.root), service, worker_mode]
            text += f'\n[services.{service}]\nproject = "demo"\ncmd = {json.dumps(args)}\nautostart = true\n'
        self.config.write_text(text)

    def cli(self, *args, success=True):
        result = subprocess.run([BINARY, '--config', str(self.config), *args], capture_output=True, timeout=22)
        if success: assert result.returncode == 0, result.stderr.decode()
        return result

    def states(self, **expected):
        deadline = time.monotonic() + 10
        while True:
            result = self.cli('status').stdout.decode()
            if all(f'{service}: {state}' in result for service, state in expected.items()): return
            assert time.monotonic() < deadline, (expected, result)
            time.sleep(.03)

    def starts(self, service):
        file = self.root / (service + '.starts')
        return file.read_text().splitlines() if file.exists() else []

    def wait_starts(self, service, count):
        deadline = time.monotonic() + 8
        while len(self.starts(service)) != count:
            assert time.monotonic() < deadline, (service, self.starts(service))
            time.sleep(.02)

    def close(self):
        for client in self.clients: client.close()
        self.cli('shutdown', success=False)
        self.directory.cleanup()


def lifecycle():
    w = Workspace()
    try:
        ui = Tui(w)
        w.states(middle='Stopped', weapp='Stopped', web='Stopped')
        assert not any(w.starts(service) for service in ['middle', 'weapp', 'web'])
        ui.choose('web')
        ui.send(b's')
        w.states(web='Running', weapp='Stopped', middle='Stopped')
        ui.wait('web-line-')
        ui.wait('web-stderr-')
        first = w.starts('web')
        ui.send(b's')
        ui.pump(.25)
        assert w.starts('web') == first and len(first) == 1
        ui.send(b'a')
        w.states(web='Running', weapp='Running', middle='Running')
        w.wait_starts('weapp', 1)
        assert w.starts('web') == first
        ui.choose('weapp')
        ui.send(b'x')
        w.states(weapp='Stopped', web='Running', middle='Running')
        ui.wait('Selected services stopped.')
        ui.send(b's')
        w.states(weapp='Running')
        w.wait_starts('weapp', 2)
        assert len(w.starts('weapp')) == 2
        ui.choose('web')
        ui.pump(.5)
        ui.send(b'\x1b[5~')
        ui.wait('reading')
        anchor = re.search(r'seq=(\d+)', ui.screen.text()).group(1)
        ui.pump(.4)
        assert re.search(r'seq=(\d+)', ui.screen.text()).group(1) == anchor
        ui.send(b'\x1b[F')
        ui.wait('following')
        ui.send(b'Q')
        ui.wait('y: confirm')
        ui.send(b'n')
        ui.send(b'?')
        ui.wait('business readiness')
        ui.send(b'\x1b')
        ui.resize(50, 14)
        ui.wait('Selected: web | Running')
        ui.wait('q leave')
        ui.resize(120, 30)
        ui.wait('Projects')
        ui.exit()
        w.states(web='Running', weapp='Running', middle='Running')
        again = Tui(w)
        again.choose('web')
        again.wait('web-line-')
        assert w.starts('web') == first
        again.send(b'X')
        w.states(web='Stopped', weapp='Stopped', middle='Stopped')
        again.wait('Selected services stopped.')
        again.send(b'a')
        w.states(web='Running', weapp='Running', middle='Running')
        again.wait('Start request accepted')
        again.send(b'Qy')
        again.exit(key=None)
        assert w.cli('status', success=False).returncode != 0
    finally: w.close()


def reconnect():
    w = Workspace()
    try:
        ui = Tui(w)
        ui.choose('web')
        ui.send(b's')
        w.states(web='Running')
        ui.wait('web-line-')
        w.cli('shutdown')
        ui.wait('state unknown')
        ui.pump(.7)
        assert w.cli('status', success=False).returncode != 0, 'Refresh recreated a workspace'
        ui.send(b's')
        ui.wait('Reconnect before controlling')
        w.cli('start', 'weapp')
        ui.wait('Connected')
        w.states(web='Stopped', weapp='Running', middle='Stopped')
        ui.choose('weapp')
        ui.wait('weapp-line-')
        ui.send(b'r')
        ui.wait('Connected')
        ui.exit(b'\x03')
        w.states(weapp='Running')
        for sig in [signal.SIGINT, signal.SIGTERM, signal.SIGHUP]:
            again = Tui(w)
            again.process.send_signal(sig)
            again.exit(key=None)
            w.states(weapp='Running')
        closed = Tui(w)
        os.close(closed.master)
        closed.master = None
        closed.process.wait(timeout=5)
        w.states(weapp='Running')
        attached = Tui(w)
        attached.choose('weapp')
        attached.wait('weapp-line-')
        attached.exit()
    finally: w.close()


def slow_stop_and_failure():
    w = Workspace('slow')
    try:
        ui = Tui(w)
        ui.send(b's')  # Sorted first project: middle.
        w.states(middle='Running')
        ui.wait('middle-line-')
        ui.send(b'x')
        w.states(middle='Stopping')
        ui.send(b'\x1b[B')
        ui.wait('Selected: weapp |', timeout=1)
        start = time.monotonic()
        ui.exit()
        assert time.monotonic() - start < 1, 'Leaving waited for service cleanup'
        w.states(middle='Stopped')
    finally: w.close()
    w = Workspace('failure')
    try:
        ui = Tui(w)
        ui.send(b'a')
        w.states(weapp='Failed', web='Running', middle='Running')
        ui.choose('weapp')
        ui.wait('Selected: weapp | Failed')
        ui.wait(lambda text: 'unexpectedly' in text and '7' in text)
        ui.exit()
        w.states(web='Running', middle='Running')
    finally: w.close()


def high_output():
    w = Workspace('high')
    try:
        ui = Tui(w)
        ui.choose('web')
        ui.send(b's')
        w.states(web='Running')
        ui.wait('web-line-')
        ui.send(b'\x1b[A')
        ui.wait('Selected: weapp |', timeout=2)
        ui.send(b'\x1b[B')
        ui.wait('web-line-')
        start = time.monotonic()
        ui.exit()
        assert time.monotonic() - start < 1.5, 'High output blocked leaving'
        w.states(web='Running')
    finally: w.close()


{'lifecycle': lifecycle, 'reconnect': reconnect, 'slow': slow_stop_and_failure, 'high': high_output}[sys.argv[2]]()
print('PASS:', sys.argv[2])
