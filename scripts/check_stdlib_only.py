#!/usr/bin/env python3
"""Fail when the repository's Python could use code from outside it.

The Python scripts use the standard library only (plan 0007, D-1; AGENTS.md,
Security), so that no package comes in without the cooldown of 7 days. The
check is an allow list over everything that runs Python from the repository
(AGENTS.md, Security; docs/incidents/0044 and its second recurrence):

- Python files (.py, .pyw, or a "#!...python" first line): the modules, the
  builtins, the attributes of the modules that start programs or change where
  modules come from (os, subprocess, shutil, sys), and the programs started
  must be on the lists below. Those modules may not be renamed, imported
  from, or used other than through an attribute. shell=True is refused.
- Files that start Python (workflows, composite actions, Claude Code hooks
  and the hook commands of .claude/settings.json): every Python start must be
  "python3 -I scripts/<file>.py" or "python3 -I -m <module on the list>".

The check guards against slips: a script that comes to need a package from a
registry without anyone meaning it (plan 0007, D-7). It does not stand
against code written to get around it, which can as well change this check;
that is left to the pull request and its review that every change to scripts/
and the workflows goes through. Not reproduced here, so not guarded: how
Python resolves a name (a module reached through another module's attribute,
a builtin rebound in one scope), what the arguments of an allowed program do
(git -c alias, cargo install, the sh -c of check_hooks.py), unittest loading
a module by name, and where ROOT and __file__ point.

What else a shell script runs is not read here: a shell has no grammar the
standard library can parse, so a list of commands would be wrong in many
places. Those files need the maintainer's approval and a full review to
change (AGENTS.md, Security), and that is what this part relies on (ADR-0030).

The files are those git lists (tracked, and untracked but not ignored), as in
check_invisible_chars.py. A new module, builtin or program is added to a list
here, which a review sees. Usage (from the repository root):

    python3 -I scripts/check_stdlib_only.py

Exit status: 0 when nothing is found, 1 otherwise. Messages are in Japanese
because the maintainer reads them (AGENTS.md).
"""

import ast
import json
import pathlib
import re
import subprocess
import sys

# Modules of the standard library the scripts may import (by the name before
# the first dot). Code loaders (importlib, runpy, site, pkgutil, zipimport,
# ctypes, builtins) are not on it.
MODULES = {
    "__future__", "argparse", "ast", "contextlib", "datetime", "html", "io", "json", "math", "os",
    "pathlib", "re", "shlex", "shutil", "struct", "subprocess", "sys", "tempfile", "tomllib",
    "unittest", "urllib", "zipfile",
}
# Builtins the scripts may name. exec, eval, compile, __import__, getattr,
# globals, vars and __builtins__ are not on it.
BUILTINS = {
    "Exception", "FileNotFoundError", "IsADirectoryError", "OSError", "SyntaxError",
    "UnicodeDecodeError", "ValueError", "__file__", "__name__", "all", "any", "bool", "bytes",
    "dict", "enumerate", "float", "hasattr", "int", "isinstance", "len", "license", "list", "map",
    "max", "min", "next", "open", "ord", "print", "range", "repr", "set", "sorted", "str", "sum",
    "super", "tuple", "zip",
}
# The attributes that may be used of the modules that start programs or change
# where modules come from. sys.path is checked further (repository_path).
ATTRIBUTES = {
    "os": {"chmod", "environ", "getuid", "name", "path", "pathsep"},
    "subprocess": {"CalledProcessError", "run"},
    "shutil": {"copy2", "copytree", "which"},
    "sys": {"argv", "exit", "path", "stderr", "stdlib_module_names", "stdout", "version_info"},
}
# The programs subprocess.run may start: the first item of the command list,
# as source text. "*" is for every file.
PROGRAMS = {
    "*": {"'git'", "'cargo'"},
    # The hook commands of .claude/settings.json are run as Claude Code runs
    # them (sh -c), and git is a list built from 'git' (check_hooks.py).
    "scripts/check_hooks.py": {"'sh'", "git"},
    "scripts/test_check_hooks.py": {"'sh'", "str(pathlib.Path(d, 'jq'))"},
    # resvg is found with shutil.which and its version is checked first.
    "scripts/icon/make_ico.py": {"resvg"},
}
# What a path added to sys.path may be built from (repository_path).
PATH_NAMES = {"str", "pathlib", "Path"}
# The environment variables a script may set for a child process (the hook
# checks run the hooks with these; check_hooks.py and its tests).
ENVIRONMENT_KEYS = {"PATH", "CLAUDE_PROJECT_DIR", "TMPDIR", "XDG_RUNTIME_DIR"}
PATH_ATTRS = {"Path", "resolve", "parent", "parents"}

