"""Checks that no tracked text file hides text with invisible characters.

Run: python3 -I -m unittest discover -s scripts

Why: an instruction for an agent can be hidden in a file with characters
that do not show on screen (bidirectional controls, zero-width characters,
tag characters), and a reviewer reading the diff would not see it (plan
0005, D-47, N-2; docs/reviews/0006).
"""

import pathlib
import subprocess
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import check_invisible_chars  # noqa: E402


class FindTest(unittest.TestCase):
    def test_finds_each_kind_of_invisible_character(self):
        for ch in ["\u202e", "\u2066", "\u200b", "\u200d", "\u2060", "\ufeff", "\U000e0041",
                   "\u061c", "\ufe0f", "\U000e0100", "\u2062", "\u00ad", "\u180e", "\u3164", "\u115f", "\u1160", "\uffa0"]:
            text = f"line one\nab{ch}cd\n"
            self.assertEqual(check_invisible_chars.find(text), [(2, 3, ch)], repr(ch))

    def test_plain_japanese_and_ascii_text_has_none(self):
        self.assertEqual(check_invisible_chars.find("日本語の文です。\nASCII text\n"), [])

    def test_a_byte_order_mark_at_the_start_is_allowed(self):
        self.assertEqual(check_invisible_chars.find("\ufefffirst line\n"), [])


class RepositoryTest(unittest.TestCase):
    def test_no_text_file_has_invisible_characters(self):
        out = subprocess.run(
            ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"], cwd=ROOT, capture_output=True, check=True
        ).stdout.decode("utf-8")
        found = check_invisible_chars.scan(ROOT, [p for p in out.split("\0") if p])
        self.assertEqual(found, [], "invisible characters in tracked or new files")


if __name__ == "__main__":
    unittest.main()
