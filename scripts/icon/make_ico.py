"""Builds crates/app/res/flying-microwave.ico from crates/app/res/icon.svg.

Each size is rendered from the SVG by Chromium (render_svg.js, Playwright)
instead of being scaled down from one large image, so that small sizes stay
sharp. Usage (from the repository root):

    python3 -I scripts/icon/make_ico.py
"""

import os
import pathlib
import subprocess
import sys
import tempfile

from PIL import Image

SIZES = [16, 20, 24, 32, 40, 48, 64, 256]
ROOT = pathlib.Path(__file__).resolve().parents[2]
SVG = ROOT / "crates/app/res/icon.svg"
ICO = ROOT / "crates/app/res/flying-microwave.ico"


def main() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        subprocess.run(
            ["node", str(ROOT / "scripts/icon/render_svg.js"), str(SVG), tmp, *map(str, SIZES)],
            check=True,
            env=os.environ,
        )
        images = {s: Image.open(f"{tmp}/icon-{s}.png").convert("RGBA") for s in SIZES}
        for s, image in images.items():
            if image.size != (s, s):
                print(f"icon-{s}.png is {image.size}, not {s}x{s}", file=sys.stderr)
                return 1
        images[256].save(
            ICO,
            format="ICO",
            sizes=[(s, s) for s in SIZES],
            append_images=[images[s] for s in SIZES if s != 256],
        )
    print(f"wrote {ICO.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
