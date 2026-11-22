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
REFERENCE = re.compile(r"incidents/(\d{4})-|[Ii]ncident (\d{4})")


def changed(names):
    """The incident numbers of the record files among `git diff --name-only`
    output lines (README.md is not a record)."""
    return {m.group(1) for m in map(RECORD.match, names) if m}


def referenced(diff):
    """The incident numbers that the added lines of a unified diff refer to."""
    numbers = set()
    for line in diff.splitlines():
        if line.startswith("+") and not line.startswith("+++"):
            for a, b in REFERENCE.findall(line):
                numbers.add(a or b)
    return numbers


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
    numbers = sorted(numbers_changed | referenced(diff))
    if not numbers:
        print("no incident records changed or referenced")
        return 0
    for number in numbers:
        how = "changed" if number in numbers_changed else "referenced"
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
