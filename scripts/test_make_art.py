"""Checks that the README banner and the app icon are the ones that
scripts/icon/make_art.py makes from the sprite sheets.

Run: python3 -I -m unittest discover -s scripts

Why: the banner and the icon are composed from the faces of the sprite
sheets (plan 0005, task 11, F-10, D-40). When a sheet changes, the banner
and the icon must be made again, and this test fails until they are.
"""

import pathlib
import re
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

    def test_the_face_positions_are_those_of_model_rs(self):
        # Review: the values are copied from crates/app/src/model.rs, so a
        # change of a face position there must fail here until the script
        # follows it.
        model = (ROOT / "crates" / "app" / "src" / "model.rs").read_text(encoding="utf-8")

        def const(name, n):
            m = re.search(rf"const {name}: \[f32; {n}\] = \[([^\]]+)\];", model)
            self.assertIsNotNone(m, name)
            return tuple(float(v) for v in m.group(1).split(","))

        def face(arm):
            m = re.search(rf"{re.escape(arm)} => [^\[\n]*\[([^\]]+)\]", model)
            self.assertIsNotNone(m, arm)
            bw, bh, bd = const("BODY_SVG", 3)
            values = m.group(1).replace("bw", str(bw)).replace("bh", str(bh)).replace("bd", str(bd))
            return tuple(float(v) for v in values.split(","))

        self.assertEqual((make_art.BW, make_art.BH, make_art.BD), const("BODY_SVG", 3))
        self.assertEqual(make_art.DOOR, const("DOOR_RECT", 4))
        self.assertEqual(make_art.OPENING, const("OPENING_RECT", 4))
        self.assertEqual(make_art.LED, const("LED_RECT", 4))
        self.assertEqual(make_art.FACES["front"], face("Face::Front(c)"))
        self.assertEqual(make_art.FACES["side"], face("Face::Side(c)"))
        self.assertEqual(make_art.FACES["top"], face("Face::Top(c)"))
        self.assertEqual(make_art.CAVITY, face("Face::Cavity"))
        self.assertEqual(make_art.POPCORN_FRONT, face("Face::PopcornFront"))
        self.assertEqual(make_art.BENTO_TOP, face("Face::BentoTop"))

    def test_every_id_is_unique_and_every_reference_is_defined(self):
        # Review: an id left without its prefix would make the sheets share
        # it (all four bodies define "body"), and the files would still match.
        for text in [make_art.banner(ROOT), make_art.icon(ROOT)]:
            ids = re.findall(r"\bid=[\"']([^\"']+)[\"']", text)
            self.assertEqual(len(ids), len(set(ids)), "an id is defined twice")
            refs = re.findall(r"url\(['\"]?#([^)'\"]+)['\"]?\)", text)
            refs += re.findall(r"href=[\"']#([^\"']+)[\"']", text)
            self.assertTrue(refs)
            for ref in refs:
                self.assertIn(ref, ids, f"#{ref} is not defined")


if __name__ == "__main__":
    unittest.main()
