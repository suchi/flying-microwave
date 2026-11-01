"""Tests for check_hooks.py. Run: python3 -I -m unittest discover -s scripts"""

import pathlib
import sys
import unittest

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

    def test_the_real_hooks_have_samples(self):
        # Every hook registered in this repository has a sample input.
        root = pathlib.Path(__file__).resolve().parent.parent
        settings = ch.load_settings(root)
        paths = [ch.hook_path(c) for c in ch.registered_commands(settings)]
        self.assertEqual(ch.sample_problems(paths, ch.SAMPLES), [])


if __name__ == "__main__":
    unittest.main()
