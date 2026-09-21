import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("setup_brave", Path(__file__).with_name("setup-brave.py"))
setup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(setup)


class SetupTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="passtui-brave-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.host = self.root / "host.json"
        self.host.write_text(json.dumps({"name": setup.APP_ID, "type": "stdio", "path": "/bin/true",
                            "allowed_origins": [f"chrome-extension://{setup.EXTENSION_ID}/"]}))
        self.brave = self.root / "Brave"

    def test_registration_is_repeatable(self):
        target = setup.register_host(self.host, self.brave)
        self.assertTrue(target.is_symlink())
        self.assertEqual(target.resolve(), self.host)
        self.assertEqual(setup.register_host(self.host, self.brave), target)

    def test_existing_custom_registration_is_preserved(self):
        target = self.brave / "NativeMessagingHosts" / f"{setup.APP_ID}.json"
        target.parent.mkdir(parents=True)
        target.write_text("existing custom configuration")
        with self.assertRaises(ValueError):
            setup.register_host(self.host, self.brave)
        self.assertEqual(target.read_text(), "existing custom configuration")

    def test_untrusted_origin_is_rejected_without_writes(self):
        manifest = json.loads(self.host.read_text())
        manifest["allowed_origins"] = ["chrome-extension://other-extension/"]
        self.host.write_text(json.dumps(manifest))
        with self.assertRaises(ValueError):
            setup.register_host(self.host, self.brave)
        self.assertFalse(self.brave.exists())


if __name__ == "__main__":
    unittest.main()
