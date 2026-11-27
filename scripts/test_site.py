"""Checks the landing page in site/ (plan 0006).

Run: python3 -I -m unittest discover -s scripts

Why: the page is published by GitHub Pages for the users of the
screensaver. It must load nothing from other sites (N-02), link to the
developers only through the GitHub icon (F-03), offer the latest release
(F-02), have both languages (F-04), stop moving for people who ask for
less motion (F-05), and use the pictures that scripts/icon/make_art.py
makes from the sprite sheets (N-03). Hidden features are checked in the
review, not here: a list of their words would itself record them.

The check of what may be loaded is an allow list (review of plan 0006,
M-2): only some file types, tags and attributes are accepted, and any
value that names another site or a scheme is refused, except the two links
to GitHub. Each way around it that the review found is tried on a copy of
the site, so that a hole that opens again fails here.
"""

import html.parser
import pathlib
import re
import shutil
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
SITE = ROOT / "site"
sys.path.insert(0, str(ROOT / "scripts" / "icon"))

import make_art  # noqa: E402

REPO = "https://github.com/suchi/flying-microwave"
LATEST = REPO + "/releases/latest"
PAGES = {"ja": pathlib.Path("index.html"), "en": pathlib.Path("en") / "index.html"}
FILE_TYPES = {".html", ".css", ".svg"}
TAGS = {
    "html", "head", "meta", "title", "link", "body", "main", "nav", "div",
    "h1", "p", "a", "img", "footer", "ul", "li", "code", "svg", "path",
}
ATTRS = {
    "lang", "charset", "name", "content", "rel", "href", "hreflang", "type",
    "class", "aria-label", "aria-hidden", "title", "src", "alt", "viewbox", "d", "fill",
}
META_NAMES = {"viewport", "description"}
LINK_RELS = {"icon", "stylesheet", "alternate"}
# A scheme (https:, javascript:, data: ...) or a path to another host (//, \\).
OUTSIDE = re.compile(r"^\s*([a-z][a-z0-9+.-]*:|[/\\]{2})", re.I)
# Why an allow list for links: a browser drops tabs and new lines from a URL
# and C0 controls and spaces at its ends, so a deny pattern can be passed
# around (re-review of plan 0006, M-A; docs/incidents/0044). A local href or
# src is a relative path of these characters only, with no "//".
LOCAL_PATH = re.compile(r"(?!.*//)[a-z0-9._-][a-z0-9._/-]*")
URL_ATTRS = {"href", "src"}
# The stylesheet loads nothing: no url(), image-set(), @import or @font-face,
# and no backslash, as CSS escapes can spell any of them.
CSS_REFUSED = re.compile(r"url\(|image-set\(|@import|@font-face|\\|[a-z][a-z0-9+.-]*:\s*//", re.I)
SVG_NS = 'xmlns="http://www.w3.org/2000/svg"'
# Plan 0006, D-8: the faces of the food sheet that the page shows (the bento
# and the mug and the bun from above, the popcorn bag from the front).
SITE_FOOD_FACES = {
    "bento": (0.0, 0.0, 256.0, 192.0),
    "popcorn": (0.0, 202.0, 192.0, 256.0),
    "mug": (446.0, 340.0, 128.0, 128.0),
    "bun": (266.0, 478.0, 128.0, 128.0),
}
# The rules of style.css that may set display: the selectors around the
# declaration, from the outermost (an @media too), and the value. The food
# may be hidden only when motion is reduced (plan 0006, D-8).
REDUCED = "@media (prefers-reduced-motion: reduce)"
DISPLAY_RULES = {
    ((".bar",), "flex"), ((".bar a",), "block"), ((".bar svg",), "block"), ((".hero",), "grid"),
    ((".dl",), "inline-block"), (("footer ul",), "flex"), ((REDUCED, ".food"), "none"),
}


def css_declarations(text):
    """The declarations of a stylesheet as (the selectors around it, from
    the outermost, and the declaration). Comments are dropped first, and
    nested rules are followed, so a declaration after a nested rule still
    belongs to the rule around it (re-review of the D-8 change, L-A)."""
    text = re.sub(r"(?s)/\*.*?\*/", "", text)
    stack, buf, out = [], "", []

    def flush():
        decl = " ".join(buf.split())
        if decl and stack:
            out.append((tuple(stack), decl))

    for ch in text:
        if ch == "{":
            stack.append(" ".join(buf.split()))
            buf = ""
        elif ch == "}":
            flush()
            if stack:
                stack.pop()
            buf = ""
        elif ch == ";":
            flush()
            buf = ""
        else:
            buf += ch
    return out


