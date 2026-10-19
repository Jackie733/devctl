"""Service control, reconnection, responsiveness and configuration discovery."""

import os
import re
import signal
import time

from .driver import Tui
from .workspace import Workspace, log_bodies


def lifecycle(binary):
    w = Workspace(binary)
    try:
        ui = Tui(w)
        w.states(middle="Stopped", weapp="Stopped", web="Stopped")
        assert not any(w.starts(service) for service in ["middle", "weapp", "web"])
        ui.choose("web")
        ui.send(b"s")
        w.states(web="Running", weapp="Stopped", middle="Stopped")
        ui.wait("web-line-")
        ui.wait("web-stderr-")
        first = w.starts("web")
        ui.send(b"s")
        ui.pump(0.25)
        assert w.starts("web") == first and len(first) == 1
        ui.send(b"A")
        w.states(web="Running", weapp="Running", middle="Running")
        w.wait_starts("weapp", 1)
        assert w.starts("web") == first
        ui.choose("weapp")
        ui.wait("weapp-line-")
        ui.send(b"x")
        w.states(weapp="Stopped", web="Running", middle="Running")
        ui.wait("weapp: stopped; cleanup confirmed.")
        ui.wait("Logs · STOPPED: weapp")
        ui.wait("STOPPED | Retained logs")
        assert "weapp-line-" in ui.screen.text(), "Stopping cleared retained logs"
        ui.send(b"s")
        w.states(weapp="Running")
        w.wait_starts("weapp", 2)
        ui.wait("Logs · RUNNING: weapp")
        ui.wait(lambda text: len(set(re.findall(r"\[(?:out|err) #(\d+)\]", text))) >= 2)
        assert " old " not in ui.screen.text() and "seq=" not in ui.screen.text()
        assert len(w.starts("weapp")) == 2
        ui.choose("web")
        ui.pump(0.5)
        ui.send(b"\x1b[5~")
        ui.wait("READ")
        anchor = log_bodies(ui.screen.text())[0]
        ui.pump(0.4)
        assert log_bodies(ui.screen.text())[0] == anchor
        ui.send(b"\x1b[F")
        ui.wait("FOLLOW")
        ui.send(b"Q")
        ui.wait("y: confirm")
        ui.resize(36, 10)
        ui.wait("Enlarge window to confirm.")
        ui.send(b"y")
        ui.pump(0.15)
        w.states(web="Running", weapp="Running", middle="Running")
        ui.resize(120, 30)
        ui.wait("y: confirm")
        ui.send(b"n")
        ui.send(b"?")
        ui.wait("business readiness")
        ui.wait("?/Esc close | q leave")
        ui.send(b"\x1b")
        ui.wait(lambda text: " Help " not in text)
        ui.resize(50, 14)
        ui.wait("Selected: web | Running")
        ui.wait("q leave")
        ui.resize(120, 30)
        ui.wait("Projects")
        ui.exit()
        w.states(web="Running", weapp="Running", middle="Running")
        again = Tui(w, explicit_tui=True)
        again.choose("web")
        again.wait("web-line-")
        assert w.starts("web") == first
        again.send(b"X")
        w.states(web="Stopped", weapp="Stopped", middle="Stopped")
        again.wait("All services stopped; background workspace retained.")
        again.send(b"A")
        w.states(web="Running", weapp="Running", middle="Running")
        again.wait("Start request accepted")
        again.send(b"Qy")
        again.exit(key=None)
        assert w.cli("status", success=False).returncode != 0
    finally:
        w.close()