# Files that start Python, besides the Python files.
RUNNERS = re.compile(r"\.github/workflows/[^/]+\.ya?ml|\.github/actions/.+|\.claude/hooks/[^/]+|\.claude/settings(\.local)?\.json")
# A word that starts Python in a shell: python, python3, python3.12, py,
# with .exe, or a variable named py ($py, "$py", ${py}).
# A path before the word (/usr/bin/python3) still starts Python, and Windows
# finds Python in any case.
PYTHON_WORD = re.compile(r"""(?<![\w.$-])(?:python(?:[0-9.]+)?|py)(?:\.exe)?(?![\w.-])|"?\$\{?py\}?"?(?![\w])""", re.I)
# The forms a Python start may take: a script of scripts/ that git lists
# (checked in runner_problems), or one of the module runs the workflows and
# hooks use, with their arguments (unittest discover over scripts/, zipfile).
PYTHON_ARGS = re.compile(
    r"\s+-I\s+(?:"
    r"(?P<script>scripts/(?:[\w-]+/)*[\w-]+\.py)"
    r"|-m\s+unittest\s+discover\s+-s\s+scripts(?:\s+-p\s+[\w.*-]+)?"
    r"|-m\s+zipfile\s+-[cl]"
    r")(?=\s|$|[;&|)])"
)
# Lines that name Python without starting it with arguments, as they are.
# Why: block-dangerous-git.sh looks for the first Python that runs (for c in
# python3 python, then "$c" -c '' to see that it starts), keeps it in py and
# tests that py is set; the line that runs a script uses
# "$py" -I -m unittest, which PYTHON_ARGS accepts.
RUNNER_LINES = {
    ".claude/hooks/block-dangerous-git.sh": {
        "py=",
        "for c in python3 python; do",
        "if command -v \"$c\" >/dev/null 2>&1 && \"$c\" -c '' >/dev/null 2>&1; then",
        "py=$c",
        "if [ -n \"$py\" ] && [ -f \"$root/scripts/test_incident_lists.py\" ] &&",
    },
}


def listed_files(root):
    """The files git lists, as paths relative to root."""
    out = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=root, capture_output=True, check=True,
    ).stdout
    return sorted({pathlib.PurePosixPath(p.decode("utf-8")) for p in out.split(b"\0") if p})


def is_python(root, rel):
    if rel.suffix in (".py", ".pyw"):
        return True
    try:
        with open(root / rel, "rb") as f:
            first = f.readline(200)
    except OSError:
        return False
    return first.startswith(b"#!") and b"python" in first


def repository_path(node):
    """Whether a path is built from __file__ or ROOT only, with plain names."""
    rooted = False
    for sub in ast.walk(node):
        if isinstance(sub, ast.Name):
            if sub.id in ("__file__", "ROOT"):
                rooted = True
            elif sub.id not in PATH_NAMES:
                return False
        elif isinstance(sub, ast.Constant):
            if not (isinstance(sub.value, str) and re.fullmatch(r"[A-Za-z0-9_-]+", sub.value)):
                return False
        elif isinstance(sub, ast.Attribute):
            if sub.attr not in PATH_ATTRS:
                return False
        elif not isinstance(sub, (ast.Call, ast.BinOp, ast.Div, ast.Load)):
            return False
    return rooted


def program(node):
    """The program of a command list, as source text, or None."""
    if isinstance(node, (ast.List, ast.Tuple)) and node.elts and not isinstance(node.elts[0], ast.Starred):
        return ast.unparse(node.elts[0])
    if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Add) and isinstance(node.left, ast.Name):
        return node.left.id
    return None


