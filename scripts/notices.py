"""Collect product release notices from the locked dependency graph and reviewed upstream texts."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

from products import ROOT


def dependency_packages(metadata, package):
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    pending = [p["id"] for p in packages.values() if p["name"] == package]
    if len(pending) != 1:
        raise ValueError(f"Expected one package named {package}")
    seen = set()
    while pending:
        key = pending.pop()
        if key in seen:
            continue
        seen.add(key)
        pending.extend(d["pkg"] for d in nodes[key]["deps"]
                       if any(kind["kind"] != "dev" for kind in d["dep_kinds"]))
    return sorted((packages[key] for key in seen), key=lambda p: (p["name"], p["version"]))


def notice_files(directory):
    for path in sorted(directory.rglob("*")):
        relative = path.relative_to(directory)
        if {"target", ".git"}.intersection(relative.parts) or not path.is_file():
            continue
        name = path.name.lower()
        if (name.startswith(("license", "licence", "copying", "notice", "copyright", "authors"))
                or name in ("ofl.txt", "ufl.txt") or "license" in name):
            if path.suffix not in (".rs", ".py", ".sh"):
                yield path


def copy_notice(source, destination):
    if destination.exists() and destination.read_bytes() != source.read_bytes():
        raise ValueError(f"Conflicting notice: {destination.name}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)


def collect(product, destination, metadata=None):
    """Write LICENSE, THIRD_PARTY.md, licenses/, and dependency-licenses.json.

    The inventory includes normal/build dependencies across targets. It can contain
    platform-specific packages not linked into a particular platform's binary.
    No development-only dependencies or private source paths are exported.
    """
    if metadata is None:
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT, text=True))
    destination = Path(destination)
    product_root = ROOT / "plugins" / product.key
    copy_notice(product_root / "LICENSE", destination / "LICENSE")
    copy_notice(product_root / "THIRD_PARTY.md", destination / "THIRD_PARTY.md")
    overrides = json.loads((ROOT / "licenses/upstream.json").read_text())
    provenance = {}
    inventory = []
    for package in dependency_packages(metadata, product.package):
        directory = Path(package["manifest_path"]).resolve().parent
        if package["source"] is None and not directory.is_relative_to((ROOT / "vendor").resolve()):
            continue
        key = f"{package['name']}-{package['version']}"
        target = destination / "licenses" / key
        notices = list(notice_files(directory))
        names = []
        for source in notices:
            relative = source.relative_to(directory)
            copy_notice(source, target / relative)
            names.append((Path("licenses") / key / relative).as_posix())
        if not notices:
            records = overrides.get(key)
            if not records:
                raise ValueError(f"Missing upstream license notices for {key}; add a reviewed entry to licenses/upstream.json")
            provenance[key] = records
            for record in records:
                source = ROOT / record["path"]
                if hashlib.sha256(source.read_bytes()).hexdigest() != record["sha256"]:
                    raise ValueError(f"Changed upstream notice: {record['path']}")
                copy_notice(source, target / source.name)
                names.append((Path("licenses") / key / source.name).as_posix())
        inventory.append({"name": package["name"], "version": package["version"],
                          "license": package["license"], "repository": package["repository"],
                          "notices": names})
    copy_notice(ROOT / "crates/oiko-ui/assets/Ubuntu-Font-Licence-1.0.txt",
                destination / "licenses/Ubuntu-Font-Licence-1.0.txt")
    if product.key in ("inton", "weft"):
        mts_notice = product_root / ("LICENSES/MTS-ESP.txt" if product.key == "weft" else "vendor/mts-esp/LICENSE")
        copy_notice(mts_notice, destination / "licenses/mts-esp/LICENSE")
    if product.key == "inton":
        copy_notice(ROOT / "vendor/tuning-library/LICENSE.md", destination / "licenses/tuning-library/LICENSE.md")
    (destination / "dependency-licenses.json").write_text(json.dumps(inventory, indent=2) + "\n")
    (destination / "licenses/upstream-sources.json").write_text(json.dumps(provenance, indent=2) + "\n")
    return inventory


if __name__ == "__main__":
    import argparse
    from products import products

    registry = products()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("product", choices=registry)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    collect(registry[args.product], args.output)