def reconnect(binary):
    w = Workspace(binary)
    try:
        ui = Tui(w)
        ui.choose("web")
        ui.send(b"s")
        w.states(web="Running")
        ui.wait("web-line-")
        w.cli("shutdown")
        ui.wait("state unknown")
        ui.wait("Logs · UNKNOWN: web")
        ui.pump(0.7)
        assert w.cli("status", success=False).returncode != 0, (
            "Refresh recreated a workspace"
        )
        ui.send(b"s")
        ui.wait("Reconnect before controlling")
        w.cli("start", "weapp")
        ui.wait("Connected")
        w.states(web="Stopped", weapp="Running", middle="Stopped")
        ui.choose("weapp")
        ui.wait("weapp-line-")
        ui.send(b"c")
        ui.wait("Connected")
        ui.exit(b"\x03")
        w.states(weapp="Running")
        for sig in [signal.SIGINT, signal.SIGTERM, signal.SIGHUP]:
            again = Tui(w)
            again.process.send_signal(sig)
            again.exit(key=None)
            w.states(weapp="Running")
        closed = Tui(w)
        os.close(closed.master)
        closed.master = None
        closed.process.wait(timeout=5)
        w.states(weapp="Running")
        attached = Tui(w)
        attached.choose("weapp")
        attached.wait("weapp-line-")
        attached.exit()
    finally:
        w.close()


def slow_stop_and_failure(binary):
    w = Workspace(binary, "slow")
    try:
        ui = Tui(w)
        ui.send(b"s")  # Sorted first project: middle.
        w.states(middle="Running")
        ui.wait("middle-line-")
        ui.send(b"x")
        w.states(middle="Stopping")
        ui.send(b"\x1b[B")
        ui.wait("Selected: weapp |", timeout=1)
        start = time.monotonic()
        ui.exit()
        assert time.monotonic() - start < 1, "Leaving waited for service cleanup"
        w.states(middle="Stopped")
    finally:
        w.close()
    w = Workspace(binary, "failure")
    try:
        ui = Tui(w)
        ui.send(b"A")
        w.states(weapp="Failed", web="Running", middle="Running")
        ui.choose("weapp")
        ui.wait("Selected: weapp | Failed")
        ui.wait(lambda text: "unexpectedly" in text and "7" in text)
        ui.exit()
        w.states(web="Running", middle="Running")
    finally:
        w.close()


def high_output(binary):
    w = Workspace(binary, "high")
    try:
        ui = Tui(w)
        ui.choose("web")
        ui.send(b"s")
        w.states(web="Running")
        ui.wait("web-line-")
        ui.send(b"\x1b[A")
        ui.wait("Selected: weapp |", timeout=2)
        ui.send(b"\x1b[B")
        ui.wait("web-line-")
        start = time.monotonic()
        ui.exit()
        assert time.monotonic() - start < 1.5, "High output blocked leaving"
        w.states(web="Running")
        # Exercise a redraw larger than the PTY queue. State polling must keep
        # draining that queue so the queued stop key can actually be consumed.
        again = Tui(w)
        again.choose("web")
        again.wait("web-line-")
        again.resize(120, 200)
        time.sleep(0.25)
        again.send(b"X")
        w.states(web="Stopped", middle="Stopped", weapp="Stopped")
    finally:
        w.close()


def config_discovery(binary):
    w = Workspace(binary)
    try:
        ui = Tui(w, discover=True)  # ~/devctl.toml, from an unrelated cwd.
        w.states(web="Stopped", weapp="Stopped", middle="Stopped")
        assert not any(w.starts(service) for service in ["middle", "weapp", "web"])
        ui.choose("web")
        ui.send(b"s")
        w.states(web="Running")
        ui.wait("web-line-")
        first = w.starts("web")
        ui.exit()
        explicit = Tui(w)
        w.states(web="Running")
        assert w.starts("web") == first, (
            "Default and explicit paths created different workspaces"
        )
        explicit.send(b"Qy")
        explicit.exit(key=None)

        # Move the same configuration to the second default location.
        config = w.root / ".config/devctl/devctl.toml"
        config.parent.mkdir(parents=True)
        w.config.rename(config)
        w.config = config
        ui = Tui(w, discover=True)
        w.states(web="Stopped", weapp="Stopped", middle="Stopped")
        assert w.starts("web") == first, (
            "Entering TUI implicitly started an autostart service"
        )
        ui.send(b"Qy")
        ui.exit(key=None)
    finally:
        w.close()