def python_problems(rel, text, local):
    try:
        tree = ast.parse(text, filename=str(rel))
    except SyntaxError as e:
        return [f"{rel}:{e.lineno}: 読めない(構文エラー)ため、確かめられない"]
    parents = {child: node for node in ast.walk(tree) for child in ast.iter_child_nodes(node)}
    programs = PROGRAMS["*"] | PROGRAMS.get(str(rel), set())
    defined = {n.id for n in ast.walk(tree) if isinstance(n, ast.Name) and isinstance(n.ctx, ast.Store)}
    defined |= {a.arg for a in ast.walk(tree) if isinstance(a, ast.arg)}
    defined |= {n.name for n in ast.walk(tree) if isinstance(n, (ast.FunctionDef, ast.ClassDef))}
    defined |= {n.name for n in ast.walk(tree) if isinstance(n, ast.ExceptHandler) and n.name}
    imported = set()
    for n in ast.walk(tree):
        if isinstance(n, (ast.Import, ast.ImportFrom)):
            imported |= {(a.asname or a.name).split(".")[0] for a in n.names}
    # Names that only ever hold a copy of os.environ with allowed keys, and
    # names changed through a subscript (env["X"] = ...).
    assigned = {}
    for n in ast.walk(tree):
        if isinstance(n, ast.Assign):
            for target in n.targets:
                if isinstance(target, ast.Name):
                    assigned.setdefault(target.id, []).append(n.value)
    environ_names = set()
    while True:
        more = {name for name, values in assigned.items() if all(is_environ_copy(v, environ_names) for v in values)}
        if more <= environ_names:
            break
        environ_names |= more
    changed = {
        n.value.id for n in ast.walk(tree)
        if isinstance(n, ast.Subscript) and isinstance(n.ctx, (ast.Store, ast.Del)) and isinstance(n.value, ast.Name)
    }
    changed |= {
        n.func.value.id for n in ast.walk(tree)
        if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and isinstance(n.func.value, ast.Name)
        and n.func.attr in ("update", "setdefault", "pop", "popitem", "clear", "__setitem__")
    }
    found = []

    def say(node, message):
        found.append(f"{rel}:{node.lineno if hasattr(node, 'lineno') else 0}: {message}")

    def check_module(node, name):
        top = name.split(".")[0]
        if top not in MODULES and top not in local:
            say(node, f"許可の一覧にないモジュール {name}")

    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                check_module(node, alias.name)
                if alias.asname and alias.name.split(".")[0] in ATTRIBUTES:
                    say(node, f"{alias.name} を別の名前({alias.asname})で取り込んでいる")
        elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
            check_module(node, node.module)
            if node.module.split(".")[0] in ATTRIBUTES:
                say(node, f"{node.module} から名前を取り込んでいる(属性として使う)")
        elif isinstance(node, ast.Name):
            if node.id in ATTRIBUTES and node.id not in defined and not isinstance(parents.get(node), ast.Attribute):
                say(node, f"{node.id} を属性の参照のほかの形で使っている")
            elif isinstance(node.ctx, ast.Load) and node.id not in defined | imported | BUILTINS:
                # A name the file neither defines nor imports is a builtin
                # (or an error): it must be on the list.
                say(node, f"許可の一覧にない組み込みの名前 {node.id}")
        elif isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name) and node.value.id in ATTRIBUTES:
            if node.attr not in ATTRIBUTES[node.value.id]:
                say(node, f"許可の一覧にない {node.value.id}.{node.attr}")
            elif node.value.id == "sys" and node.attr == "path":
                parent = parents.get(node)
                call = parents.get(parent)
                ok = (
                    isinstance(parent, ast.Attribute) and parent.attr == "insert"
                    and isinstance(call, ast.Call) and call.func is parent
                    and len(call.args) == 2 and repository_path(call.args[1])
                )
                if not ok:
                    say(node, "sys.path を、__file__かROOTから作ったリポジトリの中の道を先頭に加えるほかの形で使っている")
            elif node.value.id == "subprocess" and node.attr == "run":
                # Only called on the spot, so that the program it starts is
                # checked below.
                parent = parents.get(node)
                if not (isinstance(parent, ast.Call) and parent.func is node):
                    say(node, "subprocess.run を、その場で呼ぶほかの形で使っている")
            elif node.value.id == "os" and node.attr == "environ":
                # Read with .get, or copied for a child with keys on the list:
                # a changed PYTHONPATH and the like would change where Python
                # finds its modules.
                parent = parents.get(node)
                call = parents.get(parent)
                read = (
                    isinstance(parent, ast.Attribute) and parent.attr == "get"
                    and isinstance(call, ast.Call) and call.func is parent
                )
                copied = (
                    isinstance(parent, ast.Call) and isinstance(parent.func, ast.Name) and parent.func.id == "dict"
                    and parent.args and parent.args[0] is node and len(parent.args) == 1
                    and all(k.arg in ENVIRONMENT_KEYS for k in parent.keywords)
                )
                if not (read or copied):
                    say(node, "os.environ を、.get で読むか、許可したキーだけを足して写すほかの形で使っている")
        elif isinstance(node, ast.Call):
            f = node.func
            if isinstance(f, ast.Attribute) and isinstance(f.value, ast.Name) and f.value.id == "subprocess" and f.attr == "run":
                name = program(node.args[0]) if node.args else None
                if name not in programs:
                    say(node, f"許可の一覧にないプログラムを起動している({name or '一覧でない形'})")
            for k in node.keywords:
                if k.arg == "shell" and not (isinstance(k.value, ast.Constant) and k.value.value is False):
                    say(node, "shell= を使っている")
                if k.arg == "env" and not (
                    is_environ_copy(k.value, environ_names)
                    or (isinstance(k.value, ast.Name) and k.value.id in environ_names and k.value.id not in changed)
                ):
                    # The environment of a child is os.environ with keys on
                    # the list, and nothing else (re-review of plan 0007, M-A).
                    say(node, "env= に、os.environ に許可したキーだけを足して写したもののほかを渡している")
        if (
            isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name)
            and node.value.id in imported and node.attr.startswith("_")
        ):
            say(node, f"取り込んだモジュールの内側の名前 {node.value.id}.{node.attr} を使っている")
    return found


