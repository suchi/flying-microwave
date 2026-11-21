"""Checks that every copy of a rule carries the rule's fixed words.

Run: python3 -I -m unittest discover -s scripts

Why: some rules are written in several documents, and a change to a rule
was twice made in AGENTS.md without the same change in the other documents
(docs/incidents/0042-rule-change-not-reflected-in-other-docs.md). The table
in the "規則の写し" section of AGENTS.md names, for each such rule, its fixed
words and the documents that copy it. Every listed document must contain
every fixed word of its rule.
"""

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
HEADING = "## 規則の写し"


def rules(agents):
    """The rows of the table in the rule-copies section of `agents`:
    a list of (rule name, fixed words, documents)."""
    start = agents.find("\n" + HEADING + "\n")
    if start < 0:
        return []
    section = agents[start + len(HEADING) + 2 :]
    end = section.find("\n## ")
    if end >= 0:
        section = section[:end]
    found = []
    for line in section.splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) != 3 or not line.startswith("|") or set(cells[0]) <= set("-: "):
            continue
        words = re.findall(r"`([^`]+)`", cells[1])
        documents = re.findall(r"`([^`]+)`", cells[2])
        if words and documents:
            found.append((cells[0], words, documents))
    return found


def errors(root, table):
    """What is missing: a fixed word that a listed document does not contain,
    or a listed document that does not exist."""
    found = []
    for name, words, documents in table:
        for document in documents:
            path = pathlib.Path(root, document)
            if not path.is_file():
                found.append(f"{name}: {document} does not exist")
                continue
            text = path.read_text(encoding="utf-8")
            for word in words:
                if word not in text:
                    found.append(f"{name}: {document} does not contain {word}")
    return found


class FunctionTest(unittest.TestCase):
    AGENTS = (
        "# Rules\n\n## 規則の写し\n\n"
        "| 規則 | 決まった語 | 写しを置く文書 |\n"
        "| --- | --- | --- |\n"
        "| 例 | `alpha`、`beta` | `a.md`、`b.md` |\n\n"
        "## Next\n\n| x | `y` | `z.md` |\n"
    )

    def test_the_table_is_read(self):
        self.assertEqual(rules(self.AGENTS), [("例", ["alpha", "beta"], ["a.md", "b.md"])])

    def test_no_section_gives_no_rules(self):
        self.assertEqual(rules("# Rules\n"), [])

    def test_a_missing_word_fails(self):
        import tempfile

        with tempfile.TemporaryDirectory() as d:
            pathlib.Path(d, "a.md").write_text("alpha beta", encoding="utf-8")
            pathlib.Path(d, "b.md").write_text("alpha", encoding="utf-8")
            self.assertEqual(errors(d, rules(self.AGENTS)), ["例: b.md does not contain beta"])

    def test_a_missing_document_fails(self):
        import tempfile

        with tempfile.TemporaryDirectory() as d:
            pathlib.Path(d, "a.md").write_text("alpha beta", encoding="utf-8")
            self.assertEqual(errors(d, rules(self.AGENTS)), ["例: b.md does not exist"])


class RepositoryTest(unittest.TestCase):
    def test_every_copy_has_the_fixed_words(self):
        table = rules((ROOT / "AGENTS.md").read_text(encoding="utf-8"))
        names = [name for name, _, _ in table]
        self.assertIn("スクリプトのテストの条件", names)
        self.assertIn("個人情報の規則", names)
        self.assertEqual(errors(ROOT, table), [])


if __name__ == "__main__":
    unittest.main()
