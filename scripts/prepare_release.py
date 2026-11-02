#!/usr/bin/env python3
"""Check a release tag and write its release notes from CHANGELOG.md.

Used by .github/workflows/release.yml before anything is built. The tag must
be vX.Y.Z or vX.Y.Z-alpha.N (plan 0001, section 7.3), must match the
workspace version in Cargo.toml, and CHANGELOG.md must have a non-empty
section "## vX.Y.Z... (date)" for it. The section body becomes the notes,
followed by a fixed section on the arm64 build, which every release ships
without guarantee (ADR-0016).

Usage:
    python3 -I scripts/prepare_release.py TAG [--root DIR] [--notes FILE]

Exit status: 0 when the notes were written, 1 when a check failed, 2 for a
usage error. Messages are in Japanese because the maintainer reads them.
"""

import argparse
import pathlib
import re
import sys
import tomllib

TAG_PATTERN = re.compile(r"v((?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-alpha\.(?:0|[1-9][0-9]*))?)")


def version_of_tag(tag):
    """The version of a vX.Y.Z or vX.Y.Z-alpha.N tag, or None."""
    match = TAG_PATTERN.fullmatch(tag)
    return match.group(1) if match else None


def is_prerelease(version):
    return "-" in version


def workspace_version(cargo_text):
    return tomllib.loads(cargo_text)["workspace"]["package"]["version"]


def release_notes(changelog_text, version):
    """Body of the CHANGELOG section of the version, or None when it is
    missing or empty."""
    lines = changelog_text.splitlines(keepends=True)
    heading = re.compile(rf"## v{re.escape(version)} \(\d{{4}}-\d{{2}}-\d{{2}}\)\s*")
    for i, line in enumerate(lines):
        if heading.fullmatch(line):
            body = []
            for rest in lines[i + 1 :]:
                if rest.startswith("## "):
                    break
                body.append(rest)
            text = "".join(body).strip()
            return text + "\n" if text else None
    return None


def arm64_notes(version):
    """The section that every release note ends with (ADR-0016).

    It lists the checks of the build job in release.yml: change both
    together. A smoke test (plan 0001, Phase 1 task 14) does not exist yet.
    """
    return (
        "\n### arm64版について\n\n"
        f"`FlyingMicrowave-{version}-windows-arm64.zip`は、動作を保証しない。"
        "リリースのワークフローで、Windows 11のarm64のランナーでのビルド、単体テスト、"
        "依存DLLの確認、バージョン情報の確認だけを行った。"
        "起動と描画の自動の確認(スモークテスト)と、実機での確認はしていない。\n"
    )


def use_utf8_output():
    # Why: same as check_publish_age.py; a Windows pipe is cp1252 and strict.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8")


def main(argv=None):
    use_utf8_output()
    parser = argparse.ArgumentParser(description="リリースのタグを確かめ、リリースノートを作る")
    parser.add_argument("tag")
    parser.add_argument("--root", default=".", help="リポジトリのルート")
    parser.add_argument("--notes", default="release-notes.md", help="リリースノートの出力先")
    args = parser.parse_args(argv)

    version = version_of_tag(args.tag)
    if version is None:
        print(f"タグの形が正しくない: {args.tag!r}(vX.Y.Z か vX.Y.Z-alpha.N)", file=sys.stderr)
        return 1
    root = pathlib.Path(args.root)
    cargo = workspace_version((root / "Cargo.toml").read_text(encoding="utf-8"))
    if cargo != version:
        print(f"Cargo.tomlのバージョン({cargo})がタグ({args.tag})と一致しない", file=sys.stderr)
        return 1
    notes = release_notes((root / "CHANGELOG.md").read_text(encoding="utf-8"), version)
    if notes is None:
        print(f"CHANGELOG.mdに「## v{version} (日付)」の節がないか、中身が空", file=sys.stderr)
        return 1
    pathlib.Path(args.notes).write_text(notes + arm64_notes(version), encoding="utf-8")
    kind = "プレリリース" if is_prerelease(version) else "リリース"
    print(f"{args.tag}({kind})のリリースノートを{args.notes}に書いた")
    return 0


if __name__ == "__main__":
    sys.exit(main())
