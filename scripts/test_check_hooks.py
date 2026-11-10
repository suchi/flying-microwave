"""Tests for check_hooks.py. Run: python3 -I -m unittest discover -s scripts"""

import contextlib
import io
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import check_hooks as ch  # noqa: E402

SETTINGS = {
    "hooks": {
        "PreToolUse": [
            {"matcher": "Bash", "hooks": [{"type": "command", "command": '"$CLAUDE_PROJECT_DIR/.claude/hooks/a.sh"'}]},
        ],
        "Stop": [
            {"hooks": [
                {"type": "command", "command": '"$CLAUDE_PROJECT_DIR/.claude/hooks/b.sh"'},
                {"type": "command", "command": '"$CLAUDE_PROJECT_DIR/.claude/hooks/a.sh"'},
            ]},
        ],
    }
}

LS_FILES = """100755 1111111111111111111111111111111111111111 0\t.claude/hooks/a.sh
100644 2222222222222222222222222222222222222222 0\t.claude/hooks/b.sh
"""


class RegisteredTest(unittest.TestCase):
    def test_lists_each_command_once_in_order(self):
        self.assertEqual(
            ch.registered_commands(SETTINGS),
            ['"$CLAUDE_PROJECT_DIR/.claude/hooks/a.sh"', '"$CLAUDE_PROJECT_DIR/.claude/hooks/b.sh"'],
        )

    def test_the_path_of_a_registered_command(self):
        self.assertEqual(ch.hook_path('"$CLAUDE_PROJECT_DIR/.claude/hooks/a.sh"'), ".claude/hooks/a.sh")

    def test_an_unexpected_command_form_has_no_path(self):
        # Only the form used in settings.json is understood; anything else
        # must fail the check instead of being skipped.
        for command in ["sh .claude/hooks/a.sh", '"/tmp/a.sh"', '"$CLAUDE_PROJECT_DIR/../a.sh"']:
            with self.subTest(command=command):
                self.assertIsNone(ch.hook_path(command))


class ModeTest(unittest.TestCase):
    def test_reads_the_modes_of_the_index(self):
        self.assertEqual(
            ch.index_modes(LS_FILES),
            {".claude/hooks/a.sh": "100755", ".claude/hooks/b.sh": "100644"},
        )


class DecisionTest(unittest.TestCase):
    def test_pre_tool_use_decisions(self):
        out = '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"x"}}'
        self.assertEqual(ch.decision(out), "deny")

    def test_stop_decisions(self):
        self.assertEqual(ch.decision('{"decision":"block","reason":"x"}'), "block")

    def test_no_output_is_no_decision(self):
        self.assertIsNone(ch.decision(""))
        self.assertIsNone(ch.decision("not json"))


