#!/usr/bin/env python3
"""Render the GRAFT mark to the PNG sizes Tauri expects.

Source of truth: assets/graft-mark.svg. Uses rsvg-convert when available,
otherwise falls back to ImageMagick, and finally errors with instructions.
"""
from __future__ import annotations

import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE = ROOT / "assets" / "graft-mark.svg"
OUT = ROOT / "src-tauri" / "icons"

SIZES = {
    "32x32.png": 32,
    "128x128.png": 128,
    "128x128@2x.png": 256,
    "icon.png": 1024,
}


def render(size: int, target: pathlib.Path) -> None:
    if shutil.which("rsvg-convert"):
        subprocess.run(
            ["rsvg-convert", "-w", str(size), "-h", str(size), str(SOURCE), "-o", str(target)],
            check=True,
        )
        return
    if shutil.which("magick"):
        subprocess.run(
            ["magick", "-background", "none", str(SOURCE), "-resize", f"{size}x{size}", str(target)],
            check=True,
        )
        return
    sys.exit("Install rsvg-convert (librsvg) or ImageMagick to render the icon.")


def main() -> None:
    if not SOURCE.exists():
        sys.exit(f"Missing icon source: {SOURCE}")
    OUT.mkdir(parents=True, exist_ok=True)
    for name, size in SIZES.items():
        render(size, OUT / name)
        print(f"wrote {OUT / name} ({size}px)")


if __name__ == "__main__":
    main()
