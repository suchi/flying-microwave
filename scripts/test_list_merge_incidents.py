"""Tests of scripts/list_merge_incidents.py.

Run: python3 -I -m unittest discover -s scripts
"""

import unittest

from list_merge_incidents import changed, referenced, status_of


class ListMergeIncidentsTest(unittest.TestCase):
    def test_changed_records_are_listed_but_not_the_readme(self):
        names = [
            "docs/incidents/0039-main-ci-failed-unnoticed-after-merge.md",
            "docs/incidents/README.md",
            "AGENTS.md",
            "docs/incidents/0041-dates-written-in-utc.md",
        ]
        self.assertEqual(changed(names), {"0039", "0041"})

    def test_references_in_added_lines_are_listed(self):
        # Incident 0029, its recurrence: the countermeasure of 0040 was only
        # in AGENTS.md, which links to the record.
        diff = (
            "+++ b/AGENTS.md\n"
            "@@ -1 +1 @@\n"
            "-old text ([docs/incidents/0012](docs/incidents/0012-x.md))\n"
            "+new text ([docs/incidents/0040](docs/incidents/0040-hook.md))\n"
            "+see incident 0043 and Incident 0029\n"
        )
        self.assertEqual(referenced(diff), {"0040", "0043", "0029"})

    def test_removed_lines_and_file_headers_are_not_references(self):
        diff = "--- a/docs/incidents/0012-x.md\n+++ b/docs/incidents/0013-y.md\n-incident 0014\n"
        self.assertEqual(referenced(diff), set())

    def test_the_status_line_is_read(self):
        text = "# 0001: x\n\n- 日付: 2026-11-01\n- ステータス: 対策済み(PR #1)\n"
        self.assertEqual(status_of(text), "ステータス: 対策済み(PR #1)")
        self.assertEqual(status_of("# no status\n"), "(no status line)")


if __name__ == "__main__":
    unittest.main()
