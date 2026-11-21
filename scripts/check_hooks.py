#!/usr/bin/env python3
"""Check that every Claude Code hook in .claude/settings.json really runs.

Claude Code treats a hook that exits with a status other than 0 or 2 as a
non-blocking error and goes on. The guard hooks were registered by path but
committed without the executable bit, so each call failed with status 126
and nothing was ever blocked (docs/incidents/0009). This check:

1. requires every registered hook to be mode 100755 in the git index, and
2. runs each registered command exactly as written in settings.json, through
   `sh -c` with CLAUDE_PROJECT_DIR set, with sample inputs: at least one the
   hook must stop and one it must let through (exit 0, no decision), and
   checks the decision it prints. The passed sample also catches a hook that
   stops everything, for example when jq is missing.

A registered hook without both kinds of sample fails the check, so that a new
hook cannot be added without being exercised.

Scope: this reproduces how Claude Code starts hooks on Linux and macOS
(`sh -c`). On Windows Claude Code uses Git Bash, or PowerShell when Git Bash
is missing, which this check does not reproduce: on Windows the check only
says that it is out of scope and exits with 0 (docs/incidents/0012). Hooks
registered with the `shell` or `args` fields fail the check for the same
reason.

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
import shlex
import shutil
import subprocess
import sys
import tempfile

COMMAND_PATTERN = re.compile(r'"\$CLAUDE_PROJECT_DIR/(\.claude/hooks/[A-Za-z0-9_-]+\.sh)"')

# Sample inputs per hook: (input, expected decision). None means the hook
# must let the input through (exit 0 and no decision). Hooks start in a
# temporary git repository on the branch named by "_branch" (removed from the
# input; check-hooks by default), so the branch checked out where this script
# runs does not change the result: the CI on main once failed only there
# (docs/incidents/0039).
SAMPLES = {
    ".claude/hooks/block-dangerous-git.sh": [
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git push origin main"}}, "deny"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git push -u origin fix/x"}}, None),
        # docs/reviews/0005, K-1: other ways of pushing to main.
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git -C . push origin main"}}, "deny"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git push origin HEAD:refs/heads/main"}}, "deny"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git push origin +HEAD:main"}}, "deny"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git push origin fix/x:main"}}, "deny"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git push origin +main"}}, "deny"),
        # Branches whose names only start or end with main are not main.
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git push -u origin main-fix"}}, None),
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git push origin fix/main"}}, None),
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git push origin HEAD:feature/main.v2"}}, None),
        # Options before push are accepted; other words before it are not a push.
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git -c core.pager=cat push origin main"}}, "deny"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git commit -m 'block pushing to main'"}}, None),
        # Plan 0005, D-24: skipping the hooks also with git options or with -n.
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git -C . commit --" + "no-verify -m a"}}, "deny"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git commit -n -m a"}}, "deny"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git commit -an -m a"}}, "deny"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git commit --amend -m a"}}, None),
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git push -n -u origin fix/x"}}, None),
        # A commit on main is asked about (the samples above run on a feature
        # branch; docs/incidents/0039).
        ({"_branch": "main", "hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git commit -m a"}}, "ask"),
        # docs/incidents/0034: a commit with changed incident records runs the
        # list test, in a copy of the project ("_project"), and stops when the
        # test fails (a record missing from the lists).
        ({"_project": "unlisted", "hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git commit -m a"}}, "deny"),
        ({"_project": "listed", "hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "git commit -m a"}}, None),
    ],
    ".claude/hooks/protect-config.sh": [
        ({"hook_event_name": "PreToolUse", "tool_name": "Edit", "tool_input": {"file_path": ".claude/settings.json"}}, "ask"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Edit", "tool_input": {"file_path": "crates/app/src/main.rs"}}, None),
        # docs/reviews/0005, J-6: the rule files.
        ({"hook_event_name": "PreToolUse", "tool_name": "Edit", "tool_input": {"file_path": "AGENTS.md"}}, "ask"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Write", "tool_input": {"file_path": "CLAUDE.md"}}, "ask"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Edit", "tool_input": {"file_path": ".agents/skills/adr/SKILL.md"}}, "ask"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Edit", "tool_input": {"file_path": "docs/README.md"}}, None),
        # Plan 0005, D-24: the commands and the skills of Claude Code.
        ({"hook_event_name": "PreToolUse", "tool_name": "Edit", "tool_input": {"file_path": ".claude/commands/merge-pr.md"}}, "ask"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Write", "tool_input": {"file_path": ".claude/skills/adr/SKILL.md"}}, "ask"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Edit", "tool_input": {"file_path": "docs/development/commands.md"}}, None),
    ],
    ".claude/hooks/require-japanese-description.sh": [
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "ls", "description": "List files"}}, "deny"),
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "ls", "description": "ファイルを一覧表示する"}}, None),
        # An empty value must pass with CRLF jq output too (docs/incidents/0019).
        ({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "ls", "description": ""}}, None),
    ],
    # docs/incidents/0018 and 0022: the text written before tool calls, read
    # from sample transcripts (scripts/hook_samples/). transcript.jsonl has a
    # line whose message is not an object, a message whose text and two
    # parallel calls are on separate lines, one with text and call on one
    # line, an English reply without a call (left to the Stop hook), and a
    # last line cut off without a newline, as while it is being written. Each
    # English-only message is reported once per session, oldest first, so the
    # third call of the same session passes.
    ".claude/hooks/require-japanese-note.sh": [
        ({"hook_event_name": "PreToolUse", "session_id": "check-note", "tool_name": "Bash", "tool_input": {}, "transcript_path": "scripts/hook_samples/transcript.jsonl", "tool_use_id": "toolu_new"}, "deny"),
        ({"hook_event_name": "PreToolUse", "session_id": "check-note", "tool_name": "Bash", "tool_input": {}, "transcript_path": "scripts/hook_samples/transcript.jsonl", "tool_use_id": "toolu_new2"}, "deny"),
        ({"hook_event_name": "PreToolUse", "session_id": "check-note", "tool_name": "Bash", "tool_input": {}, "transcript_path": "scripts/hook_samples/transcript.jsonl", "tool_use_id": "toolu_new3"}, None),
        # A subagent's call is not stopped by the main agent's text.
        ({"hook_event_name": "PreToolUse", "session_id": "check-note-sub", "agent_id": "a1", "tool_name": "Bash", "tool_input": {}, "transcript_path": "scripts/hook_samples/transcript.jsonl", "tool_use_id": "toolu_sub"}, None),
        # Only Japanese text, or no transcript.
        ({"hook_event_name": "PreToolUse", "session_id": "check-note-ja", "tool_name": "Bash", "tool_input": {}, "transcript_path": "scripts/hook_samples/transcript-ja.jsonl", "tool_use_id": "toolu_new"}, None),
        ({"hook_event_name": "PreToolUse", "session_id": "check-note-missing", "tool_name": "Bash", "tool_input": {}, "transcript_path": "scripts/hook_samples/missing.jsonl", "tool_use_id": "toolu_new"}, None),
        # docs/incidents/0037: a message with no text and English-only thinking
        # is reported once, without stopping the call.
        ({"hook_event_name": "PreToolUse", "session_id": "check-note-think", "tool_name": "Bash", "tool_input": {}, "transcript_path": "scripts/hook_samples/transcript-thinking.jsonl", "tool_use_id": "toolu_new"}, "context"),
        ({"hook_event_name": "PreToolUse", "session_id": "check-note-think", "tool_name": "Bash", "tool_input": {}, "transcript_path": "scripts/hook_samples/transcript-thinking.jsonl", "tool_use_id": "toolu_new2"}, None),
    ],
    ".claude/hooks/require-japanese-reply.sh": [
        ({"hook_event_name": "Stop", "stop_hook_active": False, "last_assistant_message": "Done."}, "block"),
        ({"hook_event_name": "Stop", "stop_hook_active": False, "last_assistant_message": "終わりました。"}, None),
        ({"hook_event_name": "Stop", "stop_hook_active": False, "last_assistant_message": ""}, None),
        # docs/incidents/0015: checked also while another Stop hook keeps the
        # turn going, with a limit on the rewrites asked for in a row.
        ({"hook_event_name": "Stop", "session_id": "check-hooks-limit", "stop_hook_active": False, "last_assistant_message": "Done."}, "block"),
        ({"hook_event_name": "Stop", "session_id": "check-hooks-limit", "stop_hook_active": True, "last_assistant_message": "Done."}, "block"),
        # The main agent is at the limit here; a subagent has its own count.
        ({"hook_event_name": "SubagentStop", "session_id": "check-hooks-limit", "agent_id": "a1", "stop_hook_active": True, "last_assistant_message": "Done."}, "block"),
        ({"hook_event_name": "Stop", "session_id": "check-hooks-limit", "stop_hook_active": True, "last_assistant_message": "Done."}, None),
        ({"hook_event_name": "Stop", "session_id": "check-hooks-limit", "stop_hook_active": True, "last_assistant_message": "終わりました。"}, None),
        ({"hook_event_name": "Stop", "session_id": "check-hooks-limit", "stop_hook_active": True, "last_assistant_message": "Done."}, "block"),
    ],
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
    """The decision a hook printed (deny, ask, block), "context" for text
    passed to Claude without a decision, or None."""
    try:
        data = json.loads(output)
    except ValueError:
        return None
    if not isinstance(data, dict):
        return None
    specific = data.get("hookSpecificOutput")
    if isinstance(specific, dict) and specific.get("permissionDecision"):
        return specific["permissionDecision"]
    if isinstance(specific, dict) and specific.get("additionalContext") and not data.get("decision"):
        return "context"
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
    """A hook needs a sample it stops and a sample it lets through."""
    problems = []
    for path in paths:
        expected = [e for _, e in samples.get(path, [])]
        if not any(e is not None for e in expected) or None not in expected:
            problems.append(f"{path}: 止めるべき入力と通すべき入力のサンプルがそろっていない(check_hooks.pyのSAMPLESに加える)")
    return problems


def unsupported_problems(settings):
    """Registered hooks this check cannot start as Claude Code would."""
    problems = []
    for groups in settings.get("hooks", {}).values():
        for group in groups:
            for hook in group.get("hooks", []):
                if hook.get("type") != "command" or "shell" in hook or "args" in hook:
                    problems.append(f"{hook}: この検査で再現できない登録(typeがcommandで、shellとargsのないものだけを扱う)")
    return problems


CRLF_JQ = """#!/bin/sh
# Imitates a native jq.exe started from Git Bash on Windows, which turns
# every LF of its output into CRLF (jq manual, --binary; docs/incidents/0019).
# The exit status of jq is kept, and output that does not end with a newline
# (jq -j) gets no line ending added.
out=$(mktemp) || exit 1
{real} "$@" > "$out"
status=$?
if [ -s "$out" ] && [ -z "$(tail -c 1 "$out" | tr -d '\\n')" ]; then nl=1; else nl=0; fi
awk -v nl="$nl" 'NR > 1 {{ printf "%s\\r\\n", prev }} {{ prev = $0 }} END {{ if (NR > 0) {{ if (nl) printf "%s\\r\\n", prev; else printf "%s", prev }} }}' "$out"
rm -f "$out"
exit $status
"""


def write_crlf_jq(directory, real_jq):
    """Writes a jq into directory that ends every output line with CRLF."""
    script = pathlib.Path(directory, "jq")
    script.write_text(CRLF_JQ.format(real=shlex.quote(real_jq)), encoding="utf-8")
    os.chmod(script, 0o755)


def run_problems(commands, samples, root):
    problems = []
    # Why a temporary directory: hooks may keep state in it (docs/incidents/0015),
    # and runs at the same time must not share that state.
    with tempfile.TemporaryDirectory() as temp:
        modes = [("", os.environ.get("PATH", ""))]
        real_jq = shutil.which("jq")
        # Why a second run: a native jq.exe from Git Bash turns every LF of its
        # output into CRLF, and a stray CR made hooks miss what they guard
        # (docs/incidents/0019). Windows itself is out of scope here.
        if real_jq is not None:
            crlf_dir = pathlib.Path(temp, "crlf-jq")
            crlf_dir.mkdir()
            write_crlf_jq(crlf_dir, real_jq)
            modes.append(("(jqの出力がCRLFのとき)", str(crlf_dir) + os.pathsep + os.environ.get("PATH", "")))
        # A sample that already failed with the real jq is reported once.
        failed = set()
        for number, (label, search_path) in enumerate(modes):
            # Why a directory per mode: require-japanese-reply.sh keeps a count
            # between samples, and each mode must start from no count.
            state = pathlib.Path(temp, f"state-{number}")
            state.mkdir()
            env = dict(
                os.environ, PATH=search_path, CLAUDE_PROJECT_DIR=str(root), TMPDIR=str(state), XDG_RUNTIME_DIR=str(state)
            )
            for command in commands:
                path = hook_path(command)
                for index, (sample, expected) in enumerate(samples[path]):
                    if (path, index) in failed:
                        continue
                    sample = dict(sample)
                    cwd = branch_repo(temp, sample.pop("_branch", "check-hooks"))
                    # Why a copy: block-dangerous-git.sh looks at the working
                    # tree of CLAUDE_PROJECT_DIR, and a changed record in root
                    # must not change the result (docs/incidents/0039).
                    run_env = dict(env, CLAUDE_PROJECT_DIR=str(copy_project(temp, root, "clean")))
                    project = sample.pop("_project", None)
                    if project is not None:
                        cwd = copy_project(temp, root, project)
                        run_env = dict(env, CLAUDE_PROJECT_DIR=str(cwd))
                    # Paths in the samples are relative to the repository root,
                    # but the hooks start in the temporary repository.
                    transcript = sample.get("transcript_path")
                    if transcript and not os.path.isabs(transcript):
                        sample["transcript_path"] = str(pathlib.Path(root, transcript))
                    # Why sh -c: on Linux and macOS, Claude Code passes the command
                    # string to sh -c (Hooks reference), so this is how it starts.
                    result = subprocess.run(
                        ["sh", "-c", command], input=json.dumps(sample), capture_output=True, text=True, cwd=cwd, env=run_env
                    )
                    got = decision(result.stdout)
                    if result.returncode != 0 or got != expected:
                        failed.add((path, index))
                        detail = (result.stderr or result.stdout).strip()[:300]
                        problems.append(
                            f"{path}{label}: 終了コード{result.returncode}、判断{got!r}(期待は{expected!r}) {detail}"
                        )
    return problems


def branch_repo(temp, branch):
    """An empty git repository under temp whose current branch is branch,
    made on first use."""
    path = pathlib.Path(temp, "repo-" + branch)
    if not path.exists():
        subprocess.run(["git", "init", "-q", "-b", branch, str(path)], check=True, capture_output=True)
    return path


def copy_project(temp, root, name):
    """A copy of the hooks, the list test and the incident lists in a new git
    repository. "clean" commits everything, so no record counts as changed
    whatever the working tree of root holds. Otherwise nothing is committed,
    so every copied record counts as changed, and "unlisted" adds a record
    that no list names."""
    path = pathlib.Path(temp, "project-" + name)
    if path.exists():
        return path
    subprocess.run(["git", "init", "-q", "-b", "check-hooks", str(path)], check=True, capture_output=True)
    # The hooks come from the working tree, so a hook can be checked before it
    # is committed. The records and lists come from HEAD, so a record being
    # written in root does not change the result.
    hooks = pathlib.Path(root, ".claude", "hooks")
    # A root without some of these (the tests of this script) is copied as
    # far as it goes.
    for name_ in [str(p.relative_to(root)) for p in sorted(hooks.glob("*.sh"))] + ["scripts/test_incident_lists.py"]:
        if not pathlib.Path(root, name_).is_file():
            continue
        target = path / name_
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(pathlib.Path(root, name_), target)
    tree = subprocess.run(
        ["git", "-C", str(root), "ls-tree", "-r", "--name-only", "HEAD", "--", "docs/README.md", "docs/incidents"],
        capture_output=True, text=True,
    )
    listed = tree.stdout.split() if tree.returncode == 0 else []
    for name_ in [n for n in listed if n.endswith(".md")]:
        target = path / name_
        target.parent.mkdir(parents=True, exist_ok=True)
        content = subprocess.run(["git", "-C", str(root), "show", "HEAD:" + name_], check=True, capture_output=True).stdout
        target.write_bytes(content)
    if name == "unlisted":
        (path / "docs" / "incidents" / "9999-unlisted.md").write_text("# 9999: unlisted\n", encoding="utf-8")
    if name == "clean":
        git = ["git", "-C", str(path), "-c", "user.name=check-hooks", "-c", "user.email=check-hooks@example.invalid"]
        subprocess.run(git + ["add", "-A"], check=True, capture_output=True)
        subprocess.run(git + ["commit", "-q", "--no-gpg-sign", "-m", "copy"], check=True, capture_output=True)
    return path


def use_utf8_output():
    # Why: same as check_publish_age.py; a Windows pipe is cp1252 and strict.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8")


def running_on_windows():
    return os.name == "nt"


def main(argv=None):
    use_utf8_output()
    parser = argparse.ArgumentParser(description="Claude Codeのフックが実際に動くかを確かめる")
    parser.add_argument("--root", default=".", help="リポジトリのルート")
    args = parser.parse_args(argv)
    root = pathlib.Path(args.root).resolve()

    # Why: the check reproduces how Claude Code starts hooks on Linux and
    # macOS (sh -c, and the executable bit of docs/incidents/0009). Windows
    # has neither, so a run there would say nothing about those platforms
    # (docs/incidents/0012). CI runs the check on Linux.
    if running_on_windows():
        print("Windowsは対象外(フックの起動はLinuxとmacOSで確かめる。CIのLinuxのジョブで毎回確かめている)")
        return 0

    # Why: without jq every hook here either stops nothing or asks for
    # everything, so the result would say nothing about the hooks.
    if shutil.which("jq") is None:
        print("jqが見つからないため、フックを確かめられない(onboardingの6.3)", file=sys.stderr)
        return 1
    settings = load_settings(root)
    commands = registered_commands(settings)
    unknown = [c for c in commands if hook_path(c) is None]
    problems = unsupported_problems(settings)
    problems += [f"{c}: 想定していない形の登録(\"$CLAUDE_PROJECT_DIR/.claude/hooks/<名前>.sh\"の形にする)" for c in unknown]
    commands = [c for c in commands if hook_path(c) is not None]
    paths = [hook_path(c) for c in commands]

    try:
        ls_files = subprocess.run(
            ["git", "ls-files", "-s", "--", ".claude/hooks"], capture_output=True, text=True, cwd=root, check=True
        ).stdout
    except (OSError, subprocess.CalledProcessError) as e:
        print(f"gitでフックのモードを読めないため、確かめられない: {e}", file=sys.stderr)
        return 1
    problems += mode_problems(paths, index_modes(ls_files))
    problems += sample_problems(paths, SAMPLES)
    if not problems:
        problems += run_problems(commands, SAMPLES, root)

    for problem in problems:
        print(problem, file=sys.stderr)
    if problems:
        print(f"動かないフックがある({len(problems)}件)", file=sys.stderr)
        return 1
    print(f"{len(commands)}個のフックは、どれも登録したとおりに起動し、止めるべき入力を止め、通すべき入力を通した")
    return 0


if __name__ == "__main__":
    sys.exit(main())
