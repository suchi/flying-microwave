#!/bin/sh
# MessageDisplay hook (registered in .claude/settings.json).
#
# Every assistant message shown to the maintainer must contain Japanese
# (AGENTS.md, language rules). The Stop hook only sees the last message of a
# turn, so short notes written between tool calls slipped through
# (docs/incidents/0005). This hook sees every displayed message.
#
# MessageDisplay cannot block a message and Claude never sees its output, so
# this hook only (1) appends a Japanese notice to the displayed message and
# (2) records the violation in a private state file. require-japanese-display.sh then
# stops the next tool call or the end of the turn and asks for a rewrite.
#
# Japanese is judged over the whole message, when its final batch arrives,
# so a message that starts with a code line and continues in Japanese is
# not flagged.
#
# Requires jq. This is a language check, not a security guard, so without
# jq or a parsable payload the hook changes nothing.

input=$(cat)
command -v jq >/dev/null 2>&1 || exit 0

field() {
  printf '%s' "$input" | jq -r "$1" 2>/dev/null
}

# Messages displayed inside a subagent are not the main conversation's.
agent=$(field '.agent_id // empty') || exit 0
[ -z "$agent" ] || exit 0
session=$(field '.session_id // empty' | tr -cd 'A-Za-z0-9_-') || exit 0
message=$(field '.message_id // empty' | tr -cd 'A-Za-z0-9_-') || exit 0
[ -n "$session" ] && [ -n "$message" ] || exit 0
delta=$(field '.delta // ""') || exit 0
final=$(field '.final // false') || exit 0

# Why: the record is quoted back to Claude as the reason of a denial, so
# nobody else may be able to write it: a per-user directory created with
# mode 700 and owned by us. Why not scratchpad_dir: it is not guaranteed in
# every event's input, and both hooks must always agree on the place.
umask 077
dir="${TMPDIR:-/tmp}/claude-japanese-check-$(id -u)"
mkdir -p "$dir" 2>/dev/null || exit 0
[ -O "$dir" ] || exit 0
prefix="$dir/$session.$message"

# jq -e exits 1 only when the result is false; any other failure counts as
# "contains Japanese" so that a broken jq never produces a false alarm.
status=0
printf '%s' "$delta" | jq -Rse 'test("[\\p{Hiragana}\\p{Katakana}\\p{Han}]")' >/dev/null 2>&1 || status=$?
if [ "$status" -ne 1 ]; then
  : >"$prefix.ja"
elif printf '%s' "$delta" | grep -q '[A-Za-z]'; then
  # Keep the start of the message for the rewrite request.
  if [ ! -f "$prefix.text" ]; then
    printf '%s' "$delta" | head -c 300 >"$prefix.text"
  fi
fi

[ "$final" = "true" ] || exit 0

if [ ! -f "$prefix.ja" ] && [ -f "$prefix.text" ]; then
  {
    cat "$prefix.text"
    printf '\n---\n'
  } >>"$dir/$session.violation"
  jq -cn --arg delta "$delta" \
    '{hookSpecificOutput: {hookEventName: "MessageDisplay", displayContent: ($delta + "\n(注意: このメッセージは日本語で書かれていません。規則違反として記録し、エージェントに日本語での書き直しを求めます)\n")}}'
fi
rm -f "$prefix.ja" "$prefix.text"
exit 0
