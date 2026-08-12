#!/usr/bin/env python3
"""Download Tauri sidecar binaries (typst/deno/uv) for the current platform.

Tauri `externalBin` expects files named `<name>-<target-triple>` (plus `.exe`
on Windows) inside `crates/galen/src-tauri/binaries/`. Run this before
`tauri build` on any platform.

Usage:
    python3 download_sidecars.py            # detect host target
    TARGET=x86_64-apple-darwin python3 ...  # explicit target (CI cross)
"""

import os
import platform
import shutil
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BIN_DIR = ROOT / "crates" / "galen" / "src-tauri" / "binaries"

TYST_VERSION = "0.13.1"


def detect_target() -> str:
    machine = platform.machine().lower()
    system = platform.system().lower()
    if system == "windows":
        return "x86_64-pc-windows-msvc"
    if system == "darwin":
        return "aarch64-apple-darwin" if machine in ("arm64", "aarch64") else "x86_64-apple-darwin"
    if system == "linux":
        return "x86_64-unknown-linux-gnu"
    raise SystemExit(f"unsupported platform: {system} {machine}")


def archive_mode(url: str) -> str:
    if url.endswith(".zip"):
        return "zip"
    if url.endswith(".tar.gz"):
        return "tar.gz"
    return "tar.xz"


def extract(exe_name: str, archive: Path, dest_dir: Path) -> Path:
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as zf:
            zf.extractall(dest_dir)
    elif archive.suffix == ".gz":
        with tarfile.open(archive, "r:gz") as tf:
            tf.extractall(dest_dir)
    else:
        with tarfile.open(archive, "r:xz") as tf:
            tf.extractall(dest_dir)
    hits = list(dest_dir.rglob(exe_name))
    if not hits:
        raise SystemExit(f"{exe_name} not found in {archive.name}")
    return hits[0]


def download(url: str, dest: Path) -> None:
    print(f"downloading {url}")
    urllib.request.urlretrieve(url, dest)


def jobs(target: str) -> list[dict]:
    suffix = ".exe" if "windows" in target else ""
    return [
        {
            "name": "typst",
            "version": f"v{TYST_VERSION}",
            "url": (
                f"https://github.com/typst/typst/releases/download/v{TYST_VERSION}/"
                f"typst-{target}.zip" if "windows" in target else
                f"https://github.com/typst/typst/releases/download/v{TYST_VERSION}/"
                f"typst-{target}.tar.xz"
            ),
            "exe": "typst.exe" if "windows" in target else "typst",
            "target": f"typst-{target}{suffix}",
        },
        {
            "name": "deno",
            "version": "latest",
            "url": f"https://github.com/denoland/deno/releases/latest/download/deno-{target}.zip",
            "exe": "deno.exe" if "windows" in target else "deno",
            "target": f"deno-{target}{suffix}",
        },
        {
            "name": "uv",
            "version": "latest",
            "url": (
                f"https://github.com/astral-sh/uv/releases/latest/download/uv-{target}.zip"
                if "windows" in target
                else f"https://github.com/astral-sh/uv/releases/latest/download/uv-{target}.tar.gz"
            ),
            "exe": "uv.exe" if "windows" in target else "uv",
            "target": f"uv-{target}{suffix}",
        },
    ]


def main() -> None:
    target = os.environ.get("TARGET", "").strip() or detect_target()
    BIN_DIR.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        for job in jobs(target):
            out = BIN_DIR / job["target"]
            if out.exists():
                print(f"skip (exists): {out.name}")
                continue
            mode = archive_mode(job["url"])
            archive = tmp / f"{job['name']}.{mode}"
            download(job["url"], archive)
            extract_dir = tmp / job["name"]
            extract_dir.mkdir(exist_ok=True)
            found = extract(job["exe"], archive, extract_dir)
            shutil.copy2(found, out)
            print(f"installed {out.name} ({out.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
