"""Controlled service copied into each temporary terminal-test workspace."""

import json
import os
import re
import signal
import socket
import struct
import sys
import time
from pathlib import Path


def main():
    root, service, mode = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
    suffix = sys.argv[4]
    with (root / (service + ".starts")).open("a") as stream:
        stream.write(str(os.getpid()) + "\n")
    if mode == "failure":
        sys.exit(7)
    if mode == "slow":

        def stop(*_):
            time.sleep(1.5)
            sys.exit(0)

        signal.signal(signal.SIGTERM, stop)
    if mode == "history":

        def read_exact(stream, size):
            data = b""
            while len(data) < size:
                block = stream.recv(size - len(data))
                if not block:
                    raise RuntimeError("Log fixture control connection closed")
                data += block
            return data

        def acknowledged(expected):
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                with socket.socket(socket.AF_UNIX) as stream:
                    stream.settimeout(10)
                    stream.connect(str(root / "devctl.toml.devctl/control.sock"))
                    stream.sendall(b"CONTROL1\n")
                    assert read_exact(stream, 9) == b"DEVCTL 1\n"
                    read_exact(stream, struct.unpack("!I", read_exact(stream, 4))[0])
                    payload = (
                        "action='logs'\nservice=" + json.dumps(service) + "\nlimit=1\n"
                    ).encode()
                    stream.sendall(struct.pack("!I", len(payload)) + payload)
                    response = read_exact(
                        stream, struct.unpack("!I", read_exact(stream, 4))[0]
                    ).decode()
                latest = re.search(r"^latest = (\d+)$", response, re.M)
                if latest and int(latest.group(1)) >= expected:
                    return
                time.sleep(0.02)
            raise RuntimeError("Log fixture delivery was not acknowledged")

        emitted = 0

        def emit(text):
            nonlocal emitted
            print(text)
            emitted += 1
            # Retention tests need exact input, not producer-overload loss. Query replies
            # also wait for prior archive writes, keeping both bounded queues drained.
            if emitted % 32 == 0:
                acknowledged(emitted)

        for number in range(350):
            emit(service + "-history-" + str(number) + suffix)
        emit(service + "-history-ready")
        while not (root / "history-more").exists():
            time.sleep(0.02)
        for number in range(4200):
            emit(service + "-new-" + str(number))
        emit(service + "-history-more-done")
        acknowledged(emitted)
        while True:
            time.sleep(1)

    number = 0
    while True:
        if mode == "high":
            for _ in range(300):
                print(service + "-line-" + str(number) + "-" + "x" * 400)
                number += 1
        else:
            print(service + "-line-" + str(number) + suffix)
            if number % 3 == 0:
                print(service + "-stderr-" + str(number), file=sys.stderr)
            number += 1
        time.sleep(0.02)


if __name__ == "__main__":
    main()
