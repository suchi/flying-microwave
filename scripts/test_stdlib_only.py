"""Tests of scripts/check_stdlib_only.py (plan 0007, D-1).

Run: python3 -I -m unittest discover -s scripts

Why: the Python scripts of the repository use the standard library only, so
no package from a registry comes in without the cooldown of 7 days (AGENTS.md,
Security). The check covers everything that runs Python from the repository:
every .py file, and the workflows and hooks that start Python or a package
manager (docs/incidents/0044: the whole protected target, not one place).
"""

import pathlib
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import check_stdlib_only  # noqa: E402


def write(root, rel, text):
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


class StdlibOnlyTest(unittest.TestCase):
    def test_the_repository_uses_the_standard_library_only(self):
        self.assertEqual(check_stdlib_only.problems(ROOT), [])

    def test_it_finds_the_ways_in(self):
        tries = {
            "an import": ("scripts/a.py", "import requests\n"),
            "a from import": ("scripts/a.py", "from yaml import safe_load\n"),
            "a dotted import": ("scripts/a.py", "import google.protobuf\n"),
            "an import in a function": ("scripts/a.py", "def f():\n    import numpy\n"),
            "a file outside scripts": ("tools/b.py", "import rich\n"),
            "__import__": ("scripts/a.py", "__import__('requests')\n"),
            "importlib": ("scripts/a.py", "import importlib\nimportlib.import_module('requests')\n"),
            "importlib from": ("scripts/a.py", "from importlib import import_module\nimport_module('x')\n"),
            "a computed name": ("scripts/a.py", "import importlib\nname = 'os'\nimportlib.import_module(name)\n"),
            "python -m in a workflow": (".github/workflows/x.yml", "run: python3 -I -m pytest\n"),
            "python -m in a hook": (".claude/hooks/x.sh", "python -m black .\n"),
            "pip in a workflow": (".github/workflows/x.yml", "run: pip install requests\n"),
            "python -m pip": (".github/workflows/x.yml", "run: python3 -m pip install x\n"),
            "uv in a hook": (".claude/hooks/x.sh", "uv pip install x\n"),
            "pipx in a script": ("scripts/a.py", "import subprocess\nsubprocess.run(['pipx', 'run', 'x'])\n"),
            "npm in a workflow": (".github/workflows/x.yml", "run: npx some-tool\n"),
            "a file that does not parse": ("scripts/a.py", "import (\n"),
            # Ways to load code that an import list does not see: the modules
            # and builtins that load code are refused as a whole.
            "exec": ("scripts/a.py", "exec('import requests')\n"),
            "eval": ("scripts/a.py", "eval('1')\n"),
            "compile": ("scripts/a.py", "compile('x', 'x', 'exec')\n"),
            "runpy": ("scripts/a.py", "import runpy\nrunpy.run_module('pytest')\n"),
            "importlib.util": ("scripts/a.py", "import importlib.util\n"),
            "from importlib": ("scripts/a.py", "from importlib.util import spec_from_file_location\n"),
            "site": ("scripts/a.py", "import site\nsite.addsitedir('/opt/x')\n"),
            "pkgutil": ("scripts/a.py", "import pkgutil\n"),
            "zipimport": ("scripts/a.py", "import zipimport\n"),
            "sys.path with a path outside": ("scripts/a.py", "import sys\nsys.path.insert(0, '/opt/site-packages')\n"),
            "sys.path from the environment": ("scripts/a.py", "import os, sys\nsys.path.append(os.environ['X'])\n"),
            "sys.path assigned": ("scripts/a.py", "import sys\nsys.path = ['/opt']\n"),
            "sys.path extended": ("scripts/a.py", "import sys\nsys.path += ['/opt']\n"),
            "PYTHONPATH in a workflow": (".github/workflows/x.yml", "env:\n  PYTHONPATH: /opt/x\n"),
            "builtins.exec": ("scripts/a.py", "import builtins\nbuiltins.exec('import x')\n"),
            "__builtins__": ("scripts/a.py", "__builtins__['exec']('import x')\n"),
            "getattr": ("scripts/a.py", "getattr(__builtins__, 'exec')('import x')\n"),
            "os.execvp": ("scripts/a.py", "import os\nos.execvp('pip', ['pip', 'install', 'x'])\n"),
            "os.spawnlp": ("scripts/a.py", "import os\nos.spawnlp(os.P_WAIT, 'pip', 'pip', 'install', 'x')\n"),
            "ctypes": ("scripts/a.py", "import ctypes\nctypes.CDLL('/tmp/x.so')\n"),
        }
        for name, (rel, text) in tries.items():
            with self.subTest(name), tempfile.TemporaryDirectory() as d:
                root = pathlib.Path(d)
                write(root, "scripts/ok.py", "import os\n")
                write(root, rel, text)
                self.assertTrue(check_stdlib_only.problems(root), "見つけられなかった")

    def test_it_accepts_the_standard_library_and_local_modules(self):
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            write(root, "scripts/icon/make_art.py", "import math\n")
            write(root, "scripts/helper.py", "from __future__ import annotations\nimport json, os.path\n")
            write(root, "scripts/a.py", "import helper\nimport make_art\nfrom . import helper\n")
            # The ways the tests of the repository find their modules.
            write(
                root,
                "scripts/test_a.py",
                "import pathlib, re, sys\n"
                "ROOT = pathlib.Path(__file__).resolve().parent.parent\n"
                "sys.path.insert(0, str(ROOT / 'scripts' / 'icon'))\n"
                "sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))\n"
                "re.compile('x')\n",
            )
            write(root, ".github/workflows/x.yml", "run: python3 -I -m unittest discover -s scripts\nrun: python3 -m zipfile -l x.zip\n")
            self.assertEqual(check_stdlib_only.problems(root), [])

    def test_the_directories_of_tools_are_skipped(self):
        # Build output and git data are not the repository's code.
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            write(root, "target/x/a.py", "import requests\n")
            write(root, ".git/hooks/a.py", "import requests\n")
            self.assertEqual(check_stdlib_only.problems(root), [])


if __name__ == "__main__":
    unittest.main()
