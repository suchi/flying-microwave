"""Checks that the README banner and the app icon are the ones that
scripts/icon/make_art.py makes from the sprite sheets.

Run: python3 -I -m unittest discover -s scripts

Why: the banner and the icon are composed from the faces of the sprite
sheets (plan 0005, task 11, F-10, D-40). When a sheet changes, the banner
and the icon must be made again, and this test fails until they are.
"""

import pathlib
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts" / "icon"))

import make_art  # noqa: E402


class MakeArtTest(unittest.TestCase):
    def test_the_committed_banner_and_icon_are_up_to_date(self):
        with tempfile.TemporaryDirectory() as d:
            make_art.write(ROOT, pathlib.Path(d))
            for made, committed in [
                ("banner.svg", ROOT / "docs" / "assets" / "banner.svg"),
                ("icon.svg", ROOT / "crates" / "app" / "res" / "icon.svg"),
            ]:
                self.assertEqual(
                    (pathlib.Path(d) / made).read_text(encoding="utf-8"),
                    committed.read_text(encoding="utf-8"),
                    f"{committed} is out of date; run python3 -I scripts/icon/make_art.py",
                )

    def test_every_face_reference_points_to_a_defined_sheet(self):
        text = make_art.banner(ROOT)
        for sheet in ["body-silver", "body-graphite", "body-mint", "door", "interior", "led", "food"]:
            self.assertIn(f'id="{sheet}"', text)
        # No reference to an outside file (GitHub shows the SVG as an image).
        self.assertNotIn("<image", text)
        self.assertNotIn('href="http', text)


if __name__ == "__main__":
    unittest.main()
