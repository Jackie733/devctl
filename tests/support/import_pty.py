"""Exercise import selection, review and cancellation through an actual PTY."""
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
import termios
import time

from tui_pty import Screen

binary, config, repository, mode = sys.argv[1:]
config, repository = Path(config), Path(repository)
master, slave = pty.openpty()
original = termios.tcgetattr(slave)
screen = Screen()


def size(width, height):
    screen.resize(width, height)
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', height, width, 0, 0))


size(30, 6) if mode == 'small' else size(120, 30)
args = [binary, '--config', str(config), 'import', str(repository), '--apply']
if mode in ['add_apply', 'add_cancel']:
    args = [binary, '--config', str(config), 'add', str(repository), '--name', 'custom', '--apply', '--', 'uninstalled-runtime', 'literal argument']
if mode == 'member_changed':
    args += ['--project', 'apps/api', '--script', 'apps/api=dev']
child = subprocess.Popen(
    args, stdin=slave, stdout=slave, stderr=slave, start_new_session=True,
    preexec_fn=lambda: fcntl.ioctl(slave, termios.TIOCSCTTY, 0),
)
output = bytearray()


def pump():
    if select.select([master], [], [], .05)[0]:
        data = os.read(master, 65536)
        output.extend(data)
        screen.feed(data)
        return bool(data)
    return False


def text():
    return re.sub(r'\x1b\[[0-9;?]*[ -/]*[@-~]', '', output.decode('utf8', errors='replace'))


def wait(token):
    deadline = time.monotonic() + 15
    while token not in text() and token not in screen.text():
        pump()
        assert child.poll() is None, text()
        assert time.monotonic() < deadline, ('Waiting for', token, screen.text(), text()[-2000:])


try:
    if mode in ['choose', 'choose_cancel', 'small']:
        if mode == 'small':
            wait('Resize to at least')
            os.write(master, b'j \r')
            time.sleep(.1)
            assert not config.exists()
            size(120, 30)
            child.send_signal(signal.SIGWINCH)
        wait('Select projects to import')
        if mode == 'choose_cancel':
            os.write(master, b'\x1b')
        else:
            # Root is a disabled library; apps/api is the next candidate.
            os.write(master, b'j \r')
            wait('Choose one script for apps/api')
            os.write(master, b'j\r')
    if mode == 'manager':
        wait('Choose package manager command')
        os.write(master, b'\r')
    if mode != 'choose_cancel':
        wait('Save this configuration diff? [y/N]')
        if mode in ['source_changed', 'member_changed']:
            path = repository / ('packages/library/package.json' if mode == 'member_changed' else 'package.json')
            manifest = json.loads(path.read_text())
            manifest['description'] = 'changed while reviewing'
            path.write_text(json.dumps(manifest))
        if mode == 'target_changed':
            config.write_text("version=1\n[workspace]\nname='external editor'\n")
        if mode == 'signal':
            child.send_signal(signal.SIGTERM)
        else:
            os.write(master, b'n' if mode in ['cancel', 'add_cancel'] else b'y')
    deadline = time.monotonic() + 15
    while child.poll() is None:
        pump()
        assert time.monotonic() < deadline, text()
    while pump():
        pass
    successful = mode in ['apply', 'choose', 'small', 'manager', 'add_apply']
    assert child.returncode == (0 if successful else 1), text()
    if successful:
        assert config.exists(), text()
        assert 'Saved configuration:' in text(), text()
    elif mode == 'target_changed':
        assert 'external editor' in config.read_text()
        assert 'changed after review' in text(), text()
    else:
        assert not config.exists(), text()
    if mode in ['source_changed', 'member_changed']:
        assert 'evidence changed' in text(), text()
    assert termios.tcgetattr(master) == original, 'Terminal mode was not restored'
    if mode in ['choose', 'choose_cancel', 'small']:
        assert b'\x1b[?1049l' in output and b'\x1b[?25h' in output
    print('PASS:', mode)
finally:
    if child.poll() is None:
        child.kill()
        child.wait(timeout=3)
    os.close(master)
    os.close(slave)
