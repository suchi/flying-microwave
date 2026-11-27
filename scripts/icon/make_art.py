"""Makes the README banner (docs/assets/banner.svg), the app icon
(crates/app/res/icon.svg) and the pictures of the landing page (site/img;
plan 0006) from the sprite sheets in assets/sprites.

Each face of a sheet is cut out with a nested <svg viewBox> and mapped onto a
parallelogram, so that the microwaves look like the boxes the screensaver
draws, seen from the front, the right and the top (plan 0005, task 11, F-10,
D-40). The ids of each sheet are prefixed so that the sheets can live in one
file. Only the Python standard library is used. Messages are in Japanese
because the maintainer reads them (AGENTS.md). After running this, make the
.ico again with scripts/icon/make_ico.py. Usage (from the repository root):

    python3 -I scripts/icon/make_art.py
"""

import math
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]

BW, BH, BD = 240.0, 160.0, 210.0
# Copied from crates/app/src/model.rs (BODY_SVG, DOOR_RECT, OPENING_RECT,
# LED_RECT and Face::image); scripts/test_make_art.py checks that they match.
DOOR = (10.0, 10.0, 156.0, 140.0)
OPENING = (24.0, 24.0, 128.0, 112.0)
LED = (178.0, 18.0, 48.0, 22.0)
FACES = {
    "front": (0.0, 0.0, BW, BH),
    "side": (500.0, 0.0, BD, BH),
    "top": (220.0, 220.0, BW, BD),
}
CAVITY = (0.0, 0.0, 128.0, 128.0)
POPCORN_FRONT = (0.0, 202.0, 192.0, 256.0)
BENTO_TOP = (0.0, 0.0, 256.0, 192.0)
# The door and LED faces are cut from the origin of their sheets (model.rs:
# Face::DoorOuter and Face::Led).
DOOR_OUTER = (0.0, 0.0, DOOR[2], DOOR[3])
LED_FACE = (0.0, 0.0, LED[2], LED[3])


def sheet(root, name, prefix):
    """The inner markup of a sprite sheet, with its ids prefixed."""
    text = (root / "assets" / "sprites" / name).read_text(encoding="utf-8")
    body = re.sub(r"(?s)^.*?<svg[^>]*>", "", text, count=1)
    body = re.sub(r"(?s)</svg>\s*$", "", body)
    body = re.sub(r"(?s)<!--.*?-->", "", body)
    body = re.sub(r'id="([^"]+)"', lambda m: f'id="{prefix}-{m.group(1)}"', body)
    body = re.sub(r"url\(#([^)]+)\)", lambda m: f"url(#{prefix}-{m.group(1)})", body)
    body = re.sub(r'href="#([^"]+)"', lambda m: f'href="#{prefix}-{m.group(1)}"', body)
    return f'<g id="{prefix}">{body}</g>'


def face(sheet_id, rect, matrix):
    x, y, w, h = rect
    a, b, c, d, e, f = matrix
    return (
        f'<g transform="matrix({a:.4f} {b:.4f} {c:.4f} {d:.4f} {e:.2f} {f:.2f})">'
        f'<svg x="0" y="0" width="{w}" height="{h}" viewBox="{x} {y} {w} {h}" overflow="hidden">'
        f'<use href="#{sheet_id}"/></svg></g>'
    )


def shade(points, opacity):
    pts = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    return f'<polygon points="{pts}" fill="#000" fill-opacity="{opacity}"/>'


