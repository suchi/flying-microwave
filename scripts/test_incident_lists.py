"""Checks that the incident lists match the incident records.

Run: python3 -I -m unittest discover -s scripts

Why: a row of an incident was once added between the numbered steps of
docs/incidents/README.md instead of to its list, and was merged
(docs/incidents/0026-incident-row-placed-in-procedure.md). Each record
docs/incidents/NNNN-*.md must have one row, in number order, in the list
table of docs/incidents/README.md and of docs/README.md, and no row may
sit outside that table.
"""

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
INCIDENTS = ROOT / "docs" / "incidents"
RECORD = re.compile(r"^(\d{4})-.+\.md$")


def records(folder):
    """The numbers of the incident records in `folder`, in order."""
    return sorted(m.group(1) for p in folder.iterdir() if (m := RECORD.match(p.name)))


def list_errors(text, prefix, numbers):
    """What is wrong with the list of incident rows in `text`.

    A row is a line `| [NNNN](<prefix>NNNN-...md) | ...`. The rows must be
    consecutive lines (one table), one for each of `numbers`, in that order.
    """
    lines = text.splitlines()
    row = re.compile(r"^\| \[(\d{4})\]\(" + re.escape(prefix) + r"(\d{4})-[^)]+\.md\) \|")
    found = [(i, m) for i, line in enumerate(lines) if (m := row.match(line))]
    errors = []
    for i, m in found:
        if m.group(1) != m.group(2):
            errors.append(f"line {i + 1}: the text {m.group(1)} links to {m.group(2)}")
    for (a, _), (b, m) in zip(found, found[1:]):
        if b != a + 1:
            errors.append(f"line {b + 1}: row {m.group(1)} is not in the same table as line {a + 1}")
    shown = [m.group(1) for _, m in found]
    if shown != list(numbers):
        errors.append(f"rows {shown} do not match the records {list(numbers)}")
    return errors


TABLE = "| 番号 | 異常 | 日付 |\n| --- | --- | --- |\n"
STEPS = "1. 止める\n2. 確かめる([0002](0002-b.md)を参照)\n3. 分析する\n"


class ListErrorsTest(unittest.TestCase):
    def test_a_table_of_every_record_in_order_is_right(self):
        text = STEPS + "\n" + TABLE + "| [0001](0001-a.md) | a | d |\n| [0002](0002-b.md) | b | d |\n"
        self.assertEqual(list_errors(text, "", ["0001", "0002"]), [])

    def test_a_row_between_the_steps_is_found(self):
        # The mistake of incident 0026.
        text = (
            "1. 止める\n2. 確かめる\n| [0002](0002-b.md) | b | d |\n3. 分析する\n\n"
            + TABLE
            + "| [0001](0001-a.md) | a | d |\n"
        )
        errors = list_errors(text, "", ["0001", "0002"])
        self.assertTrue(errors)
        self.assertIn("not in the same table", " ".join(errors))

    def test_a_missing_or_misordered_row_is_found(self):
        missing = TABLE + "| [0001](0001-a.md) | a | d |\n"
        self.assertTrue(list_errors(missing, "", ["0001", "0002"]))
        swapped = TABLE + "| [0002](0002-b.md) | b | d |\n| [0001](0001-a.md) | a | d |\n"
        self.assertTrue(list_errors(swapped, "", ["0001", "0002"]))

    def test_a_row_linking_to_another_number_is_found(self):
        text = TABLE + "| [0001](0002-b.md) | a | d |\n"
        self.assertTrue(list_errors(text, "", ["0001"]))

    def test_links_in_the_text_are_not_rows(self):
        # The steps link to records too; only table rows count.
        text = STEPS + TABLE + "| [0001](0001-a.md) | a | d |\n| [0002](0002-b.md) | b | d |\n"
        self.assertEqual(list_errors(text, "", ["0001", "0002"]), [])


class RepositoryListsTest(unittest.TestCase):
    def test_the_incident_readme_lists_every_record(self):
        text = (INCIDENTS / "README.md").read_text(encoding="utf-8")
        self.assertEqual(list_errors(text, "", records(INCIDENTS)), [])

    def test_the_docs_readme_lists_every_record(self):
        text = (ROOT / "docs" / "README.md").read_text(encoding="utf-8")
        self.assertEqual(list_errors(text, "incidents/", records(INCIDENTS)), [])


if __name__ == "__main__":
    unittest.main()
