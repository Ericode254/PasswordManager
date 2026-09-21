"""Browserpass native messaging setup for native Linux browsers."""
import argparse
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys

APP_ID = "com.github.browserpass.native"
EXTENSION_ID = "naepdomgkenhinolocfifgehidddafch"
FIREFOX_EXTENSION_ID = "browserpass@maximbaz.com"
EXTENSION_URL = f"https://chromewebstore.google.com/detail/browserpass/{EXTENSION_ID}"
FIREFOX_EXTENSION_URL = "https://addons.mozilla.org/firefox/addon/browserpass-ce/"
# Chromium paths are relative to XDG_CONFIG_HOME. Firefox-family paths are
# relative to HOME, matching Browserpass's upstream user-install targets.
BROWSERS = {
    "firefox": ("Firefox", "firefox", ".mozilla/native-messaging-hosts"),
    "librewolf": ("LibreWolf", "firefox", ".librewolf/native-messaging-hosts"),
    "chrome": ("Google Chrome", "chromium", "google-chrome/NativeMessagingHosts"),
    "chromium": ("Chromium", "chromium", "chromium/NativeMessagingHosts"),
    "brave": ("Brave", "chromium", "BraveSoftware/Brave-Browser/NativeMessagingHosts"),
    "edge": ("Microsoft Edge", "chromium", "microsoft-edge/NativeMessagingHosts"),
    "vivaldi": ("Vivaldi", "chromium", "vivaldi/NativeMessagingHosts"),
}


def default_host_directory(browser, home=None, config=None):
    home = Path.home() if home is None else Path(home)
    config = Path(os.environ.get("XDG_CONFIG_HOME") or home / ".config") if config is None else Path(config)
    _, family, relative = BROWSERS[browser]
    return (home if family == "firefox" else config) / relative


def find_host_manifest(family):
    candidates = [Path(prefix) / "browserpass/hosts" / family / f"{APP_ID}.json"
                  for prefix in ("/usr/lib", "/usr/lib64", "/usr/local/lib")]
    return next((path for path in candidates if path.is_file()), candidates[0])


def validate_manifest(source, family="chromium"):
    manifest = json.loads(source.read_text())
    if not isinstance(manifest, dict) or manifest.get("name") != APP_ID or manifest.get("type") != "stdio":
        raise ValueError("This is not a Browserpass native messaging manifest.")
    if family == "firefox":
        permission, expected = "allowed_extensions", FIREFOX_EXTENSION_ID
    elif family == "chromium":
        permission, expected = "allowed_origins", f"chrome-extension://{EXTENSION_ID}/"
    else:
        raise ValueError(f"Unknown browser family: {family}")
    allowed = manifest.get(permission)
    if not isinstance(allowed, list) or expected not in allowed:
        raise ValueError(f"The {family} manifest must allow the official Browserpass extension via {permission}.")
    location = manifest.get("path")
    if not isinstance(location, str):
        raise ValueError("The Browserpass executable path is missing.")
    binary = Path(location)
    if not binary.is_absolute() or not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError("The Browserpass executable is missing or is not executable.")
    return binary


def register_host(source, native_host_dir, family="chromium"):
    validate_manifest(source, family)
    destination = native_host_dir / f"{APP_ID}.json"
    if destination.exists() or destination.is_symlink():
        if destination.resolve() == source.resolve():
            return destination
        if destination.is_file() and destination.read_bytes() == source.read_bytes():
            return destination
        raise ValueError(f"Existing registration differs; left unchanged: {destination}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.symlink_to(source.resolve())
    return destination


def probe_host(binary):
    # Echo only: no store listing, decryption, or credential output.
    payload = json.dumps({"action": "echo", "echoResponse": {"passtui": "ok"}}).encode()
    result = subprocess.run([str(binary)], input=struct.pack("=I", len(payload)) + payload,
                            capture_output=True, timeout=5, check=True)
    if len(result.stdout) < 4:
        raise ValueError("Native helper returned no response.")
    size = struct.unpack("=I", result.stdout[:4])[0]
    if len(result.stdout[4:]) != size or json.loads(result.stdout[4:]) != {"passtui": "ok"}:
        raise ValueError("Native helper returned an unexpected response.")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_mutually_exclusive_group(required=True)
    actions.add_argument("--configure", action="store_true", help="Register the native host; safe to rerun")
    actions.add_argument("--check", action="store_true", help="Check native messaging without reading passwords")
    actions.add_argument("--list-browsers", action="store_true", help="List presets and default registration directories")
    parser.add_argument("--browser", choices=BROWSERS, help="Browser to configure or check")
    parser.add_argument("--host-manifest", type=Path, help="Installed Browserpass manifest for this browser family")
    directories = parser.add_mutually_exclusive_group()
    directories.add_argument("--native-host-dir", type=Path, help="Override the complete native-messaging directory (for forks/custom installations)")
    directories.add_argument("--user-data-dir", type=Path, help="Chromium-family user-data root, not an individual profile")
    args = parser.parse_args(argv)
    if not sys.platform.startswith("linux"):
        parser.error("This helper supports native Linux browsers. For other operating systems, use Browserpass's upstream installer.")
    if args.list_browsers:
        for browser, (label, family, _) in BROWSERS.items():
            print(f"{browser:10} {label:16} {family:8} {default_host_directory(browser)}")
        return 0
    if not args.browser:
        parser.error("--browser is required with --configure or --check")
    label, family, _ = BROWSERS[args.browser]
    if args.user_data_dir and family != "chromium":
        parser.error("--user-data-dir is for Chromium browsers. For Firefox forks, use --native-host-dir.")
    native_dir = args.native_host_dir or (
        args.user_data_dir / "NativeMessagingHosts" if args.user_data_dir else default_host_directory(args.browser))
    native_dir = native_dir.expanduser().absolute()
    registration = native_dir / f"{APP_ID}.json"
    try:
        if not shutil.which("gpg"):
            raise ValueError("GnuPG is missing. Install gnupg and a graphical pinentry.")
        if args.configure:
            source = (args.host_manifest or find_host_manifest(family)).expanduser().absolute()
            if not source.is_file():
                raise ValueError(f"Missing {family} host manifest: {source}. Install browserpass (Omarchy: omarchy pkg add browserpass; Arch: sudo pacman -S browserpass), or pass --host-manifest for your installation.")
            binary = validate_manifest(source, family)
            probe_host(binary)
            register_host(source, native_dir, family)
            print(f"Registered Browserpass for {label}: {registration}")
        else:
            if not registration.is_file():
                raise ValueError(f"{label} registration is missing. Run --browser {args.browser} --configure.")
            binary = validate_manifest(registration, family)
            probe_host(binary)
        print(f"{label} native helper: OK (echo handshake passed; no passwords accessed)")
        print("GnuPG: available; unlocking still requires a working graphical pinentry")
        store = Path(os.environ.get("PASSWORD_STORE_DIR") or Path.home() / ".password-store")
        print(f"PassTUI store: {store}")
        print("For a custom store, set its absolute path in Browserpass Options → Custom store locations.")
        extension_url = FIREFOX_EXTENSION_URL if family == "firefox" else EXTENSION_URL
        print(f"Browser step: install/enable the official extension in {label}: {extension_url}")
        print("Use github.com/personal as an entry path; Ctrl+Shift+L selects a login.")
        print("This verifies the native helper, not extension installation, permissions, or live form filling.")
        return 0
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Browserpass setup: {error}", file=sys.stderr)
        return 1
