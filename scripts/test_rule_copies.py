"""Checks that every copy of a rule carries the rule's fixed words.

Run: python3 -I -m unittest discover -s scripts

Why: some rules are written in several documents, and a change to a rule
was twice made in AGENTS.md without the same change in the other documents
(docs/incidents/0042-rule-change-not-reflected-in-other-docs.md). The table
in the "規則の写し" section of AGENTS.md names, for each such rule, its fixed
words and the documents that copy it. In every listed document, one line
must hold all the fixed words of the rule. The section itself is left out of
the search, so the table cannot stand in for the rule it lists.
"""

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
HEADING = "## 規則の写し"


def section_bounds(text):
    """The start and end of the rule-copies section of `text`, or None."""
    lines = text.splitlines(keepends=True)
    offset = 0
    start = None
    for line in lines:
        if start is None and line.rstrip() == HEADING:
            start = offset
        elif start is not None and line.startswith("## "):
            return start, offset
        offset += len(line)
    return (start, len(text)) if start is not None else None


def without_section(text):
    """`text` without its rule-copies section."""
    bounds = section_bounds(text)
    return text if bounds is None else text[: bounds[0]] + text[bounds[1] :]


def backticked(cell):
    """The words in backticks of `cell`, or None when the cell has anything
    else than those words and the separator 「、」."""
    words = re.findall(r"`([^`]+)`", cell)
    rest = re.sub(r"`[^`]+`", "", cell).replace("、", "").strip()
    return words if words and not rest else None


def rules(agents):
    """The rows of the table in the rule-copies section of `agents`, as a
    list of (rule name, fixed words, documents), and the problems found in
    reading it (a table row that cannot be read is a problem, not skipped)."""
    bounds = section_bounds(agents)
    if bounds is None:
        return [], [f"no section {HEADING}"]
    found, problems = [], []
    rows = [line for line in agents[bounds[0] : bounds[1]].splitlines() if line.startswith("|")]
    for index, line in enumerate(rows):
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if index == 0 or set(line) <= set("|-: "):
            continue  # the header row and the separator row
        words = backticked(cells[1]) if len(cells) == 3 else None
        documents = backticked(cells[2]) if len(cells) == 3 else None
        if words is None or documents is None:
            problems.append(f"a row that cannot be read: {line}")
            continue
        found.append((cells[0], words, documents))
    return found, problems


def errors(root, table):
    """What is missing: a listed document with no line that holds all the
    fixed words of its rule, or a listed document that does not exist."""
    found = []
    for name, words, documents in table:
        for document in documents:
            path = pathlib.Path(root, document)
            if not path.is_file():
                found.append(f"{name}: {document} does not exist")
                continue
            text = without_section(path.read_text(encoding="utf-8"))
            if not any(all(word in line for word in words) for line in text.splitlines()):
                found.append(f"{name}: no line of {document} holds all of {words}")
    return found


class FunctionTest(unittest.TestCase):
    AGENTS = (
        "# Rules\n\nalpha and beta, written once.\n\n## 規則の写し\n\n"
        "| 規則 | 決まった語 | 写しを置く文書 |\n"
        "| --- | --- | --- |\n"
        "| 例 | `alpha`、`beta` | `AGENTS.md`、`b.md` |\n\n"
        "## Next\n\n| x | `y` | `z.md` |\n"
    )

    def write(self, d, name, text):
        pathlib.Path(d, name).write_text(text, encoding="utf-8")

    def test_the_table_is_read(self):
        self.assertEqual(rules(self.AGENTS), ([("例", ["alpha", "beta"], ["AGENTS.md", "b.md"])], []))

    def test_no_section_is_a_problem(self):
        self.assertEqual(rules("# Rules\n")[1], [f"no section {HEADING}"])

    def test_a_row_that_cannot_be_read_is_a_problem(self):
        bad = self.AGENTS.replace("`alpha`、`beta`", "`alpha`、beta")
        table, problems = rules(bad)
        self.assertEqual(table, [])
        self.assertEqual(len(problems), 1)

    def test_all_copies_with_the_words_on_one_line_pass(self):
        import tempfile

        with tempfile.TemporaryDirectory() as d:
            self.write(d, "AGENTS.md", self.AGENTS)
            self.write(d, "b.md", "alpha with beta\n")
            self.assertEqual(errors(d, rules(self.AGENTS)[0]), [])

    def test_the_table_does_not_stand_in_for_the_rule(self):
        import tempfile

        # The rule is gone from the body of AGENTS.md; only the table names it.
        agents = self.AGENTS.replace("alpha and beta, written once.", "(removed)")
        with tempfile.TemporaryDirectory() as d:
            self.write(d, "AGENTS.md", agents)
            self.write(d, "b.md", "alpha with beta\n")
            self.assertEqual(len(errors(d, rules(agents)[0])), 1)

    def test_words_on_different_lines_fail(self):
        import tempfile

        with tempfile.TemporaryDirectory() as d:
            self.write(d, "AGENTS.md", self.AGENTS)
            self.write(d, "b.md", "alpha here\nbeta elsewhere\n")
            self.assertEqual(len(errors(d, rules(self.AGENTS)[0])), 1)

    def test_a_missing_document_fails(self):
        import tempfile

        with tempfile.TemporaryDirectory() as d:
            self.write(d, "AGENTS.md", self.AGENTS)
            self.assertEqual(errors(d, rules(self.AGENTS)[0]), ["例: b.md does not exist"])


class RepositoryTest(unittest.TestCase):
    def test_every_copy_has_the_fixed_words(self):
        table, problems = rules((ROOT / "AGENTS.md").read_text(encoding="utf-8"))
        self.assertEqual(problems, [])
        names = [name for name, _, _ in table]
        self.assertIn("スクリプトのテストの条件", names)
        self.assertIn("個人情報の規則", names)
        self.assertEqual(errors(ROOT, table), [])


if __name__ == "__main__":
    unittest.main()
