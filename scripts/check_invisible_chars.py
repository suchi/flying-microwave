"""Finds invisible characters that can hide text in a file.

Run: python3 -I scripts/check_invisible_chars.py   (checks the tracked and the new files)

The characters are the bidirectional controls, the zero-width characters,
the word joiner, a byte order mark that is not at the start of the text,
and the tag characters (plan 0005, D-47, N-2; docs/reviews/0006). Files
that are not UTF-8 text (images, icons) are skipped.
"""

import pathlib
import subprocess
import sys

INVISIBLE = (
    set(range(0x202A, 0x202F))  # LRE, RLE, PDF, LRO, RLO
    | set(range(0x2066, 0x206A))  # LRI, RLI, FSI, PDI
    | {0x200B, 0x200C, 0x200D, 0x200E, 0x200F, 0x2060, 0xFEFF}
    | set(range(0xE0000, 0xE0080))  # tag characters
)


def find(text):
    """(line, column, character) for each invisible character in `text`.

    Lines and columns start at 1. A byte order mark at the very start is
    allowed."""
    found = []
    for n, line in enumerate(text.split("\n"), 1):
        for col, ch in enumerate(line, 1):
            if ord(ch) in INVISIBLE and not (n == 1 and col == 1 and ch == "\ufeff"):
                found.append((n, col, ch))
    return found


def scan(root, paths):
    """(path, line, column, code point) for each invisible character in the
    UTF-8 text files among `paths`."""
    out = []
    for rel in paths:
        try:
            text = (pathlib.Path(root) / rel).read_bytes().decode("utf-8")
        except (UnicodeDecodeError, FileNotFoundError, IsADirectoryError):
            continue
        for n, col, ch in find(text):
            out.append((rel, n, col, f"U+{ord(ch):04X}"))
    return out


def main():
    root = pathlib.Path(__file__).resolve().parent.parent
    files = subprocess.run(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"], cwd=root, capture_output=True, check=True)
    found = scan(root, [p for p in files.stdout.decode("utf-8").split("\0") if p])
    for rel, n, col, code in found:
        print(f"{rel}:{n}:{col}: {code}")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