def microwave(color, scale, tilt_deg, ox, oy):
    """A microwave seen from the front, right and top, as SVG markup.
    (ox, oy) is the top left corner of the front face on the canvas."""
    t = math.radians(tilt_deg)
    rot = lambda vx, vy: (vx * math.cos(t) - vy * math.sin(t), vx * math.sin(t) + vy * math.cos(t))
    X = rot(scale * 1.0, scale * 0.10)  # width of the front
    Y = rot(scale * -0.06, scale * 1.0)  # height
    Z = rot(scale * 0.42, scale * -0.30)  # depth, toward the back right
    O = (ox, oy)
    p = lambda u, v, w: (O[0] + X[0] * u + Y[0] * v + Z[0] * w, O[1] + X[1] * u + Y[1] * v + Z[1] * w)
    out = []
    body = f"body-{color}"
    # Top face: local x along the width, local y from the back (0) to the front (BD).
    top_o = p(0, 0, BD)
    out.append(face(body, FACES["top"], (X[0], X[1], -Z[0], -Z[1], top_o[0], top_o[1])))
    out.append(shade([p(0, 0, 0), p(BW, 0, 0), p(BW, 0, BD), p(0, 0, BD)], 0.05))
    # Right side: local x from the front (0) to the back (BD).
    side_o = p(BW, 0, 0)
    out.append(face(body, FACES["side"], (Z[0], Z[1], Y[0], Y[1], side_o[0], side_o[1])))
    out.append(shade([p(BW, 0, 0), p(BW, 0, BD), p(BW, BH, BD), p(BW, BH, 0)], 0.28))
    # Front face, then the lit cavity behind the door window, then the door and the LED.
    out.append(face(body, FACES["front"], (X[0], X[1], Y[0], Y[1], O[0], O[1])))
    dx, dy = DOOR[:2]
    ox_, oy_, ow, oh = OPENING
    cav = p(ox_, oy_, 0)
    cw, ch = CAVITY[2], CAVITY[3]
    out.append(face("interior", CAVITY, (X[0] * ow / cw, X[1] * ow / cw, Y[0] * oh / ch, Y[1] * oh / ch, cav[0], cav[1])))
    door_o = p(dx, dy, 0)
    out.append(face("door", DOOR_OUTER, (X[0], X[1], Y[0], Y[1], door_o[0], door_o[1])))
    lx, ly = LED[:2]
    led_o = p(lx, ly, 0)
    out.append(face("led", LED_FACE, (X[0], X[1], Y[0], Y[1], led_o[0], led_o[1])))
    return "\n".join(out)


def food(rect, scale, tilt_deg, ox, oy):
    t = math.radians(tilt_deg)
    a, b = scale * math.cos(t), scale * math.sin(t)
    return face("food", rect, (a, b, -b, a, ox, oy))


def defs(root, colors):
    parts = [sheet(root, f"microwave-body-{c}.svg", f"body-{c}") for c in colors]
    parts.append(sheet(root, "microwave-door.svg", "door"))
    parts.append(sheet(root, "microwave-interior.svg", "interior"))
    parts.append(sheet(root, "microwave-led.svg", "led"))
    parts.append(sheet(root, "food.svg", "food"))
    return "<defs>" + "\n".join(parts) + "</defs>"


def stars(width, height, count, seed):
    out, s = [], seed
    for _ in range(count):
        s = (s * 1103515245 + 12345) % 2**31
        x = s % width
        s = (s * 1103515245 + 12345) % 2**31
        y = s % height
        s = (s * 1103515245 + 12345) % 2**31
        r = 0.6 + (s % 10) / 10
        out.append(f'<circle cx="{x}" cy="{y}" r="{r:.1f}" fill="#cfd8ff" fill-opacity="0.55"/>')
    return "\n".join(out)


def banner(root):
    W, H = 1280, 360
    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">',
        "<!-- Made by scripts/icon/make_art.py from assets/sprites. Do not edit by hand. -->",
        defs(root, ["silver", "graphite", "white", "mint"]),
        '<linearGradient id="sky" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#060812"/><stop offset="1" stop-color="#141a3a"/></linearGradient>',
        f'<rect width="{W}" height="{H}" fill="url(#sky)"/>',
        stars(W, H, 60, 7),
        # Far to near, along the flight from the upper right to the lower left.
        microwave("mint", 0.26, -16, 1150, 22),
        food(POPCORN_FRONT, 0.20, -18, 1205, 150),  # popcorn bag
        microwave("graphite", 0.40, 12, 1040, 230),
        food(BENTO_TOP, 0.20, 14, 960, 40),  # bento
        microwave("silver", 0.56, -8, 800, 110),
        '<g font-family="\'Segoe UI\', \'Helvetica Neue\', Arial, sans-serif">',
        '<text x="88" y="168" font-size="76" font-weight="800" fill="#f4f6fb" letter-spacing="-1">Flying Microwave</text>',
        '<text x="92" y="246" font-size="24" font-weight="400" fill="#b9c2d9">A screensaver for Windows where microwaves take flight</text>',
        "</g>",
        "</svg>",
    ]
    return "\n".join(parts)


