import os
import json
import hashlib
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release
from products import products


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.products = products()
        mocked = patch.object(release, "collect_notices", side_effect=self.notice_fixtures)
        mocked.start()
        self.addCleanup(mocked.stop)

    def notice_fixtures(self, product, directory):
        (directory / "LICENSE").write_bytes(b"product license")
        (directory / "licenses").mkdir()
        (directory / "licenses/upstream.txt").write_bytes(b"upstream notice")
        (directory / "dependency-licenses.json").write_text("[]\n")

    def fixtures(self, directory, product):
        clap = directory / f"{product.bundle_name}.clap"
        clap.write_bytes(b"clap binary")
        clap.chmod(0o755)
        vst3 = directory / f"{product.bundle_name}.vst3"
        vst3.mkdir()
        (vst3 / "plugin.bin").write_bytes(b"vst3 binary")
        (directory / "Other Plugin.clap").write_bytes(b"unrelated")
        (directory / f"{product.bundle_name}.component").mkdir()
        return clap, vst3

    def test_matrix_and_tags_keep_independent_product_contracts(self):
        for key, product in self.products.items():
            with self.subTest(product=key):
                product.check_tag(f"{key}/v{product.version}")
                for wrong in (f"{key}/v0.0.0", f"other/v{product.version}", product.version):
                    with self.assertRaises(ValueError):
                        product.check_tag(wrong)
                rows = release.matrix(product)["include"]
                extension = "zip" if key == "inton" else "tar.gz"
                self.assertEqual([r["archive"] for r in rows], [
                    f"{key}-linux-x86_64.{extension}", f"{key}-windows-x86_64.zip",
                    f"{key}-macos-universal.zip",
                ])
                self.assertEqual(release.matrix(product, True)["include"], [rows[1]])

    def test_archives_contain_only_selected_bundles_and_keep_unix_modes(self):
        product = self.products["wow"]
        for platform, extension in (("Linux", "tar.gz"), ("Windows", "zip")):
            with self.subTest(platform=platform), tempfile.TemporaryDirectory() as directory:
                directory = Path(directory)
                clap, vst3 = self.fixtures(directory, product)
                output = directory / f"plugins.{extension}"
                release.package(product, platform, output, source=directory)
                if platform == "Linux":
                    with tarfile.open(output) as archive:
                        names = archive.getnames()
                        self.assertEqual(archive.extractfile(clap.name).read(), b"clap binary")
                        if os.name != "nt":
                            self.assertEqual(archive.getmember(clap.name).mode & 0o111, 0o111)
                else:
                    with zipfile.ZipFile(output) as archive:
                        names = archive.namelist()
                        self.assertIsNone(archive.testzip())
                        self.assertEqual(archive.read(clap.name), b"clap binary")
                self.assertEqual({Path(n).parts[0] for n in names},
                                 {clap.name, vst3.name, "LICENSE", "licenses", "dependency-licenses.json"})
                if platform == "Linux":
                    with tarfile.open(output) as archive:
                        self.assertEqual(archive.extractfile("licenses/upstream.txt").read(), b"upstream notice")
                else:
                    with zipfile.ZipFile(output) as archive:
                        self.assertEqual(archive.read("licenses/upstream.txt"), b"upstream notice")

    @unittest.skipIf(os.name == "nt", "Unix archive symlink semantics")
    def test_linux_archive_preserves_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            product = self.products["weft"]
            _, vst3 = self.fixtures(directory, product)
            (vst3 / "link").symlink_to("plugin.bin")
            output = directory / "plugins.tar.gz"
            release.package(product, "Linux", output, source=directory)
            with tarfile.open(output) as archive:
                link = archive.getmember(f"{vst3.name}/link")
                self.assertTrue(link.issym())
                self.assertEqual(link.linkname, "plugin.bin")

    def test_missing_bundle_does_not_replace_an_existing_archive(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            output = directory / "plugins.zip"
            output.write_bytes(b"previous archive")
            with self.assertRaises(FileNotFoundError):
                release.package(self.products["wow"], "Windows", output, source=directory)
            self.assertEqual(output.read_bytes(), b"previous archive")

    def test_inton_keeps_its_runtime_packaging_hook(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(release, "run") as run:
            output = Path(directory) / "inton.zip"
            release.package(self.products["inton"], "Windows", output)
            run.assert_called_once_with(sys.executable, str(self.products["inton"].package_script),
                                        "--platform", "Windows", "--output", str(output.resolve()))

    def test_universal_build_checks_both_binaries_and_rejects_missing_architecture(self):
        product = self.products["wow"]
        with tempfile.TemporaryDirectory() as directory, patch.object(release, "ROOT", Path(directory)):
            bundled = Path(directory) / "target/bundled"
            bundled.mkdir(parents=True)
            self.fixtures(bundled, product)
            with patch.object(release, "run", return_value="arm64 x86_64") as run:
                release.build(product, "macOS")
                self.assertEqual(sum(call.args[0] == "lipo" for call in run.call_args_list), 2)
                self.assertIn(unittest.mock.call("cargo", "xtask", "bundle-universal", "-p", product.package,
                                                "--release", "--locked"), run.call_args_list)
            with patch.object(release, "run", return_value="arm64"):
                with self.assertRaises(ValueError):
                    release.build(product, "macOS")

    def test_macos_packaging_uses_ditto_for_bundle_metadata_and_archive(self):
        product = self.products["weft"]
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            clap, vst3 = self.fixtures(directory, product)
            calls = []
            def run(*args):
                calls.append(args)
                if "--sequesterRsrc" in args:
                    Path(args[-1]).write_bytes(b"archive")
            output = directory / "weft.zip"
            with patch.object(release, "run", side_effect=run):
                release.package(product, "macOS", output, source=directory)
            self.assertEqual([call[:2] for call in calls[:2]], [("ditto", str(clap)), ("ditto", str(vst3))])
            self.assertEqual(calls[-1][:4], ("ditto", "-c", "-k", "--sequesterRsrc"))
            self.assertEqual({Path(call[1]).name for call in calls[2:-1]},
                             {"LICENSE", "licenses", "dependency-licenses.json"})
            self.assertEqual(output.read_bytes(), b"archive")

    def publication_fixture(self, directory):
        product = self.products["wow"]
        dist = directory / "dist"
        dist.mkdir()
        archives = [dist / row["archive"] for row in release.matrix(product)["include"]]
        for archive in archives:
            archive.write_bytes(b"archive")
        (dist / "unrelated.zip").write_bytes(b"other")
        env = dict(GITHUB_REF_NAME=f"wow/v{product.version}",
                   GITHUB_REPOSITORY="owner/repo", GITHUB_SHA="a" * 40)
        return product, dist, archives, env

    def test_publication_uploads_complete_assets_before_publishing_a_draft(self):
        for existing_draft in (False, True):
            with self.subTest(existing_draft=existing_draft), tempfile.TemporaryDirectory() as directory:
                directory = Path(directory)
                product, dist, archives, env = self.publication_fixture(directory)
                pages = [[{"tag_name": env["GITHUB_REF_NAME"], "draft": True}]] if existing_draft else [[]]
                with patch.object(release, "ROOT", directory), patch.dict(os.environ, env), patch.object(release, "run", return_value=json.dumps(pages)) as run:
                    release.publish(product)
                operations = [call.args for call in run.call_args_list]
                self.assertEqual(operations[0][:2], ("gh", "api"))
                uploads = [args for args in operations if args[:3] == ("gh", "release", "upload")]
                self.assertEqual(len(uploads), 1)
                upload = uploads[0]
                self.assertTrue(all(str(archive) in upload for archive in archives))
                self.assertIn(str(dist / "release.json"), upload)
                self.assertIn(str(dist / "checksums.txt"), upload)
                self.assertNotIn(str(dist / "unrelated.zip"), upload)
                self.assertIn("--clobber", upload)
                creates = [args for args in operations if args[:3] == ("gh", "release", "create")]
                self.assertEqual(len(creates), 0 if existing_draft else 1)
                if creates:
                    self.assertIn("--draft", creates[0])
                    self.assertIn("--verify-tag", creates[0])
                    self.assertLess(operations.index(creates[0]), operations.index(upload))
                self.assertEqual(operations[-1][:3], ("gh", "release", "edit"))
                self.assertIn("--draft=false", operations[-1])
                self.assertIn("--prerelease=true", operations[-1])
                manifest = json.loads((dist / "release.json").read_text())
                self.assertEqual(manifest["source_revision"], env["GITHUB_SHA"])
                self.assertEqual({file["name"] for file in manifest["files"]}, {a.name for a in archives})
                for file in manifest["files"]:
                    data = (dist / file["name"]).read_bytes()
                    self.assertEqual(file["size"], len(data))
                    self.assertEqual(file["sha256"], hashlib.sha256(data).hexdigest())
                    self.assertIn(f"{file['sha256']}  {file['name']}\n", (dist / "checksums.txt").read_text())

    def test_publication_does_not_replace_published_assets(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            product, _, _, env = self.publication_fixture(directory)
            pages = [[{"tag_name": env["GITHUB_REF_NAME"], "draft": False}]]
            with patch.object(release, "ROOT", directory), patch.dict(os.environ, env), patch.object(release, "run", return_value=json.dumps(pages)) as run:
                with self.assertRaises(ValueError):
                    release.publish(product)
                self.assertEqual(len(run.call_args_list), 1)
                self.assertEqual(run.call_args.args[:2], ("gh", "api"))

    def test_release_listing_failure_does_not_create_a_release(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            product, _, _, env = self.publication_fixture(directory)
            with patch.object(release, "ROOT", directory), patch.dict(os.environ, env), patch.object(release, "run", side_effect=subprocess.CalledProcessError(1, "gh")) as run:
                with self.assertRaises(subprocess.CalledProcessError):
                    release.publish(product)
                self.assertEqual(len(run.call_args_list), 1)



if __name__ == "__main__":
    unittest.main()
