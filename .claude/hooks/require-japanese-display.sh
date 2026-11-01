#!/bin/sh
# PreToolUse (every tool) and Stop hook (registered in .claude/settings.json).
#
# flag-non-japanese-display.sh records displayed assistant messages that
# contain no Japanese. This hook turns such a record into feedback Claude
# sees: it denies the next tool call, or keeps the turn going at Stop, and
# quotes the message so that it can be rewritten in Japanese
# (docs/incidents/0005).
#
# Hooks fired inside a subagent (agent_id is set) are ignored: the record
# belongs to the main conversation, and subagent reports are checked by
# require-japanese-reply.sh at SubagentStop.
#
# Requires jq. This is a language check, not a security guard, so without
# jq or a parsable payload the hook does not block anything.

input=$(cat)
command -v jq >/dev/null 2>&1 || exit 0

event=$(printf '%s' "$input" | jq -r '.hook_event_name // empty' 2>/dev/null) || exit 0
agent=$(printf '%s' "$input" | jq -r '.agent_id // empty' 2>/dev/null) || exit 0
session=$(printf '%s' "$input" | jq -r '.session_id // empty' 2>/dev/null | tr -cd 'A-Za-z0-9_-') || exit 0
[ -z "$agent" ] && [ -n "$session" ] || exit 0

# Why: the record is quoted back to Claude as the reason of a denial, so
# nobody else may be able to write it: a per-user directory created with
# mode 700 and owned by us. Why not scratchpad_dir: it is not guaranteed in
# every event's input, and both hooks must always agree on the place.
umask 077
dir="${TMPDIR:-/tmp}/claude-japanese-check-$(id -u)"
mkdir -p "$dir" 2>/dev/null || exit 0
[ -O "$dir" ] || exit 0
record="$dir/$session.violation"
[ -f "$record" ] || exit 0

if [ "$event" = "Stop" ]; then
  active=$(printf '%s' "$input" | jq -r '.stop_hook_active // false' 2>/dev/null) || exit 0
  # Continue the turn at most once in a row (same rule as
  # require-japanese-reply.sh). Drop the record so that it is not carried
  # over to the next turn, where it would ask to rewrite an old message.
  if [ "$active" = "true" ]; then
    rm -f "$record"
    exit 0
  fi
fi
quoted=$(head -c 1500 "$record")
rm -f "$record"

reason="日本語を含まないメッセージをメンテナーに表示しました。表示された文(先頭の部分):
$quoted
メンテナーに表示される文は、ツール呼び出しの合間のひとことも含めて日本語で書く決まりです(AGENTS.mdの言語ルールの原則)。同じ内容を日本語で書き直してから、作業を続けてください。"

if [ "$event" = "Stop" ]; then
  jq -cn --arg reason "$reason" '{decision: "block", reason: $reason}'
else
  jq -cn --arg reason "$reason" \
    '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: $reason}}'
fi
exit 0
