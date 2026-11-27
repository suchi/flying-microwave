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
import tomllib
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
        # Why fail: every crates.io version checked on 2026-11-01 (256 versions
        # in the three lock files, back to 2014) had a pubtime, so a missing one means the index format
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


TOOLCHAIN_MANIFEST_URL = "https://static.rust-lang.org/dist/channel-rust-{}.toml"
EXACT_VERSION = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+")


def toolchain_version(path):
    """The channel of a rust-toolchain.toml, which must be an exact version
    (plan 0007, D-2). A channel name or a version without its patch number
    moves, so its age cannot be known: it raises ValueError (fail closed)."""
    with open(path, "rb") as f:
        channel = tomllib.load(f).get("toolchain", {}).get("channel")
    if not isinstance(channel, str) or not EXACT_VERSION.fullmatch(channel):
        raise ValueError(f"{path} の channel が正確な版(X.Y.Z)でない: {channel!r}")
    return channel


def manifest_date(text):
    """The date at the top of a Rust channel manifest, or None."""
    date = tomllib.loads(text).get("date")
    return date if isinstance(date, str) else None


def fetch_toolchain_date(version):
    """The date of a Rust release in its channel manifest on
    static.rust-lang.org, or None when there is no such release."""
    try:
        with urllib.request.urlopen(TOOLCHAIN_MANIFEST_URL.format(version), timeout=60) as response:
            return manifest_date(response.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return None
        raise


def check_toolchain(version, now, cooldown, toolchain_date):
    """Problems with the age of a Rust toolchain (empty when it passes).

    Why the end of the day: the manifest gives a day, not a time, so the day
    is counted as ending at 24:00 UTC and the wait is never shorter than the
    cooldown. The manifest also has a date in its version strings (1.99.1:
    "1.99.1 (58cf8f9c3 2026-10-13)", next to git_commit_hash), which is not
    used: the manifest date (2026-10-15) is the later of the two."""
    date = toolchain_date(version)
    if date is None:
        return [f"Rust {version}: 配布の情報(channel-rust-{version}.toml)に日付が見つからない"]
    published = dt.datetime.fromisoformat(date).replace(tzinfo=dt.timezone.utc) + dt.timedelta(days=1)
    if published > now - cooldown:
        return [
            f"Rust {version}: 配布の情報の日付は{date}で、その日の終わりから{cooldown.days}日たつのは"
            f"{(published + cooldown).isoformat()}"
        ]
    return []


def use_utf8_output():
    """Write the Japanese messages as UTF-8 on every platform.

    Why: on Windows, a piped stdout uses the ANSI code page (cp1252 on English
    Windows) with strict errors, so a Japanese message would raise
    UnicodeEncodeError and turn a passing check into a failure. `-I` also
    ignores PYTHONUTF8 and PYTHONIOENCODING.
    """
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8")


def main(argv=None, now=None, fetch=fetch_index, toolchain_date=fetch_toolchain_date):
    use_utf8_output()
    parser = argparse.ArgumentParser(description="依存の版の公開からの日数を確かめる")
    parser.add_argument("lockfiles", nargs="*", help="確かめるCargo.lock")
    parser.add_argument("--crate", action="append", default=[], metavar="NAME@VERSION")
    parser.add_argument("--rust-toolchain", metavar="PATH", help="版を確かめるrust-toolchain.toml")
    parser.add_argument("--days", type=int, default=DEFAULT_DAYS)
    args = parser.parse_args(argv)
    now = now or dt.datetime.now(dt.timezone.utc)
    cooldown = dt.timedelta(days=args.days)

    toolchain_problems = []
    if args.rust_toolchain:
        try:
            toolchain_problems = check_toolchain(toolchain_version(args.rust_toolchain), now, cooldown, toolchain_date)
        except (OSError, ValueError) as e:
            toolchain_problems = [f"Rustのツールチェーンの公開日を確かめられない: {e}"]

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

    try:
        problems = check(packages, now, cooldown, fetch)
    except (OSError, ValueError) as e:
        # Fail closed: an unverified version must not pass.
        print(f"crates.ioの索引を読めなかったため、確かめられない: {e}", file=sys.stderr)
        return 1
    problems = others + toolchain_problems + problems
    for problem in problems:
        print(problem, file=sys.stderr)
    count = len(set(packages)) + len(others) + (1 if args.rust_toolchain else 0)
    if problems:
        print(f"公開から{args.days}日たっていない版、または確かめられない版がある({len(problems)}件 / {count}件)", file=sys.stderr)
        return 1
    print(f"{count}件の版は、どれも公開から{args.days}日以上たっている")
    return 0


if __name__ == "__main__":
    sys.exit(main())
