#!/bin/sh
# PreToolUse hook for tools that take a description (registered in
# .claude/settings.json for Bash and Agent).
#
# The description is shown to the maintainer, so AGENTS.md (language rules,
# the principle) requires it to be in Japanese (docs/incidents/0001). The hook
# denies the call when the description contains no Japanese characters, and
# tells the agent why.
#
# Requires jq (docs/development/onboarding.md, section 6.3). This is a
# language check, not a security guard, so without jq or a parsable payload
# the hook does not block anything.

input=$(cat)

command -v jq >/dev/null 2>&1 || exit 0

# Why tr: a native jq.exe started from Git Bash on Windows turns every LF
# of its output into CRLF (jq manual, --binary), so every CR is removed:
# otherwise an empty description is a lone CR, which is checked and denied
# (docs/incidents/0019).
description=$(printf '%s' "$input" | jq -r '.tool_input.description // empty' 2>/dev/null) || exit 0
description=$(printf '%s' "$description" | tr -d '\r')
[ -n "$description" ] || exit 0

# jq -e exits 1 only when the result is false. Any other failure (for
# example a jq built without regex support) must not block, so only an
# exit status of exactly 1 counts as "no Japanese".
status=0
printf '%s' "$description" | jq -Rse 'test("[\\p{Hiragana}\\p{Katakana}\\p{Han}]")' >/dev/null 2>&1 || status=$?
[ "$status" -eq 1 ] || exit 0

jq -cn --arg reason "ツールの説明文(description)に日本語が含まれていません。説明文はメンテナーに表示されるため、日本語で書き直してから呼び出してください(AGENTS.mdの言語ルールの原則)。" \
  '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: $reason}}'
exit 0
