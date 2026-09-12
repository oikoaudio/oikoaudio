import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import call, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from products import products
import release
import workspace_release


class WorkspaceReleaseTests(unittest.TestCase):
    def setUp(self):
        self.products = products()
        self.revision = "a" * 40
        self.env = dict(GITHUB_REF_NAME=f"v{next(iter(self.products.values())).version}",
                        GITHUB_REPOSITORY="owner/repo", GITHUB_SHA=self.revision)

    def test_shared_tag_and_platform_selection(self):
        workspace_release.check_tag(self.env["GITHUB_REF_NAME"])
        for tag in ("v0.0.0", "wow/" + self.env["GITHUB_REF_NAME"]):
            with self.assertRaises(ValueError):
                workspace_release.check_tag(tag)
        rows = workspace_release.matrix()["include"]
        self.assertEqual(len(rows), len(release.PLATFORMS))
        for (platform, _, runner, _), row in zip(release.PLATFORMS, rows):
            self.assertEqual(row["runner"], runner)
            self.assertEqual(workspace_release.matrix(platform)["include"], [row])

    def test_builds_all_packages_with_one_cargo_invocation(self):
        selected = list(self.products.values())
        with patch.object(release, "run") as run:
            release.build_selected(selected, "Linux")
        run.assert_called_once_with("cargo", "xtask", "bundle",
                                    *[arg for p in selected for arg in ("-p", p.package)],
                                    "--release", "--locked")
        with patch.object(release, "run") as run:
            release.test_selected([*selected, selected[0]])
        run.assert_called_once()
        args = run.call_args.args
        for package in {package for product in selected for package in product.test_packages}:
            self.assertEqual(args.count(package), 1)

    def test_universal_build_verifies_every_product_binary(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(release, "ROOT", Path(directory)):
            bundled = Path(directory) / "target/bundled"
            bundled.mkdir(parents=True)
            for product in self.products.values():
                for ext in ("clap", "vst3"):
                    (bundled / f"{product.bundle_name}.{ext}").mkdir()
            with patch.object(release, "run", return_value="arm64 x86_64") as run:
                release.build_selected(list(self.products.values()), "macOS")
            self.assertEqual(sum(c.args[:2] == ("cargo", "xtask") for c in run.call_args_list), 1)
            self.assertEqual(sum(c.args[0] == "rustup" for c in run.call_args_list), 1)
            self.assertEqual(sum(c.args[0] == "lipo" for c in run.call_args_list), 2 * len(self.products))

    def test_packages_each_product_under_its_existing_archive_name(self):
        for platform, _, _, _ in release.PLATFORMS:
            with patch.object(release, "package") as package:
                workspace_release.package(platform)
            expected = [call(product, platform, workspace_release.ROOT / "dist" / row["archive"])
                        for product in self.products.values() for row in release.matrix(product)["include"]
                        if row["platform"] == platform]
            self.assertEqual(package.call_args_list, expected)

    def publication_fixture(self, directory):
        dist = directory / "dist"
        dist.mkdir()
        plan = []
        manifests = {}
        for product in self.products.values():
            tag = f"{product.key}/v{product.version}"
            plan.append((product, tag, False, False))
            files = []
            for row in release.matrix(product)["include"]:
                archive = dist / row["archive"]
                archive.write_bytes(b"archive")
                files.append(dict(name=archive.name, size=7, sha256=hashlib.sha256(b"archive").hexdigest()))
            manifests[tag] = dict(schema=1, product=product.key, version=product.version,
                                  tag=tag, source_revision=self.revision, files=files)
        return plan, manifests

    def test_publication_creates_product_tags_at_the_shared_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            plan, _ = self.publication_fixture(directory)
            with patch.dict(os.environ, self.env), patch.object(workspace_release, "ROOT", directory), \
                    patch.object(workspace_release, "publication_plan", return_value=plan), \
                    patch.object(release, "run") as run, patch.object(release, "publish") as publish:
                workspace_release.publish()
            self.assertEqual(run.call_count, len(plan))
            for operation, (_, tag, _, _) in zip(run.call_args_list, plan):
                self.assertEqual(operation.args, ("gh", "api", "--method", "POST", "repos/owner/repo/git/refs",
                                                 "-f", f"ref=refs/tags/{tag}", "-f", f"sha={self.revision}"))
            self.assertEqual(publish.call_args_list, [call(product, tag) for product, tag, _, _ in plan])

    def test_missing_archive_prevents_all_remote_writes(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            plan, _ = self.publication_fixture(directory)
            list((directory / "dist").iterdir())[-1].unlink()
            with patch.dict(os.environ, self.env), patch.object(workspace_release, "ROOT", directory), \
                    patch.object(workspace_release, "publication_plan", return_value=plan), \
                    patch.object(release, "run") as run, patch.object(release, "publish") as publish:
                with self.assertRaises(FileNotFoundError):
                    workspace_release.publish()
                run.assert_not_called()
                publish.assert_not_called()

    def test_retry_skips_only_identical_published_assets(self):
        for changed in (False, True):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as directory:
                directory = Path(directory)
                plan, manifests = self.publication_fixture(directory)
                product, tag, _, _ = plan[0]
                plan[0] = (product, tag, True, True)
                manifest = manifests[tag]
                if changed:
                    manifest["files"][0]["sha256"] = "other"
                with patch.dict(os.environ, self.env), patch.object(workspace_release, "ROOT", directory), \
                        patch.object(workspace_release, "publication_plan", return_value=plan), \
                        patch.object(release, "run", return_value=json.dumps(manifest)) as run, \
                        patch.object(release, "publish") as publish:
                    if changed:
                        with self.assertRaises(ValueError):
                            workspace_release.publish()
                        publish.assert_not_called()
                        self.assertEqual(run.call_count, 1)
                    else:
                        workspace_release.publish()
                        self.assertEqual(publish.call_args_list, [call(p, t) for p, t, _, _ in plan[1:]])

    def test_remote_tag_checks_accept_lightweight_and_annotated_tags_but_reject_other_commits(self):
        for annotated, sha in ((False, self.revision), (True, self.revision), (False, "b" * 40)):
            with self.subTest(annotated=annotated, sha=sha):
                def run(*args, **kwargs):
                    endpoint = args[2]
                    if "releases?" in endpoint:
                        return "[[]]"
                    if "/git/tags/" in endpoint:
                        return json.dumps(dict(object=dict(type="commit", sha=sha)))
                    tag = endpoint.split("matching-refs/tags/")[1]
                    return json.dumps([dict(ref=f"refs/tags/{tag}", object=dict(
                        type="tag" if annotated else "commit", sha="tag-object" if annotated else sha))])
                with patch.dict(os.environ, self.env), patch.object(release, "run", side_effect=run):
                    if sha == self.revision:
                        self.assertTrue(all(exists for _, _, exists, _ in workspace_release.publication_plan()))
                    else:
                        with self.assertRaises(ValueError):
                            workspace_release.publication_plan()

    def test_network_failure_does_not_publish(self):
        with patch.dict(os.environ, self.env), patch.object(release, "run", side_effect=subprocess.CalledProcessError(1, "gh")) as run:
            with self.assertRaises(subprocess.CalledProcessError):
                workspace_release.publish()
            self.assertEqual(run.call_count, 1)

    def test_preflight_stops_a_build_when_every_product_is_already_published(self):
        plan = [(p, f"{p.key}/v{p.version}", True, True) for p in self.products.values()]
        with patch.object(workspace_release, "publication_plan", return_value=plan):
            with self.assertRaises(ValueError):
                workspace_release.preflight()


if __name__ == "__main__":
    unittest.main()
