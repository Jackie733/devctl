"""Development preview: play lossless Canvas PNG frames in Kitty, outside tmux."""

import argparse
import base64
import json
import os
from pathlib import Path
import re
import secrets
import select
import signal
import struct
import sys
import termios
import time
import tty


class SkipPreview(Exception):
    pass


def graphics(keys, payload=b""):
    encoded = base64.b64encode(payload)
    for offset in range(0, max(1, len(encoded)), 4096):
        chunk = encoded[offset:offset + 4096]
        more = int(offset + 4096 < len(encoded))
        header = (keys + "," if offset == 0 else "") + f"m={more}"
        sys.stdout.buffer.write(b"\x1b_G" + header.encode("ascii") + b";" + chunk + b"\x1b\\")
    sys.stdout.buffer.flush()


def reply(image_id):
    received = b""
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline:
        if not select.select([sys.stdin], [], [], max(0, deadline - time.monotonic()))[0]:
            break
        data = os.read(sys.stdin.fileno(), 4096)
        if not data:
            raise RuntimeError("Terminal input closed.")
        if b"\x03" in data:
            raise KeyboardInterrupt
        received += data
        match = re.search(rb"\x1b_Gi=" + str(image_id).encode() + rb"(?:,[^;\x1b]*)?;([^\x1b]*)\x1b\\", received)
        if match:
            if match[1] != b"OK":
                raise RuntimeError("Graphics operation rejected: " + match[1].decode("ascii", "replace"))
            return
        if b" " in data or b"\r" in data or b"\n" in data:
            raise SkipPreview
        if len(received) > 8192:
            break
    raise RuntimeError("No graphics acknowledgement; use Kitty directly, outside tmux.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    if not sys.stdin.isatty() or not sys.stdout.isatty():
        print("Pixel preview skipped: input and output must both be terminals.")
        return
    if os.environ.get("TMUX"):
        raise RuntimeError("For this preview, open a Kitty shell outside tmux.")
    if os.environ.get("TERM") != "xterm-kitty":
        raise RuntimeError("This animation preview is validated for Kitty's protocol; open Kitty first.")

    manifest = json.loads((args.directory / "manifest.json").read_text())
    fps = manifest["fps"]
    names = manifest["frames"]
    if fps != 15 or len(names) != 30:
        raise RuntimeError("Expected the prepared 30-frame, 15 FPS preview.")
    frames = []
    for name in names:
        path = (args.directory / name).resolve()
        if not path.is_relative_to(args.directory.resolve()) or path.stat().st_size > 2 * 1024 * 1024:
            raise RuntimeError("Invalid preview frame path or size.")
        png = path.read_bytes()
        if len(png) < 24 or png[:8] != b"\x89PNG\r\n\x1a\n" or struct.unpack(">II", png[16:24]) != (800, 400):
            raise RuntimeError("Expected lossless 800 x 400 PNG frames.")
        frames.append(png)

    image_id = secrets.randbelow(2**31 - 1) + 1
    original = termios.tcgetattr(sys.stdin)
    entered = False

    def terminate(signum, _frame):
        raise SystemExit(128 + signum)

    old_signals = {sig: signal.signal(sig, terminate) for sig in (signal.SIGTERM, signal.SIGHUP)}
    try:
        tty.setraw(sys.stdin.fileno())
        graphics(f"a=q,i={image_id},s=1,v=1,f=24", b"\x00\x00\x00")
        reply(image_id)
        entered = True
        sys.stdout.write("\x1b[?1049h\x1b[?25l\x1b[2J")
        sys.stdout.flush()
        graphics(f"a=t,f=100,i={image_id}", frames[0])
        reply(image_id)
        # Root + 29 full PNG frames. The terminal drives animation after upload.
        for png in frames[1:]:
            graphics(f"a=f,f=100,i={image_id},z=67", png)
            reply(image_id)

        def place():
            cols, rows = os.get_terminal_size(sys.stdout.fileno())
            graphics(f"a=d,d=i,i={image_id},p=1,q=2")
            sys.stdout.write("\x1b[2J\x1b[2;2H")
            sys.stdout.flush()
            graphics(f"a=p,i={image_id},p=1,c={max(1, cols - 2)},r={max(1, rows - 2)},C=1")
            reply(image_id)
            return cols, rows

        size = place()
        graphics(f"a=a,i={image_id},r=1,z=67,c=1,s=3,v=2,q=2")
        deadline = time.monotonic() + len(frames) / fps
        while time.monotonic() < deadline:
            if select.select([sys.stdin], [], [], min(0.05, max(0, deadline - time.monotonic())))[0]:
                data = os.read(sys.stdin.fileno(), 4096)
                if b"\x03" in data:
                    raise KeyboardInterrupt
                if not data or b" " in data or b"\r" in data or b"\n" in data:
                    break
            if os.get_terminal_size(sys.stdout.fileno()) != size:
                size = place()
    finally:
        try:
            if entered:
                graphics(f"a=d,d=I,i={image_id},q=2")
                sys.stdout.write("\x1b[0m\x1b[?25h\x1b[?1049l")
                sys.stdout.flush()
        finally:
            termios.tcsetattr(sys.stdin, termios.TCSANOW, original)
            for sig, handler in old_signals.items():
                signal.signal(sig, handler)
    print("Pixel preview finished. devctl TUI has not been entered.")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        sys.exit(130)
    except SkipPreview:
        print("Pixel preview skipped.")
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        print(f"Pixel preview failed: {error}", file=sys.stderr)
        sys.exit(1)
