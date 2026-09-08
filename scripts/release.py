#!/usr/bin/env python3
"""Build and package one hosted product using the common release configuration."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile

from products import ROOT, Product, products
from notices import collect as collect_notices

PLATFORMS = (
    ("Linux", "Linux x86_64", "ubuntu-24.04", "linux-x86_64"),
    ("Windows", "Windows x86_64", "windows-2025", "windows-x86_64"),
    ("macOS", "macOS Universal", "macos-15", "macos-universal"),
)


def run(*args, capture=False):
    return subprocess.run(args, cwd=ROOT, check=True, text=True,
                          stdout=subprocess.PIPE if capture else None).stdout


def matrix(product: Product, windows_only=False):
    rows = []
    for platform, name, runner, suffix in PLATFORMS:
        if windows_only and platform != "Windows":
            continue
        artifact = f"{product.key}-{suffix}"
        extension = product.linux_archive if platform == "Linux" else "zip"
        rows.append(dict(name=name, runner=runner, artifact=artifact,
                         archive=f"{artifact}.{extension}"))
    return {"include": rows}


def bundles(product: Product, directory: Path):
    paths = [directory / f"{product.bundle_name}.{ext}" for ext in ("clap", "vst3")]
    for path in paths:
        if not path.exists():
            raise FileNotFoundError(path)
    return paths


def test(product: Product):
    args = ["cargo", "test", "--locked"]
    for package in product.test_packages:
        args += ["-p", package]
    run(*args)


def build(product: Product, platform: str):
    universal = platform == "macOS"
    if universal:
        run("rustup", "target", "add", "aarch64-apple-darwin", "x86_64-apple-darwin")
    run("cargo", "xtask", "bundle-universal" if universal else "bundle",
        product.package, "--release", "--locked")
    if universal:
        directory = ROOT / "target/bundled"
        for bundle in bundles(product, directory):
            binary = bundle / "Contents/MacOS" / product.bundle_name
            architectures = run("lipo", "-archs", str(binary), capture=True).split()
            if not {"arm64", "x86_64"}.issubset(architectures):
                raise ValueError(f"{binary} is not universal: {architectures}")
        # AU distribution is suspended. Preserve the old scripts' stale-output cleanup.
        component = directory / f"{product.bundle_name}.component"
        if component.is_symlink() or component.is_file():
            component.unlink()
        elif component.is_dir():
            shutil.rmtree(component)


def package(product: Product, platform: str, output: Path, source: Path | None = None):
    output = output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    if product.package_script:
        run(sys.executable, str(product.package_script), "--platform", platform,
            "--output", str(output))
        return
    paths = bundles(product, source or ROOT / "target/bundled")
    temporary = output.with_name(output.name + ".tmp")
    try:
        with tempfile.TemporaryDirectory(prefix="oiko-notices-") as directory:
            notice_root = Path(directory)
            collect_notices(product, notice_root)
            paths += sorted(notice_root.iterdir())
            if platform == "macOS":
                # ditto retains resource forks, modes and bundle symlinks in both steps.
                with tempfile.TemporaryDirectory(prefix="oiko-release-") as staging:
                    for path in paths:
                        run("ditto", str(path), str(Path(staging) / path.name))
                    run("ditto", "-c", "-k", "--sequesterRsrc", staging, str(temporary))
            elif platform == "Linux" and product.linux_archive == "tar.gz":
                with tarfile.open(temporary, "w:gz") as archive:
                    for path in paths:
                        archive.add(path, arcname=path.name)
            else:
                with zipfile.ZipFile(temporary, "w", compression=zipfile.ZIP_DEFLATED) as archive:
                    for path in paths:
                        archive.write(path, path.name)
                        if path.is_dir():
                            for child in sorted(path.rglob("*")):
                                archive.write(child, child.relative_to(path.parent))
        temporary.replace(output)
    finally:
        temporary.unlink(missing_ok=True)
    print(f"Packaged {output}")


def publish(product: Product):
    tag = os.environ["GITHUB_REF_NAME"]
    repo = os.environ["GITHUB_REPOSITORY"]
    product.check_tag(tag)
    archives = [ROOT / "dist" / row["archive"] for row in matrix(product)["include"]]
    for archive in archives:
        if not archive.is_file():
            raise FileNotFoundError(archive)
    files = []
    for archive in archives:
        with archive.open("rb") as source:
            digest = hashlib.file_digest(source, "sha256").hexdigest()
        files.append({"name": archive.name, "size": archive.stat().st_size, "sha256": digest})
    metadata = {
        "schema": 1,
        "product": product.key,
        "version": product.version,
        "tag": tag,
        "source_revision": os.environ["GITHUB_SHA"],
        "files": files,
    }
    manifest = ROOT / "dist/release.json"
    manifest.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    checksums = ROOT / "dist/checksums.txt"
    checksums.write_text("".join(f"{file['sha256']}  {file['name']}\n" for file in metadata["files"]), encoding="utf-8")
    # Listing must succeed: an authentication/network error must not be mistaken for
    # a missing release. Published assets are never replaced (or their counters reset).
    pages = json.loads(run("gh", "api", f"repos/{repo}/releases?per_page=100",
                          "--paginate", "--slurp", capture=True))
    existing = next((release for page in pages for release in page if release["tag_name"] == tag), None)
    if existing and not existing["draft"]:
        raise ValueError(f"{tag} is already published; use a new version, or rerun only the website update job")
    notes = ["--notes-file", str(product.notes_file)] if product.notes_file else ["--notes", product.notes or ""]
    if not existing:
        # Keep partial uploads hidden until every archive and its metadata is ready.
        run("gh", "release", "create", tag, "--draft", "--verify-tag", "--repo", repo,
            "--title", f"{product.bundle_name} {tag}", *notes)
    run("gh", "release", "upload", tag, *map(str, archives), str(manifest), str(checksums),
        "--clobber", "--repo", repo)
    run("gh", "release", "edit", tag, "--draft=false", f"--prerelease={'true' if '-' in product.version else 'false'}",
        "--repo", repo, *notes)


def main():
    registry = products()
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for command in ("matrix", "test", "build", "package", "publish"):
        sub = commands.add_parser(command)
        sub.add_argument("product", choices=registry)
        if command == "matrix":
            sub.add_argument("--windows-only", choices=("true", "false"), default="false")
            sub.add_argument("--github-output", action="store_true")
        if command in ("build", "package"):
            sub.add_argument("--platform", choices=[p[0] for p in PLATFORMS], required=True)
        if command == "package":
            sub.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    product = registry[args.product]
    if args.command == "matrix":
        value = json.dumps(matrix(product, args.windows_only == "true"))
        if args.github_output:
            with open(os.environ["GITHUB_OUTPUT"], "a") as output:
                output.write(f"matrix={value}\n")
        else:
            print(value)
    elif args.command == "test":
        test(product)
    elif args.command == "build":
        build(product, args.platform)
    elif args.command == "package":
        package(product, args.platform, args.output)
    elif args.command == "publish":
        publish(product)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
