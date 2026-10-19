"""Keyboard/mouse selection and isolated OSC 52/native clipboard scenarios."""

import re
import subprocess
import sys
import time
import unicodedata

from .driver import Tui
from .screen import Screen
from .workspace import Workspace, log_bodies


def copying(binary):
    probe = Screen()
    # PTY reads may split a clipboard command at any byte, including its prefix.
    for byte in b"\x1b]52;c;aGVsbG8K\x1b\\":
        probe.feed(bytes([byte]))
    assert probe.clipboard == "hello\n" and probe.clipboard_requests == 1
    w = Workspace(binary)
    try:
        ui = Tui(w)
        ui.choose("web")
        ui.send(b"s")
        w.states(web="Running", weapp="Stopped", middle="Stopped")
        ui.wait("web-line-")
        ui.pump(0.6)
        ui.send(b"0ggjj")
        ui.wait(lambda text: log_bodies(text, cursor=True))
        ui.send(b"v")
        ui.wait("V-LINE")
        bodies = log_bodies(ui.screen.text())
        assert len(bodies) >= 4
        expected = "\n".join(body.rstrip() for body in bodies[2:4]) + "\n"
        ui.send(b"jsxaXQy")
        ui.wait(lambda _: ui.screen.clipboard_requests == 1)
        assert ui.screen.clipboard == expected, (
            repr(ui.screen.clipboard),
            repr(expected),
        )
        assert "seq=" not in ui.screen.clipboard
        ui.wait("Sent 2 lines via OSC 52")
        w.states(web="Running", weapp="Stopped", middle="Stopped")
        ui.send(b"y")
        ui.pump(0.1)
        assert ui.screen.clipboard_requests == 1, (
            "Normal mode y copied without a selection"
        )
        ui.send(b"V?jy")
        ui.wait(" Help ")
        ui.pump(0.1)
        assert ui.screen.clipboard_requests == 1, "Help leaked copy key"
        ui.send(b"\x1b")
        # V-LINE also exists behind help. Wait for Escape to close the modal
        # before changing PTY dimensions and resetting the screen interpreter.
        ui.wait(lambda text: " Help " not in text and "V-LINE" in text)
        ui.resize(36, 10)
        ui.wait("y copy")
        ui.send(b"\x1b")
        ui.wait(lambda text: "V-LINE" not in text and "Logs · ACTIVE" in text)
        ui.send(b"v")
        ui.wait("V-LINE")
        ui.exit()
        w.states(web="Running")
    finally:
        w.close()


def mouse_logs(binary):
    w = Workspace(binary, mode="history", suffix="-中文🙂\tend")
    try:
        w.cli("start", "web")
        ui = Tui(w)
        ui.choose("web")
        ui.send(b"0")
        ui.wait("web-history-ready")

        def first_number(text):
            match = re.search(r"\[out #\d+\] web-history-(\d+)", text)
            return int(match[1]) if match else 999

        # One client page holds only 100 records. Wheel across its boundary and
        # wait for the earlier page without sending Home/PageUp or changing focus.
        deadline = time.monotonic() + 8
        while first_number(ui.screen.text()) >= 250:
            assert time.monotonic() < deadline, ui.screen.text()
            ui.mouse(64, 50, 10)
            ui.pump(0.03)
        ui.wait(lambda text: first_number(text) < 250)
        ui.wait("READ")
        ui.pump(0.2)  # Drain the final queued wheel events and their redraw.
        anchor = first_number(ui.screen.text())
        ui.pump(0.3)
        assert first_number(ui.screen.text()) == anchor, (
            anchor,
            first_number(ui.screen.text()),
            ui.screen.text(),
        )
        for _ in range(100):
            ui.mouse(65, 50, 10)
            ui.pump(0.01)
        ui.wait("FOLLOW")
        ui.wait("web-history-ready")

        def position(needle):
            for y, row in enumerate(ui.screen.rows):
                text = "".join(row)
                index = text.find(needle)
                if index >= 0:
                    x = sum(
                        2 if unicodedata.east_asian_width(c) in ("W", "F") else 1
                        for c in text[:index]
                    )
                    return x, y
            raise AssertionError((needle, ui.screen.text()))

        def select(reverse=False):
            start = position("中文🙂")
            end = position("end")
            end = (end[0] + 2, end[1])
            if reverse:
                start, end = end, start
            ui.mouse(0, *start)
            ui.mouse(32, *end)
            ui.mouse(0, *end, release=True)
            ui.wait("V-TEXT 1 selected")
            return start

        select()
        ui.send(b"sxAXQy")
        ui.wait(lambda _: ui.screen.clipboard_requests == 1)
        assert ui.screen.clipboard == "中文🙂\tend", repr(ui.screen.clipboard)
        ui.wait(lambda text: "V-TEXT" not in text and "Sent 1 lines via OSC 52" in text)
        w.states(web="Running", middle="Stopped", weapp="Stopped")
        point = select(reverse=True)
        ui.mouse(2, *point)
        ui.wait(lambda _: ui.screen.clipboard_requests == 2)
        assert ui.screen.clipboard == "中文🙂\tend"

        ui.send(b"?")
        ui.wait(" Help ")
        ui.mouse(64, 50, 10)
        ui.mouse(0, 60, 10)
        ui.mouse(32, 70, 10)
        ui.mouse(2, 70, 10)
        ui.pump(0.1)
        assert ui.screen.clipboard_requests == 2, "Help leaked mouse input to the logs"
        ui.send(b"\x1b")
        ui.wait(lambda text: " Help " not in text)
        assert "V-TEXT" not in ui.screen.text()

        ui.resize(60, 16)
        ui.wait("Logs · ACTIVE")
        select()
        ui.send(b"y")
        ui.wait(lambda _: ui.screen.clipboard_requests == 3)
        assert ui.screen.clipboard == "中文🙂\tend"
        ui.exit()
        w.states(web="Running", middle="Stopped", weapp="Stopped")
    finally:
        w.close()


