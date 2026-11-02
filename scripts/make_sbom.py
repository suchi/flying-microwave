#!/usr/bin/env python3
"""Write an SPDX 2.3 SBOM (JSON) of the shipped executable for one target.

Used by .github/workflows/release.yml: each release zip carries the SBOM of
its target. The packages are the executable (flying-microwave) and the
crates used to build it: what `cargo tree -e normal --target <target>`
shows, the same set as THIRD-PARTY-LICENSES.txt (cargo-about). Procedural
macros are kept, because the code they generate is in the executable.
Dependencies of build scripts and tests are left out, and so are optional
dependencies whose feature is off (cargo tree resolves features; the
`resolve` of cargo metadata does not). The licenses and sources come from
`cargo metadata --filter-platform <target>`, and the checksums from
Cargo.lock (the SHA-256 that crates.io publishes).

Only the Python standard library is used, so that the release adds no
dependency (2026-11-02, maintainer decision).

Usage:
    python3 -I scripts/make_sbom.py --target TRIPLE --created RFC3339
        --namespace URI --out FILE [--root DIR]

Exit status: 0 when the SBOM was written, 1 when an input was inconsistent.
Messages are in Japanese because the maintainer reads them.
"""

import argparse
import json
import pathlib
import re
import subprocess
import sys
import tomllib

EXECUTABLE = "flying-microwave"
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
TOOL = "Tool: flying-microwave-make_sbom"


class SbomError(Exception):
    pass


def spdx_id(name, version):
    """A valid, readable SPDX identifier ([A-Za-z0-9.-] only)."""
    return "SPDXRef-Package-" + re.sub(r"[^A-Za-z0-9.\-]", "-", f"{name}-{version}")


def license_expression(license):
    """The SPDX expression of a Cargo license field. Cargo's old slash form
    (`MIT/Apache-2.0`) means OR. A slash mixed with other operators or
    parentheses has no single meaning, so it fails instead of being guessed."""
    if not license:
        return "NOASSERTION"
    if "/" not in license:
        return license
    if re.search(r"[()]|\b(AND|OR|WITH)\b", license):
        raise SbomError(f"ライセンスの表記の意味が1つに決まらない: {license!r}")
    return " OR ".join(part.strip() for part in license.split("/"))


def lock_checksums(lock_text):
    """{(name, version): sha256} of the crates.io packages in Cargo.lock."""
    lock = tomllib.loads(lock_text)
    return {
        (p["name"], p["version"]): p["checksum"]
        for p in lock.get("package", [])
        if p.get("source") == REGISTRY and "checksum" in p
    }


def tree_dependencies(metadata, tree_text):
    """Package ids shown by `cargo tree --prefix depth -f '{p}'`, starting at
    the executable, and the edges between them."""
    by_key = {}
    for p in metadata["packages"]:
        key = (p["name"], p["version"])
        if key in by_key:
            raise SbomError(f"{p['name']} {p['version']}が2つの出所にあり、区別できない")
        by_key[key] = p["id"]
    ids, edges, parents = set(), [], []
    for line in tree_text.splitlines():
        if not line.strip():
            continue
        match = re.fullmatch(r"(\d+)(\S+) v(\S+)(?: .*)?", line)
        if not match:
            raise SbomError(f"cargo treeの出力の行を読めない: {line!r}")
        depth, name, version = int(match[1]), match[2], match[3]
        pid = by_key.get((name, version))
        if pid is None:
            raise SbomError(f"cargo treeの{name} {version}が、cargo metadataにない")
        if depth == 0:
            if ids or name != EXECUTABLE:
                raise SbomError(f"cargo treeの出力が{EXECUTABLE}の1つだけから始まっていない: {line!r}")
        elif depth > len(parents):
            raise SbomError(f"cargo treeの出力の深さが飛んでいる: {line!r}")
        else:
            edges.append((parents[depth - 1], pid))
        del parents[depth:]
        parents.append(pid)
        ids.add(pid)
    if not ids:
        raise SbomError("cargo treeの出力が空")
    return by_key[(EXECUTABLE, metadata_version(metadata))], ids, edges


def metadata_version(metadata):
    roots = [p for p in metadata["packages"] if p["id"] in metadata["workspace_members"] and p["name"] == EXECUTABLE]
    if len(roots) != 1:
        raise SbomError(f"ワークスペースに{EXECUTABLE}が1つだけあるはずが、{len(roots)}個ある")
    return roots[0]["version"]


