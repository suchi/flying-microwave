#!/bin/sh
# PreToolUse hook for every tool (registered in .claude/settings.json).
#
# Text that the agent writes before a tool call is shown to the maintainer.
# AGENTS.md (language rules, the principle) requires it to be in Japanese,
# but the Stop hook checks only the last reply of a turn, and English notes
# between tool calls kept coming back (docs/incidents/0005, 0015, 0018).
# This hook reads the text of the assistant message that makes this tool
# call from the transcript, and denies the call when that text has Latin
# letters but no Japanese characters. The text was already shown; the hook
# can only make the agent rewrite it.
#
# Which message: the one whose tool_use block has this call's tool_use_id.
# The transcript is written asynchronously (Hooks reference), so the message
# may not be there yet; then, and for calls whose message has no text, the
# hook does nothing. Text of other messages is not checked again.
# Subagent calls are not in the main transcript, so they pass the same way.
#
# Requires jq (docs/development/onboarding.md, section 6.3). This is a
# language check, not a security guard, so without jq, a transcript, or a
# parsable payload the hook does not block anything.

input=$(cat)

command -v jq >/dev/null 2>&1 || exit 0

# Why tr: a native jq.exe started from Git Bash on Windows turns every LF of
# its output into CRLF (jq manual, --binary). Every CR is removed: a CR would make
# the path or the id never match.
# bash does not split words at a CR, so removing them hides nothing that
# would run (docs/incidents/0019).
transcript=$(printf '%s' "$input" | jq -r '.transcript_path // empty' 2>/dev/null) || exit 0
transcript=$(printf '%s' "$transcript" | tr -d '\r')
id=$(printf '%s' "$input" | jq -r '.tool_use_id // empty' 2>/dev/null) || exit 0
id=$(printf '%s' "$id" | tr -d '\r')
[ -n "$transcript" ] && [ -n "$id" ] && [ -f "$transcript" ] || exit 0

# Why tail: the transcript of a long session is tens of megabytes, and the
# message of this call is among its last entries. fromjson? skips a line
# that is being written.
text=$(tail -n 200 "$transcript" 2>/dev/null | jq -Rnr --arg id "$id" '
  [inputs | fromjson? | select(type == "object" and .type == "assistant" and (.message | type) == "object")] as $entries
  | ([$entries[] | select(any(.message.content[]?; type == "object" and .type == "tool_use" and .id == $id)) | .message.id] | last) as $message
  | if $message == null then empty else
      [$entries[] | select(.message.id == $message) | .message.content[]? | select(type == "object" and .type == "text") | .text] | join("\n")
    end' 2>/dev/null) || exit 0
[ -n "$text" ] || exit 0

# Only text with Latin letters counts; a line of symbols or numbers passes.
printf '%s' "$text" | jq -Rse 'test("[A-Za-z]")' >/dev/null 2>&1 || exit 0

# jq -e exits 1 only when the result is false. Any other failure (for
# example a jq built without regex support) must not block, so only an
# exit status of exactly 1 counts as "no Japanese".
status=0
printf '%s' "$text" | jq -Rse 'test("[\\p{Hiragana}\\p{Katakana}\\p{Han}]")' >/dev/null 2>&1 || status=$?
[ "$status" -eq 1 ] || exit 0

jq -cn --arg reason "ツールを呼ぶ前に書いた文に日本語が含まれていません。この文はメンテナーに表示されるため、同じ内容を日本語で書き直してから、もう一度呼び出してください(AGENTS.mdの言語ルールの原則。docs/incidents/0018)。" \
  '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: $reason}}'
exit 0