def display_problems(text):
    """Declarations that may show or hide something the page does not
    expect: a display outside DISPLAY_RULES, or the shorthand all, which
    resets display too."""
    found = []
    for where, decl in css_declarations(text):
        prop, _, value = decl.partition(":")
        prop = prop.strip().lower()
        if prop == "all" or (prop == "display" and (where, value.strip()) not in DISPLAY_RULES):
            found.append((where, decl))
    return found


def unexpected_files(site):
    """Files in the site that are not the two pages, the stylesheet or a
    picture that make_art.py makes; all of site/ is published (pages.yml),
    so a file anywhere in it counts (re-review of the D-8 change, M-A)."""
    with tempfile.TemporaryDirectory() as d:
        pictures = {pathlib.Path("img") / name for name in make_art.write_site(ROOT, pathlib.Path(d))}
    expected = set(PAGES.values()) | {pathlib.Path("style.css")} | pictures
    present = {p.relative_to(site) for p in site.rglob("*") if p.is_file()}
    return sorted(str(p) for p in present - expected)


class Page(html.parser.HTMLParser):
    """Collects the tags and attributes of a page."""

    def __init__(self):
        super().__init__()
        self.tags = []

    def handle_starttag(self, tag, attrs):
        self.tags.append((tag, attrs))

    handle_startendtag = handle_starttag


def parse(path):
    page = Page()
    page.feed(path.read_text(encoding="utf-8"))
    return page


def html_problems(path):
    out = []
    for tag, attrs in parse(path).tags:
        if tag not in TAGS:
            out.append(f"{path}: 許可していない要素 <{tag}>")
        values = dict(attrs)
        if tag == "meta" and "name" in values and values["name"] not in META_NAMES:
            out.append(f"{path}: 許可していない meta {values['name']}")
        if tag == "link" and values.get("rel") not in LINK_RELS:
            out.append(f"{path}: 許可していない link {values.get('rel')}")
        for name, value in attrs:
            if name not in ATTRS:
                out.append(f"{path}: <{tag}> の許可していない属性 {name}")
            value = value or ""
            if name in URL_ATTRS:
                github = tag == "a" and name == "href" and value in (REPO, LATEST)
                if not github and not LOCAL_PATH.fullmatch(value):
                    out.append(f"{path}: <{tag}> の {name} が相対のパスでない: {value!r}")
            elif OUTSIDE.match(re.sub(r"[\t\n\r]", "", value).strip("\x00- ")):
                out.append(f"{path}: <{tag}> の {name} がほかのサイトかスキームを指す: {value!r}")
    return out


def css_problems(path):
    text = path.read_text(encoding="utf-8")
    return [f"{path}: 読み込みに使える指定: {m.group(0)}" for m in CSS_REFUSED.finditer(text)]


def svg_problems(path):
    text = path.read_text(encoding="utf-8").replace(SVG_NS, "", 1)
    out = []
    for pattern, what in [
        (r"(?i)<\s*(script|image|foreignobject|style|a)\b", "許可していない要素"),
        (r"(?i)\son[a-z]+\s*=", "イベントの属性"),
        (r"(?i)href\s*=\s*['\"](?!#)", "シートの外を指す href"),
        (r"(?i)url\(\s*['\"]?(?!#)", "シートの外を指す url()"),
        (r"(?i)[a-z][a-z0-9+.-]*:\s*//|@import", "ほかのサイトを指す値"),
    ]:
        for m in re.finditer(pattern, text):
            out.append(f"{path}: {what}: {m.group(0)}")
    return out


def problems(site):
    """Everything in `site` that could load something from another site."""
    out = []
    for path in sorted(p for p in site.rglob("*") if p.is_file()):
        kind = path.suffix.lower()
        if kind not in FILE_TYPES:
            out.append(f"{path}: 許可していない種類のファイル")
        elif kind == ".html":
            out += html_problems(path)
        elif kind == ".css":
            out += css_problems(path)
        else:
            out += svg_problems(path)
    return out


