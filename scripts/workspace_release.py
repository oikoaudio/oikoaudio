#!/usr/bin/env python3
"""Build all hosted plugins once per platform and publish their separate archives."""
import argparse
import hashlib
import json
import os
import subprocess
import sys

from products import ROOT, products
import release


def check_tag(tag: str):
    expected = {f"v{product.version}" for product in products().values()}
    if expected != {tag}:
        raise ValueError(f"release tag {tag!r} must match the shared workspace version: {sorted(expected)}")


def matrix(platform="all"):
    return {"include": [dict(name=name, runner=runner, artifact=f"plugins-{suffix}")
                        for os_name, name, runner, suffix in release.PLATFORMS
                        if platform in ("all", os_name)]}


def package(platform: str):
    for product in products().values():
        row = next(row for row in release.matrix(product)["include"]
                   if row["platform"] == platform)
        release.package(product, platform, ROOT / "dist" / row["archive"])


def publication_plan():
    check_tag(os.environ["GITHUB_REF_NAME"])
    repo = os.environ["GITHUB_REPOSITORY"]
    revision = os.environ["GITHUB_SHA"]
    pages = json.loads(release.run("gh", "api", f"repos/{repo}/releases?per_page=100",
                                   "--paginate", "--slurp", capture=True))
    existing = {entry["tag_name"]: entry for page in pages for entry in page}
    plan = []
    for product in products().values():
        tag = f"{product.key}/v{product.version}"
        refs = json.loads(release.run("gh", "api", f"repos/{repo}/git/matching-refs/tags/{tag}", capture=True))
        ref = next((ref for ref in refs if ref["ref"] == f"refs/tags/{tag}"), None)
        if ref:
            target = ref["object"]
            while target["type"] == "tag":
                target = json.loads(release.run("gh", "api", f"repos/{repo}/git/tags/{target['sha']}",
                                                capture=True))["object"]
            if target["type"] != "commit" or target["sha"] != revision:
                raise ValueError(f"{tag} already points to another commit; use a new version")
        published = tag in existing and not existing[tag]["draft"]
        if published and not ref:
            raise ValueError(f"published release {tag} is missing its tag; refusing to recreate it")
        plan.append((product, tag, ref is not None, published))
    return plan


def preflight():
    plan = publication_plan()
    if all(published for _, _, _, published in plan):
        raise ValueError("All product releases are already published; no build is needed")


def publish():
    plan = publication_plan()
    repo = os.environ["GITHUB_REPOSITORY"]
    revision = os.environ["GITHUB_SHA"]
    # Check every archive and any already-published product before making remote changes.
    for product, tag, _, published in plan:
        files = []
        for row in release.matrix(product)["include"]:
            archive = ROOT / "dist" / row["archive"]
            with archive.open("rb") as source:
                digest = hashlib.file_digest(source, "sha256").hexdigest()
            files.append(dict(name=archive.name, size=archive.stat().st_size, sha256=digest))
        if published:
            manifest = json.loads(release.run("gh", "release", "download", tag, "--repo", repo,
                                             "--pattern", "release.json", "--output", "-", capture=True))
            expected = dict(schema=1, product=product.key, version=product.version, tag=tag,
                            source_revision=revision, files=files)
            if manifest != expected:
                raise ValueError(f"{tag} is published with different assets; use a new version")
    for _, tag, exists, _ in plan:
        if not exists:
            # Never force a tag. A competing publication must fail rather than move it.
            release.run("gh", "api", "--method", "POST", f"repos/{repo}/git/refs",
                        "-f", f"ref=refs/tags/{tag}", "-f", f"sha={revision}")
    for product, tag, _, published in plan:
        if published:
            print(f"{tag} already contains these assets; leaving it unchanged")
        else:
            release.publish(product, tag)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    configure = commands.add_parser("matrix")
    configure.add_argument("--platform", choices=["all", *[row[0] for row in release.PLATFORMS]], default="all")
    configure.add_argument("--github-output", action="store_true")
    for command in ("build", "package"):
        sub = commands.add_parser(command)
        sub.add_argument("--platform", choices=[row[0] for row in release.PLATFORMS], required=True)
    for command in ("test", "preflight", "publish"):
        commands.add_parser(command)
    args = parser.parse_args()
    if args.command == "matrix":
        value = json.dumps(matrix(args.platform))
        if args.github_output:
            with open(os.environ["GITHUB_OUTPUT"], "a") as output:
                output.write(f"matrix={value}\n")
        else:
            print(value)
    elif args.command == "test":
        release.test_selected(list(products().values()))
    elif args.command == "build":
        release.build_selected(list(products().values()), args.platform)
    elif args.command == "package":
        package(args.platform)
    elif args.command == "preflight":
        preflight()
    elif args.command == "publish":
        publish()


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
