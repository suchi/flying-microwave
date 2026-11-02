#!/usr/bin/env python3
"""Write an SPDX 2.3 SBOM (JSON) of the shipped executable for one target.

Used by .github/workflows/release.yml: each release zip carries the SBOM of
its target. The packages are the executable (flying-microwave) and the
crates it depends on through normal dependencies for the target, as
resolved by `cargo metadata --filter-platform <target>`. Build and dev
dependencies are left out, because they are not part of the executable.
Checksums come from Cargo.lock (the SHA-256 that crates.io publishes).

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
    (`MIT/Apache-2.0`) means OR."""
    if not license:
        return "NOASSERTION"
    return " OR ".join(part.strip() for part in license.split("/"))


def lock_checksums(lock_text):
    """{(name, version): sha256} of the crates.io packages in Cargo.lock."""
    lock = tomllib.loads(lock_text)
    return {
        (p["name"], p["version"]): p["checksum"]
        for p in lock.get("package", [])
        if p.get("source") == REGISTRY and "checksum" in p
    }


def normal_dependencies(metadata):
    """Package ids reachable from the executable through normal
    dependencies, and the edges between them."""
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    packages = {p["id"]: p for p in metadata["packages"]}
    roots = [pid for pid in metadata["workspace_members"] if packages[pid]["name"] == EXECUTABLE]
    if len(roots) != 1:
        raise SbomError(f"ワークスペースに{EXECUTABLE}が1つだけあるはずが、{len(roots)}個ある")
    seen, edges, stack = {roots[0]}, [], [roots[0]]
    while stack:
        pid = stack.pop()
        for dep in nodes[pid]["deps"]:
            if any(k["kind"] is None for k in dep["dep_kinds"]):
                edges.append((pid, dep["pkg"]))
                if dep["pkg"] not in seen:
                    seen.add(dep["pkg"])
                    stack.append(dep["pkg"])
    return roots[0], seen, edges


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


def build_document(metadata, lock_text, target, created, namespace):
    # SPDX 2.3 creationInfo.created is YYYY-MM-DDThh:mm:ssZ (UTC).
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", created):
        raise SbomError(f"作成日時の形が正しくない: {created!r}(YYYY-MM-DDThh:mm:ssZ)")
    root, ids, edges = normal_dependencies(metadata)
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
    metadata = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", args.target],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    try:
        doc = build_document(metadata, (root / "Cargo.lock").read_text(encoding="utf-8"), args.target, args.created, args.namespace)
    except SbomError as e:
        print(e, file=sys.stderr)
        return 1
    pathlib.Path(args.out).write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"{args.out}を書いた({len(doc['packages'])}個のパッケージ)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
