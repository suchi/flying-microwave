"""Tests for prepare_release.py. Run: python3 -I -m unittest discover -s scripts"""

import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import prepare_release as pr  # noqa: E402

CARGO = """[workspace]
members = ["crates/core"]

[workspace.package]
version = "0.1.0-alpha.2"
edition = "2024"
"""

CHANGELOG = """# 変更履歴

前書き。

## 未リリース

## v0.1.0-alpha.2 (2026-11-01)

最初の試作版。

### 追加

- 飛ぶ。

## v0.0.1 (2026-10-01)

- 古い版。
"""


class TagTest(unittest.TestCase):
    def test_release_and_alpha_tags_are_accepted(self):
        self.assertEqual(pr.version_of_tag("v1.2.3"), "1.2.3")
        self.assertEqual(pr.version_of_tag("v0.1.0-alpha.2"), "0.1.0-alpha.2")

    def test_other_tags_are_rejected(self):
        for tag in ["1.2.3", "v1.2", "v1.2.3-beta.1", "v1.2.3-alpha", "v1.2.3\n", "v01.2.3"]:
            with self.subTest(tag=tag):
                self.assertIsNone(pr.version_of_tag(tag))

    def test_only_alpha_versions_are_prereleases(self):
        self.assertTrue(pr.is_prerelease("0.1.0-alpha.2"))
        self.assertFalse(pr.is_prerelease("0.1.0"))


class CargoVersionTest(unittest.TestCase):
    def test_reads_the_workspace_version(self):
        self.assertEqual(pr.workspace_version(CARGO), "0.1.0-alpha.2")


class NotesTest(unittest.TestCase):
    def test_extracts_the_section_of_the_version(self):
        notes = pr.release_notes(CHANGELOG, "0.1.0-alpha.2")
        self.assertEqual(notes, "最初の試作版。\n\n### 追加\n\n- 飛ぶ。\n")

    def test_the_last_section_ends_at_the_end_of_the_file(self):
        self.assertEqual(pr.release_notes(CHANGELOG, "0.0.1"), "- 古い版。\n")

    def test_a_missing_section_is_none(self):
        self.assertIsNone(pr.release_notes(CHANGELOG, "9.9.9"))

    def test_an_empty_section_is_none(self):
        text = CHANGELOG.replace("最初の試作版。\n\n### 追加\n\n- 飛ぶ。\n", "")
        self.assertIsNone(pr.release_notes(text, "0.1.0-alpha.2"))


class MainTest(unittest.TestCase):
    def run_main(self, tag, cargo=CARGO, changelog=CHANGELOG):
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            (root / "Cargo.toml").write_text(cargo, encoding="utf-8")
            (root / "CHANGELOG.md").write_text(changelog, encoding="utf-8")
            out = root / "notes.md"
            code = pr.main([tag, "--root", str(root), "--notes", str(out)])
            return code, out.read_text(encoding="utf-8") if out.exists() else None

    def test_writes_the_notes_for_a_matching_tag(self):
        code, notes = self.run_main("v0.1.0-alpha.2")
        self.assertEqual(code, 0)
        self.assertIn("- 飛ぶ。", notes)

    def test_fails_when_the_version_does_not_match(self):
        self.assertEqual(self.run_main("v0.1.0-alpha.3")[0], 1)

    def test_fails_on_an_invalid_tag(self):
        self.assertEqual(self.run_main("latest")[0], 1)

    def test_writes_the_notes_for_a_full_release(self):
        cargo = CARGO.replace("0.1.0-alpha.2", "0.0.1")
        code, notes = self.run_main("v0.0.1", cargo=cargo)
        self.assertEqual(code, 0)
        self.assertTrue(notes.startswith("- 古い版。\n"))

    def test_the_notes_say_that_arm64_is_not_guaranteed(self):
        # ADR-0016: every release ships arm64 without guarantee, and the
        # notes say so and name the checks that were made.
        for tag, cargo in [("v0.1.0-alpha.2", CARGO), ("v0.0.1", CARGO.replace("0.1.0-alpha.2", "0.0.1"))]:
            with self.subTest(tag=tag):
                notes = self.run_main(tag, cargo=cargo)[1]
                self.assertIn("### arm64版について", notes)
                self.assertIn(f"FlyingMicrowave-{tag[1:]}-windows-arm64.zip", notes)
                self.assertIn("動作を保証しない", notes)
                self.assertIn("スモークテスト)と、実機での確認はしていない", notes)

    def test_fails_without_a_changelog_section(self):
        cargo = CARGO.replace("0.1.0-alpha.2", "0.2.0-alpha.1")
        self.assertEqual(self.run_main("v0.2.0-alpha.1", cargo=cargo)[0], 1)


if __name__ == "__main__":
    unittest.main()
