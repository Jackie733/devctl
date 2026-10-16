"""Exercise update presentation in a PTY, using only the caller's temporary fixture."""

import fcntl
import os
from pathlib import Path
import re
import select
import struct
import subprocess
import sys
import termios
import time

binary, fixture, mode = sys.argv[1:]
root = Path(fixture)
env = {**os.environ, 'HOME': str(root / 'home'), 'UPDATE_FIXTURE': str(root),
       'PATH': str(root / 'mock') + ':' + os.environ['PATH'], 'TERM': 'xterm-256color'}
for key in ('NO_COLOR', 'BASH_ENV', 'ENV', 'DEVCTL_UPDATE_EXPECTED_VERSION'):
    env.pop(key, None)
if mode == 'mono':
    env['NO_COLOR'] = '1'
if mode == 'dumb':
    env['TERM'] = 'dumb'

master, slave = os.openpty()
width = 24 if mode == 'narrow' else 120
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, width, 0, 0))
original = termios.tcgetattr(slave)
args = [binary, 'update']
if mode == 'check':
    args.append('--check')
if mode == 'rollback':
    args.append('--rollback')
process = subprocess.Popen(args, env=env, cwd=root, stdin=slave,
                           stdout=subprocess.PIPE if mode == 'redirect' else slave,
                           stderr=subprocess.PIPE if mode == 'stderr-redirect' else slave)
output = bytearray()
try:
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline:
        ready, _, _ = select.select([master], [], [], 0.05)
        if ready:
            output.extend(os.read(master, 65536))
        elif process.poll() is not None:
            break
    else:
        raise AssertionError('Update timed out in PTY')
    assert process.wait(timeout=1) == (1 if mode == 'failure' else 0), output.decode(errors='replace')
    assert termios.tcgetattr(slave) == original, 'Update changed terminal modes'
    stdout = process.stdout.read() if process.stdout else b''
    stderr = process.stderr.read() if process.stderr else b''
finally:
    if process.poll() is None:
        process.kill()
        process.wait()
    os.close(master)
    os.close(slave)

text = (output + stdout + stderr).decode('utf-8', errors='replace')
assert '\x1b[?1049' not in text and '\x1b[?25' not in text, text
if mode in ('dumb', 'redirect', 'stderr-redirect'):
    assert '\x1b' not in text and '\x1b' not in stdout.decode(errors='replace'), text
    assert 'Checking release...' in text and 'Update complete' in text, text
else:
    assert '\x1b[2K' in text, text
    if mode == 'mono':
        assert not re.search(r'\x1b\[[0-9;]*m', text), text
    else:
        assert re.search(r'\x1b\[[0-9;]*m', text), text

if mode in ('interactive', 'mono', 'narrow'):
    assert 'Downloaded' in text and 'Update complete' in text, text
    assert len({c for c in text if c in '⠋⠙⠹⠸⠼⠴⠦⠧'}) > 1, text
    frames = re.findall(r'\x1b\[2K(.*?)(?=\x1b\[1G|\r|\n|$)', text)
    frames = [re.sub(r'\x1b\[[0-9;]*m', '', frame) for frame in frames]
    downloads = [frame for frame in frames if 'Downloading' in frame]
    assert any(0 < int(match.group(1)) < 100
               for frame in downloads for match in re.finditer(r'(\d+)%', frame)), text
    assert all(len(frame) <= width - 1 for frame in frames), frames
    if mode == 'narrow':
        assert all('MiB/s' not in frame and 'ETA' not in frame for frame in downloads), downloads
    else:
        assert 'MiB/s' in text and 'ETA' in text, text
elif mode == 'failure':
    diagnostic = text[text.index('fixture diagnostic begins'):text.index(' and ends')]
    assert '\x1b' not in diagnostic, diagnostic
    assert ' and ends\r\n' in text, text
    assert 'Downloading failed' in text and 'Error:' in text, text
    assert 'Update complete' not in text and 'Verifying and installing' not in text, text
elif mode in ('check', 'current'):
    assert 'Downloading' not in text and 'Verifying and installing' not in text, text
    assert len((root / 'requests').read_text().splitlines()) == 1
elif mode == 'rollback':
    assert 'Rolling back' in text and 'Rollback complete' in text, text
    assert 'Downloading' not in text and 'Checking release' not in text, text
    assert not (root / 'requests').exists()

(root / 'terminal-output.txt').write_text(text)
print('PASS update PTY:', mode)
