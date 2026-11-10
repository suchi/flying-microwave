#!/bin/sh
# PreToolUse hook for every tool (registered in .claude/settings.json).
#
# Text that the agent writes before a tool call is shown to the maintainer.
# AGENTS.md (language rules, the principle) requires it to be in Japanese,
# but the Stop hook checks only the last reply of a turn, and English notes
# between tool calls kept coming back (docs/incidents/0005, 0015, 0018).
# The text was already shown; the hook can only make the agent rewrite it.
#
# What is checked: the transcript is written asynchronously and may not yet
# hold the message of this very call when the hook runs (Hooks reference,
# transcript_path). In a cloud session it was written about 0.5 seconds
# after the hook had read it, so looking only for this call's message let
# everything through (docs/incidents/0022). So the hook checks every
# assistant message with a tool call that is already in the recent part of
# the transcript. The text of a message with Latin letters but no Japanese
# characters makes the hook deny the call, once per message: the ids of the
# messages already reported are kept in a state file per session. A note
# written late is reported at the next call; one already written is
# reported at once. Replies without a tool call are left to the Stop hook.
#
# Subagent calls (agent_id set) pass: their messages are not in the main
# transcript, and the main agent's text must not stop them. Their reports
# are checked by the SubagentStop hook.
#
# State: the same directory as require-japanese-reply.sh, private to the
# user (checked; under /tmp it could be someone else's). Without a usable
# state file the hook cannot report a message only once, so it stops
# nothing.
#
# Requires jq (docs/development/onboarding.md, section 6.3). This is a
# language check, not a security guard, so without jq, a transcript, or a
# parsable payload the hook does not block anything.

input=$(cat)

command -v jq >/dev/null 2>&1 || exit 0

# Why tr: a native jq.exe started from Git Bash on Windows turns every LF
# of its output into CRLF (jq manual, --binary), so every CR is removed: a
# CR would make the path or an id never match (docs/incidents/0019).
agent=$(printf '%s' "$input" | jq -r '.agent_id // empty' 2>/dev/null) || exit 0
agent=$(printf '%s' "$agent" | tr -d '\r')
[ -z "$agent" ] || exit 0
transcript=$(printf '%s' "$input" | jq -r '.transcript_path // empty' 2>/dev/null) || exit 0
transcript=$(printf '%s' "$transcript" | tr -d '\r')
[ -n "$transcript" ] && [ -f "$transcript" ] || exit 0
key=$(printf '%s' "$input" | jq -r '.session_id // "unknown"' 2>/dev/null) || exit 0
# Only safe characters in the file name.
key=$(printf '%s' "$key" | tr -cd 'A-Za-z0-9_-')
[ -n "$key" ] || key=unknown

# Why a directory of the user's own: /tmp is shared by every user on Linux.
uid=$(id -u 2>/dev/null) && [ -n "$uid" ] || exit 0
dir="${XDG_RUNTIME_DIR:-${TMPDIR:-${TEMP:-/tmp}}}/flying-microwave-$uid"
(umask 077 && mkdir -p "$dir") 2>/dev/null
[ -d "$dir" ] && [ ! -h "$dir" ] && [ -O "$dir" ] 2>/dev/null && chmod 700 "$dir" 2>/dev/null || exit 0
state="$dir/japanese-note-$key.ids"

# One line per message that has a tool call and English-only text, oldest
# first: the message id, a tab, and the start of the text. Why tail: the
# transcript of a long session is tens of megabytes, and the messages of the
# last calls are among its last entries. fromjson? skips a line that is
# being written. A jq without regex support fails here, and then nothing is
# blocked.
found=$(tail -n 200 "$transcript" 2>/dev/null | jq -Rnr '
  reduce (inputs | fromjson? | select(type == "object" and .type == "assistant" and (.message | type) == "object" and (.message.id | type) == "string")) as $x
    ({order: [], m: {}};
     $x.message.id as $id
     | (if .m[$id] == null then .order += [$id] | .m[$id] = {text: [], tool: false} else . end)
     | .m[$id].text += [$x.message.content[]? | select(type == "object" and .type == "text") | .text | strings]
     | .m[$id].tool = (.m[$id].tool or any($x.message.content[]?; type == "object" and .type == "tool_use")))
  | . as $s
  | $s.order[]
  | . as $id
  | $s.m[$id]
  | select(.tool)
  | (.text | join("\n")) as $t
  | select(($t | test("[A-Za-z]")) and ($t | test("[\\p{Hiragana}\\p{Katakana}\\p{Han}]") | not))
  | [$id, ($t | gsub("\\s+"; " ") | .[0:80])]
  | @tsv' 2>/dev/null) || exit 0
found=$(printf '%s' "$found" | tr -d '\r')
[ -n "$found" ] || exit 0

tab=$(printf '\t')
printf '%s\n' "$found" | while IFS="$tab" read -r id snippet; do
  id=$(printf '%s' "$id" | tr -cd 'A-Za-z0-9_-')
  [ -n "$id" ] || continue
  grep -Fxq -- "$id" "$state" 2>/dev/null && continue
  # Without the record the same message would be reported at every call.
  { printf '%s\n' "$id" >> "$state"; } 2>/dev/null || break
  jq -cn --arg reason "ツールを呼ぶ前に書いた文「${snippet}」に日本語が含まれていません。この文はメンテナーに表示されたため、同じ内容を日本語で書き直してから、もう一度呼び出してください。この文について書き直しを求めるのは、この1回だけです(AGENTS.mdの言語ルールの原則。docs/incidents/0018、0022)。" \
    '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: $reason}}'
  break
done
exit 0
