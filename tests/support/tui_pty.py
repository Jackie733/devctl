"""Black-box TUI checks. Only Python's standard library and a real PTY are used."""
import codecs
import base64
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
import unicodedata

BINARY = str(Path(sys.argv[1]).resolve())


class Screen:
    """Interpret the cursor/erase sequences emitted by the Crossterm backend."""
    def __init__(self, width=120, height=30):
        self.resize(width, height)
        self.decoder = codecs.getincrementaldecoder('utf-8')('replace')
        self.pending = ''
        self.clipboard = None
        self.clipboard_requests = 0

    def resize(self, width, height):
        self.width, self.height = width, height
        self.rows = [[' '] * width for _ in range(height)]
        self.x = self.y = 0

    def feed(self, data):
        self.pending += self.decoder.decode(data)
        while self.pending:
            if self.pending.startswith('\x1b]'):
                match = re.match(r'\x1b\]52;c;([A-Za-z0-9+/=]*)(?:\x07|\x1b\\)', self.pending)
                if not match:
                    assert len(self.pending) < 100000, 'Clipboard command exceeded its budget'
                    return
                self.clipboard = base64.b64decode(match.group(1), validate=True).decode('utf-8')
                self.clipboard_requests += 1
                self.pending = self.pending[match.end():]
                continue
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
                width = 2 if unicodedata.east_asian_width(character) in ('W', 'F') else 1
                if self.x < self.width and self.y < self.height:
                    row = self.rows[self.y]
                    # Overwriting either half of a wide glyph clears its other
                    # half, as a real terminal does. Incremental redraws may
                    # replace Chinese path characters with ordinary spaces.
                    if row[self.x] == '' and self.x > 0:
                        row[self.x - 1] = ' '
                    if self.x + 1 < self.width and row[self.x + 1] == '':
                        row[self.x + 1] = ' '
                    if width == 2 and self.x + 2 < self.width and row[self.x + 2] == '':
                        row[self.x + 2] = ' '
                    row[self.x] = character
                    if width == 2 and self.x + 1 < self.width:
                        row[self.x + 1] = ''
                self.x += width

    def text(self):
        return '\n'.join(''.join(row) for row in self.rows)


class Tui:
    def __init__(self, workspace, discover=False, windows_clipboard=False, explicit_tui=False, mac_clipboard=False, wait_for="Connected", appearance=None):
        self.workspace = workspace
        self.master, self.slave = pty.openpty()
        self.screen = Screen()
        self.raw = bytearray()
        self.original = termios.tcgetattr(self.slave)
        self._winsize(120, 30)
        args = [BINARY] if discover else [BINARY, '--config', str(workspace.config)]
        if explicit_tui: args.append('tui')
        environment = os.environ.copy()
        # Never write to the real host clipboard, even when the suite runs in WSL.
        for key in ['DEVCTL_TUI_THEME', 'DEVCTL_TUI_ASCII', 'DEVCTL_TUI_REDUCED_MOTION', 'NO_COLOR']:
            environment.pop(key, None)
        environment.update(appearance or {})
        environment['SSH_CONNECTION'] = 'devctl-pty-test'
        environment['XDG_STATE_HOME'] = str(workspace.root / 'state')
        if windows_clipboard:
            environment.pop('SSH_CONNECTION')
            environment['WSL_DISTRO_NAME'] = 'devctl-pty-test'
            environment['PATH'] = str(workspace.root) + os.pathsep + environment.get('PATH', '')
        if mac_clipboard:
            environment['LC_ALL'] = 'C'
            environment.pop('SSH_CONNECTION', None)
            environment.pop('WSL_DISTRO_NAME', None)
            environment['PATH'] = str(workspace.root) + os.pathsep + environment.get('PATH', '')
        directory = workspace.root / 'unrelated-directory'
        directory.mkdir(exist_ok=True)
        if discover:
            environment['HOME'] = str(workspace.root)
        self.process = subprocess.Popen(
            args, cwd=directory, env=environment,
            stdin=self.slave, stdout=self.slave, stderr=subprocess.PIPE,
            start_new_session=True,
            preexec_fn=lambda: fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0),
        )
        workspace.clients.append(self)
        self.wait(wait_for)

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
        self.send(b'1gg')
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
        assert termios.tcgetattr(self.master) == self.original, 'Terminal modes not restored'
        assert b'\x1b[?1049l' in self.raw and b'\x1b[?25h' in self.raw

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
        if self.master is not None: os.close(self.master)
        os.close(self.slave)


