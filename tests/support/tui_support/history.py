"""Shared retained-history navigation with distinct memory/disk expectations."""

import re
import time

from .driver import Tui
from .workspace import Workspace, log_bodies


def history(binary):
    check_history(binary, disk=False)


def disk_history(binary):
    check_history(binary, disk=True)


def check_history(binary, disk):
    w = Workspace(binary, mode="disk-history" if disk else "history")
    try:
        w.cli("start", "web")
        deadline = time.monotonic() + 8
        while (
            "web-history-ready"
            not in w.cli("logs", "web", "--limit", "1").stdout.decode()
        ):
            assert time.monotonic() < deadline
            time.sleep(0.03)
        ui = Tui(w)
        ui.choose("web")
        ui.wait("web-history-ready")

        def first_sequence(text):
            match = re.search(r"\[out #\d+\] web-(history|new)-(\d+)", text)
            return (
                int(match.group(2)) + (1 if match.group(1) == "history" else 352)
                if match
                else 0
            )

        ui.send(b"0")
        for _ in range(6):
            ui.send(b"\x1b[5~")
            ui.pump(0.12)
        # An older page preserves the viewport anchor. Wait for it to arrive
        # before moving into it; disk reads can outlast the initial key burst.
        ui.wait("/ 200 cached")
        ui.send(b"\x1b[5~")
        ui.wait(lambda text: 0 < first_sequence(text) < 252)
        # Home queries the backend's retained beginning, beyond either client page.
        ui.send(b"gg")
        ui.wait(lambda text: first_sequence(text) == 1)
        assert "READ POSITION EVICTED" not in ui.screen.text(), (
            "Jumping to available history reported eviction"
        )
        for _ in range(7):
            ui.send(b"\x1b[6~")
            ui.pump(0.12)
        ui.wait(lambda text: first_sequence(text) > 100)
        ui.wait("READ")
        anchor = first_sequence(ui.screen.text())
        ui.choose("weapp")
        ui.wait("Logs · STOPPED: weapp")
        ui.choose("web")
        ui.wait(lambda text: "READ" in text and first_sequence(text) == anchor)
        ui.send(b"c")
        ui.pump(0.3)
        ui.wait(
            lambda text: (
                "Connected" in text
                and "READ" in text
                and first_sequence(text) == anchor
            )
        )

        # The backend evicts the old records, but a paused client snapshot stays put.
        (w.root / "history-more").touch()
        # Shared CI runners can take over 8 seconds to produce all paced lines.
        # Keep the exact count/eviction assertions; allow the producer to finish.
        ui.wait("+4201 new", timeout=30)
        assert first_sequence(ui.screen.text()) == anchor, (
            "New output moved paused history"
        )
        assert "READ POSITION EVICTED" not in ui.screen.text(), (
            "Backend eviction invalidated a retained client snapshot"
        )
        assert len(w.starts("web")) == 1
        if disk:
            ui.send(b"c")
            ui.pump(0.3)
            ui.wait(lambda text: "Connected" in text and first_sequence(text) == anchor)
            assert "READ POSITION EVICTED" not in ui.screen.text(), (
                "Disk history failed to restore retained output"
            )
            ui.send(b"0p")
            ui.wait("FOLLOW")
            ui.wait("web-history-more-done")
            ui.send(b"/web-history-42\r")
            ui.wait("Match seq=43;")
            ui.wait(
                lambda text: (
                    "web-history-41" in text
                    and "web-history-42" in text
                    and "web-history-43" in text
                    and "web-history-50" in text
                    and "READ" in text
                )
            )
            ui.send(b"n")
            ui.wait("No further match")
            ui.send(b"/web-history-4\r")
            ui.wait("Match seq=44;")
            ui.wait(lambda text: log_bodies(text, cursor=True) == ["web-history-43"])
            ui.send(b"N")
            ui.wait("Match seq=43;")
            ui.wait(lambda text: log_bodies(text, cursor=True) == ["web-history-42"])
            ui.send(b"v")
            ui.wait("V-LINE")
            ui.send(b"y")
            ui.wait(lambda _: ui.screen.clipboard_requests == 1)
            assert ui.screen.clipboard == "web-history-42\n", repr(ui.screen.clipboard)
            ui.send(b"\x1b")
            ui.wait("Search cleared; reading position kept.")
            assert "Logs · ACTIVE" in ui.screen.text()
            assert (
                "web-history-41" in ui.screen.text()
                and "web-history-43" in ui.screen.text()
            )
            ui.send(b"G")
            ui.wait("FOLLOW")
            ui.exit()
            again = Tui(w)
            again.choose("web")
            again.wait("web-history-more-done")
            again.wait("FOLLOW")
            assert "READ" not in again.screen.text()
            again.exit()
            w.states(web="Running")
            assert len(w.starts("web")) == 1
        else:
            ui.send(b"c")
            ui.wait("READ POSITION EVICTED")
            ui.wait(lambda text: first_sequence(text) > anchor)
            ui.send(b"0p")
            ui.wait("FOLLOW")
            ui.wait("web-history-more-done")
            ui.exit()
            w.states(web="Running")
            assert len(w.starts("web")) == 1
    finally:
        w.close()
