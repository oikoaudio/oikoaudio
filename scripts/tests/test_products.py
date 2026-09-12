from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import products


class ProductVersionTests(unittest.TestCase):
    def test_shared_version_is_resolved_and_diverging_plugin_versions_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.5.0-beta.1"\n')
            (root / "products.toml").write_text(
                '[products.test]\nmanifest = "plugin.toml"\nlinux_archive = "zip"\ntest_packages = []\n')
            (root / "bundler.toml").write_text('[test-plugin]\nname = "Test Plugin"\n')
            with patch.object(products, "ROOT", root):
                for declaration in ('version.workspace = true', 'version = "0.5.0-beta.1"'):
                    (root / "plugin.toml").write_text(
                        f'[package]\nname = "test-plugin"\n{declaration}\n')
                    product = products.products()["test"]
                    product.check_tag("test/v0.5.0-beta.1")
                    with self.assertRaises(ValueError):
                        product.check_tag("test/v0.4.0-beta.1")
                (root / "plugin.toml").write_text(
                    '[package]\nname = "test-plugin"\nversion = "0.4.0-beta.1"\n')
                with self.assertRaises(ValueError):
                    products.products()