class SiteTest(unittest.TestCase):
    def test_nothing_is_loaded_from_another_site(self):
        self.assertEqual(problems(SITE), [])

    def test_the_check_finds_the_ways_around_it(self):
        # Review of plan 0006, M-2: each of these got through the first check.
        page = PAGES["ja"]
        cases = [
            (page, "<head>", '<head><base href="https://example.com/">'),
            (page, "<footer>", '<footer style="background:url(https://example.com/a.png)">'),
            (page, "</head>", "<style>@import url(https://example.com/a.css);</style></head>"),
            (page, "</head>", '<meta http-equiv="refresh" content="0;url=https://example.com/"></head>'),
            (page, "</svg>", '<image href="https://example.com/a.png"/></svg>'),
            (page, "</footer>", '<form action="https://example.com/"></form></footer>'),
            (page, 'class="dl"', 'class="dl" ping="https://example.com/"'),
            (page, 'href="style.css"', 'href="HTTPS://example.com/a.css"'),
            (page, 'href="style.css"', 'href="/\\example.com/a.css"'),
            (page, 'src="img/microwave-white.svg"', 'src=" https://example.com/a.svg"'),
            (page, "</body>", '<script src="https://example.com/a.js"></script></body>'),
            (pathlib.Path("style.css"), ":root {", '@IMPORT "https://example.com/a.css";\n:root {'),
            (pathlib.Path("style.css"), ":root {", '.x { background: URL(https://example.com/a.png); }\n:root {'),
            (pathlib.Path("style.css"), ":root {", '.x { background: image-set("https://example.com/a.png" 1x); }\n:root {'),
            (pathlib.Path("img") / "microwave-white.svg", "</svg>", '<image href="https://example.com/a.png"/></svg>'),
            # Re-review of plan 0006, M-A: a browser drops tabs and new lines
            # from a URL and reads CSS escapes (docs/incidents/0044).
            (page, 'href="style.css"', 'href="ht&#9;tps://example.com/a.css"'),
            (page, 'href="style.css"', 'href="ht\ntps://example.com/b.css"'),
            (page, 'src="img/microwave-white.svg"', 'src="/&#9;/example.com/c.svg"'),
            (pathlib.Path("style.css"), ":root {", '.x { background: url(\\68ttps\\3a//example.com/f.png); }\n:root {'),
            (pathlib.Path("style.css"), ":root {", '.x { background: url("\\2f/example.com/g.png"); }\n:root {'),
            (pathlib.Path("style.css"), ":root {", '@\\69mport "\\68ttps\\3a//example.com/h.css";\n:root {'),
        ]
        for rel, old, new in cases:
            with self.subTest(file=str(rel), change=new), tempfile.TemporaryDirectory() as d:
                copy = pathlib.Path(d) / "site"
                shutil.copytree(SITE, copy)
                text = (copy / rel).read_text(encoding="utf-8")
                self.assertIn(old, text)
                (copy / rel).write_text(text.replace(old, new, 1), encoding="utf-8")
                self.assertTrue(problems(copy), "見つけられなかった")
        for name, body in [("extra.html", "<script src=x.js></script>"), ("a.js", "x")]:
            with self.subTest(file=name), tempfile.TemporaryDirectory() as d:
                copy = pathlib.Path(d) / "site"
                shutil.copytree(SITE, copy)
                (copy / name).write_text(body, encoding="utf-8")
                self.assertTrue(problems(copy), "見つけられなかった")

    def test_both_languages_exist_and_link_to_each_other(self):
        for lang, rel in PAGES.items():
            path = SITE / rel
            self.assertTrue(path.is_file(), f"{path} がない")
            html_tag = [dict(a) for t, a in parse(path).tags if t == "html"]
            self.assertEqual(html_tag[0].get("lang"), lang, f"{path} の html の lang")
        ja_links = [dict(a).get("href") for t, a in parse(SITE / PAGES["ja"]).tags if t == "a"]
        en_links = [dict(a).get("href") for t, a in parse(SITE / PAGES["en"]).tags if t == "a"]
        self.assertIn("en/", ja_links, "日本語のページから英語のページへのリンクがない")
        self.assertIn("../", en_links, "英語のページから日本語のページへのリンクがない")

    def test_links_to_other_sites_are_only_the_repository_and_the_latest_release(self):
        for rel in PAGES.values():
            hrefs = [dict(a).get("href") or "" for t, a in parse(SITE / rel).tags if t == "a"]
            outside = [h for h in hrefs if OUTSIDE.match(h)]
            self.assertEqual(sorted(set(outside)), sorted({REPO, LATEST}), rel)

    def test_every_local_link_and_picture_exists(self):
        for rel in PAGES.values():
            path = SITE / rel
            for tag, attrs in parse(path).tags:
                for name, value in attrs:
                    if name not in ("href", "src") or not value or OUTSIDE.match(value) or value.startswith("#"):
                        continue
                    target = (path.parent / value.split("#")[0]).resolve()
                    if value.endswith("/"):
                        target = target / "index.html"
                    self.assertTrue(target.is_file(), f"{path} の {value} がない")
        for css in SITE.rglob("*.css"):
            for value in re.findall(r"url\(\s*['\"]?([^'\")]+)", css.read_text(encoding="utf-8")):
                if not value.startswith("#"):
                    self.assertTrue((css.parent / value).resolve().is_file(), f"{css} の {value} がない")

    def test_the_github_mark_is_white_and_not_recolored(self):
        # Plan 0006, D-6: the official mark, white on the dark page, never
        # recolored by CSS (no currentColor, no fill, no filter or opacity).
        for rel in PAGES.values():
            svgs = [dict(a) for t, a in parse(SITE / rel).tags if t == "svg"]
            self.assertEqual(len(svgs), 1, rel)
            self.assertEqual(svgs[0].get("fill"), "#ffffff", f"{rel} の GitHub のマークが白でない")
        css = (SITE / "style.css").read_text(encoding="utf-8")
        self.assertNotIn("currentColor", css)
        self.assertNotRegex(css, r"(?i)\bfill\s*:")
        m = re.search(r"\.bar svg\s*\{([^}]*)\}", css)
        self.assertIsNotNone(m, ".bar svg の指定がない")
        self.assertNotRegex(m.group(1), r"(?i)opacity|filter|color")

    def test_motion_stops_for_reduced_motion(self):
        text = (SITE / "style.css").read_text(encoding="utf-8")
        m = re.search(r"@media\s*\(prefers-reduced-motion:\s*reduce\)\s*\{(.*?)\n\}", text, re.S)
        self.assertIsNotNone(m, "prefers-reduced-motion: reduce の指定がない")
        self.assertRegex(m.group(1), r"animation\s*:\s*none")

    def test_the_site_holds_only_the_pages_the_stylesheet_and_the_pictures(self):
        # An allow list of the files of the whole site, as all of it is
        # published: a picture anywhere else would pass the checks of the
        # pictures below (review of the D-8 change, M-1, and its re-review,
        # M-A).
        self.assertEqual(unexpected_files(SITE), [])
        tries = [
            pathlib.Path("sheet.svg"), pathlib.Path("en") / "img" / "sky.svg",
            pathlib.Path("img") / "sky.svg", pathlib.Path("extra.html"), pathlib.Path("en") / "style.css",
        ]
        sheet = (ROOT / "assets" / "sprites" / "food.svg").read_text(encoding="utf-8")
        for rel in tries:
            with self.subTest(file=str(rel)), tempfile.TemporaryDirectory() as d:
                copy = pathlib.Path(d) / "site"
                shutil.copytree(SITE, copy)
                (copy / rel).parent.mkdir(parents=True, exist_ok=True)
                (copy / rel).write_text(sheet, encoding="utf-8")
                self.assertEqual(unexpected_files(copy), [str(rel)], "見つけられなかった")

    def test_the_microwave_pictures_hold_only_the_microwave_sheets(self):
        # Review: other sheets (food) may get pictures of features the
        # offered release lacks, or of hidden features later (N-01).
        allowed = {"door", "interior", "led"} | {f"body-{c}" for c in make_art.SITE_COLORS}
        pictures = sorted((SITE / "img").glob("microwave-*.svg"))
        self.assertTrue(pictures, "電子レンジの絵がない")
        for path in pictures:
            text = path.read_text(encoding="utf-8")
            sheets = set(re.findall(r'<g id="([a-z-]+)">', text))
            self.assertTrue(sheets, path)
            self.assertLessEqual(sheets, allowed, f"{path} に電子レンジのほかのシートがある")
            self.assertNotRegex(text, r"[ \t]+\n", f"{path} に行末の空白がある")

    def test_the_food_pictures_hold_one_face_each(self):
        # Plan 0006, D-8: each food picture copies only its own face of the
        # food sheet, never the whole sheet (N-01). The faces are written
        # here, not taken from make_art.py, so that a change there is caught
        # (review of the D-8 change, M-2 and L-1).
        self.assertEqual({name: rect for name, (rect, _) in make_art.SITE_FOOD.items()}, SITE_FOOD_FACES)
        pictures = sorted((SITE / "img").glob("food-*.svg"))
        self.assertEqual(sorted(p.name for p in pictures), sorted(f"food-{name}.svg" for name in SITE_FOOD_FACES))
        for path in pictures:
            text = path.read_text(encoding="utf-8")
            x, y, w, h = SITE_FOOD_FACES[path.stem.removeprefix("food-")]
            self.assertIn(f'viewBox="{x:g} {y:g} {w:g} {h:g}"', text, f"{path} の面が決めたものと違う")
            self.assertNotRegex(text, r'<g id="', f"{path} にシートの全体がある")
            groups = re.findall(r'<g transform="translate\(([^)]*)\)">', text)
            self.assertEqual(groups, [f"{x:g} {y:g}"], f"{path} に決めた面が1つだけではない")
            self.assertNotRegex(text, r"<!--(?! Made by)", f"{path} にシートの注記が残っている")
            self.assertNotRegex(text, r"[ \t]+\n", f"{path} に行末の空白がある")

    def test_the_food_flies_but_is_not_shown_for_reduced_motion(self):
        # Plan 0006, D-8 (1) and (2). Every picture of food on a page counts,
        # whatever its class, and each must have the class "food".
        for rel in PAGES.values():
            imgs = [dict(a) for t, a in parse(SITE / rel).tags if t == "img"]
            food = [a for a in imgs if a.get("src", "").rsplit("/", 1)[-1].startswith("food-")]
            self.assertEqual(
                sorted(a["src"].rsplit("/", 1)[-1] for a in food),
                sorted(f"food-{name}.svg" for name in SITE_FOOD_FACES),
                rel,
            )
            for a in food:
                # "f" makes it fly (the animation of .f), "food" hides it
                # when motion is reduced (re-review of the D-8 change, L-B).
                classes = (a.get("class") or "").split()
                self.assertIn("f", classes, f"{rel} の {a['src']} に f のクラスがない")
                self.assertIn("food", classes, f"{rel} の {a['src']} に food のクラスがない")
        decls = css_declarations((SITE / "style.css").read_text(encoding="utf-8"))
        self.assertIn(((REDUCED, ".food"), "display: none"), decls, "動きを減らす設定で食べ物を隠す指定がない")

    def test_only_known_rules_set_display(self):
        # An allow list of where display may be set, with the @media around
        # it, so that no other rule shows the food again or hides it always
        # (review of the D-8 change, L-2, and its re-review, L-A and L-B).
        # The page has no style attribute or <style> element (TAGS and
        # ATTRS), so the stylesheet is the only place to look. What a browser
        # does with the rules is not run here; this reads the text only.
        self.assertEqual(display_problems((SITE / "style.css").read_text(encoding="utf-8")), [])
        css = (SITE / "style.css").read_text(encoding="utf-8")
        tries = [
            "@media (prefers-reduced-motion: reduce) { .f6, .f7, .f8, .f9 { display: block; } }",
            "@media (prefers-reduced-motion: reduce) { .flock img { display: inline; } }",
            ".flock img { all: revert; }",
            "@media (prefers-reduced-motion: reduce) { .f6 { all: unset; } }",
            ".f { & .z { color: red; } display: block; }",
            ".food { display: none; }",
            "@media (prefers-reduced-motion: reduce) { .food { display: none !important; } }",
        ]
        for extra in tries:
            with self.subTest(css=extra):
                self.assertTrue(display_problems(css + "\n" + extra + "\n"), "見つけられなかった")
        commented = css.replace("  .food {\n    display: none;\n  }", "  /* .food { display: none } */")
        self.assertNotEqual(commented, css, "試しの置き換えが当たらない")
        self.assertNotIn(((REDUCED, ".food"), "display: none"), css_declarations(commented))

    def test_the_pictures_are_the_ones_make_art_makes(self):
        with tempfile.TemporaryDirectory() as d:
            made = pathlib.Path(d)
            names = make_art.write_site(ROOT, made)
            self.assertEqual(
                sorted(names),
                sorted(p.name for p in (SITE / "img").iterdir()),
                "site/img に make_art.py が作らないファイルがあるか、足りない",
            )
            for name in names:
                self.assertEqual(
                    (made / name).read_text(encoding="utf-8"),
                    (SITE / "img" / name).read_text(encoding="utf-8"),
                    f"site/img/{name} が古い。python3 -I scripts/icon/make_art.py を実行する",
                )


if __name__ == "__main__":
    unittest.main()
