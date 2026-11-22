"""Lists the incident records to look at after merging a pull request.

Run: python3 -I scripts/list_merge_incidents.py <merge commit>

Lists the records that the pull request changed and the records that its
added lines refer to, with the status line of each, so that the status of
every one is checked after the merge (.claude/commands/merge-pr.md, step 4).

Why: a record whose countermeasure was merged without a change to the record
file itself was not in the list, and its status stayed stale
(docs/incidents/0029-incident-status-stale-again.md, its recurrence; plan
0005, D-35 (2)).
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RECORD = re.compile(r"^docs/incidents/(\d{4})-[^/]*\.md$")
# "incidents/0019", "(docs/incidents/0005, 0015, 0018)", "incident 0043".
PATH_REFERENCE = re.compile(r"incidents/(\d{4})(?!\d)((?:, ?\d{4}(?!\d))*)")
WORD_REFERENCE = re.compile(r"[Ii]ncidents? ?(\d{4})(?!\d)")
# A link from one record to another: "[0028](0028-smoke.md)".
SIBLING_LINK = re.compile(r"\]\((\d{4})-[^)]*\.md\)")
# Why: their test data name records that the tests do not refer to.
LEFT_OUT = re.compile(r"^scripts/test_[^/]*\.py$")


def changed(names):
    """The incident numbers of the record files among `git diff --name-only`
    output lines (README.md is not a record)."""
    return {m.group(1) for m in map(RECORD.match, names) if m}


def numbers_in(line, path):
    """The incident numbers that one line of the file `path` refers to."""
    found = set()
    for first, rest in PATH_REFERENCE.findall(line):
        found.add(first)
        found.update(re.findall(r"\d{4}", rest))
    found.update(WORD_REFERENCE.findall(line))
    if path.startswith("docs/incidents/"):
        found.update(SIBLING_LINK.findall(line))
    return found


def scan(diff):
    """The numbers of the added and of the removed lines, by file, of a
    unified diff (`git diff -U0`)."""
    added, removed = {}, {}
    path = None
    for line in diff.splitlines():
        if line.startswith("+++ "):
            path = line[4:].removeprefix("b/")
        elif line.startswith("--- "):
            continue
        elif path is None or LEFT_OUT.match(path):
            continue
        elif line.startswith("+"):
            added.setdefault(path, set()).update(numbers_in(line[1:], path))
        elif line.startswith("-"):
            removed.setdefault(path, set()).update(numbers_in(line[1:], path))
    return added, removed


def referenced(diff):
    """The incident numbers that the added lines of a unified diff newly
    refer to (not already in the removed lines of the same file)."""
    added, removed = scan(diff)
    return set().union(*(n - removed.get(p, set()) for p, n in added.items()))


def kept(diff):
    """The incident numbers that the added lines refer to and the removed
    lines of the same file already referred to (a rewritten line)."""
    added, removed = scan(diff)
    return set().union(*(n & removed.get(p, set()) for p, n in added.items()))


def status_of(text):
    """The status line of a record, without the leading "- "."""
    for line in text.splitlines():
        if line.startswith("- ステータス:"):
            return line[2:]
    return "(no status line)"


def git(*args):
    return subprocess.run(
        ["git", *args], cwd=ROOT, capture_output=True, text=True, encoding="utf-8", check=True
    ).stdout


def main(argv):
    if len(argv) != 2:
        print("usage: python3 -I scripts/list_merge_incidents.py <merge commit>", file=sys.stderr)
        return 2
    merge = argv[1]
    names = git("diff", "--name-only", f"{merge}^1", merge)
    diff = git("diff", "-U0", f"{merge}^1", merge)
    numbers_changed = changed(names.splitlines())
    numbers_referenced = referenced(diff)
    numbers = sorted(numbers_changed | numbers_referenced | kept(diff))
    if not numbers:
        print("no incident records changed or referenced")
        return 0
    for number in numbers:
        if number in numbers_changed:
            how = "changed"
        elif number in numbers_referenced:
            how = "referenced"
        else:
            how = "kept: in a rewritten line"
        paths = sorted(ROOT.glob(f"docs/incidents/{number}-*.md"))
        if not paths:
            print(f"{number} ({how}): no record file")
            continue
        path = paths[0]
        status = status_of(path.read_text(encoding="utf-8"))
        print(f"{number} ({how}): {path.relative_to(ROOT)}: {status}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
