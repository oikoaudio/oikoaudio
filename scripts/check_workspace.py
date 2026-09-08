#!/usr/bin/env python3
"""Check shared dependencies and thread boundaries; optionally run the test suites."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
from products import products

ROOT = Path(__file__).resolve().parents[1]


def run(*args, cwd=ROOT, capture=False):
    return subprocess.run(args, cwd=cwd, check=True, text=True,
                          stdout=subprocess.PIPE if capture else None).stdout


def check(flags):
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    for name in ("nice-plug", "nice-plug-core", "egui-baseview", "baseview"):
        source = manifest["patch"]["crates-io"][name]["path"]
        if (ROOT / source).resolve() != ROOT / "vendor" / name:
            raise SystemExit(f"{name} must use the common vendor source")
    for path, digest in json.loads((ROOT / "vendor-checksums.json").read_text()).items():
        source = ROOT / path
        if not source.is_file() or hashlib.sha256(source.read_bytes()).hexdigest() != digest:
            raise SystemExit(f"Unrecorded vendor change: {path}")
    for path in (ROOT / "plugins").glob("*/vendor/oikoaudio-plugin-kit"):
        raise SystemExit(f"Remove duplicated kit sources: {path}")
    metadata = json.loads(run("cargo", "metadata", *flags, "--no-deps", "--format-version", "1", capture=True))
    members = set(metadata["workspace_members"])
    packages = {p["name"]: p for p in metadata["packages"] if p["id"] in members}
    roles = workspace_roles(packages)
    registry = products()
    registered = {product.package for product in registry.values()}
    if {name for name, config in roles.items() if config["role"] == "plugin"} != registered:
        raise SystemExit("Every hosted plugin must appear in products.toml and declare the plugin role")
    for name, config in roles.items():
        if "product" in config and config["product"] not in registry:
            raise SystemExit(f"{name}: unknown product {config['product']}")
    for key, product in registry.items():
        if roles[product.package].get("product") != key:
            raise SystemExit(f"{product.package}: declare product = {key!r} in package.metadata.oiko")
    trees = {
        name: run("cargo", "tree", *flags, "-p", name, "--all-features",
                  "--edges", "normal,build", "--prefix", "none", capture=True)
        for name in packages
    }
    check_dependency_boundaries(roles, trees)
    print("Vendor sources and every workspace member's dependency boundaries verified.", flush=True)


# Roles describe dependencies, not product functionality. Integrations are opt-in
# for each consumer, including transitive consumers. A shared crate cannot acquire
# a product dependency; one product cannot silently acquire another product's core.
ALLOWED_ROLES = {
    "core": {"core", "integration"},
    "integration": {"core", "integration"},
    "ui": {"core", "ui"},
    "host": {"core", "ui", "host"},
    "plugin": {"core", "integration", "ui", "host"},
    "tool": {"core", "integration", "ui", "host", "plugin", "tool"},
}
GUI_OR_HOST = {"egui", "nice-plug", "nice-plug-core", "nice-plug-egui", "egui-baseview", "baseview"}


def workspace_roles(packages):
    roles = {}
    for name, package in packages.items():
        config = (package.get("metadata") or {}).get("oiko", {})
        if config.get("role") not in ALLOWED_ROLES:
            raise SystemExit(f"{name}: declare package.metadata.oiko.role in Cargo.toml")
        if not isinstance(config.get("integrations", []), list) or not all(
            isinstance(value, str) for value in config.get("integrations", [])
        ):
            raise SystemExit(f"{name}: integrations must be an array of workspace package names")
        if "product" in config and not isinstance(config["product"], str):
            raise SystemExit(f"{name}: product must be a string")
        roles[name] = config
    for name, config in roles.items():
        for integration in config.get("integrations", []):
            if roles.get(integration, {}).get("role") != "integration":
                raise SystemExit(f"{name}: unknown integration {integration}")
    return roles


def check_dependency_boundaries(roles, trees):
    for name, config in roles.items():
        dependencies = {line.split()[0] for line in trees[name].splitlines() if line.strip()} - {name}
        role = config["role"]
        if role in {"core", "integration"} and dependencies & GUI_OR_HOST:
            raise SystemExit(f"GUI or host dependency crossed the {name} {role} boundary")
        if role == "ui" and dependencies & (GUI_OR_HOST - {"egui"}):
            raise SystemExit(f"Host dependency crossed the {name} UI boundary")
        for dependency in dependencies & roles.keys():
            target = roles[dependency]
            if target["role"] not in ALLOWED_ROLES[role]:
                raise SystemExit(f"{name} ({role}) cannot depend on {dependency} ({target['role']})")
            if target.get("product") and target["product"] != config.get("product"):
                raise SystemExit(f"{name} cannot depend on another product's {dependency}")
            if target["role"] == "integration" and dependency not in config.get("integrations", []):
                raise SystemExit(f"{name} must explicitly opt into the {dependency} integration")


def test_vendor(flags):
    for name in ("nice-plug-core", "egui-baseview"):
        with tempfile.TemporaryDirectory(prefix="oiko-vendor-test-") as temporary:
            package = Path(temporary) / name
            shutil.copytree(ROOT / "vendor" / name, package, ignore=shutil.ignore_patterns("target"))
            # Resolve a temporary lockfile without modifying the recorded vendor tree.
            options = ["--offline"] if "--offline" in flags else []
            if name == "egui-baseview":
                shutil.copyfile(ROOT / "Cargo.lock", package / "Cargo.lock")
                options += ["--no-default-features", "--features", "opengl", "--config",
                            f'patch.crates-io.baseview.path="{(ROOT / "vendor/baseview").as_posix()}"']
            run("cargo", "test", "--lib", *options, cwd=package)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test", action="store_true")
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    flags = ["--locked"] + (["--offline"] if args.offline else [])
    check(flags)
    if args.test:
        run(sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests")
        run("cargo", "fmt", "--all", "--", "--check")
        run("cargo", "clippy", "--workspace", "--all-targets", *flags, "--", "-D", "warnings")
        run("cargo", "test", "--workspace", *flags)
        # Individual selection catches dependencies hidden by workspace feature unification.
        for package in (product.package for product in products().values()):
            run("cargo", "test", "-p", package, *flags)
        test_vendor(flags)


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        sys.exit(error.returncode)
