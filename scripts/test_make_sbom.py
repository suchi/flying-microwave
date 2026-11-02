"""Tests for make_sbom.py. Run: python3 -I -m unittest discover -s scripts"""

import json
import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import make_sbom as ms  # noqa: E402

REG = "registry+https://github.com/rust-lang/crates.io-index"
APP = "path+file:///w/crates/app#flying-microwave@0.1.0"
CORE = "path+file:///w/crates/core#flying-microwave-core@0.1.0"
LOG = f"{REG}#log@0.4.34"
WIN = f"{REG}#windows-sys@0.61.2"
EMBED = f"{REG}#embed-resource@3.0.12"
SYN = f"{REG}#syn@2.0.119"


def pkg(pid, name, version, license, source):
    return {"id": pid, "name": name, "version": version, "license": license, "source": source}


METADATA = {
    "workspace_members": [CORE, APP],
    "packages": [
        pkg(APP, "flying-microwave", "0.1.0", "MIT", None),
        pkg(CORE, "flying-microwave-core", "0.1.0", "MIT", None),
        pkg(LOG, "log", "0.4.34", "MIT OR Apache-2.0", REG),
        pkg(WIN, "windows-sys", "0.61.2", "Apache-2.0/MIT", REG),
        pkg(EMBED, "embed-resource", "3.0.12", "MIT", REG),
        pkg(SYN, "syn", "2.0.119", None, REG),
    ],
    "resolve": {
        "root": None,
        "nodes": [
            {
                "id": APP,
                "deps": [
                    {"pkg": EMBED, "dep_kinds": [{"kind": "build", "target": None}]},
                    {"pkg": CORE, "dep_kinds": [{"kind": None, "target": None}]},
                    {"pkg": LOG, "dep_kinds": [{"kind": None, "target": "cfg(windows)"}]},
                ],
            },
            {"id": CORE, "deps": [{"pkg": WIN, "dep_kinds": [{"kind": None, "target": None}]}]},
            {"id": LOG, "deps": [{"pkg": SYN, "dep_kinds": [{"kind": "dev", "target": None}]}]},
            {"id": WIN, "deps": []},
            {"id": EMBED, "deps": []},
            {"id": SYN, "deps": []},
        ],
    },
}

LOCK = f"""version = 4

[[package]]
name = "log"
version = "0.4.34"
source = "{REG}"
checksum = "{'a' * 64}"

[[package]]
name = "windows-sys"
version = "0.61.2"
source = "{REG}"
checksum = "{'b' * 64}"
"""


def document(**kw):
    args = dict(
        metadata=METADATA,
        lock_text=LOCK,
        target="x86_64-pc-windows-msvc",
        created="2026-11-02T00:00:00Z",
        namespace="https://example.invalid/spdx/x64",
    )
    args.update(kw)
    return ms.build_document(**args)


def by_name(doc):
    return {p["name"]: p for p in doc["packages"]}


class DocumentTest(unittest.TestCase):
    def test_has_the_required_document_fields_of_spdx_2_3(self):
        doc = document()
        self.assertEqual(doc["spdxVersion"], "SPDX-2.3")
        self.assertEqual(doc["dataLicense"], "CC0-1.0")
        self.assertEqual(doc["SPDXID"], "SPDXRef-DOCUMENT")
        self.assertEqual(doc["documentNamespace"], "https://example.invalid/spdx/x64")
        self.assertEqual(doc["creationInfo"]["created"], "2026-11-02T00:00:00Z")
        self.assertTrue(doc["creationInfo"]["creators"][0].startswith("Tool: "))
        self.assertIn("flying-microwave", doc["name"])

    def test_lists_only_the_normal_dependencies_of_the_executable(self):
        # Build and dev dependencies are not part of the shipped executable.
        self.assertEqual(sorted(by_name(document())), ["flying-microwave", "flying-microwave-core", "log", "windows-sys"])

    def test_registry_packages_have_purl_checksum_and_download_location(self):
        log = by_name(document())["log"]
        self.assertEqual(log["versionInfo"], "0.4.34")
        self.assertEqual(log["downloadLocation"], "https://crates.io/api/v1/crates/log/0.4.34/download")
        self.assertEqual(log["checksums"], [{"algorithm": "SHA256", "checksumValue": "a" * 64}])
        self.assertIn(
            {"referenceCategory": "PACKAGE-MANAGER", "referenceType": "purl", "referenceLocator": "pkg:cargo/log@0.4.34"},
            log["externalRefs"],
        )

    def test_licenses_are_spdx_expressions(self):
        packages = by_name(document())
        self.assertEqual(packages["log"]["licenseDeclared"], "MIT OR Apache-2.0")
        # The old slash form of Cargo is an OR expression.
        self.assertEqual(packages["windows-sys"]["licenseDeclared"], "Apache-2.0 OR MIT")
        self.assertEqual(packages["log"]["licenseConcluded"], "NOASSERTION")

    def test_workspace_packages_have_no_download_location(self):
        app = by_name(document())["flying-microwave"]
        self.assertEqual(app["downloadLocation"], "NOASSERTION")
        self.assertNotIn("checksums", app)

    def test_spdx_ids_are_unique_and_valid(self):
        ids = [p["SPDXID"] for p in document()["packages"]]
        self.assertEqual(len(ids), len(set(ids)))
        for spdx_id in ids:
            self.assertRegex(spdx_id, r"^SPDXRef-[A-Za-z0-9.\-]+$")

    def test_relationships_describe_the_executable_and_its_dependencies(self):
        doc = document()
        ids = {p["name"]: p["SPDXID"] for p in doc["packages"]}
        rels = {(r["spdxElementId"], r["relationshipType"], r["relatedSpdxElement"]) for r in doc["relationships"]}
        self.assertIn(("SPDXRef-DOCUMENT", "DESCRIBES", ids["flying-microwave"]), rels)
        self.assertIn((ids["flying-microwave"], "DEPENDS_ON", ids["flying-microwave-core"]), rels)
        self.assertIn((ids["flying-microwave-core"], "DEPENDS_ON", ids["windows-sys"]), rels)
        self.assertNotIn((ids["flying-microwave"], "DEPENDS_ON", "SPDXRef-Package-embed-resource-3.0.12"), rels)

    def test_is_json_serializable_and_stable(self):
        self.assertEqual(json.dumps(document(), sort_keys=True), json.dumps(document(), sort_keys=True))

    def test_the_creation_time_must_be_utc_with_seconds(self):
        # SPDX 2.3 creationInfo.created: YYYY-MM-DDThh:mm:ssZ.
        for created in ["2026-11-02", "2026-11-02T09:00:00+09:00", "2026-11-02T00:00Z"]:
            with self.subTest(created=created), self.assertRaises(ms.SbomError):
                document(created=created)

    def test_a_registry_package_without_checksum_fails(self):
        with self.assertRaises(ms.SbomError):
            document(lock_text=LOCK.replace(f'checksum = "{"a" * 64}"', ""))


if __name__ == "__main__":
    unittest.main()
