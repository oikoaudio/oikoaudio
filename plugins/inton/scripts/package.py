#!/usr/bin/env python3
"""Package native plug-ins, the official MTS runtime and installation notes."""

import argparse
import os
from pathlib import Path, PurePosixPath
import stat
import subprocess
import sys
import tempfile
import tarfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parent.parent
WORKSPACE = ROOT.parent.parent
sys.path.insert(0, str(WORKSPACE / "scripts"))
from notices import collect as collect_notices
from products import products
PLUGINS = ("Oiko Inton.clap", "Oiko Inton.vst3")
RUNTIMES = {
    "Linux": "vendor/mts-esp/libMTS/Linux/x86_64/libMTS.so",
    "macOS": "vendor/mts-esp/libMTS/Mac/x86_64_ARM/libMTSMac_v1.03.pkg",
    "Windows": "vendor/mts-esp/libMTS/Win/libMTSWin_v1.03.exe",
}
INSTALL = {
    "Linux": """Copy Oiko Inton.clap to ~/.clap/ and Oiko Inton.vst3 to ~/.vst3/.
For the MTS runtime, open a terminal in this extracted folder and run:
  bash scripts/install-mts.sh
The editor uses X11/XWayland, including on a Wayland desktop.""",
    "macOS": """Copy Oiko Inton.clap to ~/Library/Audio/Plug-Ins/CLAP/ and
Oiko Inton.vst3 to ~/Library/Audio/Plug-Ins/VST3/. Keep each bundle intact.
For the MTS runtime, double-click Install MTS-ESP.command.
This universal build supports Apple Silicon and Intel Macs.""",
    "Windows": """Copy Oiko Inton.clap to %LOCALAPPDATA%\\Programs\\Common\\CLAP\\ and
Oiko Inton.vst3 to %LOCALAPPDATA%\\Programs\\Common\\VST3\\.
Keep each bundle intact and add these folders to your DAW's scan paths if needed.
For the MTS runtime, double-click Install MTS-ESP.cmd.
Windows has not yet been manually tested in a DAW.""",
}


def entry(name, mode=stat.S_IFREG | 0o644):
    info = zipfile.ZipInfo(name)
    info.create_system = 3
    info.external_attr = mode << 16
    info.compress_type = zipfile.ZIP_DEFLATED
    return info


def add_path(archive, path, name):
    if path.is_symlink():
        archive.writestr(entry(name, stat.S_IFLNK | 0o777), os.readlink(path))
    elif path.is_dir():
        archive.writestr(entry(name.rstrip("/") + "/", stat.S_IFDIR | 0o755), b"")
        for child in sorted(path.iterdir()):
            add_path(archive, child, f"{name}/{child.name}")
    else:
        archive.write(path, name)


def plugin_member(name):
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError(f"Unsafe archive member: {name}")
    return bool(path.parts) and path.parts[0] in (*PLUGINS, "__MACOSX")


def add_plugins(archive, source):
    if source is None:
        for plugin in PLUGINS:
            path = WORKSPACE / "target/bundled" / plugin
            if not path.exists():
                raise FileNotFoundError(path)
            add_path(archive, path, plugin)
    elif zipfile.is_zipfile(source):
        # Preserve macOS resource entries, executable modes and symbolic links.
        with zipfile.ZipFile(source) as incoming:
            for info in incoming.infolist():
                if plugin_member(info.filename):
                    archive.writestr(info, incoming.read(info))
    else:
        with tarfile.open(source) as incoming:
            for member in incoming:
                name = PurePosixPath(member.name).as_posix()
                if not plugin_member(name):
                    continue
                if member.isdir():
                    archive.writestr(entry(name + "/", stat.S_IFDIR | member.mode), b"")
                elif member.issym():
                    archive.writestr(entry(name, stat.S_IFLNK | member.mode), member.linkname)
                elif member.isfile():
                    with incoming.extractfile(member) as data:
                        archive.writestr(entry(name, stat.S_IFREG | member.mode), data.read())
                else:
                    raise ValueError(f"Unsupported archive member: {name}")
    for plugin in PLUGINS:
        if not any(PurePosixPath(name).parts[0] == plugin for name in archive.namelist()):
            raise ValueError(f"Missing bundle: {plugin}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=INSTALL, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--input-archive", type=Path)
    parser.add_argument("--revision")
    args = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    revision = args.revision or subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    notes = f"""Oiko Inton {version}
Source revision: {revision}

Close your DAW before installation. Extract this entire ZIP first.

{INSTALL[args.platform]}

The runtime installer preserves an existing MTS-ESP installation. If you already
use MTS-ESP Mini or another MTS master, the runtime may already be installed.
The official ODDsound runtime is included unchanged, with its license.
Restart your DAW after installation and rescan plug-ins if necessary.

Inton does not generate sound. Load an MTS-ESP-compatible instrument and select
a scale in Inton. Only one MTS master should be active at a time.

Try scale editing, Set position automation, Morph, and saving/reopening a
project. When reporting issues, include OS, DAW/version, plugin format,
receiving instrument and steps to reproduce.
"""
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.output.with_suffix(".zip.tmp")
    try:
        with zipfile.ZipFile(temporary, "w", compression=zipfile.ZIP_DEFLATED,
                             strict_timestamps=False) as archive:
            add_plugins(archive, args.input_archive)
            archive.writestr(entry("README.txt"), notes)
            with tempfile.TemporaryDirectory(prefix="inton-notices-") as notice_dir:
                collect_notices(products()["inton"], Path(notice_dir))
                for path in sorted(Path(notice_dir).iterdir()):
                    add_path(archive, path, path.name)
            for name in ("vendor/mts-esp/LICENSE", RUNTIMES[args.platform]):
                add_path(archive, ROOT / name, name)
            installer = "scripts/install-mts.ps1" if args.platform == "Windows" else "scripts/install-mts.sh"
            add_path(archive, ROOT / installer, installer)
            if args.platform == "macOS":
                archive.writestr(entry("Install MTS-ESP.command", stat.S_IFREG | 0o755),
                    '#!/bin/bash\ncd "$(dirname "$0")"\nbash scripts/install-mts.sh\nstatus=$?\nread -r -p "Press Return to close."\nexit "$status"\n')
            elif args.platform == "Windows":
                archive.writestr(entry("Install MTS-ESP.cmd"),
                    '@echo off\r\npowershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\\install-mts.ps1"\r\nset "inton_status=%errorlevel%"\r\npause\r\nexit /b %inton_status%\r\n')
        with zipfile.ZipFile(temporary) as archive:
            if archive.testzip() is not None:
                raise ValueError("Archive integrity check failed")
        temporary.replace(args.output)
    finally:
        temporary.unlink(missing_ok=True)
    print(f"Packaged {args.output}")


if __name__ == "__main__":
    main()
