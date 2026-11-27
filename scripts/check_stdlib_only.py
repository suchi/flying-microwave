#!/usr/bin/env python3
"""Fail when the repository's Python would need a package from a registry.

The Python scripts use the standard library only (plan 0007, D-1), so that no
package comes in without the cooldown of 7 days that every dependency has
(AGENTS.md, Security). The check covers everything that runs Python from the
repository, not only scripts/ (docs/incidents/0044):

- every .py file: its imports, __import__ and importlib.import_module, which
  must name a module of the standard library or a .py file of the repository;
- the workflows and the Claude Code hooks: a module run with "python -m" must
  be in the standard library, and no package manager may be run.

In a .py file, a program started by subprocess or os (the first item of the
command) must not be a package manager, and "-m" must name a module of the
standard library. The list of package managers is a deny list, so the review
still looks for other ways in. Usage (from the repository root):

    python3 -I scripts/check_stdlib_only.py

Exit status: 0 when nothing is found, 1 otherwise. Messages are in Japanese
because the maintainer reads them (AGENTS.md).
"""

import ast
import pathlib
import re
import sys

SKIPPED_DIRS = {".git", "target", "__pycache__"}
STDLIB = set(sys.stdlib_module_names)
# Files that start Python or tools, besides the .py files.
RUNNERS = [".github/workflows/*.yml", ".github/workflows/*.yaml", ".claude/hooks/*"]
PACKAGE_MANAGERS = re.compile(
    r"(?<![\w.-])(pip3?|pipx|uv|uvx|poetry|pdm|conda|mamba|easy_install|npm|npx|yarn|pnpm|bun|gem)(?![\w.-])"
)
# Modules and builtins that load or run code by name, a path or a string:
# refused as a whole, as an import list does not see what they load. None of
# the scripts needs them.
CODE_LOADERS = {"importlib", "runpy", "site", "pkgutil", "zipimport", "imp", "builtins", "ctypes"}
CODE_BUILTINS = {"exec", "eval", "compile", "__import__"}
# Names that reach the builtins another way, and calls of os that replace or
# start a process without subprocess: refused as a whole too.
CODE_NAMES = {"__builtins__", "getattr", "globals", "vars"}
OS_STARTERS = re.compile(r"(exec|spawn|posix_spawn)[a-z]*|startfile")
# What a path added to sys.path may be built from (repository_path).
PATH_NAMES = {"str", "pathlib", "Path"}
PATH_ATTRS = {"Path", "resolve", "parent", "parents"}
# Settings in a workflow or hook that add places to look for modules.
PATH_SETTINGS = re.compile(r"\bPYTHON(PATH|HOME|STARTUP|USERBASE)\b")
# Calls that start a program (subprocess, os).
COMMAND_CALLS = {"run", "call", "check_call", "check_output", "Popen", "system", "popen", "getoutput", "getstatusoutput"}
PYTHON_M = re.compile(r"\bpython[0-9.]*(?:\.exe)?\b(?:\s+-[A-Za-z]+)*\s+-m\s+([A-Za-z_][\w.]*)")


def files(root):
    for path in sorted(root.rglob("*")):
        rel = path.relative_to(root)
        if path.is_file() and not SKIPPED_DIRS.intersection(rel.parts):
            yield path, rel


def is_sys_path(node):
    return (
        isinstance(node, ast.Attribute) and node.attr == "path"
        and isinstance(node.value, ast.Name) and node.value.id == "sys"
    )


def repository_path(node):
    """Whether a path is built from __file__ or ROOT only, with relative
    names: the way the tests of the repository find their modules."""
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


def literal(node):
    return node.value if isinstance(node, ast.Constant) and isinstance(node.value, str) else None


