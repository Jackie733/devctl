"""Appearance, help, focus and keyboard navigation scenarios."""

from .driver import Tui
from .screen import Screen
from .workspace import Workspace, log_bodies


def appearance(binary):
    # A wide glyph must not leave a hidden continuation when overwritten.
    screen = Screen(8, 2)
    screen.feed("中a".encode())
    screen.feed(b"\x1b[1;1Hx")
    assert screen.text().splitlines()[0].startswith("x a")
    screen.feed("\x1b[1;1H中".encode())
    screen.feed(b"\x1b[1;2Hy")
    assert screen.text().splitlines()[0].startswith(" ya")
    for theme in ["dark", "light", "terminal", "mono"]:
        w = Workspace(binary)
        try:
            options = {"DEVCTL_TUI_THEME": "terminal" if theme == "mono" else theme}
            if theme == "mono":
                options.update(
                    NO_COLOR="1", DEVCTL_TUI_ASCII="1", DEVCTL_TUI_REDUCED_MOTION="1"
                )
            ui = Tui(w, appearance=options)
            assert ui.screen.text().startswith("devctl "), ui.screen.text()
            ui.choose("web")
            ui.wait("- Stopped" if theme == "mono" else "○ Stopped")
            ui.send(b"s")
            ui.wait("* Running" if theme == "mono" else "● Running")
            ui.wait("web-line-")
            ui.send(b"0/web-line-\r")
            ui.wait("Match seq=")
            ui.send(b"v")
            ui.wait("V-LINE")
            ui.send(b"\x1b")
            ui.wait(lambda text: "V-LINE" not in text)
            ui.send(b"?")
            ui.wait("CURRENT PANEL")
            assert (
                "━━━━━" not in ui.screen.text() and "----- |" not in ui.screen.text()
            ), ui.screen.text()
            ui.send(b"\x1b")
            ui.wait(lambda text: " Help " not in text)
            ui.resize(60, 18)
            ui.wait("q leave")
            assert ui.screen.text().startswith("devctl "), ui.screen.text()
            ui.send(b"1")
            ui.wait("s/x start/stop | Enter logs | a add")
            ui.send(b"x")
            ui.wait("- Stopped" if theme == "mono" else "○ Stopped")
            ui.resize(36, 10)
            ui.wait("q leave")
            assert ui.screen.text().startswith("devctl "), ui.screen.text()
            assert "Connected" in ui.screen.text()
            assert b"\x1b[7m" not in ui.raw, "State changes must not invert colors"
            if theme == "mono":
                assert b"38;2;" not in ui.raw and b"48;2;" not in ui.raw
            elif theme in ["dark", "light"]:
                assert b"38;2;" in ui.raw and b"48;2;" in ui.raw
            ui.exit()
        finally:
            w.close()


def navigation(binary):
    w = Workspace(binary)
    try:
        ui = Tui(w)
        ui.choose("web")
        ui.send(b"s")
        w.states(web="Running", weapp="Stopped", middle="Stopped")
        ui.wait("web-line-")
        assert "Cleanup UNCONFIRMED" not in ui.screen.text(), (
            "Live service mislabeled as cleanup failure"
        )
        ui.pump(0.7)
        ui.send(b"0gg")
        ui.wait("Logs · ACTIVE")
        ui.wait("READ")
        selected = "Selected: web | Running"
        assert selected in ui.screen.text()
        anchor = log_bodies(ui.screen.text())[0]
        ui.send(b"j")
        ui.wait(lambda text: log_bodies(text, cursor=True))
        cursor = log_bodies(ui.screen.text(), cursor=True)[0]
        assert cursor != anchor and log_bodies(ui.screen.text())[0] == anchor
        assert selected in ui.screen.text(), "Log j changed the selected project"
        ui.send(b"?sxaXQ")
        ui.wait("CURRENT PANEL")
        w.states(web="Running", weapp="Stopped", middle="Stopped")
        assert not w.starts("weapp") and not w.starts("middle"), (
            "Help leaked a control key"
        )
        ui.send(b"G")
        ui.wait("CURRENT DETAILS")
        ui.resize(36, 10)
        ui.wait("?/Esc close | q leave")
        ui.send(b"gg")
        ui.wait("CURRENT PANEL")
        ui.resize(120, 30)
        ui.wait("CURRENT PANEL")
        ui.send(b"G")
        ui.wait("CURRENT DETAILS")
        ui.send(b"\x1b")
        ui.wait("Logs · ACTIVE")
        ui.resize(36, 10)
        ui.wait("Logs · ACTIVE")
        ui.wait("q leave")
        ui.send(b"1")
        ui.wait("Projects · ACTIVE")
        ui.send(b"0")
        ui.wait("Logs · ACTIVE")
        ui.resize(20, 6)
        ui.wait("Window too small")
        ui.send(b"QyX")
        ui.pump(0.15)
        w.states(web="Running")
        ui.resize(120, 30)
        ui.wait("Logs · ACTIVE")
        ui.send(b"G")
        ui.wait("FOLLOW")
        ui.send(b"?")
        ui.wait("CURRENT PANEL")
        ui.exit()
        w.states(web="Running")
    finally:
        w.close()
