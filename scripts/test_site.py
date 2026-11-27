"""Checks the landing page in site/ (plan 0006).

Run: python3 -I -m unittest discover -s scripts

Why: the page is published by GitHub Pages for the users of the
screensaver. It must load nothing from other sites (N-02), link to the
developers only through the GitHub icon (F-03), offer the latest release
(F-02), have both languages (F-04), stop moving for people who ask for
less motion (F-05), and use the pictures that scripts/icon/make_art.py
makes from the sprite sheets (N-03). Hidden features are checked in the
review, not here: a list of their words would itself record them.
"""

import html.parser
import pathlib
import re
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
SITE = ROOT / "site"
sys.path.insert(0, str(ROOT / "scripts" / "icon"))

import make_art  # noqa: E402

REPO = "https://github.com/suchi/flying-microwave"
LATEST = REPO + "/releases/latest"
PAGES = {"ja": SITE / "index.html", "en": SITE / "en" / "index.html"}


class Page(html.parser.HTMLParser):
    """Collects the tags, attributes and the lang of a page."""

    def __init__(self):
        super().__init__()
        self.tags = []
        self.lang = None

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        self.tags.append((tag, attrs))
        if tag == "html":
            self.lang = attrs.get("lang")


def parse(path):
    page = Page()
    page.feed(path.read_text(encoding="utf-8"))
    return page


class SiteTest(unittest.TestCase):
    def test_both_languages_exist_and_link_to_each_other(self):
        for lang, path in PAGES.items():
            self.assertTrue(path.is_file(), f"{path} がない")
            self.assertEqual(parse(path).lang, lang, f"{path} の html の lang")
        ja_links = [a.get("href") for t, a in parse(PAGES["ja"]).tags if t == "a"]
        en_links = [a.get("href") for t, a in parse(PAGES["en"]).tags if t == "a"]
        self.assertIn("en/", ja_links, "日本語のページから英語のページへのリンクがない")
        self.assertIn("../", en_links, "英語のページから日本語のページへのリンクがない")

    def test_nothing_is_loaded_from_another_site_and_no_script_runs(self):
        for path in PAGES.values():
            for tag, attrs in parse(path).tags:
                self.assertNotEqual(tag, "script", f"{path} に script がある")
                self.assertNotIn(tag, ("iframe", "object", "embed"), f"{path} に {tag} がある")
                for name in ("src", "srcset", "poster", "data"):
                    value = attrs.get(name) or ""
                    self.assertNotRegex(value, r"^(https?:)?//", f"{path} の {tag} の {name}")
                if tag == "link":
                    self.assertNotRegex(attrs.get("href") or "", r"^(https?:)?//", f"{path} の link")
                self.assertFalse(
                    any(k.startswith("on") for k in attrs), f"{path} の {tag} にイベントの属性がある"
                )
        for css in SITE.rglob("*.css"):
            text = css.read_text(encoding="utf-8")
            self.assertNotIn("@import", text, css)
            self.assertNotRegex(text, r"url\(\s*['\"]?(https?:)?//", css)

    def test_links_to_other_sites_are_only_the_repository_and_the_latest_release(self):
        for path in PAGES.values():
            hrefs = [a.get("href") or "" for t, a in parse(path).tags if t == "a"]
            outside = [h for h in hrefs if re.match(r"^(https?:)?//", h)]
            self.assertEqual(sorted(set(outside)), sorted({REPO, LATEST}), path)

    def test_every_local_link_and_picture_exists(self):
        for path in PAGES.values():
            for tag, attrs in parse(path).tags:
                for name in ("href", "src"):
                    value = attrs.get(name)
                    if not value or re.match(r"^([a-z]+:|//|#)", value):
                        continue
                    target = (path.parent / value.split("#")[0]).resolve()
                    if value.endswith("/"):
                        target = target / "index.html"
                    self.assertTrue(target.is_file(), f"{path} の {value} がない")
        for css in SITE.rglob("*.css"):
            for value in re.findall(r"url\(\s*['\"]?([^'\")]+)", css.read_text(encoding="utf-8")):
                if not value.startswith(("data:", "#")):
                    self.assertTrue((css.parent / value).resolve().is_file(), f"{css} の {value} がない")

    def test_motion_stops_for_reduced_motion(self):
        text = (SITE / "style.css").read_text(encoding="utf-8")
        m = re.search(r"@media\s*\(prefers-reduced-motion:\s*reduce\)\s*\{(.*?)\n\}", text, re.S)
        self.assertIsNotNone(m, "prefers-reduced-motion: reduce の指定がない")
        self.assertRegex(m.group(1), r"animation\s*:\s*none")

    def test_the_pictures_are_the_ones_make_art_makes(self):
        with tempfile.TemporaryDirectory() as d:
            made = pathlib.Path(d)
            names = make_art.write_site(ROOT, made)
            self.assertTrue(names)
            for name in names:
                self.assertEqual(
                    (made / name).read_text(encoding="utf-8"),
                    (SITE / "img" / name).read_text(encoding="utf-8"),
                    f"site/img/{name} が古い。python3 -I scripts/icon/make_art.py を実行する",
                )


if __name__ == "__main__":
    unittest.main()
