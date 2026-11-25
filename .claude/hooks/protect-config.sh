#!/bin/sh
# PreToolUse hook for Edit/Write (registered in .claude/settings.json).
#
# Asks for user confirmation before the agent modifies files that make up
# this repository's security guardrails (AGENTS.md: security section), so
# the guard also works in auto-accept permission modes.
#
# Requires jq to parse the hook payload (docs/development/onboarding.md,
# section 6.3). If jq is missing or the payload does not parse, the hook asks
# instead of deciding, so the guard fails safe rather than silently
# allowing everything.

input=$(cat)

ask() {
  # $1: reason (escaped by jq)
  jq -cn --arg reason "$1" \
    '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: "ask", permissionDecisionReason: $reason}}'
  exit 0
}

if ! command -v jq >/dev/null 2>&1; then
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"このフックにはjqが必要ですが、見つかりません。jqを導入してください(docs/development/onboarding.mdの6.3)。"}}\n'
  exit 0
fi

path=$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty' 2>/dev/null) ||
  ask "フックの入力をJSONとして解析できませんでした。先にユーザーに確認してください。"
# Why tr: a native jq.exe started from Git Bash on Windows turns every LF
# of its output into CRLF (jq manual, --binary), so every CR is removed: a
# trailing CR made the patterns below miss .claude/settings.json. Removing
# CRs only lets more paths match (docs/incidents/0019).
path=$(printf '%s' "$path" | tr -d '\r')

[ -n "$path" ] || exit 0

# Normalize relative paths (hooks run in the project directory) so the
# guard cannot be bypassed by passing a relative file_path. Backslashes
# are converted first because Windows-native setups pass paths such as
# C:\repo\.claude\settings.json, which the patterns below would miss.
path=$(printf '%s' "$path" | tr '\\' '/')
case "$path" in
  /*|[A-Za-z]:/*) ;;
  *) path="${CLAUDE_PROJECT_DIR:-$PWD}/$path" ;;
esac

case "$path" in
  # AGENTS.md, CLAUDE.md, the skills and the commands hold the rules, where
  # an instruction from external content could otherwise stay
  # (docs/reviews/0005, J-6; plan 0005, D-24). Everything Claude Code reads
  # from .claude/ (agents, rules, workflows and the rest) and the MCP
  # servers of the project in .mcp.json are covered too (plan 0005, D-47,
  # N-2; docs/reviews/0006).
  */.claude/*|*/.mcp.json|*/.github/workflows/*|*/.github/dependabot.yml|*/AGENTS.md|*/CLAUDE.md|*/.agents/skills/*)
    ask "セキュリティに関わる設定を変更しようとしています: $path。ユーザーの承認が必要です(AGENTS.mdのセキュリティ)。"
    ;;
esac

exit 0