def is_environ_copy(node, copies=()):
    """dict(os.environ, KEY=...), or dict(<a name in copies>, KEY=...), with
    keys on ENVIRONMENT_KEYS only."""
    if not (
        isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == "dict"
        and len(node.args) == 1 and all(k.arg in ENVIRONMENT_KEYS for k in node.keywords)
    ):
        return False
    base = node.args[0]
    if isinstance(base, ast.Name):
        return base.id in copies
    return (
        isinstance(base, ast.Attribute) and isinstance(base.value, ast.Name)
        and base.value.id == "os" and base.attr == "environ"
    )


def runner_lines(rel, text):
    """The lines of a file that starts Python, to look for Python starts.

    A hook command of settings.json is split into its lines, as sh -c runs
    each. In a workflow, the escapes \\n and \\t of a YAML double-quoted
    string are read as a new line and a space, as YAML reads them (re-review
    of plan 0007, M-B)."""
    if rel.name.startswith("settings") and rel.suffix == ".json":
        commands = []

        def walk(o):
            if isinstance(o, dict):
                for k, v in o.items():
                    if k == "command" and isinstance(v, str):
                        commands.append(v)
                    walk(v)
            elif isinstance(o, list):
                for v in o:
                    walk(v)

        walk(json.loads(text))
        return [(0, line) for c in commands for line in c.splitlines()]
    lines = []
    for n, line in enumerate(text.splitlines(), 1):
        for part in line.replace("\\n", "\n").replace("\\t", " ").split("\n"):
            lines.append((n, part))
    return lines


def runner_problems(rel, text, files):
    found = []
    allowed = RUNNER_LINES.get(str(rel), set())
    for n, line in runner_lines(rel, text):
        code = line.strip()
        if not code or code.startswith("#") or code in allowed:
            continue
        for m in PYTHON_WORD.finditer(line):
            args = PYTHON_ARGS.match(line, m.end())
            if args is None:
                found.append(f"{rel}:{n}: Pythonを、-I scripts/<ファイル>.py か、一覧にある -I -m の形のほかの形で起動している")
            elif args.group("script") and pathlib.PurePosixPath(args.group("script")) not in files:
                found.append(f"{rel}:{n}: gitが一覧にしないスクリプト {args.group('script')} を起動している")
    return found


def problems(root):
    root = pathlib.Path(root)
    try:
        files = listed_files(root)
    except (OSError, subprocess.CalledProcessError) as e:
        return [f"gitでファイルの一覧を作れないため、確かめられない: {e}"]
    # Only the .py files of scripts/ stand in for a module: python3 -I puts
    # the script's own folder on sys.path, and the tests add scripts/ and
    # scripts/icon/ (re-review of plan 0007, M-A).
    local = {p.stem for p in files if p.suffix in (".py", ".pyw") and p.parts[0] == "scripts"}
    listed = set(files)
    found = []
    for rel in files:
        path = root / rel
        if not path.is_file():
            continue
        if is_python(root, rel):
            found += python_problems(rel, path.read_text(encoding="utf-8"), local)
        elif RUNNERS.fullmatch(str(rel)):
            found += runner_problems(rel, path.read_text(encoding="utf-8"), listed)
    return found


def main():
    root = pathlib.Path(__file__).resolve().parent.parent
    found = problems(root)
    for line in found:
        print(line)
    if found:
        print("Pythonのスクリプトは標準ライブラリだけを使う(計画0007のD-1)。許可の一覧を広げるときは、レビューを受ける。")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
