"""Builds crates/app/res/flying-microwave.ico from crates/app/res/icon.svg.

Each size is rendered from the SVG with the resvg command (ADR-0008, the
same renderer as the artwork in Phase 3) instead of being scaled down from
one large image, so that small sizes stay sharp. The images are stored in
the .ico as PNG, which Windows Vista and later read. Only the Python
standard library is used. Messages are in Japanese because the maintainer
reads them (AGENTS.md). Usage (from the repository root):

    python3 -I scripts/icon/make_ico.py
"""

import pathlib
import shutil
import struct
import subprocess
import sys
import tempfile

SIZES = [16, 20, 24, 32, 40, 48, 64, 256]
RESVG_VERSION = "0.49.0"
ROOT = pathlib.Path(__file__).resolve().parents[2]
SVG = ROOT / "crates/app/res/icon.svg"
ICO = ROOT / "crates/app/res/flying-microwave.ico"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def png_size(data: bytes) -> tuple[int, int]:
    """Width and height from the IHDR chunk, which always comes first."""
    if data[:8] != PNG_SIGNATURE or data[12:16] != b"IHDR":
        raise ValueError("not a PNG file")
    return struct.unpack(">II", data[16:24])


def ico(images: list[bytes]) -> bytes:
    """An .ico file with one PNG image per entry."""
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = len(header) + 16 * len(images)
    entries, data = b"", b""
    for png in images:
        width, height = png_size(png)
        # A size of 256 is stored as 0.
        entries += struct.pack(
            "<BBBBHHII", width % 256, height % 256, 0, 0, 1, 32, len(png), offset + len(data)
        )
        data += png
    return header + entries + data


def main() -> int:
    resvg = shutil.which("resvg")
    if resvg is None:
        print(f"resvgが入っていない: cargo install --locked resvg --version {RESVG_VERSION}", file=sys.stderr)
        return 1
    version = subprocess.run([resvg, "--version"], check=True, capture_output=True, text=True).stdout.strip()
    if version != RESVG_VERSION:
        print(f"resvgの版が{version}になっている({RESVG_VERSION}が要る)", file=sys.stderr)
        return 1
    images = []
    with tempfile.TemporaryDirectory() as tmp:
        for size in SIZES:
            png = pathlib.Path(tmp) / f"icon-{size}.png"
            subprocess.run([resvg, "-w", str(size), "-h", str(size), str(SVG), str(png)], check=True)
            data = png.read_bytes()
            if png_size(data) != (size, size):
                print(f"{png.name}の大きさが{png_size(data)}になっている({size}x{size}が要る)", file=sys.stderr)
                return 1
            images.append(data)
    ICO.write_bytes(ico(images))
    print(f"{ICO.relative_to(ROOT)}を書いた")
    return 0


if __name__ == "__main__":
    sys.exit(main())