def windows_copying(binary):
    native_copying(binary, False)


def mac_copying(binary):
    if sys.platform == "darwin":
        native_copying(binary, True)


def native_copying(binary, mac):
    program = "pbcopy" if mac else "clip.exe"
    w = Workspace(binary, suffix="-中文🙂")
    try:
        clip = w.root / program
        clip.write_text("""#!/usr/bin/env python3
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
""")
        clip.chmod(0o700)
        mode = w.root / "clip.mode"
        mode.write_text("ok")
        ui = Tui(w, windows_clipboard=not mac, mac_clipboard=mac)
        ui.choose("web")
        ui.send(b"s")
        w.states(web="Running", weapp="Stopped", middle="Stopped")
        ui.wait("web-line-")
        ui.pump(0.6)
        ui.send(b"0ggjjvjy")
        ui.wait(f"Copied 2 lines via {program}")
        copied = (
            (w.root / "clipboard.bin")
            .read_bytes()
            .decode("utf-8" if mac else "utf-16le")
        )
        assert (
            "中文🙂" in copied
            and len(copied.splitlines()) == 2
            and copied.endswith("\n")
        ), repr(copied)
        assert "seq=" not in copied and not copied.startswith("\ufeff")
        assert ui.screen.clipboard_requests == 0, "Native copy also emitted OSC 52"

        mode.write_text("fail")
        ui.send(b"vy")
        ui.wait("Clipboard copy failed:")
        ui.wait("V-LINE")
        assert ui.screen.clipboard_requests == 0, (
            "Native failure silently fell back to OSC 52"
        )

        # Changing the range during an in-flight copy must preserve the new range.
        mode.write_text("slow")
        ui.send(b"y")
        ui.wait("Copying 1 log lines")
        ui.send(b"j?")
        ui.wait(" Help ")
        ui.pump(1)
        ui.send(b"\x1b")
        ui.wait("V-LINE 2 selected")
        ui.wait(f"Copied 1 lines via {program}")

        # A blocked native process must not prevent leaving or stop managed services.
        mode.write_text("hang")
        (w.root / "clip.pid").unlink()
        ui.send(b"y")
        ui.wait("Copying 2 log lines")
        deadline = time.monotonic() + 3
        while not (w.root / "clip.pid").exists():
            assert time.monotonic() < deadline
            ui.pump(0.02)
        pid = int((w.root / "clip.pid").read_text())
        ui.exit()
        w.states(web="Running", weapp="Stopped", middle="Stopped")
        deadline = time.monotonic() + 3
        while subprocess.run(
            ["ps", "-p", str(pid), "-o", "stat="], capture_output=True
        ).stdout.strip()[:1] not in (b"", b"Z", b"X"):
            assert time.monotonic() < deadline, (
                "Native clipboard process survived client exit"
            )
            time.sleep(0.02)
    finally:
        w.close()
