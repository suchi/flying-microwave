"""Tests of scripts/check_stdlib_only.py (plan 0007, D-1).

Run: python3 -I -m unittest discover -s scripts

Why: the Python scripts of the repository use the standard library only, so
no package from a registry comes in without the cooldown of 7 days (AGENTS.md,
Security). The check is an allow list (AGENTS.md, Security; docs/incidents/
0044 and its second recurrence): in Python files, the modules, the builtins,
the attributes of the modules that start programs or change where modules
come from, and the programs started; in the files that start Python
(workflows, hooks, the hook commands of .claude/settings.json, composite
actions), the forms of a Python start. The rest of a shell script is left to
the approval and review that those files need (ADR-0030).
"""

import pathlib
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import check_stdlib_only  # noqa: E402


def repo(files):
    """A git repository in a new folder with the given files, untracked."""
    d = tempfile.TemporaryDirectory()
    root = pathlib.Path(d.name)
    subprocess.run(["git", "init", "-q", str(root)], check=True, capture_output=True)
    for rel, text in files.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    return d, root


class StdlibOnlyTest(unittest.TestCase):
    def test_the_repository_passes(self):
        self.assertEqual(check_stdlib_only.problems(ROOT), [])

    def test_it_accepts_what_the_scripts_do(self):
        d, root = repo({
            "scripts/helper.py": "from __future__ import annotations\nimport json, os.path\n",
            "scripts/a.py": (
                "import helper\nfrom helper import x\nfrom unittest import mock\nimport os, pathlib, subprocess, sys\n"
                "ROOT = pathlib.Path(__file__).resolve().parent.parent\n"
                "sys.path.insert(0, str(ROOT / 'scripts'))\n"
                "subprocess.run(['git', 'status'], check=True)\n"
                "print(os.environ.get('PATH', ''), sys.argv)\n"
                "env = dict(os.environ, PATH='/bin', TMPDIR='/tmp')\n"
            ),
            ".github/workflows/x.yml": (
                "jobs:\n  a:\n    steps:\n      - run: python3 -I scripts/a.py --x\n"
                "      - run: |\n          python3 -I -m unittest discover -s scripts\n          python -I -m zipfile -l x.zip\n"
            ),
        })
        with d:
            self.assertEqual(check_stdlib_only.problems(root), [])

    def test_it_finds_the_ways_in(self):
        tries = {
            # Python files: modules, names, programs.
            "an import": ("scripts/a.py", "import requests\n"),
            "a from import": ("scripts/a.py", "from yaml import safe_load\n"),
            "an import in a function": ("scripts/a.py", "def f():\n    import numpy\n"),
            "a module not on the list": ("scripts/a.py", "import importlib\n"),
            "runpy": ("scripts/a.py", "import runpy\n"),
            "ctypes": ("scripts/a.py", "import ctypes\n"),
            "exec": ("scripts/a.py", "exec('import requests')\n"),
            "eval": ("scripts/a.py", "eval('1')\n"),
            "getattr": ("scripts/a.py", "getattr(__builtins__, 'exec')\n"),
            "__import__": ("scripts/a.py", "__import__('requests')\n"),
            "os.system": ("scripts/a.py", "import os\nos.system('pip install x')\n"),
            "os.execvp": ("scripts/a.py", "import os\nos.execvp('pip', ['pip'])\n"),
            "subprocess.Popen": ("scripts/a.py", "import subprocess\nsubprocess.Popen(['git'])\n"),
            "a program not on the list": ("scripts/a.py", "import subprocess\nsubprocess.run(['pip', 'install', 'x'])\n"),
            "sh -c": ("scripts/a.py", "import subprocess\nsubprocess.run(['sh', '-c', 'pip install x'])\n"),
            "shell=True": ("scripts/a.py", "import subprocess\nsubprocess.run('git status', shell=True)\n"),
            "a command in a variable": ("scripts/a.py", "import subprocess\ncmd = ['pip']\nsubprocess.run(cmd)\n"),
            "a split string": ("scripts/a.py", "import subprocess\nsubprocess.run('pip install x'.split())\n"),
            "an alias": ("scripts/a.py", "import subprocess as sp\nsp.Popen(['pip'])\n"),
            "a from import of subprocess": ("scripts/a.py", "from subprocess import Popen\nPopen(['pip'])\n"),
            "a module in a variable": ("scripts/a.py", "import os\nx = os\nx.system('pip')\n"),
            "sys.path outside": ("scripts/a.py", "import sys\nsys.path.insert(0, '/opt/site-packages')\n"),
            "sys.path assigned": ("scripts/a.py", "import sys\nsys.path = ['/opt']\n"),
            "sys.modules": ("scripts/a.py", "import sys\nsys.modules['x'] = None\n"),
            "a file that does not parse": ("scripts/a.py", "import (\n"),
            "subprocess.run kept in a name": ("scripts/a.py", "import subprocess\nr = subprocess.run\nr(['pip'])\n"),
            "os.environ written": ("scripts/a.py", "import os\nos.environ['PYTHONPATH'] = '/opt'\n"),
            "os.environ updated": ("scripts/a.py", "import os\nos.environ.update(PYTHONPATH='/opt')\n"),
            "an environment key not on the list": ("scripts/a.py", "import os\nenv = dict(os.environ, PYTHONPATH='/opt')\n"),
            "a file outside scripts": ("tools/b.py", "import rich\n"),
            "a .pyw file": ("tools/b.pyw", "import rich\n"),
            "a python shebang without .py": ("tools/run", "#!/usr/bin/env python3\nimport rich\n"),
            "a deep target folder": ("scripts/target/a.py", "import rich\n"),
            # Files that start Python.
            "python -c": (".github/workflows/x.yml", "run: python3 -c 'import yaml'\n"),
            "python from stdin": (".github/workflows/x.yml", "run: |\n  python3 - <<'PY'\n  import yaml\n  PY\n"),
            "python -m not on the list": (".github/workflows/x.yml", "run: python3 -I -m pytest\n"),
            "python -m pip": (".github/workflows/x.yml", "run: python3 -m pip install x\n"),
            "python without -I": (".github/workflows/x.yml", "run: python3 scripts/a.py\n"),
            "flags together": (".github/workflows/x.yml", "run: python3 -Im pytest\n"),
            "py launcher": (".github/workflows/x.yml", "run: py -m pytest\n"),
            "a path outside scripts": (".github/workflows/x.yml", "run: python3 -I /tmp/get-pip.py\n"),
            "after a hash in a string": (".github/workflows/x.yml", "run: echo 'a #1'; python3 -m pytest\n"),
            "a hook": (".claude/hooks/x.sh", "python3 -c 'import yaml'\n"),
            "a hook in a variable": (".claude/hooks/x.sh", "py=python3\n\"$py\" -c 'import yaml'\n"),
            "a hook command in settings": (".claude/settings.json", '{"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "python3 -c x"}]}]}}\n'),
            "a composite action": (".github/actions/a/action.yml", "runs:\n  using: composite\n  steps:\n    - run: python3 -m pytest\n"),
            # Slips that can happen without meaning to (re-review of plan
            # 0007, M-A and M-B; the check guards against slips, D-7).
            "an absolute path to python": (".github/workflows/x.yml", "run: /usr/bin/python3 -c 'import yaml'\n"),
            "Python in capitals": (".github/workflows/x.yml", "run: Python -c 'import yaml'\n"),
            "an escaped new line in YAML": (".github/workflows/x.yml", "run: \"echo a\\npython3 -c 'import yaml'\"\n"),
            "a second line in a settings command": (".claude/settings.json", '{"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "# x\\npython3 -c x"}]}]}}\n'),
            "a script path leaving scripts": (".github/workflows/x.yml", "run: python3 -I scripts/../tmp/get-pip.py\n"),
            "a script git does not list": (".github/workflows/x.yml", "run: python3 -I scripts/missing.py\n"),
            "unittest with a module name": (".github/workflows/x.yml", "run: python3 -I -m unittest yaml\n"),
            "env as a dict literal": ("scripts/a.py", "import subprocess\nsubprocess.run(['git'], env={'PYTHONPATH': '/opt'})\n"),
            "env changed after the copy": ("scripts/a.py", "import os, subprocess\nenv = dict(os.environ)\nenv['PYTHONPATH'] = '/opt'\nsubprocess.run(['git'], env=env)\n"),
            "a private attribute of a module": ("scripts/a.py", "import tempfile\ntempfile._os.system('pip install x')\n"),
        }
        for name, (rel, text) in tries.items():
            with self.subTest(name):
                d, root = repo({"scripts/ok.py": "import os\n", rel: text})
                with d:
                    self.assertTrue(check_stdlib_only.problems(root), "見つけられなかった")

    def test_a_file_outside_scripts_does_not_make_a_module_local(self):
        # python3 -I puts only the script's own folder on sys.path, so a
        # .py elsewhere does not stand in for a package (re-review, M-A).
        d, root = repo({"docs/requests.py": "x = 1\n", "scripts/a.py": "import requests\n"})
        with d:
            self.assertTrue(check_stdlib_only.problems(root))

    def test_ignored_files_are_left_out(self):
        # Build output that git ignores is not the repository's code.
        d, root = repo({".gitignore": "/target\n", "target/x/a.py": "import requests\n"})
        with d:
            self.assertEqual(check_stdlib_only.problems(root), [])

    def test_a_folder_that_is_not_a_repository_fails(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertTrue(check_stdlib_only.problems(pathlib.Path(d)))


if __name__ == "__main__":
    unittest.main()
