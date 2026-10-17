"""Review/cancel and managed-start checks through a real terminal."""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import subprocess
import sys
import termios
import time

binary, config, node, mode = sys.argv[1:]
config = Path(config)
project = config.parent / 'project'


def setup():
    before = set(Path(os.environ['HOME']).rglob('*'))
    master, slave = pty.openpty()
    original = termios.tcgetattr(slave)
    child = subprocess.Popen(
        [binary, '--config', str(config), 'setup', '--node', node, '--apply'],
        stdin=slave, stdout=slave, stderr=slave, start_new_session=True,
        preexec_fn=lambda: fcntl.ioctl(slave, termios.TIOCSCTTY, 0),
    )
    output = bytearray()
    try:
        deadline = time.monotonic() + 10
        while b'Apply these bindings?' not in output:
            if select.select([master], [], [], .05)[0]:
                output.extend(os.read(master, 65536))
            assert child.poll() is None, output
            assert time.monotonic() < deadline, output
        assert set(Path(os.environ['HOME']).rglob('*')) == before, ('Preview wrote files', list(Path(os.environ['HOME']).rglob('*')))
        if mode == 'changed':
            manifest = project / 'package.json'
            value = json.loads(manifest.read_text())
            value['description'] = 'changed after review'
            manifest.write_text(json.dumps(value))
        os.write(master, b'n' if mode == 'cancel' else b'y')
        while child.poll() is None:
            if select.select([master], [], [], .05)[0]:
                output.extend(os.read(master, 65536))
            assert time.monotonic() < deadline, output
        while select.select([master], [], [], .05)[0]:
            data = os.read(master, 65536)
            if not data:
                break
            output.extend(data)
        assert child.returncode == (0 if mode == 'apply' else 1), output
        if mode == 'changed':
            assert b'declarations changed' in output, output
        if mode != 'apply':
            assert set(Path(os.environ['HOME']).rglob('*')) == before, output
        assert termios.tcgetattr(master) == original, 'Terminal mode not restored'
    finally:
        if child.poll() is None:
            child.kill()
            child.wait(timeout=3)
        os.close(master)
        os.close(slave)


def tui():
    from tui_pty import Tui

    class Workspace:
        root = config.parent
        clients = []
    workspace = Workspace()
    workspace.config = config
    observed = project / 'observed'
    try:
        ui = Tui(workspace)
        ui.send(b's')
        ui.wait('dependencies unknown')
        assert not observed.exists()
        ui.send(b'S')
        ui.wait('Confirm start')
        ui.wait('y: confirm')
        ui.resize(20, 6)
        ui.send(b'y')
        ui.pump(.2)
        assert not observed.exists(), 'Clipped confirmation started service'
        ui.resize(120, 30)
        ui.wait('y: confirm')
        ui.send(b'n')
        ui.pump(.1)
        assert not observed.exists()
        ui.send(b'Sy')
        ui.wait('fixture service started')
        assert observed.exists()
        ui.send(b'x')
        ui.wait('Selected: demo | Stopped')
        observed.unlink()
        ui.send(b's')
        ui.wait('dependencies unknown')
        assert not observed.exists(), 'Acknowledgement leaked into a later start'
        ui.send(b'Qy')
        ui.exit(key=None)
    finally:
        for client in workspace.clients:
            client.close()
        subprocess.run([binary, '--config', str(config), 'shutdown'], capture_output=True, timeout=10)


if mode == 'tui':
    tui()
else:
    setup()
print('PASS:', mode)
