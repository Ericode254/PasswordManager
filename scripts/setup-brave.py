#!/usr/bin/env python3
"""Backward-compatible Brave entry point; new users can use setup-browser.py."""
import sys
from browser_setup import APP_ID, EXTENSION_ID, EXTENSION_URL, validate_manifest, probe_host
from browser_setup import main as browser_main, find_host_manifest, register_host as register_browser_host

HOST_MANIFEST = find_host_manifest("chromium")


def register_host(source, brave_dir):
    return register_browser_host(source, brave_dir / "NativeMessagingHosts", "chromium")


def main():
    # Preserve the previous --brave-dir option, including --brave-dir=PATH.
    arguments = []
    for argument in sys.argv[1:]:
        if argument == "--brave-dir" or argument.startswith("--brave-dir="):
            argument = argument.replace("--brave-dir", "--user-data-dir", 1)
        arguments.append(argument)
    return browser_main([*arguments, "--browser", "brave"])


if __name__ == "__main__":
    raise SystemExit(main())