WORKER = r'''
import json, os, re, signal, socket, struct, sys, time
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
if mode == 'history':
    def read_exact(stream, size):
        data = b''
        while len(data) < size:
            block = stream.recv(size - len(data))
            if not block: raise RuntimeError('Log fixture control connection closed')
            data += block
        return data

    def acknowledged(expected):
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            with socket.socket(socket.AF_UNIX) as stream:
                stream.settimeout(10)
                stream.connect(str(root / 'devctl.toml.devctl/control.sock'))
                stream.sendall(b'CONTROL1\n')
                assert read_exact(stream, 9) == b'DEVCTL 1\n'
                read_exact(stream, struct.unpack('!I', read_exact(stream, 4))[0])
                payload = ("action='logs'\nservice=" + json.dumps(service) + "\nlimit=1\n").encode()
                stream.sendall(struct.pack('!I', len(payload)) + payload)
                response = read_exact(stream, struct.unpack('!I', read_exact(stream, 4))[0]).decode()
            latest = re.search(r'^latest = (\d+)$', response, re.M)
            if latest and int(latest.group(1)) >= expected: return
            time.sleep(.02)
        raise RuntimeError('Log fixture delivery was not acknowledged')

    emitted = 0
    def emit(text):
        global emitted
        print(text)
        emitted += 1
        # Retention tests need exact input, not producer-overload loss. Query replies
        # also wait for prior archive writes, keeping both bounded queues drained.
        if emitted % 32 == 0: acknowledged(emitted)

    for number in range(350): emit(service + '-history-' + str(number))
    emit(service + '-history-ready')
    while not (root / 'history-more').exists(): time.sleep(.02)
    for number in range(4200): emit(service + '-new-' + str(number))
    emit(service + '-history-more-done')
    acknowledged(emitted)
    while True: time.sleep(1)

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
        self.directory = tempfile.TemporaryDirectory(prefix='devctl-tui-', dir='/tmp' if sys.platform == 'darwin' else None)
        self.root = Path(self.directory.name)
        self.config = self.root / 'devctl.toml'
        self.clients = []
        worker = self.root / 'worker.py'
        worker.write_text(WORKER)
        text = f'version = 1\n[logs]\nenabled = {str(mode == "disk-history").lower()}\n[workspace]\nname = "tui-test"\n[projects.demo]\npath = {json.dumps(str(self.root))}\n'
        for service in ['middle', 'weapp', 'web']:
            worker_mode = ('slow' if mode == 'slow' and service == 'middle' else
                           'failure' if mode == 'failure' and service == 'weapp' else
                           'high' if mode == 'high' and service == 'web' else
                           'history' if mode in ['history', 'disk-history'] and service == 'web' else 'normal')
            args = [sys.executable, '-u', str(worker), str(self.root), service, worker_mode]
            text += f'\n[services.{service}]\nproject = "demo"\ncmd = {json.dumps(args)}\nautostart = true\n'
        self.config.write_text(text)

    def cli(self, *args, success=True):
        result = subprocess.run([BINARY, '--config', str(self.config), *args], capture_output=True, timeout=22, env={**os.environ, 'XDG_STATE_HOME': str(self.root / 'state')})
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
        ui.send(b'A')
        w.states(web='Running', weapp='Running', middle='Running')
        w.wait_starts('weapp', 1)
        assert w.starts('web') == first
        ui.choose('weapp')
        ui.wait('weapp-line-')
        ui.send(b'x')
        w.states(weapp='Stopped', web='Running', middle='Running')
        ui.wait('weapp: stopped; cleanup confirmed.')
        ui.wait('Logs · STOPPED: weapp')
        ui.wait('STOPPED | Retained logs')
        assert 'weapp-line-' in ui.screen.text(), 'Stopping cleared retained logs'
        ui.send(b's')
        w.states(weapp='Running')
        w.wait_starts('weapp', 2)
        ui.wait('Logs · RUNNING: weapp')
        ui.wait(lambda text: re.search(r'#[0-9]+ old seq=', text) is not None)
        assert len(w.starts('weapp')) == 2
        ui.choose('web')
        ui.pump(.5)
        ui.send(b'\x1b[5~')
        ui.wait('READ')
        anchor = re.search(r'seq=(\d+)', ui.screen.text()).group(1)
        ui.pump(.4)
        assert re.search(r'seq=(\d+)', ui.screen.text()).group(1) == anchor
        ui.send(b'\x1b[F')
        ui.wait('FOLLOW')
        ui.send(b'Q')
        ui.wait('y: confirm')
        ui.resize(36, 10)
        ui.wait('Enlarge window to confirm.')
        ui.send(b'y')
        ui.pump(.15)
        w.states(web='Running', weapp='Running', middle='Running')
        ui.resize(120, 30)
        ui.wait('y: confirm')
        ui.send(b'n')
        ui.send(b'?')
        ui.wait('business readiness')
        ui.wait('?/Esc close | q leave')
        ui.send(b'\x1b')
        ui.wait(lambda text: ' Help ' not in text)
        ui.resize(50, 14)
        ui.wait('Selected: web | Running')
        ui.wait('q leave')
        ui.resize(120, 30)
        ui.wait('Projects')
        ui.exit()
        w.states(web='Running', weapp='Running', middle='Running')
        again = Tui(w, explicit_tui=True)
        again.choose('web')
        again.wait('web-line-')
        assert w.starts('web') == first
        again.send(b'X')
        w.states(web='Stopped', weapp='Stopped', middle='Stopped')
        again.wait('All services stopped; background workspace retained.')
        again.send(b'A')
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
        ui.wait('Logs · UNKNOWN: web')
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
        ui.send(b'A')
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


def appearance():
    # A wide glyph must not leave a hidden continuation when overwritten.
    screen = Screen(8, 2)
    screen.feed('中a'.encode())
    screen.feed(b'\x1b[1;1Hx')
    assert screen.text().splitlines()[0].startswith('x a')
    screen.feed('\x1b[1;1H中'.encode())
    screen.feed(b'\x1b[1;2Hy')
    assert screen.text().splitlines()[0].startswith(' ya')
    for theme in ['dark', 'light', 'terminal', 'mono']:
        w = Workspace()
        try:
            options = {'DEVCTL_TUI_THEME': 'terminal' if theme == 'mono' else theme}
            if theme == 'mono':
                options.update(NO_COLOR='1', DEVCTL_TUI_ASCII='1', DEVCTL_TUI_REDUCED_MOTION='1')
            ui = Tui(w, appearance=options)
            ui.choose('web')
            ui.wait('- Stopped' if theme == 'mono' else '○ Stopped')
            ui.send(b's')
            ui.wait('* Running' if theme == 'mono' else '● Running')
            ui.wait('web-line-')
            ui.send(b'0/web-line-\r')
            ui.wait('Match seq=')
            ui.send(b'v')
            ui.wait('V-LINE')
            ui.send(b'\x1b')
            ui.wait(lambda text: 'V-LINE' not in text)
            ui.send(b'?')
            ui.wait('CURRENT PANEL')
            ui.send(b'\x1b')
            ui.wait(lambda text: ' Help ' not in text)
            ui.resize(60,18)
            ui.wait('q leave')
            ui.send(b'1')
            ui.wait('s/x start/stop | Enter logs | a add')
            ui.send(b'x')
            ui.wait('- Stopped' if theme == 'mono' else '○ Stopped')
            assert b'\x1b[7m' not in ui.raw, 'State changes must not invert colors'
            if theme == 'mono':
                assert b'38;2;' not in ui.raw and b'48;2;' not in ui.raw
            elif theme in ['dark','light']:
                assert b'38;2;' in ui.raw and b'48;2;' in ui.raw
            ui.exit()
        finally:
            w.close()


def navigation():
    w = Workspace()
    try:
        ui = Tui(w)
        ui.choose('web')
        ui.send(b's')
        w.states(web='Running', weapp='Stopped', middle='Stopped')
        ui.wait('web-line-')
        assert 'Cleanup UNCONFIRMED' not in ui.screen.text(), 'Live service mislabeled as cleanup failure'
        ui.pump(.7)
        ui.send(b'0gg')
        ui.wait('Logs · ACTIVE')
        ui.wait('READ')
        selected = 'Selected: web | Running'
        assert selected in ui.screen.text()
        anchor = re.search(r'seq=(\d+)', ui.screen.text()).group(1)
        ui.send(b'j')
        ui.wait(lambda text: re.search(r'> \[(?:stdout|stderr) #\d+ seq=(\d+)\]', text))
        cursor = re.search(r'> \[(?:stdout|stderr) #\d+ seq=(\d+)\]', ui.screen.text()).group(1)
        assert cursor != anchor and re.search(r'seq=(\d+)', ui.screen.text()).group(1) == anchor
        assert selected in ui.screen.text(), 'Log j changed the selected project'
        ui.send(b'?sxaXQ')
        ui.wait('CURRENT PANEL')
        w.states(web='Running', weapp='Stopped', middle='Stopped')
        assert not w.starts('weapp') and not w.starts('middle'), 'Help leaked a control key'
        ui.send(b'G')
        ui.wait('CURRENT DETAILS')
        ui.resize(36, 10)
        ui.wait('?/Esc close | q leave')
        ui.send(b'gg')
        ui.wait('CURRENT PANEL')
        ui.resize(120, 30)
        ui.wait('CURRENT PANEL')
        ui.send(b'G')
        ui.wait('CURRENT DETAILS')
        ui.send(b'\x1b')
        ui.wait('Logs · ACTIVE')
        ui.resize(36, 10)
        ui.wait('Logs · ACTIVE')
        ui.wait('q leave')
        ui.send(b'1')
        ui.wait('Projects · ACTIVE')
        ui.send(b'0')
        ui.wait('Logs · ACTIVE')
        ui.resize(20, 6)
        ui.wait('Window too small')
        ui.send(b'QyX')
        ui.pump(.15)
        w.states(web='Running')
        ui.resize(120, 30)
        ui.wait('Logs · ACTIVE')
        ui.send(b'G')
        ui.wait('FOLLOW')
        ui.send(b'?')
        ui.wait('CURRENT PANEL')
        ui.exit()
        w.states(web='Running')
    finally: w.close()


def copying():
    probe = Screen()
    # PTY reads may split a clipboard command at any byte, including its prefix.
    for byte in b'\x1b]52;c;aGVsbG8K\x1b\\':
        probe.feed(bytes([byte]))
    assert probe.clipboard == 'hello\n' and probe.clipboard_requests == 1
    w = Workspace()
    try:
        ui = Tui(w)
        ui.choose('web')
        ui.send(b's')
        w.states(web='Running', weapp='Stopped', middle='Stopped')
        ui.wait('web-line-')
        ui.pump(.6)
        ui.send(b'0ggjj')
        ui.wait(lambda text: '> [stdout' in text or '> [stderr' in text)
        ui.send(b'v')
        ui.wait('V-LINE')
        bodies = re.findall(r'\[(?:stdout|stderr) #\d+ seq=\d+\] ([^│\n]*)', ui.screen.text())
        assert len(bodies) >= 4
        expected = '\n'.join(body.rstrip() for body in bodies[2:4]) + '\n'
        ui.send(b'jsxaXQy')
        ui.wait(lambda _: ui.screen.clipboard_requests == 1)
        assert ui.screen.clipboard == expected, (repr(ui.screen.clipboard), repr(expected))
        assert 'seq=' not in ui.screen.clipboard
        ui.wait('Sent 2 lines via OSC 52')
        w.states(web='Running', weapp='Stopped', middle='Stopped')
        ui.send(b'y')
        ui.pump(.1)
        assert ui.screen.clipboard_requests == 1, 'Normal mode y copied without a selection'
        ui.send(b'V?jy')
        ui.wait(' Help ')
        ui.pump(.1)
        assert ui.screen.clipboard_requests == 1, 'Help leaked copy key'
        ui.send(b'\x1b')
        # V-LINE also exists behind help. Wait for Escape to close the modal
        # before changing PTY dimensions and resetting the screen interpreter.
        ui.wait(lambda text: ' Help ' not in text and 'V-LINE' in text)
        ui.resize(36, 10)
        ui.wait('y copy')
        ui.send(b'\x1b')
        ui.wait(lambda text: 'V-LINE' not in text and 'Logs · ACTIVE' in text)
        ui.send(b'v')
        ui.wait('V-LINE')
        ui.exit()
        w.states(web='Running')
    finally: w.close()


def windows_copying():
    native_copying(False)


def mac_copying():
    if sys.platform == 'darwin':
        native_copying(True)


def native_copying(mac):
    program = 'pbcopy' if mac else 'clip.exe'
    w = Workspace()
    try:
        worker = w.root / 'worker.py'
        worker.write_text(worker.read_text().replace(
            "print(service + '-line-' + str(number))",
            "print(service + '-line-' + str(number) + '-中文🙂')"))
        clip = w.root / program
        clip.write_text('''#!/usr/bin/env python3