def module_problems(rel, tree, local):
    found = []

    def check(name, where):
        top = name.split(".")[0]
        if top in CODE_LOADERS:
            found.append(f"{rel}:{where}: コードを読み込むためのモジュール {name} を使っている")
        elif top not in STDLIB and top not in local:
            found.append(f"{rel}:{where}: 標準ライブラリにもリポジトリにもないモジュール {name}")

    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                check(alias.name, node.lineno)
        elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
            check(node.module, node.lineno)
        elif isinstance(node, ast.Name) and node.id in CODE_NAMES:
            found.append(f"{rel}:{node.lineno}: 組み込みの関数に名前の文字列で届きうる {node.id} を使っている")
        elif isinstance(node, (ast.Assign, ast.AugAssign, ast.AnnAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            if any(is_sys_path(t) for t in targets):
                found.append(f"{rel}:{node.lineno}: sys.path を置き換えている")
        elif isinstance(node, ast.Call):
            f = node.func
            name = f.attr if isinstance(f, ast.Attribute) else f.id if isinstance(f, ast.Name) else None
            if isinstance(f, ast.Name) and name in CODE_BUILTINS:
                found.append(f"{rel}:{node.lineno}: 文字列からコードを動かす {name} を使っている")
            elif (
                isinstance(f, ast.Attribute) and isinstance(f.value, ast.Name) and f.value.id == "os"
                and OS_STARTERS.fullmatch(f.attr)
            ):
                found.append(f"{rel}:{node.lineno}: subprocess を通さずにプログラムを起動する os.{f.attr} を使っている")
            elif isinstance(f, ast.Attribute) and is_sys_path(f.value):
                if not (name == "insert" and len(node.args) == 2 and repository_path(node.args[1])):
                    found.append(f"{rel}:{node.lineno}: sys.path にリポジトリの外の場所を加えうる(sys.path.{name})")
            elif name in ("__import__", "import_module"):
                arg = literal(node.args[0]) if node.args else None
                if arg is None:
                    found.append(f"{rel}:{node.lineno}: 読み込むモジュールの名前が文字列で書かれていない({name})")
                else:
                    check(arg, node.lineno)
            elif name in COMMAND_CALLS and node.args:
                found += command_problems(rel, node.lineno, node.args[0])
    return found


def command_problems(rel, lineno, arg):
    """The program a command call starts: the first item of a list, or the
    first word of a shell string."""
    if isinstance(arg, (ast.List, ast.Tuple)):
        words = [literal(e) for e in arg.elts]
    else:
        text = literal(arg)
        words = text.split() if text else []
    found = []
    if words and words[0] and PACKAGE_MANAGERS.fullmatch(pathlib.PurePath(words[0]).name):
        found.append(f"{rel}:{lineno}: パッケージを入れる道具 {words[0]}")
    for i, w in enumerate(words[:-1]):
        nxt = words[i + 1]
        if w == "-m" and nxt and nxt.split(".")[0] not in STDLIB:
            found.append(f"{rel}:{lineno}: -m で標準ライブラリにないモジュール {nxt}")
    return found


def runner_problems(rel, text):
    found = []
    for n, line in enumerate(text.splitlines(), 1):
        code = line.split("#", 1)[0] if not line.lstrip().startswith("#!") else ""
        for m in PYTHON_M.finditer(code):
            if m.group(1).split(".")[0] not in STDLIB:
                found.append(f"{rel}:{n}: python -m で標準ライブラリにないモジュール {m.group(1)}")
        m = PACKAGE_MANAGERS.search(code)
        if m:
            found.append(f"{rel}:{n}: パッケージを入れる道具 {m.group(1)}")
        m = PATH_SETTINGS.search(code)
        if m:
            found.append(f"{rel}:{n}: モジュールを探す場所を変える設定 {m.group(0)}")
    return found


def problems(root):
    root = pathlib.Path(root)
    all_files = list(files(root))
    local = {path.stem for path, _ in all_files if path.suffix == ".py"}
    runners = {p for pattern in RUNNERS for p in root.glob(pattern) if p.is_file()}
    found = []
    for path, rel in all_files:
        if path.suffix == ".py":
            try:
                tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(rel))
            except SyntaxError as e:
                found.append(f"{rel}:{e.lineno}: 読めない(構文エラー)ため、確かめられない")
                continue
            found += module_problems(rel, tree, local)
        elif path in runners:
            found += runner_problems(rel, path.read_text(encoding="utf-8"))
    return found


def main():
    root = pathlib.Path(__file__).resolve().parent.parent
    found = problems(root)
    for line in found:
        print(line)
    if found:
        print("Pythonのスクリプトは標準ライブラリだけを使う(計画0007のD-1)。外部のパッケージが要るときは、メンテナーの承認を得る。")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