def icon(root):
    parts = [
        '<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256">',
        "<!-- Made by scripts/icon/make_art.py from assets/sprites. Do not edit by hand. -->",
        defs(root, ["silver"]),
        '<linearGradient id="sky" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#0d1430"/><stop offset="1" stop-color="#2a1d66"/></linearGradient>',
        '<rect x="8" y="8" width="240" height="240" rx="48" fill="url(#sky)"/>',
        microwave("silver", 0.56, -10, 30, 104),
        "</svg>",
    ]
    return "\n".join(parts)


def write(root, out):
    """Writes banner.svg and icon.svg into the folder `out`."""
    out.mkdir(parents=True, exist_ok=True)
    (out / "banner.svg").write_text(banner(root) + "\n", encoding="utf-8")
    (out / "icon.svg").write_text(icon(root) + "\n", encoding="utf-8")


SITE_COLORS = ["silver", "graphite", "white", "mint"]


def standalone(root, colors, width, height, minx, miny, body):
    """One picture with a transparent background, cut to its bounding box."""
    return "\n".join([
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0f}" height="{height:.0f}" '
        f'viewBox="{minx:.2f} {miny:.2f} {width:.2f} {height:.2f}">',
        "<!-- Made by scripts/icon/make_art.py from assets/sprites. Do not edit by hand. -->",
        defs(root, colors),
        body,
        "</svg>",
    ])


def site_microwave(root, color):
    """A microwave for the landing page (plan 0006), seen as in the banner."""
    # The corners of the box, with the same axes as microwave() at scale 1
    # and no tilt; the viewBox is their bounding box.
    X, Y, Z = (1.0, 0.10), (-0.06, 1.0), (0.42, -0.30)
    xs, ys = [], []
    for u in (0, BW):
        for v in (0, BH):
            for w in (0, BD):
                xs.append(X[0] * u + Y[0] * v + Z[0] * w)
                ys.append(X[1] * u + Y[1] * v + Z[1] * w)
    minx, miny = min(xs), min(ys)
    return standalone(root, [color], max(xs) - minx, max(ys) - miny, minx, miny, microwave(color, 1.0, 0, 0, 0))


def write_site(root, out):
    """Writes the pictures of the landing page into `out`; returns their names.
    Only microwaves: the release the page offers (v0.2.0) has no flying food
    (plan 0006, F-01)."""
    out.mkdir(parents=True, exist_ok=True)
    names = []
    for color in SITE_COLORS:
        name = f"microwave-{color}.svg"
        (out / name).write_text(site_microwave(root, color) + "\n", encoding="utf-8")
        names.append(name)
    return names


def main():
    made = ROOT / "target" / "make-art"
    write(ROOT, made)
    (ROOT / "docs" / "assets" / "banner.svg").write_text((made / "banner.svg").read_text(encoding="utf-8"), encoding="utf-8")
    (ROOT / "crates" / "app" / "res" / "icon.svg").write_text((made / "icon.svg").read_text(encoding="utf-8"), encoding="utf-8")
    for name in write_site(ROOT, made / "site"):
        (ROOT / "site" / "img").mkdir(parents=True, exist_ok=True)
        (ROOT / "site" / "img" / name).write_text((made / "site" / name).read_text(encoding="utf-8"), encoding="utf-8")
    print("docs/assets/banner.svg、crates/app/res/icon.svg、site/img/ の絵を作った。続けて python3 -I scripts/icon/make_ico.py で .ico を作り直す。")


if __name__ == "__main__":
    main()
