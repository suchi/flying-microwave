"""Tests of scripts/list_merge_incidents.py.

Run: python3 -I -m unittest discover -s scripts
"""

import unittest

from list_merge_incidents import changed, kept, referenced, status_of


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

    def test_references_in_code_comments_and_lists_are_listed(self):
        # Review: the forms that hooks, scripts and workflows use.
        diff = (
            "+++ b/.claude/hooks/x.sh\n"
            "+# Why: see (docs/incidents/0019).\n"
            "+# also docs/incidents/0036: and (docs/incidents/0005, 0015, 0018)\n"
        )
        self.assertEqual(referenced(diff), {"0019", "0036", "0005", "0015", "0018"})

    def test_numbers_listed_in_japanese_are_all_listed(self):
        # Re-review: "incident 0034、0038、0042" and "incident 0041と0029".
        diff = (
            "+++ b/docs/plans/0005-x.md\n"
            "+(incident 0034、0038、0042) and incident 0041と0029の再発\n"
            "+(docs/incidents/0005、0015)\n"
        )
        self.assertEqual(
            referenced(diff), {"0034", "0038", "0042", "0041", "0029", "0005", "0015"}
        )

    def test_sibling_links_inside_the_records_are_listed(self):
        diff = (
            "+++ b/docs/incidents/0044-x.md\n"
            "+関連: [0028](0028-smoke.md)\n"
            "+++ b/docs/plans/0005-x.md\n"
            "+[0029](0029-not-a-record-link.md)\n"
        )
        self.assertEqual(referenced(diff), {"0028"})

    def test_the_tests_of_the_scripts_are_left_out(self):
        diff = "+++ b/scripts/test_x.py\n+    'docs/incidents/0012-x.md'\n"
        self.assertEqual(referenced(diff), set())

    def test_a_reference_also_in_a_removed_line_of_the_file_is_kept_not_new(self):
        # Review: a rewrapped paragraph re-adds every link it had.
        diff = (
            "+++ b/AGENTS.md\n"
            "-old ([docs/incidents/0029](docs/incidents/0029-x.md))\n"
            "+new ([docs/incidents/0029](docs/incidents/0029-x.md)) incident 0043\n"
        )
        self.assertEqual(referenced(diff), {"0043"})
        self.assertEqual(kept(diff), {"0029"})

    def test_removed_lines_and_file_headers_are_not_references(self):
        diff = "--- a/docs/incidents/0012-x.md\n+++ b/docs/incidents/0013-y.md\n-incident 0014\n"
        self.assertEqual(referenced(diff), set())

    def test_the_status_line_is_read(self):
        text = "# 0001: x\n\n- 日付: 2026-11-01\n- ステータス: 対策済み(PR #1)\n"
        self.assertEqual(status_of(text), "ステータス: 対策済み(PR #1)")
        self.assertEqual(status_of("# no status\n"), "(no status line)")


if __name__ == "__main__":
    unittest.main()
