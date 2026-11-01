#!/usr/bin/env python3
"""Check that every Claude Code hook in .claude/settings.json really runs.

Claude Code treats a hook that exits with a status other than 0 or 2 as a
non-blocking error and goes on. The guard hooks were registered by path but
committed without the executable bit, so each call failed with status 126
and nothing was ever blocked (docs/incidents/0009). This check:

1. requires every registered hook to be mode 100755 in the git index, and
2. runs each registered command exactly as written in settings.json, through
   `sh -c` with CLAUDE_PROJECT_DIR set, with a sample input that the hook
   must stop, and checks the decision it prints.

A registered hook without a sample input fails the check, so that a new hook
cannot be added without being exercised.

Usage:
    python3 -I scripts/check_hooks.py [--root DIR]

Exit status: 0 when every hook passed, 1 otherwise. Requires git and jq.
Messages are in Japanese because the maintainer reads them.
"""

import argparse
import json
import os
import pathlib
import re
import subprocess
import sys

COMMAND_PATTERN = re.compile(r'"\$CLAUDE_PROJECT_DIR/(\.claude/hooks/[A-Za-z0-9_-]+\.sh)"')

# A sample input per hook that the hook must stop, and the expected decision.
SAMPLES = {
    ".claude/hooks/block-dangerous-git.sh": (
        {"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git push origin main"}},
        "deny",
    ),
    ".claude/hooks/protect-config.sh": (
        {"hook_event_name": "PreToolUse", "tool_name": "Edit", "tool_input": {"file_path": ".claude/settings.json"}},
        "ask",
    ),
    ".claude/hooks/require-japanese-description.sh": (
        {"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "ls", "description": "List files"}},
        "deny",
    ),
    ".claude/hooks/require-japanese-reply.sh": (
        {"hook_event_name": "Stop", "stop_hook_active": False, "last_assistant_message": "Done."},
        "block",
    ),
}


def load_settings(root):
    return json.loads((pathlib.Path(root) / ".claude" / "settings.json").read_text(encoding="utf-8"))


def registered_commands(settings):
    """Every hook command in settings.json, once each, in order."""
    commands = []
    for groups in settings.get("hooks", {}).values():
        for group in groups:
            for hook in group.get("hooks", []):
                command = hook.get("command")
                if command is not None and command not in commands:
                    commands.append(command)
    return commands


def hook_path(command):
    """The repository path of a command in the form settings.json uses, or None."""
    match = COMMAND_PATTERN.fullmatch(command)
    return match.group(1) if match else None


def index_modes(ls_files_text):
    """{path: mode} from the output of `git ls-files -s`."""
    modes = {}
    for line in ls_files_text.splitlines():
        meta, _, path = line.partition("\t")
        if path:
            modes[path] = meta.split()[0]
    return modes


def decision(output):
    """The decision a hook printed (deny, ask, block), or None."""
    try:
        data = json.loads(output)
    except ValueError:
        return None
    if not isinstance(data, dict):
        return None
    specific = data.get("hookSpecificOutput")
    if isinstance(specific, dict) and specific.get("permissionDecision"):
        return specific["permissionDecision"]
    return data.get("decision")


def mode_problems(paths, modes):
    problems = []
    for path in paths:
        mode = modes.get(path)
        if mode is None:
            problems.append(f"{path}: gitで追跡されていない")
        elif mode != "100755":
            problems.append(f"{path}: 実行権限がない(gitのモードが{mode}。100755にする)")
    return problems


def sample_problems(paths, samples):
    return [f"{path}: 確かめるためのサンプル入力がない(check_hooks.pyのSAMPLESに加える)" for path in paths if path not in samples]


def run_problems(commands, samples, root):
    problems = []
    env = dict(os.environ, CLAUDE_PROJECT_DIR=str(root))
    for command in commands:
        path = hook_path(command)
        sample, expected = samples[path]
        # Why sh -c: Claude Code runs the command string through a shell, so
        # this reproduces how the hook is really started.
        result = subprocess.run(
            ["sh", "-c", command], input=json.dumps(sample), capture_output=True, text=True, cwd=root, env=env
        )
        got = decision(result.stdout)
        if result.returncode != 0 or got != expected:
            detail = (result.stderr or result.stdout).strip()[:300]
            problems.append(f"{path}: 終了コード{result.returncode}、判断{got!r}(期待は{expected!r}) {detail}")
    return problems


def use_utf8_output():
    # Why: same as check_publish_age.py; a Windows pipe is cp1252 and strict.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8")


def main(argv=None):
    use_utf8_output()
    parser = argparse.ArgumentParser(description="Claude Codeのフックが実際に動くかを確かめる")
    parser.add_argument("--root", default=".", help="リポジトリのルート")
    args = parser.parse_args(argv)
    root = pathlib.Path(args.root).resolve()

    commands = registered_commands(load_settings(root))
    unknown = [c for c in commands if hook_path(c) is None]
    problems = [f"{c}: 想定していない形の登録(\"$CLAUDE_PROJECT_DIR/.claude/hooks/<名前>.sh\"の形にする)" for c in unknown]
    commands = [c for c in commands if hook_path(c) is not None]
    paths = [hook_path(c) for c in commands]

    ls_files = subprocess.run(
        ["git", "ls-files", "-s", "--", ".claude/hooks"], capture_output=True, text=True, cwd=root, check=True
    ).stdout
    problems += mode_problems(paths, index_modes(ls_files))
    problems += sample_problems(paths, SAMPLES)
    if not problems:
        problems += run_problems(commands, SAMPLES, root)

    for problem in problems:
        print(problem, file=sys.stderr)
    if problems:
        print(f"動かないフックがある({len(problems)}件)", file=sys.stderr)
        return 1
    print(f"{len(commands)}個のフックは、どれも登録したとおりに起動し、サンプル入力を止めた")
    return 0


if __name__ == "__main__":
    sys.exit(main())