class ProblemsTest(unittest.TestCase):
    def test_reports_a_hook_without_the_executable_bit(self):
        problems = ch.mode_problems([".claude/hooks/a.sh", ".claude/hooks/b.sh"], ch.index_modes(LS_FILES))
        self.assertEqual(len(problems), 1)
        self.assertIn(".claude/hooks/b.sh", problems[0])

    def test_reports_a_hook_missing_from_the_index(self):
        self.assertEqual(len(ch.mode_problems([".claude/hooks/c.sh"], {})), 1)

    def test_every_registered_hook_needs_a_sample(self):
        problems = ch.sample_problems([".claude/hooks/a.sh"], {})
        self.assertEqual(len(problems), 1)

    def test_a_hook_needs_both_a_stopped_and_a_passed_sample(self):
        # A hook that stops every input must not pass the check.
        only_stop = {".claude/hooks/a.sh": [({}, "deny")]}
        self.assertEqual(len(ch.sample_problems([".claude/hooks/a.sh"], only_stop)), 1)
        both = {".claude/hooks/a.sh": [({}, "deny"), ({}, None)]}
        self.assertEqual(ch.sample_problems([".claude/hooks/a.sh"], both), [])

    def test_shell_and_args_fields_are_not_supported(self):
        # The check starts hooks with sh -c only, so it cannot reproduce them.
        settings = {"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "x", "shell": "powershell"}]}]}}
        self.assertEqual(len(ch.unsupported_problems(settings)), 1)
        self.assertEqual(ch.unsupported_problems(SETTINGS), [])

    def test_the_real_hooks_have_samples(self):
        # Every hook registered in this repository has a sample input.
        root = pathlib.Path(__file__).resolve().parent.parent
        settings = ch.load_settings(root)
        paths = [ch.hook_path(c) for c in ch.registered_commands(settings)]
        self.assertEqual(ch.sample_problems(paths, ch.SAMPLES), [])


HOOK = """#!/bin/sh
cat > /dev/null
echo '{"decision":"block","reason":"x"}'
"""


class PlatformTest(unittest.TestCase):
    def test_windows_is_out_of_scope(self):
        # The check reproduces the Linux and macOS launch (sh -c and the
        # executable bit); on Windows it says so and does not run the hooks.
        out = io.StringIO()
        with mock.patch.object(ch, "running_on_windows", return_value=True), contextlib.redirect_stdout(out):
            self.assertEqual(ch.main(["--root", "."]), 0)
        self.assertIn("対象外", out.getvalue())

    @unittest.skipIf(os.name == "nt", "checks the platforms where the hooks run")
    def test_linux_and_macos_are_in_scope(self):
        # A wrong platform check would turn the CI check into a silent pass
        # (the failure mode of docs/incidents/0009).
        self.assertFalse(ch.running_on_windows())


# Why skip on Windows: these tests start hooks with sh -c and rely on the
# executable bit, which only Linux and macOS have (docs/incidents/0012).
@unittest.skipIf(ch.running_on_windows(), "the hook launch is checked on Linux and macOS only")
class RunTest(unittest.TestCase):
    def run_hook(self, executable):
        with tempfile.TemporaryDirectory() as d:
            hook = pathlib.Path(d, ".claude", "hooks", "a.sh")
            hook.parent.mkdir(parents=True)
            hook.write_text(HOOK, encoding="utf-8")
            os.chmod(hook, 0o755 if executable else 0o644)
            command = '"$CLAUDE_PROJECT_DIR/.claude/hooks/a.sh"'
            return ch.run_problems([command], {".claude/hooks/a.sh": [({}, "block")]}, pathlib.Path(d))

    def test_a_hook_started_as_registered_passes(self):
        self.assertEqual(self.run_hook(executable=True), [])

    def test_a_hook_without_the_executable_bit_fails_to_start(self):
        # The case of docs/incidents/0009: sh -c exits with 126.
        problems = self.run_hook(executable=False)
        self.assertEqual(len(problems), 1)
        self.assertIn("126", problems[0])

    def test_an_unexpected_decision_fails(self):
        with tempfile.TemporaryDirectory() as d:
            hook = pathlib.Path(d, ".claude", "hooks", "a.sh")
            hook.parent.mkdir(parents=True)
            hook.write_text(HOOK, encoding="utf-8")
            os.chmod(hook, 0o755)
            command = '"$CLAUDE_PROJECT_DIR/.claude/hooks/a.sh"'
            problems = ch.run_problems([command], {".claude/hooks/a.sh": [({}, None)]}, pathlib.Path(d))
            self.assertEqual(len(problems), 1)


# docs/incidents/0019: a native jq.exe started from Git Bash ends its output
# lines with CRLF. Every sample is also run with a jq that does the same.
COMPARE_HOOK = """#!/bin/sh
v=$(cat | jq -r '.v')
[ "$v" = "x" ] && printf '{"decision": "block"}\\n'
exit 0
"""

STRIP_HOOK = """#!/bin/sh
v=$(cat | jq -r '.v' | tr -d '\\r')
[ "$v" = "x" ] && printf '{"decision": "block"}\\n'
exit 0
"""


@unittest.skipIf(ch.running_on_windows(), "the hook launch is checked on Linux and macOS only")
@unittest.skipIf(shutil.which("jq") is None, "needs jq")
class CrlfTest(unittest.TestCase):
    def run_hook(self, text):
        with tempfile.TemporaryDirectory() as d:
            hook = pathlib.Path(d, ".claude", "hooks", "a.sh")
            hook.parent.mkdir(parents=True)
            hook.write_text(text, encoding="utf-8")
            os.chmod(hook, 0o755)
            command = '"$CLAUDE_PROJECT_DIR/.claude/hooks/a.sh"'
            return ch.run_problems([command], {".claude/hooks/a.sh": [({"v": "x"}, "block")]}, pathlib.Path(d))

    def test_a_hook_that_keeps_the_cr_fails_with_crlf_output(self):
        problems = self.run_hook(COMPARE_HOOK)
        self.assertEqual(len(problems), 1)
        self.assertIn("CRLF", problems[0])

    def test_a_hook_that_removes_the_cr_passes(self):
        self.assertEqual(self.run_hook(STRIP_HOOK), [])

    def run_crlf_jq(self, args, stdin):
        with tempfile.TemporaryDirectory() as d:
            ch.write_crlf_jq(d, shutil.which("jq"))
            return subprocess.run([str(pathlib.Path(d, "jq")), *args], input=stdin, capture_output=True)

    def test_the_crlf_jq_ends_every_line_with_crlf(self):
        result = self.run_crlf_jq(["-r", ".[]"], b'["a", "b"]')
        self.assertEqual(result.stdout, b"a\r\nb\r\n")

    def test_the_crlf_jq_keeps_the_exit_status(self):
        # jq -e exits with 1 when the result is false; hooks rely on it.
        self.assertEqual(self.run_crlf_jq(["-e", ".a"], b'{"a": false}').returncode, 1)
        self.assertEqual(self.run_crlf_jq(["."], b"not json").returncode, 5)

    def test_the_crlf_jq_adds_no_line_ending_to_output_without_one(self):
        self.assertEqual(self.run_crlf_jq(["-j", ".a"], b'{"a": "x"}').stdout, b"x")


if __name__ == "__main__":
    unittest.main()
