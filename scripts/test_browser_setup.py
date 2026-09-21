import contextlib
import io
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import browser_setup as setup


class BrowserSetupTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="passtui-browser-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.manifests = {}
        for family in ("firefox", "chromium"):
            manifest = {"name": setup.APP_ID, "type": "stdio", "path": sys.executable}
            if family == "firefox":
                manifest["allowed_extensions"] = [setup.FIREFOX_EXTENSION_ID]
            else:
                manifest["allowed_origins"] = [f"chrome-extension://{setup.EXTENSION_ID}/"]
            source = self.root / f"{family}.json"
            source.write_text(json.dumps(manifest))
            self.manifests[family] = source

    def test_all_presets_register_correct_family_idempotently(self):
        for browser, (_, family, _) in setup.BROWSERS.items():
            with self.subTest(browser=browser):
                directory = setup.default_host_directory(browser, self.root / "home", self.root / "config")
                source = self.manifests[family]
                target = setup.register_host(source, directory, family)
                self.assertEqual(target.resolve(), source)
                self.assertEqual(setup.register_host(source, directory, family), target)
                self.assertEqual(setup.validate_manifest(target, family), Path(sys.executable))

    def test_firefox_uses_home_and_chromium_honors_xdg(self):
        with patch.dict("os.environ", {"XDG_CONFIG_HOME": str(self.root / "xdg")}):
            self.assertEqual(setup.default_host_directory("firefox", self.root), self.root / ".mozilla/native-messaging-hosts")
            self.assertEqual(setup.default_host_directory("librewolf", self.root), self.root / ".librewolf/native-messaging-hosts")
            self.assertEqual(setup.default_host_directory("chrome", self.root), self.root / "xdg/google-chrome/NativeMessagingHosts")
        for browser, folder in [("chromium", "chromium"), ("brave", "BraveSoftware/Brave-Browser"), ("edge", "microsoft-edge"), ("vivaldi", "vivaldi")]:
            self.assertEqual(setup.default_host_directory(browser, self.root, self.root / "config"), self.root / "config" / folder / "NativeMessagingHosts")

    def test_wrong_family_is_rejected_before_writing(self):
        for family, other in [("firefox", "chromium"), ("chromium", "firefox")]:
            destination = self.root / f"wrong-{family}"
            with self.assertRaises(ValueError):
                setup.register_host(self.manifests[other], destination, family)
            self.assertFalse(destination.exists())

    def test_unknown_extension_and_missing_binary_are_rejected(self):
        for change in [{"allowed_extensions": ["unrelated@example.com"]}, {"path": str(self.root / "missing")}, {"path": "relative/path"}]:
            manifest = json.loads(self.manifests["firefox"].read_text())
            manifest.update(change)
            candidate = self.root / "invalid.json"
            candidate.write_text(json.dumps(manifest))
            with self.assertRaises(ValueError):
                setup.validate_manifest(candidate, "firefox")

    def test_custom_registration_and_broken_symlink_are_not_overwritten(self):
        directory = self.root / "custom"
        directory.mkdir()
        target = directory / f"{setup.APP_ID}.json"
        target.write_text("existing registration")
        with self.assertRaises(ValueError):
            setup.register_host(self.manifests["firefox"], directory, "firefox")
        self.assertEqual(target.read_text(), "existing registration")
        target.unlink()
        target.symlink_to(self.root / "missing.json")
        with self.assertRaises(ValueError):
            setup.register_host(self.manifests["firefox"], directory, "firefox")
        self.assertTrue(target.is_symlink())

    def test_echo_probe_sends_no_store_or_credentials(self):
        payload = json.dumps({"passtui": "ok"}).encode()
        response = subprocess.CompletedProcess([], 0, struct.pack("=I", len(payload)) + payload, b"")
        with patch.object(setup.subprocess, "run", return_value=response) as run:
            setup.probe_host(Path(sys.executable))
        packet = run.call_args.kwargs["input"]
        self.assertEqual(struct.unpack("=I", packet[:4])[0], len(packet[4:]))
        self.assertEqual(json.loads(packet[4:]), {"action": "echo", "echoResponse": {"passtui": "ok"}})
        self.assertEqual(run.call_args.kwargs["timeout"], 5)

    def test_probe_rejects_truncated_and_unexpected_replies(self):
        for output in [b"", b"abc", struct.pack("=I", 80) + b"{}", struct.pack("=I", 2) + b"{}"]:
            response = subprocess.CompletedProcess([], 0, output, b"")
            with patch.object(setup.subprocess, "run", return_value=response), self.assertRaises(ValueError):
                setup.probe_host(Path(sys.executable))

    def test_cli_custom_directory_and_check_do_not_require_source_manifest(self):
        for browser, family in [("firefox", "firefox"), ("chromium", "chromium")]:
            directory = self.root / browser
            with patch.object(setup, "probe_host"), patch.object(setup.shutil, "which", return_value="/usr/bin/gpg"), contextlib.redirect_stdout(io.StringIO()) as output:
                args = ["--browser", browser, "--native-host-dir", str(directory)]
                self.assertEqual(setup.main([*args, "--configure", "--host-manifest", str(self.manifests[family])]), 0)
                self.assertEqual(setup.main([*args, "--check", "--host-manifest", str(self.root / "absent.json")]), 0)
                expected_url = setup.FIREFOX_EXTENSION_URL if family == "firefox" else setup.EXTENSION_URL
                self.assertIn(expected_url, output.getvalue())

    def test_failed_preflight_does_not_register_host(self):
        directory = self.root / "unwritten"
        with patch.object(setup, "probe_host", side_effect=ValueError("echo failed")), patch.object(setup.shutil, "which", return_value="/usr/bin/gpg"), contextlib.redirect_stderr(io.StringIO()):
            result = setup.main(["--browser", "firefox", "--configure", "--host-manifest", str(self.manifests["firefox"]), "--native-host-dir", str(directory)])
        self.assertEqual(result, 1)
        self.assertFalse(directory.exists())

    def test_invalid_browser_and_firefox_user_data_override_fail(self):
        for args in [["--browser", "unknown", "--check"], ["--browser", "firefox", "--configure", "--user-data-dir", str(self.root / "wrong")], ["--configure"]]:
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as error:
                setup.main(args)
            self.assertEqual(error.exception.code, 2)
        self.assertFalse((self.root / "wrong").exists())


if __name__ == "__main__":
    unittest.main()