def package_entry(package, checksums):
    name, version = package["name"], package["version"]
    entry = {
        "SPDXID": spdx_id(name, version),
        "name": name,
        "versionInfo": version,
        "filesAnalyzed": False,
        "licenseConcluded": "NOASSERTION",
        "licenseDeclared": license_expression(package.get("license")),
        "copyrightText": "NOASSERTION",
    }
    if package.get("source") == REGISTRY:
        checksum = checksums.get((name, version))
        if checksum is None:
            raise SbomError(f"Cargo.lockに{name} {version}のチェックサムがない")
        entry["downloadLocation"] = f"https://crates.io/api/v1/crates/{name}/{version}/download"
        entry["checksums"] = [{"algorithm": "SHA256", "checksumValue": checksum}]
        entry["externalRefs"] = [
            {
                "referenceCategory": "PACKAGE-MANAGER",
                "referenceType": "purl",
                "referenceLocator": f"pkg:cargo/{name}@{version}",
            }
        ]
    else:
        # Workspace crates: built from this repository, not downloaded.
        entry["downloadLocation"] = "NOASSERTION"
    return entry


def build_document(metadata, tree_text, lock_text, target, created, namespace):
    # SPDX 2.3 creationInfo.created is YYYY-MM-DDThh:mm:ssZ (UTC).
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", created):
        raise SbomError(f"作成日時の形が正しくない: {created!r}(YYYY-MM-DDThh:mm:ssZ)")
    root, ids, edges = tree_dependencies(metadata, tree_text)
    packages = {p["id"]: p for p in metadata["packages"]}
    checksums = lock_checksums(lock_text)
    entries = sorted((package_entry(packages[pid], checksums) for pid in ids), key=lambda e: e["SPDXID"])
    spdx = {pid: spdx_id(packages[pid]["name"], packages[pid]["version"]) for pid in ids}
    relationships = [{"spdxElementId": "SPDXRef-DOCUMENT", "relationshipType": "DESCRIBES", "relatedSpdxElement": spdx[root]}]
    relationships += sorted(
        ({"spdxElementId": spdx[a], "relationshipType": "DEPENDS_ON", "relatedSpdxElement": spdx[b]} for a, b in set(edges)),
        key=lambda r: (r["spdxElementId"], r["relatedSpdxElement"]),
    )
    version = packages[root]["version"]
    return {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": f"{EXECUTABLE}-{version}-{target}",
        "documentNamespace": namespace,
        "creationInfo": {"created": created, "creators": [TOOL]},
        "packages": entries,
        "relationships": relationships,
    }


def cargo(root, *args):
    # Why encoding: cargo writes UTF-8, and on Windows text=True alone would
    # decode it with the locale code page.
    return subprocess.run(
        ["cargo", *args], cwd=root, check=True, capture_output=True, text=True, encoding="utf-8"
    ).stdout


def use_utf8_output():
    # Why: same as check_publish_age.py; a Windows pipe is cp1252 and strict.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8")


def main(argv=None):
    use_utf8_output()
    parser = argparse.ArgumentParser(description="実行ファイルのSBOM(SPDX 2.3のJSON)を作る")
    parser.add_argument("--target", required=True, help="Rustのターゲット(例: x86_64-pc-windows-msvc)")
    parser.add_argument("--created", required=True, help="作成日時(RFC 3339。リリースのコミットの日時)")
    parser.add_argument("--namespace", required=True, help="文書の名前空間(リリースと対象ごとに一意のURI)")
    parser.add_argument("--out", required=True, help="出力するファイル")
    parser.add_argument("--root", default=".", help="リポジトリのルート")
    args = parser.parse_args(argv)
    root = pathlib.Path(args.root)
    try:
        metadata = json.loads(
            cargo(root, "metadata", "--format-version", "1", "--locked", "--filter-platform", args.target)
        )
        tree_text = cargo(
            root, "tree", "--locked", "-p", EXECUTABLE, "-e", "normal",
            "--target", args.target, "--prefix", "depth", "-f", "{p}",
        )
    except subprocess.CalledProcessError as e:
        print(f"{' '.join(e.cmd)}が失敗した(終了コード{e.returncode}):\n{e.stderr}", file=sys.stderr)
        return 1
    try:
        doc = build_document(metadata, tree_text, (root / "Cargo.lock").read_text(encoding="utf-8"), args.target, args.created, args.namespace)
    except SbomError as e:
        print(e, file=sys.stderr)
        return 1
    pathlib.Path(args.out).write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"{args.out}を書いた({len(doc['packages'])}個のパッケージ)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
