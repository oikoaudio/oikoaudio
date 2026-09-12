"""One registry for hosted product checks and releases; no build or side effects."""
from dataclasses import dataclass
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class Product:
    key: str
    package: str
    version: str
    bundle_name: str
    test_packages: tuple[str, ...]
    linux_archive: str
    notes: str | None
    notes_file: Path | None
    package_script: Path | None

    def check_tag(self, tag: str) -> None:
        expected = f"{self.key}/v{self.version}"
        if tag != expected:
            raise ValueError(f"tag {tag!r} does not match package version; expected {expected!r}")


def products() -> dict[str, Product]:
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    registry = tomllib.loads((ROOT / "products.toml").read_text())["products"]
    bundles = tomllib.loads((ROOT / "bundler.toml").read_text())
    result = {}
    for key, config in registry.items():
        package = tomllib.loads((ROOT / config["manifest"]).read_text())["package"]
        if package["version"] not in ({"workspace": True}, version):
            raise ValueError(f"{key}: plugin version must match workspace release {version}")
        result[key] = Product(
            key, package["name"], version, bundles[package["name"]]["name"],
            (package["name"], *config["test_packages"]), config["linux_archive"],
            config.get("notes"), ROOT / config["notes_file"] if "notes_file" in config else None,
            ROOT / config["package_script"] if "package_script" in config else None,
        )
    return result
