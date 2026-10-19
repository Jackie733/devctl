"""Run black-box TUI checks with Python's standard library and a real PTY."""

import argparse

from tui_support import clipboard, history, lifecycle, navigation

SCENARIOS = {
    "appearance": navigation.appearance,
    "lifecycle": lifecycle.lifecycle,
    "reconnect": lifecycle.reconnect,
    "slow": lifecycle.slow_stop_and_failure,
    "high": lifecycle.high_output,
    "discovery": lifecycle.config_discovery,
    "navigation": navigation.navigation,
    "copying": clipboard.copying,
    "mouse": clipboard.mouse_logs,
    "windows-copying": clipboard.windows_copying,
    "mac-copying": clipboard.mac_copying,
    "history": history.history,
    "disk-history": history.disk_history,
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary")
    parser.add_argument("scenario", choices=SCENARIOS)
    args = parser.parse_args()
    SCENARIOS[args.scenario](args.binary)
    print("PASS:", args.scenario)


if __name__ == "__main__":
    main()
