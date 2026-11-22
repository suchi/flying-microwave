"""Checks that a date written as "YYYY-MM-DDにPR #NNでマージ" is the JST date
of the merge commit of that pull request.

Run: python3 -I -m unittest discover -s scripts

Why: the date of a merge was written from memory as the UTC date
(docs/incidents/0041-dates-written-in-utc.md, its recurrence; plan 0005,
D-35 (1)). The merge commits in the history tell the real time. Other forms
of dates cannot be checked this way. With a shallow history the merge
commits are missing, so the check is skipped (plan 0005, D-36).
"""

import datetime
import pathlib
import re
import subprocess
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
JST = datetime.timezone(datetime.timedelta(hours=9))
MENTION = re.compile(r"(\d{4}-\d{2}-\d{2})に、?PR #(\d+)でマージ")
MERGE = re.compile(r"^Merge pull request #(\d+) ")


def mentions(text):
    """The (date, pull request number) of each "YYYY-MM-DDにPR #NNでマージ"."""
    return [(date, int(number)) for date, number in MENTION.findall(text)]


def merge_dates(log):
    """The JST date of each merge commit, by pull request number, from
    `git log --merges --format=%cI%x09%s` output."""
    dates = {}
    for line in log.splitlines():
        stamp, _, subject = line.partition("\t")
        match = MERGE.match(subject)
        if match:
            # Why replace: Python before 3.11 does not read a trailing "Z".
            when = datetime.datetime.fromisoformat(stamp.replace("Z", "+00:00"))
            dates.setdefault(int(match.group(1)), when.astimezone(JST).date().isoformat())
    return dates


def errors(texts, dates):
    """What does not match: for each (name, text), a mention whose date is
    not the JST date of the merge. A pull request without a merge commit in
    the history is not checked."""
    found = []
    for name, text in texts:
        for date, number in mentions(text):
            merged = dates.get(number)
            if merged is not None and merged != date:
                found.append(f"{name}: PR #{number} is written as merged on {date}, but was merged on {merged} (JST)")
    return found


class FunctionTest(unittest.TestCase):
    LOG = (
        "2026-11-22T05:56:15+09:00\tMerge pull request #94 from a/b\n"
        "2026-11-21T13:43:37Z\tMerge pull request #93 from a/b\n"
        "2026-11-21T10:00:00+09:00\tMerge branch 'main' into topic\n"
    )

    def test_mentions_are_found(self):
        text = "(1)〜(3)は2026-11-22にPR #94でマージした。2026-11-21に、PR #93でマージした。"
        self.assertEqual(mentions(text), [("2026-11-22", 94), ("2026-11-21", 93)])

    def test_merge_dates_are_given_in_jst(self):
        # 13:43 UTC is 22:43 JST on the same day; 05:56 JST stays 11-22.
        self.assertEqual(merge_dates(self.LOG), {94: "2026-11-22", 93: "2026-11-21"})

    def test_a_utc_date_is_an_error(self):
        dates = merge_dates("2026-11-21T20:56:15Z\tMerge pull request #94 from a/b\n")
        found = errors([("a.md", "2026-11-21にPR #94でマージした。")], dates)
        self.assertEqual(len(found), 1)
        self.assertIn("a.md", found[0])
        self.assertIn("2026-11-22", found[0])

    def test_the_jst_date_passes(self):
        dates = merge_dates("2026-11-21T20:56:15Z\tMerge pull request #94 from a/b\n")
        self.assertEqual(errors([("a.md", "2026-11-22にPR #94でマージした。")], dates), [])

    def test_a_pull_request_not_in_the_history_is_not_checked(self):
        self.assertEqual(errors([("a.md", "2026-11-22にPR #99でマージした。")], {}), [])


def shallow():
    out = subprocess.run(
        ["git", "rev-parse", "--is-shallow-repository"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return out.stdout.strip() == "true"


class RepositoryTest(unittest.TestCase):
    def test_merge_dates_in_the_documents_are_jst(self):
        if shallow():
            self.skipTest("shallow history; run `git fetch --unshallow` to check merge dates")
        log = subprocess.run(
            ["git", "log", "--merges", "--format=%cI%x09%s"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=True,
        ).stdout
        paths = sorted(ROOT.glob("docs/**/*.md")) + [ROOT / "CHANGELOG.md"]
        texts = [(str(p.relative_to(ROOT)), p.read_text(encoding="utf-8")) for p in paths]
        self.assertEqual(errors(texts, merge_dates(log)), [])


if __name__ == "__main__":
    unittest.main()
