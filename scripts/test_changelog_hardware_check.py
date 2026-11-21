"""Checks that the changelog does not deny a hardware check that has results.

Run: python3 -I -m unittest discover -s scripts

Why: the hardware check of v0.3.0-alpha.4 was done and its results were
recorded in its procedure document, but the changelog section of that version
(and so its release notes) kept saying that no hardware check was done
(docs/incidents/0038-release-notes-not-updated-after-hardware-check.md).
For each procedure document docs/development/hardware-check-vX.Y.Z*.md with
a "## 結果" section, the changelog section of that version must not contain a
sentence saying that the hardware check was not done.
"""

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
DOCUMENT = re.compile(r"^hardware-check-v(.+)\.md$")
# The sentences a section is written with before the hardware check.
NOT_CHECKED = ("実機での動作の確認はしていない", "実機での動作の確認をしていない")


def checked_versions(folder):
    """The versions whose procedure document in `folder` has results."""
    versions = []
    for path in sorted(folder.iterdir()):
        m = DOCUMENT.match(path.name)
        if m and re.search(r"^## 結果$", path.read_text(encoding="utf-8"), re.MULTILINE):
            versions.append(m.group(1))
    return versions


def section(changelog, version):
    """The text of the section of `version` in `changelog`, or None."""
    lines = changelog.splitlines()
    head = re.compile(r"^## v" + re.escape(version) + r" \(")
    for i, line in enumerate(lines):
        if head.match(line):
            end = next((j for j in range(i + 1, len(lines)) if lines[j].startswith("## ")), len(lines))
            return "\n".join(lines[i:end])
    return None


def errors(changelog, versions):
    """What is wrong with the sections of `versions` in `changelog`."""
    found = []
    for version in versions:
        text = section(changelog, version)
        if text is None:
            found.append(f"v{version}: no section in the changelog")
            continue
        for sentence in NOT_CHECKED:
            # A quoted sentence (「...」) tells what the section used to say.
            if re.search(r"(?<!「)" + re.escape(sentence), text):
                found.append(f"v{version}: the section says 「{sentence}」, but the hardware check has results")
    return found


class FunctionTest(unittest.TestCase):
    CHANGELOG = "## v1.0.0-alpha.2 (2026-01-02)\n\n実機での動作の確認はしていない。\n\n## v1.0.0-alpha.1 (2026-01-01)\n\nok\n"

    def test_a_checked_version_that_says_not_checked_fails(self):
        self.assertEqual(len(errors(self.CHANGELOG, ["1.0.0-alpha.2"])), 1)

    def test_the_sentence_of_another_version_does_not_count(self):
        self.assertEqual(errors(self.CHANGELOG, ["1.0.0-alpha.1"]), [])

    def test_a_quoted_sentence_does_not_count(self):
        changelog = "## v1.0.0 (2026-01-01)\n\nこの節には「実機での動作の確認はしていない」と書いていた。\n"
        self.assertEqual(errors(changelog, ["1.0.0"]), [])

    def test_a_missing_section_fails(self):
        self.assertEqual(len(errors(self.CHANGELOG, ["9.9.9"])), 1)

    def test_a_version_is_not_a_prefix_of_another(self):
        # v1.0.0-alpha.1 must not match the heading of v1.0.0-alpha.10.
        changelog = "## v1.0.0-alpha.10 (2026-01-03)\n\n実機での動作の確認をしていない。\n"
        self.assertIsNone(section(changelog, "1.0.0-alpha.1"))


class RepositoryTest(unittest.TestCase):
    def test_checked_versions_do_not_say_not_checked(self):
        changelog = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
        versions = checked_versions(ROOT / "docs" / "development")
        self.assertIn("0.3.0-alpha.4", versions)
        self.assertEqual(errors(changelog, versions), [])


if __name__ == "__main__":
    unittest.main()
