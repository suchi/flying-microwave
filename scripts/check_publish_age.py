#!/usr/bin/env python3
"""Fail when a crates.io version is younger than the dependency cooldown.

Dependabot waits 7 days before proposing a new release (`cooldown` in
.github/dependabot.yml), but versions resolved by a local `cargo` run are not
covered by that. Cargo has no stable setting for it (`min-publish-age` needs
`-Zmin-publish-age` in cargo 1.99), so this script checks every registry
package in the given Cargo.lock files, and any extra `name@version`, against
the publish time recorded in the crates.io sparse index (docs/incidents/0007).

Usage:
    python3 -I scripts/check_publish_age.py [--days 7] [--crate NAME@VERSION]... [LOCKFILE]...

Exit status: 0 when every version is old enough, 1 when one is not or cannot
be verified (the check fails closed), 2 for a usage error.

Messages are in Japanese because the maintainer reads them (AGENTS.md).
"""

import argparse
import datetime as dt
import json
import pathlib
import re
import sys
import urllib.error
import urllib.request

INDEX_URL = "https://index.crates.io/"
REGISTRY_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
DEFAULT_DAYS = 7


def index_path(name):
    """Path of a crate's file in the sparse index (the crates.io layout)."""
    n = name.lower()
    if len(n) <= 2:
        return f"{len(n)}/{n}"
    if len(n) == 3:
        return f"3/{n[0]}/{n}"
    return f"{n[:2]}/{n[2:4]}/{n}"


def fetch_index(name):
    """All index entries of a crate, or None when the crate is not in the index."""
    url = INDEX_URL + index_path(name)
    try:
        with urllib.request.urlopen(url, timeout=30) as response:
            body = response.read().decode("utf-8")
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return None
        raise
    return [json.loads(line) for line in body.splitlines() if line.strip()]


def lock_packages(lock_text):
    """(name, version, source) of every package in a Cargo.lock. Local path
    packages (the workspace itself) have no source and are left out."""
    packages = []
    for block in lock_text.split("[[package]]")[1:]:
        fields = dict(re.findall(r'^(name|version|source) = "([^"]*)"', block, re.M))
        if "source" in fields:
            packages.append((fields["name"], fields["version"], fields["source"]))
    return packages


def registry_packages(lock_text):
    """(name, version) of every crates.io package in a Cargo.lock."""
    return [(n, v) for n, v, s in lock_packages(lock_text) if s == REGISTRY_SOURCE]


def other_sources(lock_text):
    """Packages from another source (git, another registry). Their publish
    time cannot be checked here, so they fail the check."""
    return [(n, v, s) for n, v, s in lock_packages(lock_text) if s != REGISTRY_SOURCE]


def parse_time(text):
    return dt.datetime.fromisoformat(text.replace("Z", "+00:00"))


def check(packages, now, cooldown, fetch):
    """Problems found, one message per package (empty when all pass)."""
    problems = []
    cache = {}
    limit = now - cooldown
    for name, version in sorted(set(packages)):
        if name not in cache:
            cache[name] = fetch(name)
        entries = cache[name]
        entry = next((e for e in entries or [] if e.get("vers") == version), None)
        if entry is None:
            problems.append(f"{name} {version}: crates.ioの索引に見つからない")
            continue
        pubtime = entry.get("pubtime")
        # Why fail: every crates.io version checked on 2026-11-01 (648 versions,
        # back to 2014) had a pubtime, so a missing one means the index format
        # changed or the entry is unusual; it must not pass silently.
        if pubtime is None:
            problems.append(f"{name} {version}: 索引に公開日時(pubtime)がない")
            continue
        published = parse_time(pubtime)
        if published > limit:
            age = now - published
            problems.append(
                f"{name} {version}: {pubtime}に公開され、まだ{age.days}日"
                f"{age.seconds // 3600}時間しかたっていない"
                f"({cooldown.days}日たつのは{(published + cooldown).isoformat()})"
            )
    return problems


def main(argv=None, now=None, fetch=fetch_index):
    parser = argparse.ArgumentParser(description="依存の版の公開からの日数を確かめる")
    parser.add_argument("lockfiles", nargs="*", help="確かめるCargo.lock")
    parser.add_argument("--crate", action="append", default=[], metavar="NAME@VERSION")
    parser.add_argument("--days", type=int, default=DEFAULT_DAYS)
    args = parser.parse_args(argv)

    packages = []
    others = []
    for spec in args.crate:
        name, sep, version = spec.partition("@")
        if not sep or not name or not version:
            print(f"--crateの形が正しくない: {spec}(NAME@VERSIONの形で書く)", file=sys.stderr)
            return 2
        packages.append((name, version))
    for lock in args.lockfiles:
        text = pathlib.Path(lock).read_text(encoding="utf-8")
        packages += registry_packages(text)
        others += [f"{n} {v}: crates.io以外の依存({s})で、公開日時を確かめられない({lock})" for n, v, s in other_sources(text)]

    now = now or dt.datetime.now(dt.timezone.utc)
    try:
        problems = check(packages, now, dt.timedelta(days=args.days), fetch)
    except (OSError, ValueError) as e:
        # Fail closed: an unverified version must not pass.
        print(f"crates.ioの索引を読めなかったため、確かめられない: {e}", file=sys.stderr)
        return 1
    problems = others + problems
    for problem in problems:
        print(problem, file=sys.stderr)
    count = len(set(packages)) + len(others)
    if problems:
        print(f"公開から{args.days}日たっていない版、または確かめられない版がある({len(problems)}件 / {count}件)", file=sys.stderr)
        return 1
    print(f"{count}件の版は、どれも公開から{args.days}日以上たっている")
    return 0


if __name__ == "__main__":
    sys.exit(main())