import os, sys, time
from pathlib import Path
root = Path(__file__).parent
if Path(__file__).name == 'pbcopy': assert os.environ.get('LC_ALL') == 'en_US.UTF-8'
(root / 'clip.pid').write_text(str(os.getpid()))
mode = (root / 'clip.mode').read_text()
if mode == 'fail': sys.exit(7)
if mode == 'slow': time.sleep(.8)
if mode == 'hang': time.sleep(60)
(root / 'clipboard.bin').write_bytes(sys.stdin.buffer.read())
''')
        clip.chmod(0o700)
        mode = w.root / 'clip.mode'
        mode.write_text('ok')
        ui = Tui(w, windows_clipboard=not mac, mac_clipboard=mac)
        ui.choose('web')
        ui.send(b's')
        w.states(web='Running', weapp='Stopped', middle='Stopped')
        ui.wait('web-line-')
        ui.pump(.6)
        ui.send(b'0ggjjvjy')
        ui.wait(f'Copied 2 lines via {program}')
        copied = (w.root / 'clipboard.bin').read_bytes().decode('utf-8' if mac else 'utf-16le')
        assert '中文🙂' in copied and len(copied.splitlines()) == 2 and copied.endswith('\n'), repr(copied)
        assert 'seq=' not in copied and not copied.startswith('\ufeff')
        assert ui.screen.clipboard_requests == 0, 'Native copy also emitted OSC 52'

        mode.write_text('fail')
        ui.send(b'vy')
        ui.wait('Clipboard copy failed:')
        ui.wait('V-LINE')
        assert ui.screen.clipboard_requests == 0, 'Native failure silently fell back to OSC 52'

        # Changing the range during an in-flight copy must preserve the new range.
        mode.write_text('slow')
        ui.send(b'y')
        ui.wait('Copying 1 log lines')
        ui.send(b'j?')
        ui.wait(' Help ')
        ui.pump(1)
        ui.send(b'\x1b')
        ui.wait('V-LINE 2 selected')
        ui.wait(f'Copied 1 lines via {program}')

        # A blocked native process must not prevent leaving or stop managed services.
        mode.write_text('hang')
        (w.root / 'clip.pid').unlink()
        ui.send(b'y')
        ui.wait('Copying 2 log lines')
        deadline = time.monotonic() + 3
        while not (w.root / 'clip.pid').exists():
            assert time.monotonic() < deadline
            ui.pump(.02)
        pid = int((w.root / 'clip.pid').read_text())
        ui.exit()
        w.states(web='Running', weapp='Stopped', middle='Stopped')
        deadline = time.monotonic() + 3
        while subprocess.run(['ps', '-p', str(pid), '-o', 'stat='], capture_output=True).stdout.strip()[:1] not in (b'', b'Z', b'X'):
            assert time.monotonic() < deadline, 'Native clipboard process survived client exit'
            time.sleep(.02)
    finally: w.close()


def history():
    w = Workspace(mode='history')
    try:
        w.cli('start', 'web')
        deadline = time.monotonic() + 8
        while 'web-history-ready' not in w.cli('logs', 'web', '--limit', '1').stdout.decode():
            assert time.monotonic() < deadline
            time.sleep(.03)
        ui = Tui(w)
        ui.choose('web')
        ui.wait('web-history-ready')
        def first_sequence(text):
            match = re.search(r'seq=(\d+)', text)
            return int(match.group(1)) if match else 0
        ui.send(b'0')
        for _ in range(6):
            ui.send(b'\x1b[5~')
            ui.pump(.12)
        # An older page preserves the viewport anchor. Wait for it to arrive
        # before moving into it; disk reads can outlast the initial key burst.
        ui.wait('/ 200 cached')
        ui.send(b'\x1b[5~')
        ui.wait(lambda text: 0 < first_sequence(text) < 252)
        # Home queries the backend's retained beginning, beyond either client page.
        ui.send(b'gg')
        ui.wait(lambda text: first_sequence(text) == 1)
        assert 'READ POSITION EVICTED' not in ui.screen.text(), 'Jumping to available history reported eviction'
        for _ in range(7):
            ui.send(b'\x1b[6~')
            ui.pump(.12)
        ui.wait(lambda text: first_sequence(text) > 100)
        ui.wait('READ')
        anchor = first_sequence(ui.screen.text())
        ui.choose('weapp')
        ui.wait('Logs · STOPPED: weapp')
        ui.choose('web')
        ui.wait(lambda text: 'READ' in text and first_sequence(text) == anchor)
        ui.send(b'r')
        ui.pump(.3)
        ui.wait(lambda text: 'Connected' in text and 'READ' in text and first_sequence(text) == anchor)

        # The backend evicts the old records, but a paused client snapshot stays put.
        (w.root / 'history-more').touch()
        # Shared CI runners can take over 8 seconds to produce all paced lines.
        # Keep the exact count/eviction assertions; allow the producer to finish.
        ui.wait('+4201 new', timeout=30)
        assert first_sequence(ui.screen.text()) == anchor, 'New output moved paused history'
        assert 'READ POSITION EVICTED' not in ui.screen.text(), 'Backend eviction invalidated a retained client snapshot'
        assert len(w.starts('web')) == 1
        ui.send(b'r')
        ui.wait('READ POSITION EVICTED')
        ui.wait(lambda text: first_sequence(text) > anchor)
        ui.send(b'0p')
        ui.wait('FOLLOW')
        ui.wait('web-history-more-done')
        ui.exit()
        w.states(web='Running')
        assert len(w.starts('web')) == 1
    finally: w.close()


def disk_history():
    w = Workspace(mode='disk-history')
    try:
        w.cli('start', 'web')
        deadline = time.monotonic() + 8
        while 'web-history-ready' not in w.cli('logs', 'web', '--limit', '1').stdout.decode():
            assert time.monotonic() < deadline
            time.sleep(.03)
        ui = Tui(w)
        ui.choose('web')
        ui.wait('web-history-ready')
        def first_sequence(text):
            match = re.search(r'seq=(\d+)', text)
            return int(match.group(1)) if match else 0
        ui.send(b'0')
        for _ in range(6):
            ui.send(b'\x1b[5~')
            ui.pump(.12)
        # An older page preserves the viewport anchor. Wait for it to arrive
        # before moving into it; disk reads can outlast the initial key burst.
        ui.wait('/ 200 cached')
        ui.send(b'\x1b[5~')
        ui.wait(lambda text: 0 < first_sequence(text) < 252)
        # Home queries the backend's retained beginning, beyond either client page.
        ui.send(b'gg')
        ui.wait(lambda text: first_sequence(text) == 1)
        assert 'READ POSITION EVICTED' not in ui.screen.text(), 'Jumping to available history reported eviction'
        for _ in range(7):
            ui.send(b'\x1b[6~')
            ui.pump(.12)
        ui.wait(lambda text: first_sequence(text) > 100)
        ui.wait('READ')
        anchor = first_sequence(ui.screen.text())
        ui.choose('weapp')
        ui.wait('Logs · STOPPED: weapp')
        ui.choose('web')
        ui.wait(lambda text: 'READ' in text and first_sequence(text) == anchor)
        ui.send(b'r')
        ui.pump(.3)
        ui.wait(lambda text: 'Connected' in text and 'READ' in text and first_sequence(text) == anchor)

        # The backend evicts the old records, but a paused client snapshot stays put.
        (w.root / 'history-more').touch()
        # Shared CI runners can take over 8 seconds to produce all paced lines.
        # Keep the exact count/eviction assertions; allow the producer to finish.
        ui.wait('+4201 new', timeout=30)
        assert first_sequence(ui.screen.text()) == anchor, 'New output moved paused history'
        assert 'READ POSITION EVICTED' not in ui.screen.text(), 'Backend eviction invalidated a retained client snapshot'
        assert len(w.starts('web')) == 1
        ui.send(b'r')
        ui.pump(.3)
        ui.wait(lambda text: 'Connected' in text and first_sequence(text) == anchor)
        assert 'READ POSITION EVICTED' not in ui.screen.text(), 'Disk history failed to restore retained output'
        ui.send(b'0p')
        ui.wait('FOLLOW')
        ui.wait('web-history-more-done')
        ui.send(b'/web-history-42\r')
        ui.wait('Match seq=43;')
        ui.wait(lambda text: 'web-history-41' in text and 'web-history-42' in text and 'web-history-43' in text and 'web-history-50' in text and 'READ' in text)
        ui.send(b'n')
        ui.wait('No further match')
        ui.send(b'/web-history-4\r')
        ui.wait('Match seq=44;')
        ui.wait(lambda text: re.search(r'> \[stdout.*seq=44\]', text) is not None)
        ui.send(b'N')
        ui.wait('Match seq=43;')
        ui.wait(lambda text: re.search(r'> \[stdout.*seq=43\]', text) is not None)
        ui.send(b'v')
        ui.wait('V-LINE')
        ui.send(b'y')
        ui.wait(lambda _: ui.screen.clipboard_requests == 1)
        assert ui.screen.clipboard == 'web-history-42\n', repr(ui.screen.clipboard)
        ui.send(b'\x1b')
        ui.wait('Search cleared; reading position kept.')
        assert 'Logs · ACTIVE' in ui.screen.text()
        assert 'web-history-41' in ui.screen.text() and 'web-history-43' in ui.screen.text()
        ui.send(b'G')
        ui.wait('FOLLOW')
        ui.exit()
        again = Tui(w)
        again.choose('web')
        again.wait('web-history-more-done')
        again.wait('FOLLOW')
        assert 'READ' not in again.screen.text()
        again.exit()
        w.states(web='Running')
        assert len(w.starts('web')) == 1
    finally: w.close()


def config_discovery():
    w = Workspace()
    try:
        ui = Tui(w, discover=True)  # ~/devctl.toml, from an unrelated cwd.
        w.states(web='Stopped', weapp='Stopped', middle='Stopped')
        assert not any(w.starts(service) for service in ['middle', 'weapp', 'web'])
        ui.choose('web')
        ui.send(b's')
        w.states(web='Running')
        ui.wait('web-line-')
        first = w.starts('web')
        ui.exit()
        explicit = Tui(w)
        w.states(web='Running')
        assert w.starts('web') == first, 'Default and explicit paths created different workspaces'
        explicit.send(b'Qy')
        explicit.exit(key=None)

        # Move the same configuration to the second default location.
        config = w.root / '.config/devctl/devctl.toml'
        config.parent.mkdir(parents=True)
        w.config.rename(config)
        w.config = config
        ui = Tui(w, discover=True)
        w.states(web='Stopped', weapp='Stopped', middle='Stopped')
        assert w.starts('web') == first, 'Entering TUI implicitly started an autostart service'
        ui.send(b'Qy')
        ui.exit(key=None)
    finally: w.close()


if __name__ == '__main__':
    {'appearance': appearance, 'lifecycle': lifecycle, 'reconnect': reconnect, 'slow': slow_stop_and_failure, 'high': high_output,
     'discovery': config_discovery, 'navigation': navigation, 'copying': copying,
     'windows-copying': windows_copying, 'mac-copying': mac_copying, 'history': history, 'disk-history': disk_history}[sys.argv[2]]()
    print('PASS:', sys.argv[2])
