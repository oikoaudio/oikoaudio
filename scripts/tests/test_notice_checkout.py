import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class NoticeCheckoutTests(unittest.TestCase):
    def test_reviewed_notice_bytes_survive_windows_line_ending_settings(self):
        records = json.loads((ROOT / "licenses/upstream.json").read_text())
        notices = {record["path"]: record["sha256"]
                   for group in records.values() for record in group}
        with tempfile.TemporaryDirectory() as directory:
            staging = Path(directory) / "source"
            checkout = Path(directory) / "checkout"
            staging.mkdir()
            checkout.mkdir()
            shutil.copyfile(ROOT / ".gitattributes", staging / ".gitattributes")
            for name in notices:
                target = staging / name
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / name, target)
            subprocess.run(["git", "init", "--quiet", str(staging)], check=True)
            subprocess.run(["git", "-c", "core.autocrlf=false", "add", "."],
                           cwd=staging, check=True)
            subprocess.run(["git", "-c", "core.autocrlf=true", "checkout-index", "--all",
                            f"--prefix={checkout.as_posix()}/"], cwd=staging, check=True)
            for name, expected in notices.items():
                with self.subTest(notice=name):
                    self.assertEqual(hashlib.sha256((checkout / name).read_bytes()).hexdigest(), expected)
