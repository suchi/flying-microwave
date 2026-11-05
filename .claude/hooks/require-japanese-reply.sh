#!/bin/sh
# Stop and SubagentStop hook (registered in .claude/settings.json).
#
# Replies and subagent reports are shown to the maintainer, so AGENTS.md
# (language rules, the principle) requires them to be in Japanese
# (docs/incidents/0001). When the last assistant message contains no Japanese
# characters, the hook keeps the conversation going and asks for a rewrite.
# A Stop hook cannot hide a message that was already shown; it can only make
# Claude continue (Hooks reference, Stop).
#
# Loop protection: stop_hook_active is true when Claude is already continuing
# because of a Stop hook. That can be another Stop hook (for example the
# reminder about uncommitted changes), so the reply is still checked then
# (docs/incidents/0015). A state file per session counts the rewrites asked
# for in a row; after MAX_BLOCKS the hook lets Claude stop. A reply with
# Japanese resets the count. The file is per session and per agent
# (agent_id of SubagentStop), in a directory that the user owns (checked;
# under /tmp it could be someone else's). If that directory or the file
# cannot be used while continuing, the hook does not block, as before.
# Claude Code also ends a turn after eight Stop blocks in a row, but that
# count resets whenever Claude calls a tool (Hooks reference, Stop), so this
# hook keeps its own limit.
#
# Requires jq (docs/development/onboarding.md, section 6.3). Without jq or a
# parsable payload the hook does nothing.

input=$(cat)

command -v jq >/dev/null 2>&1 || exit 0

MAX_BLOCKS=2 # must be 1 to 9 (the count is read as one digit)

active=$(printf '%s' "$input" | jq -r '.stop_hook_active // false' 2>/dev/null) || exit 0
key=$(printf '%s' "$input" | jq -r '[.session_id // "unknown", .agent_id // "main"] | join("-")' 2>/dev/null) || exit 0
# Only safe characters in the file name.
key=$(printf '%s' "$key" | tr -cd 'A-Za-z0-9_-')
[ -n "$key" ] || key=unknown
# Why a directory of the user's own: /tmp is shared by every user on Linux.
state=
if uid=$(id -u 2>/dev/null) && [ -n "$uid" ]; then
  dir="${XDG_RUNTIME_DIR:-${TMPDIR:-${TEMP:-/tmp}}}/flying-microwave-$uid"
  (umask 077 && mkdir -p "$dir") 2>/dev/null
  # Use it only if it is a real directory of this user, private to them.
  if [ -d "$dir" ] && [ ! -h "$dir" ] && [ -O "$dir" ] 2>/dev/null && chmod 700 "$dir" 2>/dev/null; then
    state="$dir/japanese-reply-$key.count"
  fi
fi

message=$(printf '%s' "$input" | jq -r '.last_assistant_message // empty' 2>/dev/null) || exit 0
[ -n "$message" ] || exit 0

# jq -e exits 1 only when the result is false. Any other failure (for
# example a jq built without regex support) must not block, so only an
# exit status of exactly 1 counts as "no Japanese".
status=0
printf '%s' "$message" | jq -Rse 'test("[\\p{Hiragana}\\p{Katakana}\\p{Han}]")' >/dev/null 2>&1 || status=$?
if [ "$status" -ne 1 ]; then
  [ -n "$state" ] && rm -f "$state" 2>/dev/null
  exit 0
fi

if [ "$active" = "true" ]; then
  # Without a usable state file there is no limit: do not block.
  [ -n "$state" ] || exit 0
  count=$(cat "$state" 2>/dev/null) || count=0
  # The hook writes only 1 to MAX_BLOCKS; anything else counts as 0.
  case "$count" in [0-9]) ;; *) count=0 ;; esac
  # At the limit the count stays, so later replies in this run pass too.
  [ "$count" -ge "$MAX_BLOCKS" ] && exit 0
  count=$((count + 1))
else
  count=1
fi
if [ -z "$state" ] || ! { printf '%s\n' "$count" > "$state"; } 2>/dev/null; then
  # Without the count there is no limit, so do not block while continuing.
  [ "$active" = "true" ] && exit 0
fi

jq -cn --arg reason "直前の返答(または報告)に日本語が含まれていません。メンテナーに表示される文章は日本語で書く決まりです(AGENTS.mdの言語ルールの原則)。同じ内容を日本語で書き直してください。" \
  '{decision: "block", reason: $reason}'
exit 0
